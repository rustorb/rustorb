//! The POA: object adapters, their policies, POA managers and object keys.
//!
//! Object keys follow `docs/poa-design.md`:
//!
//! | POA | Key |
//! | --- | --- |
//! | PERSISTENT | `\xff` POA path `\x00` ObjectId (the omniORB format) |
//! | TRANSIENT | `\xff` POA path `\xfe` boot id (8 bytes) ObjectId |
//! | RootPOA | `\xfe` boot id ObjectId |
//!
//! The POA path is the names from below the RootPOA down, separated by
//! `\xff`. The boot id is a random number chosen at each start of the ORB,
//! so TRANSIENT references die with the process. System generated
//! ObjectIds are the boot id and a 4-byte big-endian serial number.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt;
use std::panic::{self, AssertUnwindSafe};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, Weak};
use std::thread::{self, ThreadId};

use rustorb_giop::IiopProfile;

use crate::pool::Job;
use crate::servant::{Cookie, ServantLocator};
use crate::server::Server;
use crate::{
    AdapterActivator, CompletionStatus, ObjectRef, Orb, Servant, ServantActivator, ServantManager,
    ServantManagerError, SystemException, SystemExceptionKind, minor,
};

/// `PortableServer::ObjectId`.
pub type ObjectId = Vec<u8>;

const PATH_START: u8 = 0xff;
const PATH_SEPARATOR: u8 = 0xff;
const PERSISTENT_END: u8 = 0x00;
const TRANSIENT_END: u8 = 0xfe;
/// Length of the boot id in TRANSIENT keys and system generated ids.
pub(crate) const BOOT_ID_LEN: usize = 8;

/// `PortableServer::ThreadPolicyValue`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ThreadPolicy {
    /// `ORB_CTRL_MODEL`: requests run on the upcall thread pool, in
    /// parallel and in no particular order.
    OrbCtrl,
    /// `SINGLE_THREAD_MODEL`: requests to the POA run one at a time, in
    /// the order they arrive, on any upcall thread. A servant may call an
    /// object of its own POA in the same ORB: the call runs at once on the
    /// servant's thread.
    SingleThread,
    /// `MAIN_THREAD_MODEL`: requests to the POA run one at a time on a
    /// thread that is in [`Orb::run`] or [`Orb::perform_work`], shared with
    /// every other POA with this policy.
    MainThread,
}

/// `PortableServer::LifespanPolicyValue`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Lifespan {
    /// References are valid while this ORB runs.
    Transient,
    /// References stay valid across restarts of a server that listens on
    /// the same address and recreates the POA.
    Persistent,
}

/// `PortableServer::IdUniquenessPolicyValue`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IdUniqueness {
    /// A servant is active under one ObjectId at most.
    Unique,
    /// A servant may be active under several ObjectIds.
    Multiple,
}

/// `PortableServer::IdAssignmentPolicyValue`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IdAssignment {
    /// The application chooses the ObjectIds.
    User,
    /// The POA generates the ObjectIds.
    System,
}

/// `PortableServer::ImplicitActivationPolicyValue`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ImplicitActivation {
    /// `servant_to_reference` activates an inactive servant.
    Implicit,
    /// Servants are activated explicitly only.
    NoImplicit,
}

/// `PortableServer::ServantRetentionPolicyValue`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ServantRetention {
    /// Active servants are kept in the active object map.
    Retain,
    /// No active object map: every request goes to the default servant or
    /// to a [`ServantLocator`].
    NonRetain,
}

/// `PortableServer::RequestProcessingPolicyValue`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RequestProcessing {
    /// Requests go to servants in the active object map only.
    ActiveObjectMapOnly,
    /// Requests for objects not in the active object map go to the
    /// default servant ([`Poa::set_servant`]).
    UseDefaultServant,
    /// Requests for objects not in the active object map go to the
    /// servant manager ([`Poa::set_servant_manager`]).
    UseServantManager,
}

/// A POA policy, given to [`Poa::create_poa`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Policy {
    /// `ThreadPolicy`.
    Thread(ThreadPolicy),
    /// `LifespanPolicy`.
    Lifespan(Lifespan),
    /// `IdUniquenessPolicy`.
    IdUniqueness(IdUniqueness),
    /// `IdAssignmentPolicy`.
    IdAssignment(IdAssignment),
    /// `ImplicitActivationPolicy`.
    ImplicitActivation(ImplicitActivation),
    /// `ServantRetentionPolicy`.
    ServantRetention(ServantRetention),
    /// `RequestProcessingPolicy`.
    RequestProcessing(RequestProcessing),
}

/// The policies of one POA.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Policies {
    /// Thread policy.
    pub thread: ThreadPolicy,
    /// Lifespan policy.
    pub lifespan: Lifespan,
    /// ObjectId uniqueness policy.
    pub id_uniqueness: IdUniqueness,
    /// ObjectId assignment policy.
    pub id_assignment: IdAssignment,
    /// Implicit activation policy.
    pub implicit_activation: ImplicitActivation,
    /// Servant retention policy.
    pub servant_retention: ServantRetention,
    /// Request processing policy.
    pub request_processing: RequestProcessing,
}

impl Default for Policies {
    /// The policies of a POA created with no policies.
    fn default() -> Policies {
        Policies {
            thread: ThreadPolicy::OrbCtrl,
            lifespan: Lifespan::Transient,
            id_uniqueness: IdUniqueness::Unique,
            id_assignment: IdAssignment::System,
            implicit_activation: ImplicitActivation::NoImplicit,
            servant_retention: ServantRetention::Retain,
            request_processing: RequestProcessing::ActiveObjectMapOnly,
        }
    }
}

impl Policies {
    /// The policies of the RootPOA.
    pub(crate) fn root() -> Policies {
        Policies {
            implicit_activation: ImplicitActivation::Implicit,
            ..Policies::default()
        }
    }

    /// The defaults overridden by `list`. Fails with the index of the first
    /// policy that is repeated, or of one inconsistent with the others.
    fn from_list(list: &[Policy]) -> Result<Policies, u16> {
        let mut p = Policies::default();
        let mut seen = Vec::new();
        let index = |i: usize| u16::try_from(i).unwrap_or(u16::MAX);
        for (i, policy) in list.iter().enumerate() {
            let kind = std::mem::discriminant(policy);
            if seen.contains(&kind) {
                return Err(index(i));
            }
            seen.push(kind);
            match *policy {
                Policy::Thread(v) => p.thread = v,
                Policy::Lifespan(v) => p.lifespan = v,
                Policy::IdUniqueness(v) => p.id_uniqueness = v,
                Policy::IdAssignment(v) => p.id_assignment = v,
                Policy::ImplicitActivation(v) => p.implicit_activation = v,
                Policy::ServantRetention(v) => p.servant_retention = v,
                Policy::RequestProcessing(v) => p.request_processing = v,
            }
        }
        // The index of the last policy of a kind in the list, as the one in
        // conflict with the defaults or with the policies before it.
        let position = |f: fn(&Policy) -> bool| index(list.iter().rposition(f).unwrap_or(0));
        if p.implicit_activation == ImplicitActivation::Implicit
            && (p.id_assignment != IdAssignment::System
                || p.servant_retention != ServantRetention::Retain)
        {
            return Err(position(|q| matches!(q, Policy::ImplicitActivation(_))));
        }
        if p.servant_retention == ServantRetention::NonRetain
            && p.request_processing == RequestProcessing::ActiveObjectMapOnly
        {
            return Err(position(|q| {
                matches!(
                    q,
                    Policy::ServantRetention(_) | Policy::RequestProcessing(_)
                )
            }));
        }
        if p.request_processing == RequestProcessing::UseDefaultServant
            && p.id_uniqueness != IdUniqueness::Multiple
        {
            return Err(position(|q| {
                matches!(q, Policy::RequestProcessing(_) | Policy::IdUniqueness(_))
            }));
        }
        Ok(p)
    }
}

/// The exceptions of the `PortableServer::POA` and `POAManager`
/// operations, and system exceptions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PoaError {
    /// `AdapterAlreadyExists`.
    AdapterAlreadyExists,
    /// `AdapterNonExistent`.
    AdapterNonExistent,
    /// `InvalidPolicy`, with the index of the offending policy.
    InvalidPolicy(u16),
    /// `ServantAlreadyActive`.
    ServantAlreadyActive,
    /// `ObjectAlreadyActive`.
    ObjectAlreadyActive,
    /// `ServantNotActive`.
    ServantNotActive,
    /// `ObjectNotActive`.
    ObjectNotActive,
    /// `WrongPolicy`.
    WrongPolicy,
    /// `WrongAdapter`.
    WrongAdapter,
    /// `POAManager::AdapterInactive`.
    AdapterInactive,
    /// `POA::NoServant`: no default servant was set.
    NoServant,
    /// `Current::NoContext`: the thread is not running a request.
    NoContext,
    /// A system exception, such as `OBJECT_NOT_EXIST` for a destroyed POA.
    System(SystemException),
}

impl fmt::Display for PoaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PoaError::AdapterAlreadyExists => {
                f.write_str("PortableServer::POA::AdapterAlreadyExists")
            }
            PoaError::AdapterNonExistent => f.write_str("PortableServer::POA::AdapterNonExistent"),
            PoaError::InvalidPolicy(i) => {
                write!(f, "PortableServer::POA::InvalidPolicy (index {i})")
            }
            PoaError::ServantAlreadyActive => {
                f.write_str("PortableServer::POA::ServantAlreadyActive")
            }
            PoaError::ObjectAlreadyActive => {
                f.write_str("PortableServer::POA::ObjectAlreadyActive")
            }
            PoaError::ServantNotActive => f.write_str("PortableServer::POA::ServantNotActive"),
            PoaError::ObjectNotActive => f.write_str("PortableServer::POA::ObjectNotActive"),
            PoaError::WrongPolicy => f.write_str("PortableServer::POA::WrongPolicy"),
            PoaError::WrongAdapter => f.write_str("PortableServer::POA::WrongAdapter"),
            PoaError::AdapterInactive => f.write_str("PortableServer::POAManager::AdapterInactive"),
            PoaError::NoServant => f.write_str("PortableServer::POA::NoServant"),
            PoaError::NoContext => f.write_str("PortableServer::Current::NoContext"),
            PoaError::System(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for PoaError {}

impl From<SystemException> for PoaError {
    fn from(e: SystemException) -> Self {
        PoaError::System(e)
    }
}

fn sys(kind: SystemExceptionKind, minor: u32) -> SystemException {
    SystemException::new(kind, minor, CompletionStatus::No)
}

/// `OBJECT_NOT_EXIST` for a request whose key names no active object.
pub(crate) fn no_such_object() -> SystemException {
    sys(SystemExceptionKind::OBJECT_NOT_EXIST, minor::NO_SUCH_OBJECT)
}

// --- upcall bookkeeping ------------------------------------------------------

thread_local! {
    static UPCALL_DEPTH: Cell<usize> = const { Cell::new(0) };
    static MAIN_DEPTH: Cell<usize> = const { Cell::new(0) };
    static CURRENT: RefCell<Vec<CurrentEntry>> = const { RefCell::new(Vec::new()) };
}

/// The request a thread is running, for [`PoaCurrent`].
#[derive(Clone)]
struct CurrentEntry {
    poa: Poa,
    id: ObjectId,
    servant: Arc<dyn Servant>,
}

/// Makes a request the current one of the thread while it lives.
pub(crate) struct CurrentGuard(());

impl CurrentGuard {
    pub(crate) fn enter(poa: &Poa, id: &[u8], servant: &Arc<dyn Servant>) -> CurrentGuard {
        CURRENT.with(|c| {
            c.borrow_mut().push(CurrentEntry {
                poa: poa.clone(),
                id: id.to_vec(),
                servant: Arc::clone(servant),
            });
        });
        CurrentGuard(())
    }
}

impl Drop for CurrentGuard {
    fn drop(&mut self) {
        CURRENT.with(|c| c.borrow_mut().pop());
    }
}

/// `PortableServer::Current`: the POA and ObjectId of the request the
/// calling thread is running. Outside a request, every method fails with
/// [`PoaError::NoContext`].
#[derive(Debug, Clone, Copy, Default)]
pub struct PoaCurrent;

impl PoaCurrent {
    fn with<T>(f: impl FnOnce(&CurrentEntry) -> T) -> Result<T, PoaError> {
        CURRENT.with(|c| c.borrow().last().map(f).ok_or(PoaError::NoContext))
    }

    /// `get_POA`: the POA of the target object.
    pub fn get_poa() -> Result<Poa, PoaError> {
        PoaCurrent::with(|e| e.poa.clone())
    }

    /// `get_object_id`: the ObjectId of the target object.
    pub fn get_object_id() -> Result<ObjectId, PoaError> {
        PoaCurrent::with(|e| e.id.clone())
    }

    /// `get_reference`: a reference to the target object.
    pub fn get_reference() -> Result<ObjectRef, PoaError> {
        let (poa, id, servant) =
            PoaCurrent::with(|e| (e.poa.clone(), e.id.clone(), Arc::clone(&e.servant)))?;
        poa.reference(&id, servant.repository_id())
    }

    /// `get_servant`: the servant running the request.
    pub fn get_servant() -> Result<Arc<dyn Servant>, PoaError> {
        PoaCurrent::with(|e| Arc::clone(&e.servant))
    }
}

/// Marks the current thread as one that runs MAIN_THREAD_MODEL requests
/// (in `Orb::run` or `Orb::perform_work`) while it lives.
pub(crate) struct MainThreadGuard(());

impl MainThreadGuard {
    pub(crate) fn enter() -> MainThreadGuard {
        MAIN_DEPTH.with(|d| d.set(d.get() + 1));
        MainThreadGuard(())
    }
}

impl Drop for MainThreadGuard {
    fn drop(&mut self) {
        MAIN_DEPTH.with(|d| d.set(d.get() - 1));
    }
}

/// Whether the current thread runs MAIN_THREAD_MODEL requests.
pub(crate) fn is_main_thread() -> bool {
    MAIN_DEPTH.with(Cell::get) > 0
}

/// Whether the current thread is running a servant.
pub(crate) fn in_upcall() -> bool {
    UPCALL_DEPTH.with(Cell::get) > 0
}

/// Marks the current thread as running a servant while it lives.
pub(crate) struct UpcallGuard(());

impl UpcallGuard {
    pub(crate) fn enter() -> UpcallGuard {
        UPCALL_DEPTH.with(|d| d.set(d.get() + 1));
        UpcallGuard(())
    }
}

impl Drop for UpcallGuard {
    fn drop(&mut self) {
        UPCALL_DEPTH.with(|d| d.set(d.get() - 1));
    }
}

/// A count of requests in progress that can be waited on.
#[derive(Debug, Default)]
pub(crate) struct Activity {
    count: Mutex<usize>,
    idle: Condvar,
}

impl Activity {
    pub(crate) fn enter(self: &Arc<Self>) -> ActivityGuard {
        *self.count.lock().unwrap() += 1;
        ActivityGuard(Arc::clone(self))
    }

    pub(crate) fn wait_idle(&self) {
        let mut count = self.count.lock().unwrap();
        while *count > 0 {
            count = self.idle.wait(count).unwrap();
        }
    }
}

pub(crate) struct ActivityGuard(Arc<Activity>);

impl Drop for ActivityGuard {
    fn drop(&mut self) {
        let mut count = self.0.count.lock().unwrap();
        *count -= 1;
        if *count == 0 {
            self.0.idle.notify_all();
        }
    }
}

/// Fails with `BAD_INV_ORDER` (would deadlock) when a servant asks to wait
/// for the completion of requests.
fn check_wait(wait_for_completion: bool) -> Result<(), SystemException> {
    if wait_for_completion && in_upcall() {
        return Err(sys(
            SystemExceptionKind::BAD_INV_ORDER,
            minor::WOULD_DEADLOCK,
        ));
    }
    Ok(())
}

// --- POA manager -------------------------------------------------------------

/// `PortableServer::POAManager::State`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PoaManagerState {
    /// Requests are queued until the manager becomes active.
    Holding,
    /// Requests are processed.
    Active,
    /// Requests are refused with `TRANSIENT`.
    Discarding,
    /// Requests are refused with `OBJ_ADAPTER`; the manager cannot become
    /// active again.
    Inactive,
}

/// What happens to a request that reached its POA manager.
pub(crate) enum Disposition {
    /// Run it. The dispatch is called on whichever thread admitted the
    /// request (an I/O thread, or the one activating the manager), so it
    /// must only hand the request on, not run user code.
    Run,
    /// Answer it with this exception.
    Reject(SystemException),
}

pub(crate) type Dispatch = Box<dyn FnOnce(Disposition) + Send>;

/// Requests a holding POA manager keeps at most; more are refused with
/// `TRANSIENT`.
const MAX_HELD_REQUESTS: usize = 4096;

/// `PortableServer::POAManager`: switches the processing of requests for
/// the POAs it manages. A manager starts in the holding state.
///
/// A cheap handle; clones manage the same POAs.
#[derive(Clone)]
pub struct PoaManager {
    pub(crate) inner: Arc<ManagerInner>,
}

pub(crate) struct ManagerInner {
    state: Mutex<ManagerState>,
    activity: Arc<Activity>,
    server: Weak<Server>,
    /// The POAs the manager manages, for `deactivate(etherealize)`.
    poas: Mutex<Vec<Weak<PoaInner>>>,
}

struct ManagerState {
    state: PoaManagerState,
    held: VecDeque<Dispatch>,
}

impl fmt::Debug for PoaManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PoaManager")
            .field("state", &self.state())
            .finish_non_exhaustive()
    }
}

impl PartialEq for PoaManager {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }
}

impl Eq for PoaManager {}

impl PoaManager {
    pub(crate) fn new(server: &Arc<Server>) -> PoaManager {
        let inner = Arc::new(ManagerInner {
            state: Mutex::new(ManagerState {
                state: PoaManagerState::Holding,
                held: VecDeque::new(),
            }),
            activity: Arc::default(),
            server: Arc::downgrade(server),
            poas: Mutex::new(Vec::new()),
        });
        server.register_manager(&inner);
        PoaManager { inner }
    }

    /// The current state.
    pub fn state(&self) -> PoaManagerState {
        self.inner.state.lock().unwrap().state
    }

    /// Starts processing requests, including those held so far. Fails
    /// with `AdapterInactive` after [`deactivate`](PoaManager::deactivate).
    pub fn activate(&self) -> Result<(), PoaError> {
        let held = {
            let mut st = self.inner.state.lock().unwrap();
            if st.state == PoaManagerState::Inactive {
                return Err(PoaError::AdapterInactive);
            }
            st.state = PoaManagerState::Active;
            std::mem::take(&mut st.held)
        };
        for dispatch in held {
            dispatch(Disposition::Run);
        }
        Ok(())
    }

    /// Queues new requests until the manager is activated again. With
    /// `wait_for_completion`, returns once the requests in progress have
    /// finished (`BAD_INV_ORDER` when called from a servant).
    pub fn hold_requests(&self, wait_for_completion: bool) -> Result<(), PoaError> {
        check_wait(wait_for_completion)?;
        self.set(PoaManagerState::Holding)?;
        if wait_for_completion {
            self.inner.activity.wait_idle();
        }
        Ok(())
    }

    /// Refuses new requests with `TRANSIENT`, and the held ones too.
    pub fn discard_requests(&self, wait_for_completion: bool) -> Result<(), PoaError> {
        check_wait(wait_for_completion)?;
        self.set(PoaManagerState::Discarding)?;
        if wait_for_completion {
            self.inner.activity.wait_idle();
        }
        Ok(())
    }

    /// Refuses new requests with `OBJ_ADAPTER` for good, and the held ones
    /// too. With `etherealize`, the objects of the POAs it manages that
    /// have a [`ServantActivator`] are deactivated and etherealized once
    /// their requests have finished (before this returns with
    /// `wait_for_completion`).
    pub fn deactivate(&self, etherealize: bool, wait_for_completion: bool) -> Result<(), PoaError> {
        check_wait(wait_for_completion)?;
        self.set(PoaManagerState::Inactive)?;
        if etherealize {
            let poas: Vec<_> = self
                .inner
                .poas
                .lock()
                .unwrap()
                .iter()
                .filter_map(Weak::upgrade)
                .collect();
            let mut work = Vec::new();
            for poa in poas {
                work.extend(poa.take_for_etherealize());
            }
            if let Some(server) = self.inner.server.upgrade() {
                server.etherealize(work, wait_for_completion);
            }
        }
        if wait_for_completion {
            self.inner.activity.wait_idle();
        }
        Ok(())
    }

    fn set(&self, state: PoaManagerState) -> Result<(), PoaError> {
        let held = {
            let mut st = self.inner.state.lock().unwrap();
            if st.state == PoaManagerState::Inactive {
                return Err(PoaError::AdapterInactive);
            }
            st.state = state;
            if state == PoaManagerState::Holding {
                return Ok(());
            }
            std::mem::take(&mut st.held)
        };
        let e = refusal(state);
        for dispatch in held {
            dispatch(Disposition::Reject(e.clone()));
        }
        Ok(())
    }
}

/// The exception for a request refused in `state`.
fn refusal(state: PoaManagerState) -> SystemException {
    match state {
        PoaManagerState::Inactive => sys(SystemExceptionKind::OBJ_ADAPTER, minor::POA_INACTIVE),
        _ => sys(SystemExceptionKind::TRANSIENT, minor::POA_DISCARDING),
    }
}

impl ManagerInner {
    /// Runs, holds or refuses a request according to the state.
    pub(crate) fn dispatch(&self, dispatch: Dispatch) {
        let mut st = self.state.lock().unwrap();
        match st.state {
            PoaManagerState::Active => {
                drop(st);
                dispatch(Disposition::Run);
            }
            PoaManagerState::Holding if st.held.len() < MAX_HELD_REQUESTS => {
                st.held.push_back(dispatch);
            }
            PoaManagerState::Holding => {
                drop(st);
                dispatch(Disposition::Reject(sys(SystemExceptionKind::TRANSIENT, 0)));
            }
            state => {
                drop(st);
                dispatch(Disposition::Reject(refusal(state)));
            }
        }
    }

    /// Whether a `LocateRequest` may answer `OBJECT_HERE`.
    pub(crate) fn is_inactive(&self) -> bool {
        self.state.lock().unwrap().state == PoaManagerState::Inactive
    }

    pub(crate) fn activity(&self) -> &Arc<Activity> {
        &self.activity
    }

    fn add_poa(&self, poa: &Arc<PoaInner>) {
        let mut poas = self.poas.lock().unwrap();
        poas.retain(|p| p.strong_count() > 0);
        poas.push(Arc::downgrade(poa));
    }

    /// Stops the manager at ORB shutdown: held requests are dropped
    /// without a reply (the connections are closed with `CloseConnection`,
    /// which tells clients they were not processed).
    pub(crate) fn shut_down(&self) {
        let held = {
            let mut st = self.state.lock().unwrap();
            st.state = PoaManagerState::Inactive;
            std::mem::take(&mut st.held)
        };
        drop(held);
    }
}

// --- POA -----------------------------------------------------------------------

/// `PortableServer::POA`: an object adapter, with its active object map
/// and its child POAs.
///
/// A cheap handle; clones refer to the same POA.
#[derive(Clone)]
pub struct Poa {
    pub(crate) orb: Orb,
    pub(crate) inner: Arc<PoaInner>,
}

pub(crate) struct PoaInner {
    name: String,
    /// Names from below the RootPOA down to this POA; empty for the
    /// RootPOA.
    path: Vec<String>,
    policies: Policies,
    pub(crate) manager: PoaManager,
    parent: Option<Weak<PoaInner>>,
    /// The key up to the ObjectId.
    key_prefix: Vec<u8>,
    boot_id: [u8; BOOT_ID_LEN],
    activity: Arc<Activity>,
    state: Mutex<PoaState>,
    /// Signalled when an `incarnate` or `unknown_adapter` call ends.
    changed: Condvar,
    /// The queue of a SINGLE_THREAD_MODEL POA.
    serial: Mutex<Serial>,
}

#[derive(Default)]
struct PoaState {
    destroyed: bool,
    children: HashMap<String, Arc<PoaInner>>,
    active: HashMap<ObjectId, Arc<dyn Servant>>,
    /// The ObjectId of each active servant, by address, for UNIQUE_ID.
    ids: HashMap<usize, ObjectId>,
    next_id: u32,
    default_servant: Option<Arc<dyn Servant>>,
    servant_manager: Option<ServantManager>,
    activator: Option<Arc<dyn AdapterActivator>>,
    /// ObjectIds whose `incarnate` call is under way.
    incarnating: HashSet<ObjectId>,
    /// Child names whose `unknown_adapter` call is under way.
    activating: HashSet<String>,
    /// Requests running on each object that was found in the active object
    /// map or incarnated.
    running: HashMap<ObjectId, usize>,
    /// Deactivated objects to etherealize once their last request ends.
    deactivated: HashMap<ObjectId, Arc<dyn Servant>>,
}

/// The queue of a SINGLE_THREAD_MODEL POA: at most one of its requests
/// runs at a time.
#[derive(Default)]
struct Serial {
    queue: VecDeque<Job>,
    /// Whether a request of the POA is running or about to.
    busy: bool,
    /// The thread running it.
    owner: Option<ThreadId>,
}

/// How a colocated call may enter a SINGLE_THREAD_MODEL POA.
pub(crate) enum SerialEntry {
    /// The thread already runs a request of the POA.
    Reentrant,
    /// The POA was idle and is now held by the thread until the guard is
    /// dropped.
    Acquired(SerialGuard),
    /// Another thread runs a request of the POA: queue the call.
    Busy,
}

pub(crate) struct SerialGuard {
    poa: Arc<PoaInner>,
    server: Arc<Server>,
}

impl Drop for SerialGuard {
    fn drop(&mut self) {
        self.poa.release_serial(&self.server);
    }
}

/// An object to etherealize: its POA, ObjectId and servant.
pub(crate) struct Etherealization {
    poa: Arc<PoaInner>,
    activator: Arc<dyn ServantActivator>,
    id: ObjectId,
    servant: Arc<dyn Servant>,
    cleanup_in_progress: bool,
}

impl Etherealization {
    pub(crate) fn poa(&self) -> &Arc<PoaInner> {
        &self.poa
    }

    /// Calls `etherealize`. `remaining` says whether the servant is still
    /// active under another ObjectId; `None` looks it up in the active
    /// object map.
    pub(crate) fn run(self, orb: &Orb, remaining: Option<bool>) {
        let remaining = remaining.unwrap_or_else(|| {
            let st = self.poa.state.lock().unwrap();
            st.active.values().any(|s| same_servant(s, &self.servant))
        });
        let adapter = Poa {
            orb: orb.clone(),
            inner: self.poa,
        };
        let _upcall = UpcallGuard::enter();
        let result = panic::catch_unwind(AssertUnwindSafe(|| {
            self.activator.etherealize(
                &self.id,
                &adapter,
                self.servant,
                self.cleanup_in_progress,
                remaining,
            );
        }));
        if result.is_err() {
            log::warn("servant activator panicked in etherealize");
        }
    }
}

/// Runs `etherealizations` in order, telling each whether its servant
/// still has a later one.
pub(crate) fn etherealize_all(orb: &Orb, etherealizations: Vec<Etherealization>) {
    let mut rest = etherealizations;
    while !rest.is_empty() {
        let e = rest.remove(0);
        let remaining = rest.iter().any(|r| same_servant(&r.servant, &e.servant));
        e.run(orb, Some(remaining));
    }
}

/// The servant found for a request, and what to do when it has run.
pub(crate) struct Resolved {
    pub(crate) servant: Arc<dyn Servant>,
    /// Counts the request against its object while it runs (RETAIN).
    _object: Option<ObjectGuard>,
    /// The locator and cookie for `postinvoke` (NON_RETAIN).
    locator: Option<(Arc<dyn ServantLocator>, Cookie)>,
}

impl Resolved {
    /// Calls `postinvoke` if a servant locator provided the servant.
    pub(crate) fn finish(self, adapter: &Poa, id: &[u8], operation: &str) {
        if let Some((locator, cookie)) = self.locator {
            let servant = self.servant;
            let result = panic::catch_unwind(AssertUnwindSafe(|| {
                locator.postinvoke(id, adapter, operation, cookie, servant);
            }));
            if result.is_err() {
                log::warn("servant locator panicked in postinvoke");
            }
        }
    }
}

pub(crate) struct ObjectGuard {
    poa: Arc<PoaInner>,
    server: Weak<Server>,
    id: ObjectId,
}

impl Drop for ObjectGuard {
    fn drop(&mut self) {
        let pending = {
            let mut st = self.poa.state.lock().unwrap();
            let Some(count) = st.running.get_mut(&self.id) else {
                return;
            };
            *count -= 1;
            if *count > 0 {
                return;
            }
            st.running.remove(&self.id);
            if st.destroyed {
                return;
            }
            let Some(servant) = st.deactivated.remove(&self.id) else {
                return;
            };
            let Some(ServantManager::Activator(activator)) = st.servant_manager.clone() else {
                return;
            };
            Etherealization {
                poa: Arc::clone(&self.poa),
                activator,
                id: std::mem::take(&mut self.id),
                servant,
                cleanup_in_progress: false,
            }
        };
        if let Some(server) = self.server.upgrade() {
            server.etherealize_later(pending);
        }
    }
}

impl fmt::Debug for Poa {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Poa")
            .field("name", &self.inner.name)
            .field("path", &self.inner.path)
            .field("policies", &self.inner.policies)
            .finish_non_exhaustive()
    }
}

impl PartialEq for Poa {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }
}

impl Eq for Poa {}

fn servant_addr(servant: &Arc<dyn Servant>) -> usize {
    Arc::as_ptr(servant).cast::<()>() as usize
}

fn same_servant(a: &Arc<dyn Servant>, b: &Arc<dyn Servant>) -> bool {
    servant_addr(a) == servant_addr(b)
}

/// The objects taken out of a POA, in ObjectId order, and its servant
/// activator.
type Drained = (
    Vec<(ObjectId, Arc<dyn Servant>)>,
    Option<Arc<dyn ServantActivator>>,
);

/// Empties the active object map, deactivated objects included.
fn drain(st: &mut PoaState) -> Drained {
    st.ids.clear();
    let mut entries: Vec<_> = std::mem::take(&mut st.active)
        .into_iter()
        .chain(std::mem::take(&mut st.deactivated))
        .collect();
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    let activator = match &st.servant_manager {
        Some(ServantManager::Activator(a)) => Some(Arc::clone(a)),
        _ => None,
    };
    (entries, activator)
}

fn catch<T>(
    what: &str,
    f: impl FnOnce() -> Result<T, ServantManagerError>,
) -> Result<T, ServantManagerError> {
    let _upcall = UpcallGuard::enter();
    panic::catch_unwind(AssertUnwindSafe(f)).unwrap_or_else(|_| {
        log::warn(&format!("servant manager panicked in {what}"));
        Err(SystemException::new(SystemExceptionKind::UNKNOWN, 0, CompletionStatus::Maybe).into())
    })
}

impl PoaInner {
    fn new(
        name: String,
        path: Vec<String>,
        policies: Policies,
        manager: PoaManager,
        parent: Option<Weak<PoaInner>>,
        key_prefix: Vec<u8>,
        boot_id: [u8; BOOT_ID_LEN],
    ) -> Arc<PoaInner> {
        let poa = Arc::new(PoaInner {
            name,
            path,
            policies,
            manager,
            parent,
            key_prefix,
            boot_id,
            activity: Arc::default(),
            state: Mutex::default(),
            changed: Condvar::new(),
            serial: Mutex::default(),
        });
        poa.manager.inner.add_poa(&poa);
        poa
    }

    pub(crate) fn new_root(manager: PoaManager, boot_id: [u8; BOOT_ID_LEN]) -> Arc<PoaInner> {
        let mut key_prefix = vec![TRANSIENT_END];
        key_prefix.extend_from_slice(&boot_id);
        PoaInner::new(
            "RootPOA".into(),
            Vec::new(),
            Policies::root(),
            manager,
            None,
            key_prefix,
            boot_id,
        )
    }

    fn new_child(
        parent: &Arc<PoaInner>,
        name: &str,
        manager: PoaManager,
        policies: Policies,
    ) -> Arc<PoaInner> {
        let mut path = parent.path.clone();
        path.push(name.to_owned());
        let mut key_prefix = vec![PATH_START];
        for (i, n) in path.iter().enumerate() {
            if i > 0 {
                key_prefix.push(PATH_SEPARATOR);
            }
            key_prefix.extend_from_slice(n.as_bytes());
        }
        match policies.lifespan {
            Lifespan::Persistent => key_prefix.push(PERSISTENT_END),
            Lifespan::Transient => {
                key_prefix.push(TRANSIENT_END);
                key_prefix.extend_from_slice(&parent.boot_id);
            }
        }
        PoaInner::new(
            name.to_owned(),
            path,
            policies,
            manager,
            Some(Arc::downgrade(parent)),
            key_prefix,
            parent.boot_id,
        )
    }

    pub(crate) fn policies(&self) -> &Policies {
        &self.policies
    }

    pub(crate) fn activity(&self) -> &Arc<Activity> {
        &self.activity
    }

    pub(crate) fn key(&self, id: &[u8]) -> Vec<u8> {
        let mut key = self.key_prefix.clone();
        key.extend_from_slice(id);
        key
    }

    /// The child `name`, or `Err(true)` if there is none but an adapter
    /// activator could create it.
    pub(crate) fn child(&self, name: &str) -> Result<Arc<PoaInner>, bool> {
        let st = self.state.lock().unwrap();
        if st.destroyed {
            return Err(false);
        }
        st.children.get(name).cloned().ok_or(st.activator.is_some())
    }

    /// The child `name`, created by the adapter activator if there is none.
    /// Calls user code: only on an upcall thread or in a colocated call.
    pub(crate) fn find_or_activate_child(
        self: &Arc<Self>,
        orb: &Orb,
        name: &str,
    ) -> Option<Arc<PoaInner>> {
        let mut st = self.state.lock().unwrap();
        loop {
            if st.destroyed {
                return None;
            }
            if let Some(child) = st.children.get(name) {
                return Some(Arc::clone(child));
            }
            let activator = st.activator.clone()?;
            if st.activating.contains(name) {
                st = self.changed.wait(st).unwrap();
                continue;
            }
            st.activating.insert(name.to_owned());
            drop(st);
            let parent = Poa {
                orb: orb.clone(),
                inner: Arc::clone(self),
            };
            let created = {
                let _upcall = UpcallGuard::enter();
                panic::catch_unwind(AssertUnwindSafe(|| {
                    activator.unknown_adapter(&parent, name)
                }))
                .unwrap_or_else(|_| {
                    log::warn("adapter activator panicked in unknown_adapter");
                    false
                })
            };
            st = self.state.lock().unwrap();
            st.activating.remove(name);
            self.changed.notify_all();
            if !created {
                return None;
            }
            return st.children.get(name).cloned();
        }
    }

    /// Whether a `LocateRequest` for `id` finds an object: one in the
    /// active object map, or any with a default servant or a servant
    /// manager (which are not asked).
    pub(crate) fn has_object(&self, id: &[u8]) -> bool {
        let st = self.state.lock().unwrap();
        !st.destroyed
            && (st.active.contains_key(id)
                || self.policies.request_processing != RequestProcessing::ActiveObjectMapOnly)
    }

    pub(crate) fn is_destroyed(&self) -> bool {
        self.state.lock().unwrap().destroyed
    }

    /// The servant for a request to `id`: from the active object map, the
    /// default servant or the servant manager. Calls user code.
    pub(crate) fn resolve(
        self: &Arc<Self>,
        adapter: &Poa,
        server: &Arc<Server>,
        id: &[u8],
        operation: &str,
    ) -> Result<Resolved, ServantManagerError> {
        let retain = self.policies.servant_retention == ServantRetention::Retain;
        let mut st = self.state.lock().unwrap();
        loop {
            if st.destroyed {
                return Err(no_such_object().into());
            }
            if retain {
                if let Some(servant) = st.active.get(id).cloned() {
                    return Ok(self.resolved(&mut st, server, id, servant));
                }
            }
            match self.policies.request_processing {
                RequestProcessing::ActiveObjectMapOnly => return Err(no_such_object().into()),
                RequestProcessing::UseDefaultServant => {
                    let servant = st.default_servant.clone().ok_or_else(|| {
                        sys(SystemExceptionKind::OBJ_ADAPTER, minor::NO_DEFAULT_SERVANT)
                    })?;
                    return Ok(Resolved {
                        servant,
                        _object: None,
                        locator: None,
                    });
                }
                RequestProcessing::UseServantManager => {}
            }
            match st.servant_manager.clone() {
                None => {
                    return Err(
                        sys(SystemExceptionKind::OBJ_ADAPTER, minor::NO_SERVANT_MANAGER).into(),
                    );
                }
                Some(ServantManager::Locator(locator)) => {
                    drop(st);
                    let (servant, cookie) =
                        catch("preinvoke", || locator.preinvoke(id, adapter, operation))?;
                    return Ok(Resolved {
                        servant,
                        _object: None,
                        locator: Some((locator, cookie)),
                    });
                }
                Some(ServantManager::Activator(activator)) => {
                    if st.incarnating.contains(id) {
                        st = self.changed.wait(st).unwrap();
                        continue;
                    }
                    st.incarnating.insert(id.to_vec());
                    drop(st);
                    let result = catch("incarnate", || activator.incarnate(id, adapter));
                    st = self.state.lock().unwrap();
                    st.incarnating.remove(id);
                    self.changed.notify_all();
                    let servant = result?;
                    if st.destroyed {
                        return Err(no_such_object().into());
                    }
                    match self.activate(&mut st, id.to_vec(), Arc::clone(&servant)) {
                        Ok(()) => return Ok(self.resolved(&mut st, server, id, servant)),
                        // Activated by the application meanwhile.
                        Err(PoaError::ObjectAlreadyActive) => continue,
                        Err(_) => {
                            return Err(sys(
                                SystemExceptionKind::OBJ_ADAPTER,
                                minor::SERVANT_ALREADY_ACTIVE,
                            )
                            .into());
                        }
                    }
                }
            }
        }
    }

    fn resolved(
        self: &Arc<Self>,
        st: &mut PoaState,
        server: &Arc<Server>,
        id: &[u8],
        servant: Arc<dyn Servant>,
    ) -> Resolved {
        *st.running.entry(id.to_vec()).or_insert(0) += 1;
        Resolved {
            servant,
            _object: Some(ObjectGuard {
                poa: Arc::clone(self),
                server: Arc::downgrade(server),
                id: id.to_vec(),
            }),
            locator: None,
        }
    }

    /// Marks this POA and its descendants destroyed, children first, and
    /// drops their servants. With `etherealize`, returns the objects to
    /// etherealize.
    pub(crate) fn destroy_tree(self: &Arc<Self>, etherealize: bool) -> Vec<Etherealization> {
        let (children, drained, rest) = {
            let mut st = self.state.lock().unwrap();
            if st.destroyed {
                return Vec::new();
            }
            st.destroyed = true;
            let children = std::mem::take(&mut st.children);
            let drained = drain(&mut st);
            // Dropped once the lock is released.
            let rest = (
                st.default_servant.take(),
                st.servant_manager.take(),
                st.activator.take(),
            );
            (children, drained, rest)
        };
        drop(rest);
        let mut all = Vec::new();
        for child in children.into_values() {
            all.extend(child.destroy_tree(etherealize));
        }
        all.extend(self.etherealizations(drained, etherealize));
        all
    }

    /// Deactivates every object for `POAManager::deactivate(etherealize)`.
    fn take_for_etherealize(self: &Arc<Self>) -> Vec<Etherealization> {
        let drained = {
            let mut st = self.state.lock().unwrap();
            if st.destroyed {
                return Vec::new();
            }
            drain(&mut st)
        };
        self.etherealizations(drained, true)
    }

    fn etherealizations(
        self: &Arc<Self>,
        (entries, activator): Drained,
        etherealize: bool,
    ) -> Vec<Etherealization> {
        let Some(activator) = activator.filter(|_| etherealize) else {
            return Vec::new();
        };
        entries
            .into_iter()
            .map(|(id, servant)| Etherealization {
                poa: Arc::clone(self),
                activator: Arc::clone(&activator),
                id,
                servant,
                cleanup_in_progress: true,
            })
            .collect()
    }

    fn generate_id(&self, st: &mut PoaState) -> ObjectId {
        let serial = st.next_id;
        st.next_id = st.next_id.wrapping_add(1);
        let mut id = self.boot_id.to_vec();
        id.extend_from_slice(&serial.to_be_bytes());
        id
    }

    fn activate(
        &self,
        st: &mut PoaState,
        id: ObjectId,
        servant: Arc<dyn Servant>,
    ) -> Result<(), PoaError> {
        if st.active.contains_key(&id) {
            return Err(PoaError::ObjectAlreadyActive);
        }
        if self.policies.id_uniqueness == IdUniqueness::Unique {
            let addr = servant_addr(&servant);
            if st.ids.contains_key(&addr) {
                return Err(PoaError::ServantAlreadyActive);
            }
            st.ids.insert(addr, id.clone());
        }
        st.active.insert(id, servant);
        Ok(())
    }

    // --- SINGLE_THREAD_MODEL ---------------------------------------------------

    /// Queues `job` behind the other requests of this POA.
    pub(crate) fn run_serially(self: &Arc<Self>, server: &Arc<Server>, job: Job) {
        let mut serial = self.serial.lock().unwrap();
        serial.queue.push_back(job);
        if !serial.busy {
            serial.busy = true;
            drop(serial);
            self.submit_serial(server);
        }
    }

    fn submit_serial(self: &Arc<Self>, server: &Arc<Server>) {
        let poa = Arc::clone(self);
        let srv = Arc::clone(server);
        server.submit(Box::new(move || poa.run_one_serially(&srv)));
    }

    /// Runs the first queued request; on an upcall thread.
    fn run_one_serially(self: &Arc<Self>, server: &Arc<Server>) {
        let job = {
            let mut serial = self.serial.lock().unwrap();
            let Some(job) = serial.queue.pop_front() else {
                serial.busy = false;
                return;
            };
            serial.owner = Some(thread::current().id());
            job
        };
        let _ = panic::catch_unwind(AssertUnwindSafe(job));
        self.release_serial(server);
    }

    fn release_serial(self: &Arc<Self>, server: &Arc<Server>) {
        let mut serial = self.serial.lock().unwrap();
        serial.owner = None;
        if serial.queue.is_empty() {
            serial.busy = false;
        } else {
            drop(serial);
            self.submit_serial(server);
        }
    }

    /// Lets a colocated call run on the calling thread if that keeps the
    /// requests of the POA one at a time.
    pub(crate) fn enter_serially(self: &Arc<Self>, server: &Arc<Server>) -> SerialEntry {
        let me = thread::current().id();
        let mut serial = self.serial.lock().unwrap();
        if serial.owner == Some(me) {
            SerialEntry::Reentrant
        } else if serial.busy {
            SerialEntry::Busy
        } else {
            serial.busy = true;
            serial.owner = Some(me);
            SerialEntry::Acquired(SerialGuard {
                poa: Arc::clone(self),
                server: Arc::clone(server),
            })
        }
    }
}

/// How a request's object key splits up.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ParsedKey<'a> {
    /// A key made by a POA of this format: the path of POA names, the boot
    /// id for a TRANSIENT POA, and the ObjectId.
    Poa {
        path: Vec<&'a str>,
        boot_id: Option<&'a [u8]>,
        id: &'a [u8],
    },
    /// Any other key, looked up among the objects registered by name.
    Simple(&'a [u8]),
    /// A key that starts like a POA key but is cut short or not UTF-8.
    Invalid,
}

pub(crate) fn parse_key(key: &[u8]) -> ParsedKey<'_> {
    fn transient<'a>(rest: &'a [u8], path: Vec<&'a str>) -> ParsedKey<'a> {
        if rest.len() < BOOT_ID_LEN {
            return ParsedKey::Invalid;
        }
        let (boot_id, id) = rest.split_at(BOOT_ID_LEN);
        ParsedKey::Poa {
            path,
            boot_id: Some(boot_id),
            id,
        }
    }
    match key.first() {
        Some(&TRANSIENT_END) => transient(&key[1..], Vec::new()),
        Some(&PATH_START) => {
            let rest = &key[1..];
            let Some(end) = rest
                .iter()
                .position(|&b| b == PERSISTENT_END || b == TRANSIENT_END)
            else {
                return ParsedKey::Invalid;
            };
            let mut path = Vec::new();
            for name in rest[..end].split(|&b| b == PATH_SEPARATOR) {
                match std::str::from_utf8(name) {
                    Ok(name) if !name.is_empty() => path.push(name),
                    _ => return ParsedKey::Invalid,
                }
            }
            if rest[end] == PERSISTENT_END {
                ParsedKey::Poa {
                    path,
                    boot_id: None,
                    id: &rest[end + 1..],
                }
            } else {
                transient(&rest[end + 1..], path)
            }
        }
        _ => ParsedKey::Simple(key),
    }
}

impl Poa {
    fn state(&self) -> Result<MutexGuard<'_, PoaState>, PoaError> {
        let st = self.inner.state.lock().unwrap();
        if st.destroyed {
            return Err(sys(SystemExceptionKind::OBJECT_NOT_EXIST, 0).into());
        }
        Ok(st)
    }

    fn server(&self) -> Result<Arc<Server>, PoaError> {
        self.orb
            .server()
            .ok_or_else(|| sys(SystemExceptionKind::OBJECT_NOT_EXIST, 0).into())
    }

    fn handle(&self, inner: Arc<PoaInner>) -> Poa {
        Poa {
            orb: self.orb.clone(),
            inner,
        }
    }

    /// `the_name`.
    pub fn the_name(&self) -> &str {
        &self.inner.name
    }

    /// `the_parent`; `None` for the RootPOA.
    pub fn the_parent(&self) -> Option<Poa> {
        let parent = self.inner.parent.as_ref()?.upgrade()?;
        Some(self.handle(parent))
    }

    /// `the_children`, in no particular order.
    pub fn the_children(&self) -> Vec<Poa> {
        let st = self.inner.state.lock().unwrap();
        st.children
            .values()
            .map(|c| self.handle(Arc::clone(c)))
            .collect()
    }

    /// `the_POAManager`.
    pub fn the_poa_manager(&self) -> PoaManager {
        self.inner.manager.clone()
    }

    /// The policies of the POA.
    pub fn policies(&self) -> Policies {
        self.inner.policies
    }

    /// `create_POA`: a child POA named `name`, managed by `manager` or, if
    /// `None`, by a new manager. Policies not in `policies` take their
    /// default values (not those of this POA). Inconsistent policies fail
    /// with `InvalidPolicy`, and a name containing NUL with `BAD_PARAM`.
    pub fn create_poa(
        &self,
        name: &str,
        manager: Option<&PoaManager>,
        policies: &[Policy],
    ) -> Result<Poa, PoaError> {
        if name.is_empty() || name.contains('\0') {
            return Err(sys(SystemExceptionKind::BAD_PARAM, 0).into());
        }
        let policies = Policies::from_list(policies).map_err(PoaError::InvalidPolicy)?;
        let server = self.server()?;
        let mut st = self.state()?;
        if st.children.contains_key(name) {
            return Err(PoaError::AdapterAlreadyExists);
        }
        let manager = match manager {
            Some(m) => m.clone(),
            None => PoaManager::new(&server),
        };
        if policies.lifespan == Lifespan::Persistent && !server.has_fixed_port() {
            log::warn(&format!(
                "PERSISTENT POA {name:?} on an endpoint without a fixed port: \
                 its references will not survive a restart"
            ));
        }
        let child = PoaInner::new_child(&self.inner, name, manager, policies);
        st.children.insert(name.to_owned(), Arc::clone(&child));
        Ok(self.handle(child))
    }

    /// `find_POA`: the child `name`. With `activate_it`, a missing child
    /// is asked of the adapter activator, if the POA has one.
    pub fn find_poa(&self, name: &str, activate_it: bool) -> Result<Poa, PoaError> {
        drop(self.state()?);
        let child = if activate_it {
            self.inner.find_or_activate_child(&self.orb, name)
        } else {
            self.inner.child(name).ok()
        };
        child
            .map(|c| self.handle(c))
            .ok_or(PoaError::AdapterNonExistent)
    }

    /// `the_activator`.
    pub fn the_activator(&self) -> Option<Arc<dyn AdapterActivator>> {
        self.inner.state.lock().unwrap().activator.clone()
    }

    /// `the_activator` (set): the adapter activator for unknown children,
    /// or none.
    pub fn set_the_activator(
        &self,
        activator: Option<Arc<dyn AdapterActivator>>,
    ) -> Result<(), PoaError> {
        let old = std::mem::replace(&mut self.state()?.activator, activator);
        drop(old);
        Ok(())
    }

    /// `get_servant_manager` (USE_SERVANT_MANAGER only).
    pub fn get_servant_manager(&self) -> Result<Option<ServantManager>, PoaError> {
        if self.inner.policies.request_processing != RequestProcessing::UseServantManager {
            return Err(PoaError::WrongPolicy);
        }
        Ok(self.state()?.servant_manager.clone())
    }

    /// `set_servant_manager` (USE_SERVANT_MANAGER only): a
    /// [`ServantActivator`] with RETAIN, a [`ServantLocator`] with
    /// NON_RETAIN (otherwise `OBJ_ADAPTER`). It can be set once
    /// (`BAD_INV_ORDER` after that).
    pub fn set_servant_manager(&self, manager: ServantManager) -> Result<(), PoaError> {
        let p = self.inner.policies;
        if p.request_processing != RequestProcessing::UseServantManager {
            return Err(PoaError::WrongPolicy);
        }
        let fits = matches!(
            (&manager, p.servant_retention),
            (ServantManager::Activator(_), ServantRetention::Retain)
                | (ServantManager::Locator(_), ServantRetention::NonRetain)
        );
        if !fits {
            return Err(sys(SystemExceptionKind::OBJ_ADAPTER, minor::NO_SERVANT_MANAGER).into());
        }
        let mut st = self.state()?;
        if st.servant_manager.is_some() {
            return Err(sys(
                SystemExceptionKind::BAD_INV_ORDER,
                minor::SERVANT_MANAGER_ALREADY_SET,
            )
            .into());
        }
        st.servant_manager = Some(manager);
        Ok(())
    }

    /// `get_servant`: the default servant (USE_DEFAULT_SERVANT only).
    pub fn get_servant(&self) -> Result<Arc<dyn Servant>, PoaError> {
        if self.inner.policies.request_processing != RequestProcessing::UseDefaultServant {
            return Err(PoaError::WrongPolicy);
        }
        self.state()?
            .default_servant
            .clone()
            .ok_or(PoaError::NoServant)
    }

    /// `set_servant`: the default servant (USE_DEFAULT_SERVANT only), which
    /// runs the requests for objects not in the active object map.
    pub fn set_servant(&self, servant: Arc<dyn Servant>) -> Result<(), PoaError> {
        if self.inner.policies.request_processing != RequestProcessing::UseDefaultServant {
            return Err(PoaError::WrongPolicy);
        }
        let old = self.state()?.default_servant.replace(servant);
        drop(old);
        Ok(())
    }

    /// `destroy`: destroys the POA and its descendants and drops their
    /// servants. Requests already running finish; later ones get
    /// `OBJECT_NOT_EXIST`. With `etherealize`, the objects of POAs with a
    /// [`ServantActivator`] are etherealized once their requests have
    /// finished. With `wait_for_completion`, returns once the running
    /// requests of this POA have finished and the objects are etherealized
    /// (`BAD_INV_ORDER` when called from a servant).
    pub fn destroy(&self, etherealize: bool, wait_for_completion: bool) -> Result<(), PoaError> {
        check_wait(wait_for_completion)?;
        if let Some(parent) = self.inner.parent.as_ref().and_then(Weak::upgrade) {
            let mut st = parent.state.lock().unwrap();
            if st
                .children
                .get(&self.inner.name)
                .is_some_and(|c| Arc::ptr_eq(c, &self.inner))
            {
                st.children.remove(&self.inner.name);
            }
        }
        let work = self.inner.destroy_tree(etherealize);
        if wait_for_completion {
            self.inner.activity.wait_idle();
        }
        if let Some(server) = self.orb.server() {
            server.etherealize(work, wait_for_completion);
        }
        Ok(())
    }

    fn check_retain(&self) -> Result<(), PoaError> {
        if self.inner.policies.servant_retention != ServantRetention::Retain {
            return Err(PoaError::WrongPolicy);
        }
        Ok(())
    }

    /// `activate_object`: activates `servant` under a new ObjectId
    /// (SYSTEM_ID and RETAIN only).
    pub fn activate_object(&self, servant: Arc<dyn Servant>) -> Result<ObjectId, PoaError> {
        self.check_retain()?;
        if self.inner.policies.id_assignment != IdAssignment::System {
            return Err(PoaError::WrongPolicy);
        }
        let mut st = self.state()?;
        let id = self.inner.generate_id(&mut st);
        self.inner.activate(&mut st, id.clone(), servant)?;
        Ok(id)
    }

    /// `activate_object_with_id` (RETAIN only).
    pub fn activate_object_with_id(
        &self,
        id: &[u8],
        servant: Arc<dyn Servant>,
    ) -> Result<(), PoaError> {
        self.check_retain()?;
        let mut st = self.state()?;
        self.inner.activate(&mut st, id.to_vec(), servant)
    }

    /// `deactivate_object` (RETAIN only): requests already running finish;
    /// later ones get `OBJECT_NOT_EXIST`, or incarnate the object again
    /// with a [`ServantActivator`]. The activator's `etherealize` is called
    /// on an upcall thread once the running requests have finished; this
    /// does not wait for it.
    pub fn deactivate_object(&self, id: &[u8]) -> Result<(), PoaError> {
        self.check_retain()?;
        let (servant, now) = {
            let mut st = self.state()?;
            let servant = st.active.remove(id).ok_or(PoaError::ObjectNotActive)?;
            let addr = servant_addr(&servant);
            if st.ids.get(&addr).is_some_and(|i| i == id) {
                st.ids.remove(&addr);
            }
            match st.servant_manager.clone() {
                Some(ServantManager::Activator(activator)) => {
                    if st.running.contains_key(id) {
                        st.deactivated.insert(id.to_vec(), servant);
                        (None, None)
                    } else {
                        (None, Some((activator, servant)))
                    }
                }
                _ => (Some(servant), None),
            }
        };
        drop(servant);
        if let Some((activator, servant)) = now {
            let work = Etherealization {
                poa: Arc::clone(&self.inner),
                activator,
                id: id.to_vec(),
                servant,
                cleanup_in_progress: false,
            };
            if let Some(server) = self.orb.server() {
                server.etherealize_later(work);
            }
        }
        Ok(())
    }

    /// `servant_to_id`: the ObjectId of an active servant (UNIQUE_ID), a
    /// new one with IMPLICIT_ACTIVATION, or, for the default servant inside
    /// a request it runs, the ObjectId of that request.
    pub fn servant_to_id(&self, servant: &Arc<dyn Servant>) -> Result<ObjectId, PoaError> {
        let p = self.inner.policies;
        let retain = p.servant_retention == ServantRetention::Retain;
        let default_servant = p.request_processing == RequestProcessing::UseDefaultServant;
        if !default_servant
            && !(retain
                && (p.id_uniqueness == IdUniqueness::Unique
                    || p.implicit_activation == ImplicitActivation::Implicit))
        {
            return Err(PoaError::WrongPolicy);
        }
        let mut st = self.state()?;
        if retain && p.id_uniqueness == IdUniqueness::Unique {
            if let Some(id) = st.ids.get(&servant_addr(servant)) {
                return Ok(id.clone());
            }
        }
        if retain && p.implicit_activation == ImplicitActivation::Implicit {
            let id = self.inner.generate_id(&mut st);
            self.inner
                .activate(&mut st, id.clone(), Arc::clone(servant))?;
            return Ok(id);
        }
        let is_default = st
            .default_servant
            .as_ref()
            .is_some_and(|d| same_servant(d, servant));
        drop(st);
        if is_default {
            let current = CURRENT.with(|c| {
                c.borrow()
                    .last()
                    .filter(|e| e.poa == *self && same_servant(&e.servant, servant))
                    .map(|e| e.id.clone())
            });
            if let Some(id) = current {
                return Ok(id);
            }
        }
        Err(PoaError::ServantNotActive)
    }

    /// `servant_to_reference`, activating the servant like
    /// [`servant_to_id`](Poa::servant_to_id).
    pub fn servant_to_reference(&self, servant: &Arc<dyn Servant>) -> Result<ObjectRef, PoaError> {
        let id = self.servant_to_id(servant)?;
        self.reference(&id, servant.repository_id())
    }

    /// `id_to_servant` (RETAIN or USE_DEFAULT_SERVANT): the servant active
    /// under `id`, or else the default servant.
    pub fn id_to_servant(&self, id: &[u8]) -> Result<Arc<dyn Servant>, PoaError> {
        let p = self.inner.policies;
        let retain = p.servant_retention == ServantRetention::Retain;
        let default_servant = p.request_processing == RequestProcessing::UseDefaultServant;
        if !retain && !default_servant {
            return Err(PoaError::WrongPolicy);
        }
        let st = self.state()?;
        let active = retain.then(|| st.active.get(id).cloned()).flatten();
        active
            .or_else(|| st.default_servant.clone().filter(|_| default_servant))
            .ok_or(PoaError::ObjectNotActive)
    }

    /// `id_to_reference` (RETAIN only): a reference to the active object
    /// `id`, with the servant's repository id as type id.
    pub fn id_to_reference(&self, id: &[u8]) -> Result<ObjectRef, PoaError> {
        self.check_retain()?;
        let servant = self
            .state()?
            .active
            .get(id)
            .cloned()
            .ok_or(PoaError::ObjectNotActive)?;
        self.reference(id, servant.repository_id())
    }

    /// `create_reference`: a reference with a new ObjectId and the type id
    /// `intf`, without activating anything (SYSTEM_ID only).
    pub fn create_reference(&self, intf: &str) -> Result<ObjectRef, PoaError> {
        if self.inner.policies.id_assignment != IdAssignment::System {
            return Err(PoaError::WrongPolicy);
        }
        let id = {
            let mut st = self.state()?;
            self.inner.generate_id(&mut st)
        };
        self.reference(&id, intf)
    }

    /// `create_reference_with_id`: a reference to `id` with the type id
    /// `intf`, without activating anything.
    pub fn create_reference_with_id(&self, id: &[u8], intf: &str) -> Result<ObjectRef, PoaError> {
        drop(self.state()?);
        self.reference(id, intf)
    }

    /// `reference_to_id`: the ObjectId of a reference made by this POA
    /// (`WrongAdapter` for any other).
    pub fn reference_to_id(&self, reference: &ObjectRef) -> Result<ObjectId, PoaError> {
        drop(self.state()?);
        let prefix = &self.inner.key_prefix;
        reference
            .ior()
            .iiop_profiles()
            .filter_map(Result::ok)
            .map(|p: IiopProfile| p.object_key)
            .find(|key| key.starts_with(prefix))
            .map(|key| key[prefix.len()..].to_vec())
            .ok_or(PoaError::WrongAdapter)
    }

    /// `reference_to_servant`.
    pub fn reference_to_servant(
        &self,
        reference: &ObjectRef,
    ) -> Result<Arc<dyn Servant>, PoaError> {
        let id = self.reference_to_id(reference)?;
        self.id_to_servant(&id)
    }

    pub(crate) fn reference(&self, id: &[u8], type_id: &str) -> Result<ObjectRef, PoaError> {
        let server = self.server()?;
        let ior = server.ior(type_id, self.inner.key(id))?;
        Ok(ObjectRef::new(self.orb.clone(), ior))
    }
}

pub(crate) mod log {
    /// Writes a warning to standard error, like omniORB's trace output.
    pub(crate) fn warn(msg: &str) {
        eprintln!("rustorb: warning: {msg}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_parse_back() {
        let boot = [1, 2, 3, 4, 5, 6, 7, 8];
        assert_eq!(
            parse_key(b"\xffpersistent_user\x00obj"),
            ParsedKey::Poa {
                path: vec!["persistent_user"],
                boot_id: None,
                id: b"obj"
            }
        );
        assert_eq!(
            parse_key(b"\xffa\xffnested\x00"),
            ParsedKey::Poa {
                path: vec!["a", "nested"],
                boot_id: None,
                id: b""
            }
        );
        let mut transient = b"\xffchild\xfe".to_vec();
        transient.extend_from_slice(&boot);
        transient.extend_from_slice(b"id");
        assert_eq!(
            parse_key(&transient),
            ParsedKey::Poa {
                path: vec!["child"],
                boot_id: Some(&boot),
                id: b"id"
            }
        );
        let mut root = vec![0xfe];
        root.extend_from_slice(&boot);
        assert_eq!(
            parse_key(&root),
            ParsedKey::Poa {
                path: vec![],
                boot_id: Some(&boot),
                id: b""
            }
        );
        assert_eq!(parse_key(b"NameService"), ParsedKey::Simple(b"NameService"));
        assert_eq!(parse_key(b""), ParsedKey::Simple(b""));
        assert_eq!(parse_key(b"\xffno-end"), ParsedKey::Invalid);
        assert_eq!(parse_key(b"\xfe\x01\x02"), ParsedKey::Invalid);
        assert_eq!(parse_key(b"\xff\xff\x00x"), ParsedKey::Invalid);
    }

    #[test]
    fn policy_lists() {
        assert_eq!(Policies::from_list(&[]), Ok(Policies::default()));
        let p = Policies::from_list(&[
            Policy::Lifespan(Lifespan::Persistent),
            Policy::IdAssignment(IdAssignment::User),
        ])
        .unwrap();
        assert_eq!(p.lifespan, Lifespan::Persistent);
        assert_eq!(p.id_assignment, IdAssignment::User);
        assert_eq!(
            Policies::from_list(&[
                Policy::Lifespan(Lifespan::Persistent),
                Policy::Lifespan(Lifespan::Transient),
            ]),
            Err(1)
        );
        assert_eq!(
            Policies::from_list(&[
                Policy::IdAssignment(IdAssignment::User),
                Policy::ImplicitActivation(ImplicitActivation::Implicit),
            ]),
            Err(1)
        );
        assert_eq!(
            Policies::from_list(&[Policy::Thread(ThreadPolicy::MainThread)])
                .unwrap()
                .thread,
            ThreadPolicy::MainThread
        );
        // NON_RETAIN needs a default servant or a servant manager.
        assert_eq!(
            Policies::from_list(&[Policy::ServantRetention(ServantRetention::NonRetain)]),
            Err(0)
        );
        assert!(
            Policies::from_list(&[
                Policy::ServantRetention(ServantRetention::NonRetain),
                Policy::RequestProcessing(RequestProcessing::UseServantManager),
            ])
            .is_ok()
        );
        // A default servant serves many ObjectIds.
        assert_eq!(
            Policies::from_list(&[Policy::RequestProcessing(
                RequestProcessing::UseDefaultServant
            )]),
            Err(0)
        );
        assert!(
            Policies::from_list(&[
                Policy::RequestProcessing(RequestProcessing::UseDefaultServant),
                Policy::IdUniqueness(IdUniqueness::Multiple),
            ])
            .is_ok()
        );
        assert_eq!(
            Policies::from_list(&[
                Policy::ImplicitActivation(ImplicitActivation::Implicit),
                Policy::ServantRetention(ServantRetention::NonRetain),
                Policy::RequestProcessing(RequestProcessing::UseServantManager),
            ]),
            Err(0)
        );
    }
}

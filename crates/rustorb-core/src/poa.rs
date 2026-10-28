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

use std::cell::Cell;
use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::sync::{Arc, Condvar, Mutex, Weak};

use rustorb_giop::IiopProfile;

use crate::server::Server;
use crate::{
    CompletionStatus, ObjectRef, Orb, Servant, SystemException, SystemExceptionKind, minor,
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
    /// `SINGLE_THREAD_MODEL` (not supported yet).
    SingleThread,
    /// `MAIN_THREAD_MODEL` (not supported yet).
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
    /// No active object map (not supported yet).
    NonRetain,
}

/// `PortableServer::RequestProcessingPolicyValue`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RequestProcessing {
    /// Requests go to servants in the active object map only.
    ActiveObjectMapOnly,
    /// A default servant (not supported yet).
    UseDefaultServant,
    /// A servant manager (not supported yet).
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
    /// policy that is repeated, not supported yet, or inconsistent with the
    /// others.
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
                Policy::Thread(ThreadPolicy::OrbCtrl) => p.thread = ThreadPolicy::OrbCtrl,
                Policy::Lifespan(v) => p.lifespan = v,
                Policy::IdUniqueness(v) => p.id_uniqueness = v,
                Policy::IdAssignment(v) => p.id_assignment = v,
                Policy::ImplicitActivation(v) => p.implicit_activation = v,
                Policy::ServantRetention(ServantRetention::Retain) => {}
                Policy::RequestProcessing(RequestProcessing::ActiveObjectMapOnly) => {}
                // Planned with the rest of the POA (see docs/poa-design.md).
                Policy::Thread(_) | Policy::ServantRetention(_) | Policy::RequestProcessing(_) => {
                    return Err(index(i));
                }
            }
        }
        if p.implicit_activation == ImplicitActivation::Implicit
            && p.id_assignment != IdAssignment::System
        {
            let i = list
                .iter()
                .position(|q| matches!(q, Policy::ImplicitActivation(_)))
                .unwrap_or(0);
            return Err(index(i));
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
    /// Run it; called on an upcall thread.
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
        if let Some(server) = self.inner.server.upgrade() {
            for dispatch in held {
                server.submit(Box::new(move || dispatch(Disposition::Run)));
            }
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
    /// too. `etherealize` is for servant managers, which are not supported
    /// yet.
    pub fn deactivate(
        &self,
        _etherealize: bool,
        wait_for_completion: bool,
    ) -> Result<(), PoaError> {
        check_wait(wait_for_completion)?;
        self.set(PoaManagerState::Inactive)?;
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
    pub(crate) fn dispatch(&self, server: &Server, dispatch: Dispatch) {
        let mut st = self.state.lock().unwrap();
        match st.state {
            PoaManagerState::Active => {
                drop(st);
                server.submit(Box::new(move || dispatch(Disposition::Run)));
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
}

#[derive(Default)]
struct PoaState {
    destroyed: bool,
    children: HashMap<String, Arc<PoaInner>>,
    active: HashMap<ObjectId, Arc<dyn Servant>>,
    /// The ObjectId of each active servant, by address, for UNIQUE_ID.
    ids: HashMap<usize, ObjectId>,
    next_id: u32,
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

impl PoaInner {
    pub(crate) fn new_root(manager: PoaManager, boot_id: [u8; BOOT_ID_LEN]) -> Arc<PoaInner> {
        let mut key_prefix = vec![TRANSIENT_END];
        key_prefix.extend_from_slice(&boot_id);
        Arc::new(PoaInner {
            name: "RootPOA".into(),
            path: Vec::new(),
            policies: Policies::root(),
            manager,
            parent: None,
            key_prefix,
            boot_id,
            activity: Arc::default(),
            state: Mutex::default(),
        })
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
        Arc::new(PoaInner {
            name: name.to_owned(),
            path,
            policies,
            manager,
            parent: Some(Arc::downgrade(parent)),
            key_prefix,
            boot_id: parent.boot_id,
            activity: Arc::default(),
            state: Mutex::default(),
        })
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

    pub(crate) fn child(&self, name: &str) -> Option<Arc<PoaInner>> {
        let st = self.state.lock().unwrap();
        if st.destroyed {
            return None;
        }
        st.children.get(name).cloned()
    }

    /// The servant active under `id`, if the POA still exists.
    pub(crate) fn servant(&self, id: &[u8]) -> Option<Arc<dyn Servant>> {
        let st = self.state.lock().unwrap();
        if st.destroyed {
            return None;
        }
        st.active.get(id).cloned()
    }

    pub(crate) fn is_destroyed(&self) -> bool {
        self.state.lock().unwrap().destroyed
    }

    /// Marks this POA and its descendants destroyed and drops their
    /// servants, children first.
    pub(crate) fn destroy_tree(&self) {
        let (children, active) = {
            let mut st = self.state.lock().unwrap();
            if st.destroyed {
                return;
            }
            st.destroyed = true;
            st.ids.clear();
            (
                std::mem::take(&mut st.children),
                std::mem::take(&mut st.active),
            )
        };
        for child in children.into_values() {
            child.destroy_tree();
        }
        drop(active);
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
    fn state(&self) -> Result<std::sync::MutexGuard<'_, PoaState>, PoaError> {
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
    /// default values (not those of this POA).
    ///
    /// SINGLE_THREAD_MODEL, MAIN_THREAD_MODEL, NON_RETAIN,
    /// USE_DEFAULT_SERVANT and USE_SERVANT_MANAGER are not supported yet
    /// and fail with `InvalidPolicy`. A name containing NUL is refused with
    /// `BAD_PARAM`.
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

    /// `find_POA`. Adapter activators are not supported yet, so
    /// `activate_it` has no effect.
    pub fn find_poa(&self, name: &str, _activate_it: bool) -> Result<Poa, PoaError> {
        let st = self.state()?;
        st.children
            .get(name)
            .map(|c| self.handle(Arc::clone(c)))
            .ok_or(PoaError::AdapterNonExistent)
    }

    /// `destroy`: destroys the POA and its descendants and drops their
    /// servants. Requests already running finish; later ones get
    /// `OBJECT_NOT_EXIST`. With `wait_for_completion`, returns once the
    /// running requests of this POA have finished (`BAD_INV_ORDER` when
    /// called from a servant). `etherealize` is for servant managers, which
    /// are not supported yet.
    pub fn destroy(&self, _etherealize: bool, wait_for_completion: bool) -> Result<(), PoaError> {
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
        self.inner.destroy_tree();
        if wait_for_completion {
            self.inner.activity.wait_idle();
        }
        Ok(())
    }

    /// `activate_object`: activates `servant` under a new ObjectId
    /// (SYSTEM_ID only).
    pub fn activate_object(&self, servant: Arc<dyn Servant>) -> Result<ObjectId, PoaError> {
        if self.inner.policies.id_assignment != IdAssignment::System {
            return Err(PoaError::WrongPolicy);
        }
        let mut st = self.state()?;
        let id = self.inner.generate_id(&mut st);
        self.inner.activate(&mut st, id.clone(), servant)?;
        Ok(id)
    }

    /// `activate_object_with_id`.
    pub fn activate_object_with_id(
        &self,
        id: &[u8],
        servant: Arc<dyn Servant>,
    ) -> Result<(), PoaError> {
        let mut st = self.state()?;
        self.inner.activate(&mut st, id.to_vec(), servant)
    }

    /// `deactivate_object`: requests already running finish; later ones get
    /// `OBJECT_NOT_EXIST`.
    pub fn deactivate_object(&self, id: &[u8]) -> Result<(), PoaError> {
        let servant = {
            let mut st = self.state()?;
            let servant = st.active.remove(id).ok_or(PoaError::ObjectNotActive)?;
            let addr = servant_addr(&servant);
            if st.ids.get(&addr).is_some_and(|i| i == id) {
                st.ids.remove(&addr);
            }
            servant
        };
        drop(servant);
        Ok(())
    }

    /// `servant_to_id`: the ObjectId of an active servant (UNIQUE_ID), or
    /// a new one with IMPLICIT_ACTIVATION.
    pub fn servant_to_id(&self, servant: &Arc<dyn Servant>) -> Result<ObjectId, PoaError> {
        let p = self.inner.policies;
        let mut st = self.state()?;
        if p.id_uniqueness == IdUniqueness::Unique {
            if let Some(id) = st.ids.get(&servant_addr(servant)) {
                return Ok(id.clone());
            }
        }
        if p.implicit_activation == ImplicitActivation::Implicit {
            let id = self.inner.generate_id(&mut st);
            self.inner
                .activate(&mut st, id.clone(), Arc::clone(servant))?;
            return Ok(id);
        }
        if p.id_uniqueness == IdUniqueness::Unique {
            Err(PoaError::ServantNotActive)
        } else {
            Err(PoaError::WrongPolicy)
        }
    }

    /// `servant_to_reference`, activating the servant like
    /// [`servant_to_id`](Poa::servant_to_id).
    pub fn servant_to_reference(&self, servant: &Arc<dyn Servant>) -> Result<ObjectRef, PoaError> {
        let id = self.servant_to_id(servant)?;
        self.reference(&id, servant.repository_id())
    }

    /// `id_to_servant`.
    pub fn id_to_servant(&self, id: &[u8]) -> Result<Arc<dyn Servant>, PoaError> {
        let st = self.state()?;
        st.active.get(id).cloned().ok_or(PoaError::ObjectNotActive)
    }

    /// `id_to_reference`: a reference to the active object `id`, with the
    /// servant's repository id as type id.
    pub fn id_to_reference(&self, id: &[u8]) -> Result<ObjectRef, PoaError> {
        let servant = self.id_to_servant(id)?;
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

    fn reference(&self, id: &[u8], type_id: &str) -> Result<ObjectRef, PoaError> {
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
            Policies::from_list(&[Policy::Thread(ThreadPolicy::MainThread)]),
            Err(0)
        );
    }
}

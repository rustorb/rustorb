//! Portable Interceptors (`PortableInterceptor`): hooks on the requests an
//! ORB sends and receives and on the references its POAs make.
//!
//! An [`OrbInitializer`], registered for every ORB
//! ([`register_orb_initializer`]) or for one
//! ([`OrbOptions::initializers`](crate::OrbOptions::initializers)), adds
//! interceptors while the ORB is created:
//!
//! - a [`ClientRequestInterceptor`] sees each request the ORB sends, before
//!   it goes out and when it ends;
//! - a [`ServerRequestInterceptor`] sees each request the ORB's POAs
//!   receive, before the servant runs and before the reply goes out;
//! - an [`IorInterceptor`] adds tagged components to the references of a
//!   new POA.
//!
//! Interceptors carry data in service contexts, usually encoded with an
//! [`IOP::Codec`](crate::iop::Codec), and hand it to the application and
//! back through the slots of [`PiCurrent`]. The design, and how rustorb
//! fits the specification, is in docs/portable-interceptors.md.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::fmt;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rustorb_giop::{Ior, TAG_INTERNET_IOP};
pub use rustorb_giop::{ServiceContext, TaggedComponent, TaggedProfile};

use crate::dii::{ArgMode, NamedValue};
use crate::iop::CodecFactory;
use crate::poa::{Policies, Policy};
use crate::policy::{PolicyType, PolicyValue};
use crate::typecode::TCKind;
use crate::{
    Any, CompletionStatus, Exception, ObjectRef, Orb, Poa, Reply, ResolveError, Servant,
    SystemException, SystemExceptionKind, TypeCode, Value, trace,
};

/// Minor codes of the exceptions the interceptor interfaces raise.
pub mod minor {
    use crate::OMG_VMCID;

    /// `BAD_INV_ORDER`: the attribute or operation cannot be used at this
    /// interception point (or, on `ORBInitInfo`, at this stage).
    pub const INVALID_POINT: u32 = OMG_VMCID | 14;
    /// `BAD_INV_ORDER`: a service context with that id is there already,
    /// and `replace` was false.
    pub const DUPLICATE_SERVICE_CONTEXT: u32 = OMG_VMCID | 15;
    /// `BAD_PARAM`: no service context with that id.
    pub const NO_SERVICE_CONTEXT: u32 = OMG_VMCID | 26;
    /// `BAD_PARAM`: no tagged component with that tag.
    pub const NO_COMPONENT: u32 = OMG_VMCID | 28;
    /// `BAD_PARAM`: a profile id other than `TAG_INTERNET_IOP`.
    pub const UNKNOWN_PROFILE: u32 = OMG_VMCID | 29;
    /// `NO_RESOURCES`: the language binding does not have the
    /// information (arguments of a static stub, say).
    pub const NOT_AVAILABLE: u32 = OMG_VMCID | 1;
    /// `INV_POLICY`: the policy type is not allowed here.
    pub const INVALID_POLICY_TYPE: u32 = OMG_VMCID | 2;
    /// `OBJECT_NOT_EXIST`: the `ORBInitInfo` was used after the ORB was
    /// created (a binding checks this).
    pub const INIT_INFO_GONE: u32 = OMG_VMCID | 10;
}

/// `PortableInterceptor::ReplyStatus` values.
pub mod reply_status {
    /// `SUCCESSFUL`.
    pub const SUCCESSFUL: i16 = 0;
    /// `SYSTEM_EXCEPTION`.
    pub const SYSTEM_EXCEPTION: i16 = 1;
    /// `USER_EXCEPTION`.
    pub const USER_EXCEPTION: i16 = 2;
    /// `LOCATION_FORWARD`.
    pub const LOCATION_FORWARD: i16 = 3;
    /// `TRANSPORT_RETRY`.
    pub const TRANSPORT_RETRY: i16 = 4;
    /// `UNKNOWN`.
    pub const UNKNOWN: i16 = 5;
}

/// `Messaging::SyncScope` values ([`ClientRequestInfo::sync_scope`]).
pub mod sync_scope {
    /// `SYNC_NONE`: a oneway request.
    pub const SYNC_NONE: i16 = 0;
    /// `SYNC_WITH_TRANSPORT`.
    pub const SYNC_WITH_TRANSPORT: i16 = 1;
    /// `SYNC_WITH_SERVER`.
    pub const SYNC_WITH_SERVER: i16 = 2;
    /// `SYNC_WITH_TARGET`: a request with a reply.
    pub const SYNC_WITH_TARGET: i16 = 3;
}

/// `PortableInterceptor::SlotId`.
pub type SlotId = u32;

/// `PortableInterceptor::InvalidSlot`: no slot has this id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidSlot;

impl fmt::Display for InvalidSlot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PortableInterceptor::InvalidSlot")
    }
}

impl std::error::Error for InvalidSlot {}

/// `PortableInterceptor::ORBInitInfo::DuplicateName`: an interceptor of
/// the same kind already has this name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateName(pub String);

impl fmt::Display for DuplicateName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "an interceptor named {:?} is registered already", self.0)
    }
}

impl std::error::Error for DuplicateName {}

/// What an interception point may raise besides returning: a system
/// exception, or `PortableInterceptor::ForwardRequest` to send the request
/// to another object.
#[derive(Debug, Clone)]
pub enum InterceptError {
    /// A system exception, which becomes the result of the request.
    System(SystemException),
    /// `ForwardRequest`: the request goes to `forward` instead.
    ForwardRequest(ObjectRef),
}

impl From<SystemException> for InterceptError {
    fn from(e: SystemException) -> InterceptError {
        InterceptError::System(e)
    }
}

// --- registration --------------------------------------------------------------

/// `PortableInterceptor::ORBInitializer`: adds interceptors and slots to
/// an ORB while it is created.
pub trait OrbInitializer: Send + Sync {
    /// Called first, before the initial references of the ORB can be
    /// resolved.
    fn pre_init(&self, info: &mut OrbInitInfo<'_>) -> Result<(), SystemException> {
        let _ = info;
        Ok(())
    }

    /// Called after every initializer's `pre_init`.
    fn post_init(&self, info: &mut OrbInitInfo<'_>) -> Result<(), SystemException> {
        let _ = info;
        Ok(())
    }
}

impl fmt::Debug for dyn OrbInitializer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("OrbInitializer")
    }
}

static INITIALIZERS: Mutex<Vec<Arc<dyn OrbInitializer>>> = Mutex::new(Vec::new());

/// `PortableInterceptor::register_orb_initializer`: runs `init` for every
/// ORB created from now on in this process, before the initializers of
/// [`OrbOptions::initializers`](crate::OrbOptions::initializers).
pub fn register_orb_initializer(init: Arc<dyn OrbInitializer>) {
    INITIALIZERS.lock().unwrap().push(init);
}

pub(crate) fn registered_initializers() -> Vec<Arc<dyn OrbInitializer>> {
    INITIALIZERS.lock().unwrap().clone()
}

/// The interceptors and slots initializers add.
#[derive(Default)]
pub(crate) struct Additions {
    client: Vec<Arc<dyn ClientRequestInterceptor>>,
    server: Vec<Arc<dyn ServerRequestInterceptor>>,
    ior: Vec<Arc<dyn IorInterceptor>>,
    slots: u32,
}

impl Additions {
    fn destroy(&self) {
        for i in &self.client {
            i.destroy();
        }
        for i in &self.server {
            i.destroy();
        }
        for i in &self.ior {
            i.destroy();
        }
    }
}

/// `PortableInterceptor::ORBInitInfo`, given to the initializers.
pub struct OrbInitInfo<'a> {
    orb: &'a Orb,
    post: bool,
    additions: &'a mut Additions,
}

impl fmt::Debug for OrbInitInfo<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OrbInitInfo")
            .field("post_init", &self.post)
            .finish_non_exhaustive()
    }
}

fn add_named<T: ?Sized + Interceptor>(
    list: &mut Vec<Arc<T>>,
    interceptor: Arc<T>,
) -> Result<(), DuplicateName> {
    let name = interceptor.name();
    if !name.is_empty() && list.iter().any(|i| i.name() == name) {
        return Err(DuplicateName(name.to_owned()));
    }
    list.push(interceptor);
    Ok(())
}

impl OrbInitInfo<'_> {
    /// `arguments`: the arguments the ORB was created with
    /// ([`OrbOptions::arguments`](crate::OrbOptions::arguments)).
    pub fn arguments(&self) -> &[String] {
        &self.orb.inner.options.arguments
    }

    /// `orb_id` (`-ORBid`).
    pub fn orb_id(&self) -> &str {
        &self.orb.inner.options.orb_id
    }

    /// `codec_factory`.
    pub fn codec_factory(&self) -> CodecFactory {
        CodecFactory::new(self.orb)
    }

    /// The ORB being created. Calls on it may not work until it is.
    pub fn orb(&self) -> &Orb {
        self.orb
    }

    /// `register_initial_reference`, as [`Orb::register_initial_reference`].
    pub fn register_initial_reference(
        &self,
        name: &str,
        obj: Option<&ObjectRef>,
    ) -> Result<(), ResolveError> {
        self.orb.register_initial_reference(name, obj)
    }

    /// `resolve_initial_references`, only in `post_init` (`BAD_INV_ORDER`
    /// in `pre_init`).
    pub fn resolve_initial_references(&self, name: &str) -> Result<ObjectRef, ResolveError> {
        if !self.post {
            return Err(ResolveError::System(invalid_point()));
        }
        self.orb.resolve_initial_references(name)
    }

    /// `add_client_request_interceptor`. Names other than `""` must be
    /// unique among the client interceptors.
    pub fn add_client_request_interceptor(
        &mut self,
        interceptor: Arc<dyn ClientRequestInterceptor>,
    ) -> Result<(), DuplicateName> {
        add_named(&mut self.additions.client, interceptor)
    }

    /// `add_server_request_interceptor`.
    pub fn add_server_request_interceptor(
        &mut self,
        interceptor: Arc<dyn ServerRequestInterceptor>,
    ) -> Result<(), DuplicateName> {
        add_named(&mut self.additions.server, interceptor)
    }

    /// `add_ior_interceptor`.
    pub fn add_ior_interceptor(
        &mut self,
        interceptor: Arc<dyn IorInterceptor>,
    ) -> Result<(), DuplicateName> {
        add_named(&mut self.additions.ior, interceptor)
    }

    /// `allocate_slot_id`: a new slot of [`PiCurrent`].
    pub fn allocate_slot_id(&mut self) -> SlotId {
        self.additions.slots += 1;
        self.additions.slots - 1
    }

    /// `register_policy_factory`: not supported yet (`NO_IMPLEMENT`).
    pub fn register_policy_factory(
        &mut self,
        policy_type: PolicyType,
    ) -> Result<(), SystemException> {
        let _ = policy_type;
        Err(SystemException::new(
            SystemExceptionKind::NO_IMPLEMENT,
            0,
            CompletionStatus::No,
        ))
    }

    /// The [`PiCurrent`] of the ORB, with the slots allocated so far.
    pub fn pi_current(&self) -> PiCurrent {
        PiCurrent {
            serial: self.orb.inner.serial,
            slots: self.additions.slots,
        }
    }
}

/// Runs the initializers of a new ORB. `Err` when one failed: the
/// interceptors added are destroyed, and the ORB is not to be used.
pub(crate) fn initialize(
    orb: &Orb,
    initializers: &[Arc<dyn OrbInitializer>],
) -> Result<Option<Arc<Interceptors>>, SystemException> {
    if initializers.is_empty() {
        return Ok(None);
    }
    let mut additions = Additions::default();
    for post in [false, true] {
        for init in initializers {
            let mut info = OrbInitInfo {
                orb,
                post,
                additions: &mut additions,
            };
            let result = if post {
                init.post_init(&mut info)
            } else {
                init.pre_init(&mut info)
            };
            if let Err(e) = result {
                trace::warning!(
                    "ORBInitializer {} raised {e}: the ORB is not created",
                    if post { "post_init" } else { "pre_init" }
                );
                additions.destroy();
                return Err(e);
            }
        }
    }
    if additions.client.is_empty()
        && additions.server.is_empty()
        && additions.ior.is_empty()
        && additions.slots == 0
    {
        return Ok(None);
    }
    Ok(Some(Arc::new(Interceptors {
        serial: orb.inner.serial,
        client: additions.client,
        server: additions.server,
        ior: additions.ior,
        slots: additions.slots,
        next_request_id: AtomicU32::new(0),
    })))
}

/// The interceptors and slots of one ORB, fixed once it is created.
pub(crate) struct Interceptors {
    serial: u64,
    pub(crate) client: Vec<Arc<dyn ClientRequestInterceptor>>,
    pub(crate) server: Vec<Arc<dyn ServerRequestInterceptor>>,
    pub(crate) ior: Vec<Arc<dyn IorInterceptor>>,
    pub(crate) slots: u32,
    next_request_id: AtomicU32,
}

impl Interceptors {
    /// Calls `destroy` on every interceptor (`ORB::destroy`).
    pub(crate) fn destroy(&self) {
        for i in &self.client {
            i.destroy();
        }
        for i in &self.server {
            i.destroy();
        }
        for i in &self.ior {
            i.destroy();
        }
    }

    pub(crate) fn has_client(&self) -> bool {
        !self.client.is_empty()
    }

    pub(crate) fn has_server(&self) -> bool {
        !self.server.is_empty()
    }
}

static NEXT_SERIAL: AtomicU64 = AtomicU64::new(1);

/// A number for an ORB that is never used again, to keep the slots of
/// [`PiCurrent`] apart.
pub(crate) fn new_serial() -> u64 {
    NEXT_SERIAL.fetch_add(1, Ordering::Relaxed)
}

// --- interceptors --------------------------------------------------------------

/// `PortableInterceptor::Interceptor`: what every interceptor has.
pub trait Interceptor: Send + Sync {
    /// `name`; `""` for an anonymous interceptor.
    fn name(&self) -> &str;

    /// `destroy`: called once, when the ORB is destroyed.
    fn destroy(&self) {}
}

/// `PortableInterceptor::ClientRequestInterceptor`.
///
/// For each attempt to send a request (one address of the target, see
/// docs/portable-interceptors.md), `send_request` is called on each
/// interceptor in the order they were added, and then one of
/// `receive_reply`, `receive_exception` or `receive_other` in the reverse
/// order, on those whose `send_request` returned `Ok`. They run on the
/// thread that makes the call (for an async call, the thread that polls
/// it): keep them short.
pub trait ClientRequestInterceptor: Interceptor {
    /// Before the request is sent: may add service contexts.
    fn send_request(&self, ri: &ClientRequestInfo<'_>) -> Result<(), InterceptError> {
        let _ = ri;
        Ok(())
    }

    /// `send_poll`: never called (rustorb has no time-independent
    /// invocations).
    fn send_poll(&self, ri: &ClientRequestInfo<'_>) -> Result<(), SystemException> {
        let _ = ri;
        Ok(())
    }

    /// After a normal reply.
    fn receive_reply(&self, ri: &ClientRequestInfo<'_>) -> Result<(), SystemException> {
        let _ = ri;
        Ok(())
    }

    /// After an exception, from the server or from the ORB.
    fn receive_exception(&self, ri: &ClientRequestInfo<'_>) -> Result<(), InterceptError> {
        let _ = ri;
        Ok(())
    }

    /// After another outcome: a forward, a retry, or a oneway request
    /// sent.
    fn receive_other(&self, ri: &ClientRequestInfo<'_>) -> Result<(), InterceptError> {
        let _ = ri;
        Ok(())
    }
}

/// `PortableInterceptor::ServerRequestInterceptor`.
///
/// For each request, `receive_request_service_contexts` is called on each
/// interceptor in the order they were added, then `receive_request` on
/// each, the servant, and one of `send_reply`, `send_exception` or
/// `send_other` in the reverse order. They all run on the thread the
/// servant runs on.
pub trait ServerRequestInterceptor: Interceptor {
    /// Before the servant is found: reads the service contexts and keeps
    /// what it needs in the slots ([`ServerRequestInfo::set_slot`]).
    fn receive_request_service_contexts(
        &self,
        ri: &ServerRequestInfo<'_>,
    ) -> Result<(), InterceptError> {
        let _ = ri;
        Ok(())
    }

    /// Just before the servant runs, on its thread.
    fn receive_request(&self, ri: &ServerRequestInfo<'_>) -> Result<(), InterceptError> {
        let _ = ri;
        Ok(())
    }

    /// Before a normal reply goes out.
    fn send_reply(&self, ri: &ServerRequestInfo<'_>) -> Result<(), SystemException> {
        let _ = ri;
        Ok(())
    }

    /// Before an exception reply goes out.
    fn send_exception(&self, ri: &ServerRequestInfo<'_>) -> Result<(), InterceptError> {
        let _ = ri;
        Ok(())
    }

    /// Before another reply (a forward) goes out.
    fn send_other(&self, ri: &ServerRequestInfo<'_>) -> Result<(), InterceptError> {
        let _ = ri;
        Ok(())
    }
}

/// `PortableInterceptor::IORInterceptor`.
pub trait IorInterceptor: Interceptor {
    /// Adds the tagged components of the references of a new POA. Called
    /// on the thread that creates the POA. An error is ignored (with a
    /// warning).
    fn establish_components(&self, info: &IorInfo<'_>) -> Result<(), SystemException>;
}

// --- PICurrent -----------------------------------------------------------------

thread_local! {
    /// The thread scope slots of each ORB, by serial.
    static THREAD_SLOTS: RefCell<HashMap<u64, Vec<Any>>> = RefCell::new(HashMap::new());
}

fn empty_slot() -> Any {
    Any::new(TypeCode::null(), Value::Void)
}

/// `PortableInterceptor::Current` of an ORB ([`Orb::pi_current`]): the
/// slots of the calling thread. A request copies them when it starts, so
/// client interceptors see what the application set; a server
/// interceptor's slots are what the servant sees.
#[derive(Debug, Clone)]
pub struct PiCurrent {
    serial: u64,
    slots: u32,
}

impl PiCurrent {
    pub(crate) fn new(orb: &Orb) -> PiCurrent {
        PiCurrent {
            serial: orb.inner.serial,
            slots: orb.interceptors().map_or(0, |pi| pi.slots),
        }
    }

    /// `get_slot`: the value of slot `id` for this thread; an Any of type
    /// `null` when it has none.
    pub fn get_slot(&self, id: SlotId) -> Result<Any, InvalidSlot> {
        if id >= self.slots {
            return Err(InvalidSlot);
        }
        Ok(THREAD_SLOTS.with(|t| {
            t.borrow()
                .get(&self.serial)
                .and_then(|s| s.get(id as usize))
                .cloned()
                .unwrap_or_else(empty_slot)
        }))
    }

    /// `set_slot`.
    pub fn set_slot(&self, id: SlotId, data: Any) -> Result<(), InvalidSlot> {
        if id >= self.slots {
            return Err(InvalidSlot);
        }
        THREAD_SLOTS.with(|t| {
            let mut t = t.borrow_mut();
            let slots = t
                .entry(self.serial)
                .or_insert_with(|| vec![empty_slot(); self.slots as usize]);
            if slots.len() < self.slots as usize {
                slots.resize_with(self.slots as usize, empty_slot);
            }
            slots[id as usize] = data;
        });
        Ok(())
    }
}

/// The thread scope slots of the ORB `pi` belongs to, copied: the request
/// scope slots of a new request.
pub(crate) fn thread_slots(pi: &Interceptors) -> Vec<Any> {
    THREAD_SLOTS.with(|t| {
        let mut slots = t.borrow().get(&pi.serial).cloned().unwrap_or_default();
        slots.resize_with(pi.slots as usize, empty_slot);
        slots
    })
}

/// Sets the thread scope slots while it lives, and puts the old ones back
/// after.
pub(crate) struct ThreadSlotsGuard {
    serial: u64,
    old: Option<Vec<Any>>,
}

impl ThreadSlotsGuard {
    fn enter(serial: u64, slots: Vec<Any>) -> ThreadSlotsGuard {
        let old = THREAD_SLOTS.with(|t| t.borrow_mut().insert(serial, slots));
        ThreadSlotsGuard { serial, old }
    }
}

impl Drop for ThreadSlotsGuard {
    fn drop(&mut self) {
        let old = self.old.take();
        // The thread may be ending (its locals going away).
        let _ = THREAD_SLOTS.try_with(|t| match old {
            Some(old) => {
                t.borrow_mut().insert(self.serial, old);
            }
            None => {
                t.borrow_mut().remove(&self.serial);
            }
        });
    }
}

// --- request information ---------------------------------------------------------

fn invalid_point() -> SystemException {
    SystemException::new(
        SystemExceptionKind::BAD_INV_ORDER,
        minor::INVALID_POINT,
        CompletionStatus::No,
    )
}

fn not_available() -> SystemException {
    SystemException::new(
        SystemExceptionKind::NO_RESOURCES,
        minor::NOT_AVAILABLE,
        CompletionStatus::No,
    )
}

fn no_context() -> SystemException {
    SystemException::new(
        SystemExceptionKind::BAD_PARAM,
        minor::NO_SERVICE_CONTEXT,
        CompletionStatus::No,
    )
}

fn duplicate_context() -> SystemException {
    SystemException::new(
        SystemExceptionKind::BAD_INV_ORDER,
        minor::DUPLICATE_SERVICE_CONTEXT,
        CompletionStatus::No,
    )
}

/// `Dynamic::Parameter`: an argument and its direction.
#[derive(Debug, Clone)]
pub struct Parameter {
    /// The value; for an `out` argument before the reply, an Any of its
    /// type holding `Value::Void`.
    pub argument: Any,
    /// `in`, `out` or `inout`.
    pub mode: ArgMode,
}

fn parameters(list: &[NamedValue]) -> Vec<Parameter> {
    list.iter()
        .map(|nv| Parameter {
            argument: nv.value.clone(),
            mode: nv.mode,
        })
        .collect()
}

/// The ids rustorb writes itself; interceptors cannot add them.
fn is_core_context(id: u32) -> bool {
    id == rustorb_giop::service_id::CODE_SETS || id == rustorb_giop::service_id::BI_DIR_IIOP
}

fn find_context(list: &[ServiceContext], id: u32) -> Result<ServiceContext, SystemException> {
    list.iter()
        .find(|c| c.context_id == id)
        .cloned()
        .ok_or_else(no_context)
}

fn add_context(
    list: &mut Vec<ServiceContext>,
    sc: ServiceContext,
    replace: bool,
) -> Result<(), SystemException> {
    match list.iter_mut().find(|c| c.context_id == sc.context_id) {
        Some(_) if !replace => Err(duplicate_context()),
        Some(old) => {
            *old = sc;
            Ok(())
        }
        None => {
            list.push(sc);
            Ok(())
        }
    }
}

/// The TypeCode of a system exception: `minor` and `completed`.
fn system_exception_tc(id: &str) -> TypeCode {
    let completion = TypeCode::enum_tc(
        "IDL:omg.org/CORBA/CompletionStatus:1.0",
        "CompletionStatus",
        &["COMPLETED_YES", "COMPLETED_NO", "COMPLETED_MAYBE"],
    )
    .expect("a valid enum");
    let name = id
        .rsplit('/')
        .next()
        .and_then(|n| n.split(':').next())
        .unwrap_or("UNKNOWN");
    TypeCode::exception(
        id,
        name,
        &[
            ("minor", TypeCode::simple(TCKind::ULong).expect("simple")),
            ("completed", completion),
        ],
    )
    .unwrap_or_else(|_| TypeCode::null())
}

/// A system exception as an Any.
pub fn system_exception_any(e: &SystemException) -> Any {
    let completed = match e.completed {
        CompletionStatus::Yes => 0,
        CompletionStatus::No => 1,
        CompletionStatus::Maybe => 2,
    };
    Any::new(
        system_exception_tc(&e.repository_id),
        Value::List(vec![Value::ULong(e.minor), Value::Enum(completed)]),
    )
}

fn unknown_any() -> Any {
    system_exception_any(&SystemException::new(
        SystemExceptionKind::UNKNOWN,
        crate::minor::UNLISTED_USER_EXCEPTION,
        CompletionStatus::Yes,
    ))
}

/// What the DII gives the client interceptors: the arguments, the result
/// type and the exception list.
#[derive(Debug, Clone)]
pub(crate) struct DiiParams {
    pub(crate) arguments: Vec<NamedValue>,
    pub(crate) result: TypeCode,
    pub(crate) exceptions: Vec<TypeCode>,
}

/// The client interception points.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientPoint {
    /// `send_request`.
    SendRequest,
    /// `send_poll`.
    SendPoll,
    /// `receive_reply`.
    ReceiveReply,
    /// `receive_exception`.
    ReceiveException,
    /// `receive_other`.
    ReceiveOther,
}

/// `PortableInterceptor::ClientRequestInfo`: one request as the client
/// interceptors see it, during one interception point.
pub struct ClientRequestInfo<'a> {
    point: Cell<ClientPoint>,
    request_id: u32,
    operation: &'a str,
    response_expected: bool,
    target: &'a ObjectRef,
    effective_target: ObjectRef,
    effective_profile: TaggedProfile,
    timeout: Option<Duration>,
    slots: Vec<Any>,
    dii: Option<&'a DiiParams>,
    request_contexts: RefCell<Vec<ServiceContext>>,
    reply_contexts: RefCell<Vec<ServiceContext>>,
    reply_status: Cell<i16>,
    exception: RefCell<Option<Exception>>,
    forward: RefCell<Option<ObjectRef>>,
    reply: RefCell<Option<Reply>>,
}

impl fmt::Debug for ClientRequestInfo<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClientRequestInfo")
            .field("point", &self.point.get())
            .field("request_id", &self.request_id)
            .field("operation", &self.operation)
            .finish_non_exhaustive()
    }
}

/// The parts of a request [`ClientRequestInfo::new`] takes.
pub(crate) struct ClientRequest<'a> {
    pub(crate) operation: &'a str,
    pub(crate) response_expected: bool,
    pub(crate) target: &'a ObjectRef,
    pub(crate) effective: Arc<Ior>,
    pub(crate) profile_index: Option<usize>,
    pub(crate) timeout: Option<Duration>,
    pub(crate) slots: Vec<Any>,
    pub(crate) dii: Option<&'a DiiParams>,
}

impl<'a> ClientRequestInfo<'a> {
    pub(crate) fn new(pi: &Interceptors, r: ClientRequest<'a>) -> ClientRequestInfo<'a> {
        let effective_profile = r
            .profile_index
            .and_then(|i| r.effective.profiles.get(i))
            .cloned()
            .unwrap_or(TaggedProfile {
                tag: TAG_INTERNET_IOP,
                profile_data: Vec::new(),
            });
        let effective_target = if Arc::ptr_eq(&r.effective, &r.target.ior_arc()) {
            r.target.clone()
        } else {
            ObjectRef::new(r.target.orb().clone(), (*r.effective).clone())
        };
        ClientRequestInfo {
            point: Cell::new(ClientPoint::SendRequest),
            request_id: pi.next_request_id.fetch_add(1, Ordering::Relaxed),
            operation: r.operation,
            response_expected: r.response_expected,
            target: r.target,
            effective_target,
            effective_profile,
            timeout: r.timeout,
            slots: r.slots,
            dii: r.dii,
            request_contexts: RefCell::new(Vec::new()),
            reply_contexts: RefCell::new(Vec::new()),
            reply_status: Cell::new(reply_status::SUCCESSFUL),
            exception: RefCell::new(None),
            forward: RefCell::new(None),
            reply: RefCell::new(None),
        }
    }

    fn check(&self, allowed: &[ClientPoint]) -> Result<(), SystemException> {
        if allowed.contains(&self.point.get()) {
            Ok(())
        } else {
            Err(invalid_point())
        }
    }

    fn after_send(&self) -> Result<(), SystemException> {
        use ClientPoint::*;
        self.check(&[ReceiveReply, ReceiveException, ReceiveOther])
    }

    /// The interception point being run.
    pub fn point(&self) -> ClientPoint {
        self.point.get()
    }

    /// `request_id`: unique among the requests of this ORB in progress
    /// (not the GIOP request id).
    pub fn request_id(&self) -> u32 {
        self.request_id
    }

    /// `operation`.
    pub fn operation(&self) -> &str {
        self.operation
    }

    /// `arguments`: only for a DII request (else `NO_RESOURCES`), in
    /// `send_request` and `receive_reply`.
    pub fn arguments(&self) -> Result<Vec<Parameter>, SystemException> {
        use ClientPoint::*;
        self.check(&[SendRequest, ReceiveReply])?;
        let dii = self.dii.ok_or_else(not_available)?;
        if self.point.get() == SendRequest {
            return Ok(parameters(&dii.arguments));
        }
        let (_, out) = self.decode_reply(dii)?;
        let mut out = out.into_iter();
        Ok(dii
            .arguments
            .iter()
            .map(|nv| Parameter {
                argument: if nv.mode.is_out() {
                    out.next().unwrap_or_else(|| nv.value.clone())
                } else {
                    nv.value.clone()
                },
                mode: nv.mode,
            })
            .collect())
    }

    /// The result and the `out` values of a DII reply.
    fn decode_reply(&self, dii: &DiiParams) -> Result<(Any, Vec<Any>), SystemException> {
        let reply = self.reply.borrow();
        let reply = reply.as_ref().ok_or_else(not_available)?;
        let mut input = reply.input();
        let result = Any::new(dii.result.clone(), input.read_value(&dii.result)?);
        let mut out = Vec::new();
        for nv in dii.arguments.iter().filter(|nv| nv.mode.is_out()) {
            let tc = nv.value.typecode().clone();
            let value = input.read_value(&tc)?;
            out.push(Any::new(tc, value));
        }
        Ok((result, out))
    }

    /// `exceptions`: the exception list of a DII request.
    pub fn exceptions(&self) -> Result<Vec<TypeCode>, SystemException> {
        self.check_not_poll()?;
        Ok(self.dii.ok_or_else(not_available)?.exceptions.clone())
    }

    fn check_not_poll(&self) -> Result<(), SystemException> {
        if self.point.get() == ClientPoint::SendPoll {
            Err(invalid_point())
        } else {
            Ok(())
        }
    }

    /// `contexts`: `NO_RESOURCES` (rustorb has no `context` clauses).
    pub fn contexts(&self) -> Result<Vec<String>, SystemException> {
        self.check_not_poll()?;
        Err(not_available())
    }

    /// `operation_context`: `NO_RESOURCES`.
    pub fn operation_context(&self) -> Result<Vec<String>, SystemException> {
        self.check_not_poll()?;
        Err(not_available())
    }

    /// `result`: only in `receive_reply` of a DII request.
    pub fn result(&self) -> Result<Any, SystemException> {
        self.check(&[ClientPoint::ReceiveReply])?;
        let dii = self.dii.ok_or_else(not_available)?;
        Ok(self.decode_reply(dii)?.0)
    }

    /// `response_expected`.
    pub fn response_expected(&self) -> bool {
        self.response_expected
    }

    /// `sync_scope`.
    pub fn sync_scope(&self) -> i16 {
        if self.response_expected {
            sync_scope::SYNC_WITH_TARGET
        } else {
            sync_scope::SYNC_NONE
        }
    }

    /// `reply_status` ([`reply_status`]).
    pub fn reply_status(&self) -> Result<i16, SystemException> {
        self.after_send()?;
        Ok(self.reply_status.get())
    }

    /// `forward_reference`: in `receive_other` of a `LOCATION_FORWARD`.
    pub fn forward_reference(&self) -> Result<ObjectRef, SystemException> {
        self.check(&[ClientPoint::ReceiveOther])?;
        self.forward.borrow().clone().ok_or_else(invalid_point)
    }

    /// `get_slot`: the request's copy of the slots.
    pub fn get_slot(&self, id: SlotId) -> Result<Any, InvalidSlot> {
        self.slots.get(id as usize).cloned().ok_or(InvalidSlot)
    }

    /// `get_request_service_context`: one the interceptors added (the ones
    /// rustorb writes itself, such as `CodeSets`, are not here).
    pub fn get_request_service_context(&self, id: u32) -> Result<ServiceContext, SystemException> {
        self.check_not_poll()?;
        find_context(&self.request_contexts.borrow(), id)
    }

    /// `get_reply_service_context`.
    pub fn get_reply_service_context(&self, id: u32) -> Result<ServiceContext, SystemException> {
        self.after_send()?;
        find_context(&self.reply_contexts.borrow(), id)
    }

    /// `target`: the reference the call was made on.
    pub fn target(&self) -> &ObjectRef {
        self.target
    }

    /// `effective_target`: where the request goes, after forwards.
    pub fn effective_target(&self) -> &ObjectRef {
        &self.effective_target
    }

    /// `effective_profile`: the profile of the address being tried.
    pub fn effective_profile(&self) -> &TaggedProfile {
        &self.effective_profile
    }

    /// `received_exception`: in `receive_exception`. A user exception
    /// whose TypeCode is not known (anything but a DII request with an
    /// exception list) is an Any of `UNKNOWN`.
    pub fn received_exception(&self) -> Result<Any, SystemException> {
        self.check(&[ClientPoint::ReceiveException])?;
        match &*self.exception.borrow() {
            Some(Exception::System(e)) => Ok(system_exception_any(e)),
            Some(Exception::User(e)) => Ok(self
                .dii
                .and_then(|dii| crate::dii::exception_any(&dii.exceptions, e).ok())
                .unwrap_or_else(unknown_any)),
            None => Err(invalid_point()),
        }
    }

    /// `received_exception_id`.
    pub fn received_exception_id(&self) -> Result<String, SystemException> {
        self.check(&[ClientPoint::ReceiveException])?;
        match &*self.exception.borrow() {
            Some(e) => Ok(e.repository_id().to_owned()),
            None => Err(invalid_point()),
        }
    }

    /// The exception, in `receive_exception` (rustorb).
    pub fn exception(&self) -> Option<Exception> {
        self.exception.borrow().clone()
    }

    fn components(&self) -> Vec<TaggedComponent> {
        self.effective_profile
            .iiop()
            .map(|p| p.components)
            .unwrap_or_default()
    }

    /// `get_effective_component`: the first component with `tag` in the
    /// effective profile (`BAD_PARAM` if none).
    pub fn get_effective_component(&self, tag: u32) -> Result<TaggedComponent, SystemException> {
        self.components()
            .into_iter()
            .find(|c| c.tag == tag)
            .ok_or_else(no_component)
    }

    /// `get_effective_components`.
    pub fn get_effective_components(
        &self,
        tag: u32,
    ) -> Result<Vec<TaggedComponent>, SystemException> {
        let found: Vec<_> = self
            .components()
            .into_iter()
            .filter(|c| c.tag == tag)
            .collect();
        if found.is_empty() {
            Err(no_component())
        } else {
            Ok(found)
        }
    }

    /// `get_request_policy`: the client policy of `policy_type` in effect
    /// (`None` if none is). `INV_POLICY` for a type that is not a client
    /// policy.
    pub fn get_request_policy(
        &self,
        policy_type: PolicyType,
    ) -> Result<Option<PolicyValue>, SystemException> {
        if policy_type != crate::policy::RELATIVE_RT_TIMEOUT_POLICY_TYPE {
            return Err(SystemException::new(
                SystemExceptionKind::INV_POLICY,
                minor::INVALID_POLICY_TYPE,
                CompletionStatus::No,
            ));
        }
        Ok(self
            .timeout
            .map(|t| PolicyValue::RelativeRoundtripTimeout(crate::timeout::duration_to_time_t(t))))
    }

    /// `add_request_service_context`: in `send_request`.
    pub fn add_request_service_context(
        &self,
        sc: ServiceContext,
        replace: bool,
    ) -> Result<(), SystemException> {
        self.check(&[ClientPoint::SendRequest])?;
        if is_core_context(sc.context_id) {
            return Err(duplicate_context());
        }
        add_context(&mut self.request_contexts.borrow_mut(), sc, replace)
    }

    pub(crate) fn take_request_contexts(&self) -> Vec<ServiceContext> {
        self.request_contexts.borrow().clone()
    }

    fn enter(&self, ending: &ClientEnding) {
        *self.exception.borrow_mut() = None;
        *self.forward.borrow_mut() = None;
        let (point, status) = match ending {
            ClientEnding::Reply(_) => (ClientPoint::ReceiveReply, reply_status::SUCCESSFUL),
            ClientEnding::Oneway => (ClientPoint::ReceiveOther, reply_status::SUCCESSFUL),
            ClientEnding::Exception(e) => {
                *self.exception.borrow_mut() = Some(e.clone());
                let status = match e {
                    Exception::System(_) => reply_status::SYSTEM_EXCEPTION,
                    Exception::User(_) => reply_status::USER_EXCEPTION,
                };
                (ClientPoint::ReceiveException, status)
            }
            ClientEnding::Forward(to) => {
                *self.forward.borrow_mut() = Some(to.clone());
                (ClientPoint::ReceiveOther, reply_status::LOCATION_FORWARD)
            }
            ClientEnding::Retry => (ClientPoint::ReceiveOther, reply_status::TRANSPORT_RETRY),
        };
        if let ClientEnding::Reply(reply) = ending {
            *self.reply.borrow_mut() = Some(reply.clone());
        }
        self.point.set(point);
        self.reply_status.set(status);
    }

    pub(crate) fn set_reply_contexts(&self, contexts: Vec<ServiceContext>) {
        *self.reply_contexts.borrow_mut() = contexts;
    }
}

fn no_component() -> SystemException {
    SystemException::new(
        SystemExceptionKind::BAD_PARAM,
        minor::NO_COMPONENT,
        CompletionStatus::No,
    )
}

/// How a client interception ends.
#[derive(Debug, Clone)]
pub(crate) enum ClientEnding {
    /// A normal reply.
    Reply(Reply),
    /// A oneway request was sent.
    Oneway,
    Exception(Exception),
    Forward(ObjectRef),
    /// The ORB sends the request again.
    Retry,
}

impl ClientEnding {
    fn from_error(e: InterceptError) -> ClientEnding {
        match e {
            InterceptError::System(e) => ClientEnding::Exception(Exception::System(e)),
            InterceptError::ForwardRequest(to) => ClientEnding::Forward(to),
        }
    }
}

/// Runs `send_request` on each interceptor. `Err` is how the interception
/// ended when one of them raised: the request is not sent.
pub(crate) fn send_request(
    pi: &Interceptors,
    info: &ClientRequestInfo<'_>,
) -> Result<(), ClientEnding> {
    info.point.set(ClientPoint::SendRequest);
    for (k, i) in pi.client.iter().enumerate() {
        if let Err(e) = catch_client(|| i.send_request(info)) {
            return Err(end_client(pi, info, ClientEnding::from_error(e), k));
        }
    }
    Ok(())
}

/// Runs the ending points on every interceptor, in the reverse order, and
/// returns how the request ends.
pub(crate) fn receive(
    pi: &Interceptors,
    info: &ClientRequestInfo<'_>,
    ending: ClientEnding,
) -> ClientEnding {
    end_client(pi, info, ending, pi.client.len())
}

fn end_client(
    pi: &Interceptors,
    info: &ClientRequestInfo<'_>,
    mut ending: ClientEnding,
    started: usize,
) -> ClientEnding {
    for i in pi.client[..started].iter().rev() {
        info.enter(&ending);
        let result = match &ending {
            ClientEnding::Reply(_) => {
                catch_client(|| i.receive_reply(info).map_err(InterceptError::System))
            }
            ClientEnding::Exception(_) => catch_client(|| i.receive_exception(info)),
            ClientEnding::Oneway | ClientEnding::Forward(_) | ClientEnding::Retry => {
                catch_client(|| i.receive_other(info))
            }
        };
        if let Err(e) = result {
            ending = ClientEnding::from_error(e);
        }
    }
    ending
}

fn catch_client(f: impl FnOnce() -> Result<(), InterceptError>) -> Result<(), InterceptError> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).unwrap_or_else(|_| {
        trace::warning!("client request interceptor panicked");
        Err(InterceptError::System(SystemException::new(
            SystemExceptionKind::UNKNOWN,
            0,
            CompletionStatus::Maybe,
        )))
    })
}

// --- the server side -----------------------------------------------------------

/// The server interception points.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerPoint {
    /// `receive_request_service_contexts`.
    ReceiveRequestServiceContexts,
    /// `receive_request`.
    ReceiveRequest,
    /// `send_reply`.
    SendReply,
    /// `send_exception`.
    SendException,
    /// `send_other`.
    SendOther,
}

/// What the DSI tells the server interceptors, after the servant ran.
#[derive(Debug, Clone, Default)]
pub(crate) struct DsiParams {
    pub(crate) arguments: Option<Vec<NamedValue>>,
    pub(crate) result: Option<Any>,
    pub(crate) exception: Option<Any>,
}

thread_local! {
    /// Set while a request with server interceptors runs its servant: a
    /// DSI servant leaves its parameters here.
    static DSI: RefCell<Option<DsiParams>> = const { RefCell::new(None) };
}

/// Whether a DSI servant should leave its parameters for the
/// interceptors ([`leave_dsi_params`]).
pub(crate) fn wants_dsi_params() -> bool {
    DSI.with(|d| d.borrow().is_some())
}

pub(crate) fn leave_dsi_params(params: DsiParams) {
    DSI.with(|d| {
        let mut d = d.borrow_mut();
        if d.is_some() {
            *d = Some(params);
        }
    });
}

/// Collects what a DSI servant leaves while it lives.
pub(crate) struct DsiGuard {
    old: Option<DsiParams>,
}

impl DsiGuard {
    pub(crate) fn enter() -> DsiGuard {
        DsiGuard {
            old: DSI.with(|d| d.borrow_mut().replace(DsiParams::default())),
        }
    }

    pub(crate) fn take(self) -> DsiParams {
        DSI.with(|d| d.borrow_mut().take()).unwrap_or_default()
    }
}

impl Drop for DsiGuard {
    fn drop(&mut self) {
        let old = self.old.take();
        let _ = DSI.try_with(|d| *d.borrow_mut() = old);
    }
}

/// The target of a request, once its POA is known.
pub(crate) struct ServerTarget {
    pub(crate) poa: Poa,
    pub(crate) id: Vec<u8>,
}

/// `PortableInterceptor::ServerRequestInfo`: one request as the server
/// interceptors see it, during one interception point.
pub struct ServerRequestInfo<'a> {
    point: Cell<ServerPoint>,
    request_id: u32,
    operation: &'a str,
    response_expected: bool,
    orb: &'a Orb,
    target: Option<&'a ServerTarget>,
    servant: RefCell<Option<Arc<dyn Servant>>>,
    request_contexts: &'a [ServiceContext],
    reply_contexts: RefCell<Vec<ServiceContext>>,
    slots: RefCell<Vec<Any>>,
    reply_status: Cell<i16>,
    exception: RefCell<Option<ServerException>>,
    forward: RefCell<Option<ObjectRef>>,
    dsi: RefCell<Option<DsiParams>>,
}

impl fmt::Debug for ServerRequestInfo<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ServerRequestInfo")
            .field("point", &self.point.get())
            .field("request_id", &self.request_id)
            .field("operation", &self.operation)
            .finish_non_exhaustive()
    }
}

/// The exception of a server reply.
#[derive(Debug, Clone)]
pub(crate) enum ServerException {
    System(SystemException),
    /// A user exception the servant wrote, by repository id.
    User(String),
}

/// How a server interception ends: the reply that goes out.
#[derive(Debug, Clone)]
pub(crate) enum ServerEnding {
    Reply,
    Exception(ServerException),
    Forward(ObjectRef),
}

impl ServerEnding {
    fn from_error(e: InterceptError) -> ServerEnding {
        match e {
            InterceptError::System(e) => ServerEnding::Exception(ServerException::System(e)),
            InterceptError::ForwardRequest(to) => ServerEnding::Forward(to),
        }
    }
}

/// The parts of a request [`ServerRequestInfo::new`] takes.
pub(crate) struct ServerRequest<'a> {
    pub(crate) request_id: u32,
    pub(crate) operation: &'a str,
    pub(crate) response_expected: bool,
    pub(crate) orb: &'a Orb,
    pub(crate) target: Option<&'a ServerTarget>,
    pub(crate) contexts: &'a [ServiceContext],
}

impl<'a> ServerRequestInfo<'a> {
    pub(crate) fn new(pi: &Interceptors, r: ServerRequest<'a>) -> ServerRequestInfo<'a> {
        ServerRequestInfo {
            point: Cell::new(ServerPoint::ReceiveRequestServiceContexts),
            request_id: r.request_id,
            operation: r.operation,
            response_expected: r.response_expected,
            orb: r.orb,
            target: r.target,
            servant: RefCell::new(None),
            request_contexts: r.contexts,
            reply_contexts: RefCell::new(Vec::new()),
            slots: RefCell::new(vec![empty_slot(); pi.slots as usize]),
            reply_status: Cell::new(reply_status::SUCCESSFUL),
            exception: RefCell::new(None),
            forward: RefCell::new(None),
            dsi: RefCell::new(None),
        }
    }

    fn check(&self, allowed: &[ServerPoint]) -> Result<(), SystemException> {
        if allowed.contains(&self.point.get()) {
            Ok(())
        } else {
            Err(invalid_point())
        }
    }

    fn sending(&self) -> Result<(), SystemException> {
        use ServerPoint::*;
        self.check(&[SendReply, SendException, SendOther])
    }

    fn not_first(&self) -> Result<(), SystemException> {
        if self.point.get() == ServerPoint::ReceiveRequestServiceContexts {
            Err(invalid_point())
        } else {
            Ok(())
        }
    }

    /// The interception point being run.
    pub fn point(&self) -> ServerPoint {
        self.point.get()
    }

    /// `request_id`: the GIOP request id (0 for a call within the ORB).
    pub fn request_id(&self) -> u32 {
        self.request_id
    }

    /// `operation`.
    pub fn operation(&self) -> &str {
        self.operation
    }

    /// `arguments`: only for a DSI servant, in `send_reply` (else
    /// `NO_RESOURCES`).
    pub fn arguments(&self) -> Result<Vec<Parameter>, SystemException> {
        use ServerPoint::*;
        self.check(&[ReceiveRequest, SendReply])?;
        self.dsi
            .borrow()
            .as_ref()
            .and_then(|d| d.arguments.as_deref().map(parameters))
            .ok_or_else(not_available)
    }

    /// `exceptions`: `NO_RESOURCES` (the servant's exception list is not
    /// known).
    pub fn exceptions(&self) -> Result<Vec<TypeCode>, SystemException> {
        self.not_first()?;
        Err(not_available())
    }

    /// `contexts`: `NO_RESOURCES`.
    pub fn contexts(&self) -> Result<Vec<String>, SystemException> {
        self.not_first()?;
        Err(not_available())
    }

    /// `operation_context`: `NO_RESOURCES`.
    pub fn operation_context(&self) -> Result<Vec<String>, SystemException> {
        self.not_first()?;
        Err(not_available())
    }

    /// `result`: in `send_reply`, for a DSI servant.
    pub fn result(&self) -> Result<Any, SystemException> {
        self.check(&[ServerPoint::SendReply])?;
        self.dsi
            .borrow()
            .as_ref()
            .filter(|d| d.arguments.is_some())
            .map(|d| {
                d.result
                    .clone()
                    .unwrap_or_else(|| Any::new(TypeCode::void(), Value::Void))
            })
            .ok_or_else(not_available)
    }

    /// `response_expected`.
    pub fn response_expected(&self) -> bool {
        self.response_expected
    }

    /// `sync_scope`.
    pub fn sync_scope(&self) -> i16 {
        if self.response_expected {
            sync_scope::SYNC_WITH_TARGET
        } else {
            sync_scope::SYNC_NONE
        }
    }

    /// `reply_status`: in the `send_*` points.
    pub fn reply_status(&self) -> Result<i16, SystemException> {
        self.sending()?;
        Ok(self.reply_status.get())
    }

    /// `forward_reference`: in `send_other`.
    pub fn forward_reference(&self) -> Result<ObjectRef, SystemException> {
        self.check(&[ServerPoint::SendOther])?;
        self.forward.borrow().clone().ok_or_else(invalid_point)
    }

    /// `get_slot`: the request's slots.
    pub fn get_slot(&self, id: SlotId) -> Result<Any, InvalidSlot> {
        self.slots
            .borrow()
            .get(id as usize)
            .cloned()
            .ok_or(InvalidSlot)
    }

    /// `set_slot`: what the servant will see in [`PiCurrent`] (when set
    /// before `receive_request`).
    pub fn set_slot(&self, id: SlotId, data: Any) -> Result<(), InvalidSlot> {
        match self.slots.borrow_mut().get_mut(id as usize) {
            Some(slot) => {
                *slot = data;
                Ok(())
            }
            None => Err(InvalidSlot),
        }
    }

    /// `get_request_service_context`: any context the client sent.
    pub fn get_request_service_context(&self, id: u32) -> Result<ServiceContext, SystemException> {
        find_context(self.request_contexts, id)
    }

    /// The contexts the client sent (rustorb).
    pub fn request_service_contexts(&self) -> &[ServiceContext] {
        self.request_contexts
    }

    /// `get_reply_service_context`: one added for the reply.
    pub fn get_reply_service_context(&self, id: u32) -> Result<ServiceContext, SystemException> {
        self.sending()?;
        find_context(&self.reply_contexts.borrow(), id)
    }

    /// `add_reply_service_context`.
    pub fn add_reply_service_context(
        &self,
        sc: ServiceContext,
        replace: bool,
    ) -> Result<(), SystemException> {
        if is_core_context(sc.context_id) {
            return Err(duplicate_context());
        }
        add_context(&mut self.reply_contexts.borrow_mut(), sc, replace)
    }

    /// `sending_exception`: in `send_exception`. A user exception a static
    /// skeleton wrote is an Any of `UNKNOWN`; one a DSI servant set is
    /// itself.
    pub fn sending_exception(&self) -> Result<Any, SystemException> {
        self.check(&[ServerPoint::SendException])?;
        match &*self.exception.borrow() {
            Some(ServerException::System(e)) => Ok(system_exception_any(e)),
            Some(ServerException::User(_)) => Ok(self
                .dsi
                .borrow()
                .as_ref()
                .and_then(|d| d.exception.clone())
                .unwrap_or_else(unknown_any)),
            None => Err(invalid_point()),
        }
    }

    /// The repository id of the exception, in `send_exception` (rustorb).
    pub fn sending_exception_id(&self) -> Result<String, SystemException> {
        self.check(&[ServerPoint::SendException])?;
        match &*self.exception.borrow() {
            Some(ServerException::System(e)) => Ok(e.repository_id.clone()),
            Some(ServerException::User(id)) => Ok(id.clone()),
            None => Err(invalid_point()),
        }
    }

    fn target(&self) -> Result<&ServerTarget, SystemException> {
        self.not_first()?;
        self.target.ok_or_else(not_available)
    }

    /// `object_id`.
    pub fn object_id(&self) -> Result<Vec<u8>, SystemException> {
        Ok(self.target()?.id.clone())
    }

    /// `adapter_id`: the POA's part of the object key.
    pub fn adapter_id(&self) -> Result<Vec<u8>, SystemException> {
        Ok(self.target()?.poa.inner.key(&[]))
    }

    /// `adapter_name`: the names of the POAs from the RootPOA down.
    pub fn adapter_name(&self) -> Result<Vec<String>, SystemException> {
        let t = self.target()?;
        let mut names = vec!["RootPOA".to_owned()];
        names.extend(t.poa.inner.path().iter().cloned());
        Ok(names)
    }

    /// `orb_id` (`-ORBid`).
    pub fn orb_id(&self) -> &str {
        &self.orb.inner.options.orb_id
    }

    /// `server_id` (`-ORBServerId`).
    pub fn server_id(&self) -> &str {
        &self.orb.inner.options.server_id
    }

    fn servant(&self) -> Result<Arc<dyn Servant>, SystemException> {
        self.not_first()?;
        self.servant.borrow().clone().ok_or_else(not_available)
    }

    /// `target_most_derived_interface`: from `receive_request` on.
    pub fn target_most_derived_interface(&self) -> Result<String, SystemException> {
        Ok(self.servant()?.repository_id().to_owned())
    }

    /// `target_is_a`.
    pub fn target_is_a(&self, id: &str) -> Result<bool, SystemException> {
        let servant = self.servant()?;
        Ok(id == "IDL:omg.org/CORBA/Object:1.0" || servant.is_a(id))
    }

    /// `get_server_policy`: a policy of the target's POA.
    pub fn get_server_policy(
        &self,
        policy_type: PolicyType,
    ) -> Result<PolicyValue, SystemException> {
        let t = self.target.ok_or_else(not_available)?;
        policy_of(t.poa.inner.policies(), policy_type)
    }

    pub(crate) fn set_servant(&self, servant: &Arc<dyn Servant>) {
        *self.servant.borrow_mut() = Some(Arc::clone(servant));
    }

    pub(crate) fn set_dsi(&self, dsi: DsiParams) {
        *self.dsi.borrow_mut() = Some(dsi);
    }

    pub(crate) fn reply_contexts(&self) -> Vec<ServiceContext> {
        self.reply_contexts.borrow().clone()
    }

    /// Makes the request's slots those of the thread while the guard
    /// lives (the servant sees them).
    pub(crate) fn enter_thread_scope(&self, pi: &Interceptors) -> ThreadSlotsGuard {
        ThreadSlotsGuard::enter(pi.serial, self.slots.borrow().clone())
    }

    fn enter(&self, ending: &ServerEnding) {
        *self.exception.borrow_mut() = None;
        *self.forward.borrow_mut() = None;
        let (point, status) = match ending {
            ServerEnding::Reply => (ServerPoint::SendReply, reply_status::SUCCESSFUL),
            ServerEnding::Exception(e) => {
                *self.exception.borrow_mut() = Some(e.clone());
                let status = match e {
                    ServerException::System(_) => reply_status::SYSTEM_EXCEPTION,
                    ServerException::User(_) => reply_status::USER_EXCEPTION,
                };
                (ServerPoint::SendException, status)
            }
            ServerEnding::Forward(to) => {
                *self.forward.borrow_mut() = Some(to.clone());
                (ServerPoint::SendOther, reply_status::LOCATION_FORWARD)
            }
        };
        self.point.set(point);
        self.reply_status.set(status);
    }
}

fn policy_of(p: &Policies, policy_type: PolicyType) -> Result<PolicyValue, SystemException> {
    use crate::policy::*;
    let policy = match policy_type {
        THREAD_POLICY_ID => Policy::Thread(p.thread),
        LIFESPAN_POLICY_ID => Policy::Lifespan(p.lifespan),
        ID_UNIQUENESS_POLICY_ID => Policy::IdUniqueness(p.id_uniqueness),
        ID_ASSIGNMENT_POLICY_ID => Policy::IdAssignment(p.id_assignment),
        IMPLICIT_ACTIVATION_POLICY_ID => Policy::ImplicitActivation(p.implicit_activation),
        SERVANT_RETENTION_POLICY_ID => Policy::ServantRetention(p.servant_retention),
        REQUEST_PROCESSING_POLICY_ID => Policy::RequestProcessing(p.request_processing),
        _ => {
            return Err(SystemException::new(
                SystemExceptionKind::INV_POLICY,
                minor::INVALID_POLICY_TYPE,
                CompletionStatus::No,
            ));
        }
    };
    Ok(PolicyValue::Poa(policy))
}

/// Runs `receive_request_service_contexts` on each interceptor. `Err` is
/// how the request ends when one of them raised.
pub(crate) fn receive_request_service_contexts(
    pi: &Interceptors,
    info: &ServerRequestInfo<'_>,
) -> Result<(), ServerEnding> {
    info.point.set(ServerPoint::ReceiveRequestServiceContexts);
    for (k, i) in pi.server.iter().enumerate() {
        if let Err(e) = catch_server(|| i.receive_request_service_contexts(info)) {
            return Err(end_server(pi, info, ServerEnding::from_error(e), k));
        }
    }
    Ok(())
}

/// Runs `receive_request` on each interceptor.
pub(crate) fn receive_request(
    pi: &Interceptors,
    info: &ServerRequestInfo<'_>,
) -> Result<(), ServerEnding> {
    info.point.set(ServerPoint::ReceiveRequest);
    for i in &pi.server {
        if let Err(e) = catch_server(|| i.receive_request(info)) {
            return Err(send_reply(pi, info, ServerEnding::from_error(e)));
        }
    }
    Ok(())
}

/// Runs the `send_*` points on every interceptor, in the reverse order,
/// and returns the reply to send.
pub(crate) fn send_reply(
    pi: &Interceptors,
    info: &ServerRequestInfo<'_>,
    ending: ServerEnding,
) -> ServerEnding {
    end_server(pi, info, ending, pi.server.len())
}

fn end_server(
    pi: &Interceptors,
    info: &ServerRequestInfo<'_>,
    mut ending: ServerEnding,
    started: usize,
) -> ServerEnding {
    for i in pi.server[..started].iter().rev() {
        info.enter(&ending);
        let result = match &ending {
            ServerEnding::Reply => {
                catch_server(|| i.send_reply(info).map_err(InterceptError::System))
            }
            ServerEnding::Exception(_) => catch_server(|| i.send_exception(info)),
            ServerEnding::Forward(_) => catch_server(|| i.send_other(info)),
        };
        if let Err(e) = result {
            ending = ServerEnding::from_error(e);
        }
    }
    ending
}

fn catch_server(f: impl FnOnce() -> Result<(), InterceptError>) -> Result<(), InterceptError> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).unwrap_or_else(|_| {
        trace::warning!("server request interceptor panicked");
        Err(InterceptError::System(SystemException::new(
            SystemExceptionKind::UNKNOWN,
            0,
            CompletionStatus::Maybe,
        )))
    })
}

// --- IOR interceptors ----------------------------------------------------------

/// `PortableInterceptor::IORInfo`: a new POA, for
/// [`IorInterceptor::establish_components`].
pub struct IorInfo<'a> {
    policies: &'a Policies,
    adapter_name: Vec<String>,
    components: RefCell<Vec<TaggedComponent>>,
}

impl fmt::Debug for IorInfo<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IorInfo")
            .field("adapter_name", &self.adapter_name)
            .finish_non_exhaustive()
    }
}

impl IorInfo<'_> {
    /// `get_effective_policy`: a policy of the POA.
    pub fn get_effective_policy(
        &self,
        policy_type: PolicyType,
    ) -> Result<PolicyValue, SystemException> {
        policy_of(self.policies, policy_type)
    }

    /// The names of the POAs from the RootPOA down to the new one
    /// (rustorb).
    pub fn adapter_name(&self) -> &[String] {
        &self.adapter_name
    }

    /// `add_ior_component`: adds `component` to every profile of the
    /// POA's references.
    pub fn add_ior_component(&self, component: TaggedComponent) {
        self.components.borrow_mut().push(component);
    }

    /// `add_ior_component_to_profile`: only `TAG_INTERNET_IOP`, the one
    /// profile rustorb makes (`BAD_PARAM` for another).
    pub fn add_ior_component_to_profile(
        &self,
        component: TaggedComponent,
        profile_id: u32,
    ) -> Result<(), SystemException> {
        if profile_id != TAG_INTERNET_IOP {
            return Err(SystemException::new(
                SystemExceptionKind::BAD_PARAM,
                minor::UNKNOWN_PROFILE,
                CompletionStatus::No,
            ));
        }
        self.add_ior_component(component);
        Ok(())
    }
}

/// The components the IOR interceptors of `orb` add to the references of
/// a new POA with `policies`, whose names below the RootPOA are `path`.
pub(crate) fn establish_components(
    orb: &Orb,
    policies: &Policies,
    path: &[String],
) -> Vec<TaggedComponent> {
    let Some(pi) = orb.interceptors() else {
        return Vec::new();
    };
    if pi.ior.is_empty() {
        return Vec::new();
    }
    let mut adapter_name = vec!["RootPOA".to_owned()];
    adapter_name.extend(path.iter().cloned());
    let info = IorInfo {
        policies,
        adapter_name,
        components: RefCell::new(Vec::new()),
    };
    for i in &pi.ior {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            i.establish_components(&info)
        }));
        match result {
            Ok(Ok(())) => {}
            Ok(Err(e)) => trace::warning!(
                "IORInterceptor {:?} raised {e} in establish_components (ignored)",
                i.name()
            ),
            Err(_) => trace::warning!(
                "IORInterceptor {:?} panicked in establish_components (ignored)",
                i.name()
            ),
        }
    }
    info.components.into_inner()
}

/// The information of a client request is held across the awaits of an
/// async call.
#[allow(dead_code)]
fn assert_send() {
    fn is_send<T: Send>() {}
    is_send::<ClientRequestInfo<'static>>();
}

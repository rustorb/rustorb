use std::fmt;
use std::future::Future;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use rustorb_cdr::CdrDecode;
use rustorb_giop::{
    AddressingDisposition, AlternateIiopAddress, Body, CancelRequestHeader, CodeSetComponentInfo,
    Ior, LocateRequestHeader, LocateStatus, Message, MessageEncoder, OutMessage, ReplyStatus,
    RequestHeader, SslSecTrans, SystemExceptionBody, TAG_INTERNET_IOP, TAG_MULTIPLE_COMPONENTS,
    TargetAddress, Version, component_tag, decode_message, response_flags,
};

use crate::connection::{Closed, Connection, SendError};
use crate::server::Server;
use crate::stream::{InStream, OutStream, marshal};
use crate::{
    CodeSets, CompletionStatus, Exception, Orb, SystemException, SystemExceptionKind,
    UserException, minor, timeout,
};

/// A reference to a CORBA object, from an IOR.
///
/// Invocations try each IIOP profile of the IOR, and each address in it,
/// until one can be reached. An address of the ORB's own server side is
/// not reached through the network: the request is dispatched on the
/// calling thread. A `LOCATION_FORWARD` reply makes the
/// reference use the forwarded IOR from then on (shared by its clones);
/// when the forwarded object cannot be reached any more, the original IOR
/// is used again.
#[derive(Clone)]
pub struct ObjectRef {
    orb: Orb,
    ior: Arc<Ior>,
    forward: Arc<Mutex<Option<Arc<Ior>>>>,
    /// [`ObjectRef::set_call_timeout`], shared by clones.
    call_timeout: Arc<AtomicU64>,
}

impl fmt::Debug for ObjectRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ObjectRef")
            .field("type_id", &self.ior.type_id)
            .field("profiles", &self.ior.profiles.len())
            .finish_non_exhaustive()
    }
}

/// `==` is [`ObjectRef::is_equivalent`].
impl PartialEq for ObjectRef {
    fn eq(&self, other: &ObjectRef) -> bool {
        self.is_equivalent(other)
    }
}

type ArgsFn<'a> = dyn Fn(&mut OutStream<'_>) -> Result<(), SystemException> + 'a;

/// The arguments of an [`AsyncRequest`], whose future may move between
/// threads.
type SendArgsFn<'a> = dyn Fn(&mut OutStream<'_>) -> Result<(), SystemException> + Send + Sync + 'a;

/// Writes the `in` and `inout` arguments of a request: either kind of
/// argument function.
trait WriteArgs {
    fn write_args(&self, out: &mut OutStream<'_>) -> Result<(), SystemException>;
}

impl<F> WriteArgs for F
where
    F: Fn(&mut OutStream<'_>) -> Result<(), SystemException> + ?Sized,
{
    fn write_args(&self, out: &mut OutStream<'_>) -> Result<(), SystemException> {
        self(out)
    }
}

impl ObjectRef {
    pub(crate) fn new(orb: Orb, ior: Ior) -> ObjectRef {
        ObjectRef {
            orb,
            ior: Arc::new(ior),
            forward: Arc::new(Mutex::new(None)),
            call_timeout: Arc::new(AtomicU64::new(timeout::UNSET)),
        }
    }

    /// A reference to the same object (sharing its forward) whose call
    /// timeout is its own, starting from this one's.
    pub(crate) fn with_own_settings(&self) -> ObjectRef {
        ObjectRef {
            call_timeout: Arc::new(AtomicU64::new(self.call_timeout.load(Ordering::Relaxed))),
            ..self.clone()
        }
    }

    /// The IOR the reference was created from.
    pub fn ior(&self) -> &Ior {
        &self.ior
    }

    /// The IOR invocations currently go to: the last forwarded one, or the
    /// original.
    pub fn effective_ior(&self) -> Ior {
        (*self.current_target()).clone()
    }

    /// The repository id in the IOR, which may be empty.
    pub fn type_id(&self) -> &str {
        &self.ior.type_id
    }

    /// The ORB the reference belongs to.
    pub fn orb(&self) -> &Orb {
        &self.orb
    }

    /// `CORBA::Object::_is_equivalent`: whether both references have the
    /// same profiles, so they certainly denote the same object. `false`
    /// does not mean they are different objects.
    pub fn is_equivalent(&self, other: &ObjectRef) -> bool {
        Arc::ptr_eq(&self.ior, &other.ior) || self.ior.profiles == other.ior.profiles
    }

    /// Starts a request for `operation`.
    pub fn request<'a>(&'a self, operation: &str) -> Request<'a> {
        Request {
            obj: self,
            operation: operation.to_owned(),
            args: None,
            timeout: None,
        }
    }

    /// Starts a request for `operation` that runs as a future
    /// ([`AsyncRequest`]).
    pub fn request_async<'a>(&'a self, operation: &str) -> AsyncRequest<'a> {
        AsyncRequest {
            obj: self,
            operation: operation.to_owned(),
            args: None,
            timeout: None,
        }
    }

    /// Sets how long calls on this reference and its clones wait for a
    /// reply: `Some(t)` (where `t` is `None` for no limit) overrides the
    /// ORB's [`call_timeout`](Orb::call_timeout) and the calling thread's
    /// ([`set_thread_call_timeout`](crate::set_thread_call_timeout)), and
    /// `None` unsets it. Like omniORB's `setClientCallTimeout(obj,
    /// millisecs)`. References made from the same IOR by
    /// [`Orb::string_to_object`] do not share it.
    pub fn set_call_timeout(&self, timeout: Option<Option<Duration>>) {
        let v = timeout.map_or(timeout::UNSET, timeout::encode);
        self.call_timeout.store(v, Ordering::Relaxed);
    }

    /// The setting of [`set_call_timeout`](ObjectRef::set_call_timeout).
    pub fn call_timeout(&self) -> Option<Option<Duration>> {
        match self.call_timeout.load(Ordering::Relaxed) {
            timeout::UNSET => None,
            v => Some(timeout::decode(v)),
        }
    }

    /// The timeout of a call on this reference from the calling thread:
    /// the reference's, else the thread's, else the ORB's (the order
    /// omniORB uses).
    pub(crate) fn effective_timeout(&self) -> Option<Duration> {
        self.call_timeout()
            .or_else(timeout::thread_call_timeout)
            .unwrap_or_else(|| self.orb.call_timeout())
    }

    /// `CORBA::Object::_is_a`.
    pub fn is_a(&self, repository_id: &str) -> Result<bool, Exception> {
        let reply = self
            .request("_is_a")
            .args(|out| out.write_string(repository_id))
            .invoke()?;
        Ok(reply.input().read::<bool>()?)
    }

    /// `CORBA::Object::_non_existent`. `OBJECT_NOT_EXIST` from the server
    /// counts as `true`.
    pub fn non_existent(&self) -> Result<bool, Exception> {
        match self.request("_non_existent").invoke() {
            Ok(reply) => Ok(reply.input().read::<bool>()?),
            Err(Exception::System(e)) if e.is(SystemExceptionKind::OBJECT_NOT_EXIST) => Ok(true),
            Err(e) => Err(e),
        }
    }

    /// The async form of [`is_a`](ObjectRef::is_a).
    pub async fn is_a_async(&self, repository_id: &str) -> Result<bool, Exception> {
        let id = repository_id.to_owned();
        let reply = self
            .request_async("_is_a")
            .args(move |out| out.write_string(&id))
            .invoke()
            .await?;
        Ok(reply.input().read::<bool>()?)
    }

    /// The async form of [`non_existent`](ObjectRef::non_existent).
    pub async fn non_existent_async(&self) -> Result<bool, Exception> {
        match self.request_async("_non_existent").invoke().await {
            Ok(reply) => Ok(reply.input().read::<bool>()?),
            Err(Exception::System(e)) if e.is(SystemExceptionKind::OBJECT_NOT_EXIST) => Ok(true),
            Err(e) => Err(e),
        }
    }

    /// Sends a `LocateRequest`, following forwards. Returns whether the
    /// server has the object (`OBJECT_HERE`) or not (`UNKNOWN_OBJECT`).
    pub fn locate(&self) -> Result<bool, Exception> {
        let kind = Kind::<ArgsFn<'_>>::Locate;
        let outcome = invoke(self, &kind, self.effective_timeout())?;
        match outcome {
            Outcome::Located(here) => Ok(here),
            Outcome::Reply(_) => unreachable!("a locate request gives a locate outcome"),
        }
    }

    /// The async form of [`locate`](ObjectRef::locate).
    pub fn locate_async(&self) -> impl Future<Output = Result<bool, Exception>> + Send + '_ {
        let timeout = self.effective_timeout();
        self.orb.in_runtime(async move {
            let kind = Kind::<SendArgsFn<'_>>::Locate;
            match invoke_async(self, &kind, timeout).await? {
                Outcome::Located(here) => Ok(here),
                Outcome::Reply(_) => unreachable!("a locate request gives a locate outcome"),
            }
        })
    }

    /// `CORBA::Object::validate_connection`: binds the reference with a
    /// `LocateRequest` (following forwards) and returns true. rustorb has
    /// no policy that can forbid a call, so it never returns false. The
    /// failure to bind raises its exception (`TRANSIENT`, `TIMEOUT`...),
    /// and an object the server does not have `OBJECT_NOT_EXIST`.
    pub fn validate_connection(&self) -> Result<bool, Exception> {
        if self.locate()? {
            Ok(true)
        } else {
            Err(SystemException::new(
                SystemExceptionKind::OBJECT_NOT_EXIST,
                minor::NO_SUCH_OBJECT,
                CompletionStatus::No,
            )
            .into())
        }
    }

    fn current_target(&self) -> Arc<Ior> {
        match &*self.forward.lock().unwrap() {
            Some(ior) => Arc::clone(ior),
            None => Arc::clone(&self.ior),
        }
    }

    fn set_forward(&self, ior: Option<Arc<Ior>>) {
        *self.forward.lock().unwrap() = ior;
    }
}

/// A request being built: the operation, its arguments, and a timeout.
pub struct Request<'a> {
    obj: &'a ObjectRef,
    operation: String,
    args: Option<Box<ArgsFn<'a>>>,
    /// Set by [`Request::timeout`].
    timeout: Option<Option<Duration>>,
}

impl fmt::Debug for Request<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Request")
            .field("operation", &self.operation)
            .field("timeout", &self.timeout)
            .finish_non_exhaustive()
    }
}

impl<'a> Request<'a> {
    /// Sets the function that writes the `in` and `inout` arguments. It may
    /// run more than once, when the request is forwarded or retried.
    pub fn args<F>(mut self, f: F) -> Request<'a>
    where
        F: Fn(&mut OutStream<'_>) -> Result<(), SystemException> + 'a,
    {
        self.args = Some(Box::new(f));
        self
    }

    /// Sets how long this request waits for a reply (`None`: no limit),
    /// overriding the timeouts of the thread, the reference and the ORB
    /// (see [`Orb::set_call_timeout`]).
    pub fn timeout(mut self, timeout: Option<Duration>) -> Request<'a> {
        self.timeout = Some(timeout);
        self
    }

    fn effective_timeout(&self) -> Option<Duration> {
        self.timeout.unwrap_or_else(|| self.obj.effective_timeout())
    }

    /// Sends the request and waits for the reply.
    pub fn invoke(self) -> Result<Reply, Exception> {
        let kind = Kind::Request {
            operation: &self.operation,
            args: self.args.as_deref(),
            response: true,
        };
        match invoke(self.obj, &kind, self.effective_timeout())? {
            Outcome::Reply(Some(reply)) => Ok(reply),
            _ => unreachable!("a two-way request gives a reply"),
        }
    }

    /// Sends the request as a oneway call: returns once the message is
    /// written, with no reply.
    pub fn send_oneway(self) -> Result<(), SystemException> {
        let kind = Kind::Request {
            operation: &self.operation,
            args: self.args.as_deref(),
            response: false,
        };
        match invoke(self.obj, &kind, self.effective_timeout()) {
            Ok(_) => Ok(()),
            Err(Exception::System(e)) => Err(e),
            Err(Exception::User(_)) => unreachable!("a oneway request has no reply"),
        }
    }
}

/// A request that runs as a future: [`Request`] for async code.
///
/// The future can be polled by any executor, or none in particular: the
/// sockets and timers of the call belong to the ORB's runtime. It does not
/// block the thread that polls it, except that a call to an object of the
/// ORB's own server side runs the servant on an upcall thread and waits
/// for it without blocking. The timeout is taken when
/// [`invoke`](AsyncRequest::invoke) is called, so a timeout of the calling
/// thread ([`set_thread_call_timeout`](crate::set_thread_call_timeout))
/// counts as it does for [`Request`].
pub struct AsyncRequest<'a> {
    obj: &'a ObjectRef,
    operation: String,
    args: Option<Box<SendArgsFn<'a>>>,
    timeout: Option<Option<Duration>>,
}

impl fmt::Debug for AsyncRequest<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AsyncRequest")
            .field("operation", &self.operation)
            .field("timeout", &self.timeout)
            .finish_non_exhaustive()
    }
}

impl<'a> AsyncRequest<'a> {
    /// [`Request::args`]; the function must be `Send` and `Sync`, as the
    /// future may move between threads.
    pub fn args<F>(mut self, f: F) -> AsyncRequest<'a>
    where
        F: Fn(&mut OutStream<'_>) -> Result<(), SystemException> + Send + Sync + 'a,
    {
        self.args = Some(Box::new(f));
        self
    }

    /// [`Request::timeout`].
    pub fn timeout(mut self, timeout: Option<Duration>) -> AsyncRequest<'a> {
        self.timeout = Some(timeout);
        self
    }

    /// Sends the request; the future resolves to the reply.
    pub fn invoke(self) -> impl Future<Output = Result<Reply, Exception>> + Send + 'a {
        let timeout = self.timeout.unwrap_or_else(|| self.obj.effective_timeout());
        let orb = self.obj.orb.clone();
        orb.in_runtime(async move {
            let kind = Kind::Request {
                operation: &self.operation,
                args: self.args.as_deref(),
                response: true,
            };
            match invoke_async(self.obj, &kind, timeout).await? {
                Outcome::Reply(Some(reply)) => Ok(reply),
                _ => unreachable!("a two-way request gives a reply"),
            }
        })
    }

    /// Sends the request as a oneway call; the future resolves once the
    /// message is written.
    pub fn send_oneway(self) -> impl Future<Output = Result<(), SystemException>> + Send + 'a {
        let timeout = self.timeout.unwrap_or_else(|| self.obj.effective_timeout());
        let orb = self.obj.orb.clone();
        orb.in_runtime(async move {
            let kind = Kind::Request {
                operation: &self.operation,
                args: self.args.as_deref(),
                response: false,
            };
            match invoke_async(self.obj, &kind, timeout).await {
                Ok(_) => Ok(()),
                Err(Exception::System(e)) => Err(e),
                Err(Exception::User(_)) => unreachable!("a oneway request has no reply"),
            }
        })
    }
}

/// A normal reply: the return value and the `out` and `inout` arguments.
#[derive(Debug, Clone)]
pub struct Reply {
    body: Body,
    code_sets: CodeSets,
    orb: Orb,
}

impl Reply {
    /// A stream over the reply body.
    pub fn input(&self) -> InStream<'_> {
        InStream {
            dec: self.body.decoder(),
            code_sets: self.code_sets,
            orb: Some(&self.orb),
            completed: CompletionStatus::Yes,
            wchar_needs_code_set: false,
            values: Vec::new(),
            typed_values: Vec::new(),
            custom_items: None,
            body: Some(&self.body),
        }
    }

    /// The raw reply body.
    pub fn body(&self) -> &Body {
        &self.body
    }
}

enum Kind<'k, A: ?Sized = ArgsFn<'k>> {
    Request {
        operation: &'k str,
        args: Option<&'k A>,
        response: bool,
    },
    Locate,
}

enum Outcome {
    Reply(Option<Reply>),
    Located(bool),
}

enum Attempt {
    Done(Result<Outcome, Exception>),
    /// The request was not processed here; try the next address.
    NotSent(SystemException),
    Forward(Ior),
    Addressing(AddressingDisposition),
}

/// One address an object can be reached at.
struct Endpoint {
    profile_index: usize,
    version: Version,
    host: String,
    port: u16,
    /// A TLS endpoint (from `TAG_SSL_SEC_TRANS`).
    tls: bool,
    object_key: Vec<u8>,
    code_sets: Option<CodeSetComponentInfo>,
}

/// Every IIOP address in `ior` this ORB can use, in order: for each
/// profile, its TLS endpoint if it has one and the ORB can use TLS
/// (docs/ssliop.md), else its own address and its alternate addresses.
/// A port of 0 is no address.
fn endpoints(ior: &Ior, can_tls: bool) -> Vec<Endpoint> {
    let shared: Vec<_> = ior
        .profiles
        .iter()
        .filter(|p| p.tag == TAG_MULTIPLE_COMPONENTS)
        .filter_map(|p| p.multiple_components().ok())
        .flatten()
        .collect();
    let mut out = Vec::new();
    for (index, profile) in ior.profiles.iter().enumerate() {
        if profile.tag != TAG_INTERNET_IOP {
            continue;
        }
        let Ok(iiop) = profile.iiop() else { continue };
        if iiop.version.major != 1 {
            continue;
        }
        // IIOP 1.0 profiles have no components of their own; they use
        // those of the TAG_MULTIPLE_COMPONENTS profile.
        let components = if iiop.version.minor == 0 {
            &shared
        } else {
            &iiop.components
        };
        let code_sets = components
            .iter()
            .find(|c| c.tag == component_tag::CODE_SETS)
            .and_then(|c| CodeSetComponentInfo::from_component(c).ok());
        let ssl = components
            .iter()
            .find(|c| c.tag == component_tag::SSL_SEC_TRANS)
            .and_then(|c| SslSecTrans::from_component(c).ok());
        let mut addrs = Vec::new();
        if let (Some(ssl), true) = (ssl, can_tls) {
            // Not unprotected instead, should the TLS endpoint fail.
            addrs.push((iiop.host.clone(), ssl.port, true));
        } else {
            addrs.push((iiop.host.clone(), iiop.port, false));
            addrs.extend(
                components
                    .iter()
                    .filter(|c| c.tag == component_tag::ALTERNATE_IIOP_ADDRESS)
                    .filter_map(|c| AlternateIiopAddress::from_component(c).ok())
                    .map(|a| (a.host, a.port, false)),
            );
        }
        for (host, port, tls) in addrs {
            if port == 0 {
                continue;
            }
            out.push(Endpoint {
                profile_index: index,
                version: iiop.version,
                host,
                port,
                tls,
                object_key: iiop.object_key.clone(),
                code_sets: code_sets.clone(),
            });
        }
    }
    out
}

fn sys(kind: SystemExceptionKind, minor: u32, completed: CompletionStatus) -> SystemException {
    SystemException::new(kind, minor, completed)
}

/// The walk of a call over the addresses of its target, the forwards and
/// the addressing modes; the same for blocking and async calls.
struct Walk<'o> {
    obj: &'o ObjectRef,
    max_hops: u32,
    hops: u32,
    ior: Arc<Ior>,
    disposition: AddressingDisposition,
    endpoints: std::vec::IntoIter<Endpoint>,
    /// Why the addresses tried so far could not be used.
    last: SystemException,
    /// The target was just set (and is still to be checked for nil).
    fresh: bool,
}

enum Step {
    Try(Endpoint),
    Done(Result<Outcome, Exception>),
}

impl<'o> Walk<'o> {
    fn new(obj: &'o ObjectRef) -> Walk<'o> {
        let mut walk = Walk {
            obj,
            max_hops: obj.orb.inner.options.max_forwards,
            hops: 0,
            ior: obj.current_target(),
            disposition: AddressingDisposition::Key,
            endpoints: Vec::new().into_iter(),
            last: no_usable_profile(),
            fresh: true,
        };
        walk.retarget(Arc::clone(&walk.ior));
        walk
    }

    fn retarget(&mut self, ior: Arc<Ior>) {
        self.endpoints = endpoints(&ior, self.obj.orb.has_tls_client()).into_iter();
        self.ior = ior;
        self.last = no_usable_profile();
        self.fresh = true;
    }

    /// The next address to try, or the end of the call.
    fn next(&mut self) -> Step {
        loop {
            if std::mem::take(&mut self.fresh) && self.ior.is_nil() {
                return Step::Done(Err(sys(
                    SystemExceptionKind::INV_OBJREF,
                    0,
                    CompletionStatus::No,
                )
                .into()));
            }
            if let Some(ep) = self.endpoints.next() {
                return Step::Try(ep);
            }
            // Nothing could be reached. A forwarded target falls back to
            // the original reference.
            if !Arc::ptr_eq(&self.ior, &self.obj.ior) {
                self.hops += 1;
                if self.hops <= self.max_hops {
                    self.obj.set_forward(None);
                    self.disposition = AddressingDisposition::Key;
                    self.retarget(Arc::clone(&self.obj.ior));
                    continue;
                }
            }
            return Step::Done(Err(self.last.clone().into()));
        }
    }

    /// Takes in the attempt at the address [`next`](Walk::next) gave:
    /// `Some` ends the call.
    fn record(&mut self, attempt: Attempt) -> Option<Result<Outcome, Exception>> {
        match attempt {
            Attempt::Done(result) => Some(result),
            Attempt::NotSent(e) => {
                self.last = e;
                None
            }
            Attempt::Forward(next) => {
                self.hops += 1;
                if self.hops > self.max_hops {
                    return Some(Err(sys(
                        SystemExceptionKind::TRANSIENT,
                        0,
                        CompletionStatus::No,
                    )
                    .into()));
                }
                let next = Arc::new(next);
                self.obj.set_forward(Some(Arc::clone(&next)));
                self.disposition = AddressingDisposition::Key;
                self.retarget(next);
                None
            }
            Attempt::Addressing(wanted) => {
                self.hops += 1;
                if self.hops > self.max_hops || wanted == self.disposition {
                    return Some(Err(sys(
                        SystemExceptionKind::MARSHAL,
                        0,
                        CompletionStatus::No,
                    )
                    .into()));
                }
                self.disposition = wanted;
                self.retarget(Arc::clone(&self.ior));
                None
            }
        }
    }
}

fn no_usable_profile() -> SystemException {
    sys(
        SystemExceptionKind::TRANSIENT,
        minor::NO_USABLE_PROFILE,
        CompletionStatus::No,
    )
}

fn invoke<A: WriteArgs + ?Sized>(
    obj: &ObjectRef,
    kind: &Kind<'_, A>,
    timeout: Option<Duration>,
) -> Result<Outcome, Exception> {
    let orb = &obj.orb;
    orb.check_running()?;
    // The timeout covers the whole call: connecting, forwards and the
    // wait for the reply.
    let deadline = timeout.map(|t| Instant::now() + t);
    let mut walk = Walk::new(obj);
    loop {
        let ep = match walk.next() {
            Step::Try(ep) => ep,
            Step::Done(result) => return result,
        };
        let (ior, disposition) = (Arc::clone(&walk.ior), walk.disposition);
        let attempt = match orb.local_server(&ep.host, ep.port, ep.tls) {
            Some(server) => match local_call(orb, &ior, &ep, kind, deadline, disposition) {
                Ok(call) => {
                    let result = call.run(&server);
                    local_outcome(orb, result)
                }
                Err(done) => done,
            },
            None => orb.block_on(attempt(orb, &ior, &ep, kind, deadline, disposition, true))?,
        };
        if let Some(result) = walk.record(attempt) {
            return result;
        }
    }
}

/// [`invoke`] as a future, run inside the ORB's runtime
/// ([`Orb::in_runtime`]). A call to the ORB's own server side runs on an
/// upcall thread.
async fn invoke_async<A: WriteArgs + ?Sized>(
    obj: &ObjectRef,
    kind: &Kind<'_, A>,
    timeout: Option<Duration>,
) -> Result<Outcome, Exception> {
    let orb = &obj.orb;
    orb.check_running()?;
    let deadline = timeout.map(|t| Instant::now() + t);
    let mut walk = Walk::new(obj);
    loop {
        let ep = match walk.next() {
            Step::Try(ep) => ep,
            Step::Done(result) => return result,
        };
        let (ior, disposition) = (Arc::clone(&walk.ior), walk.disposition);
        let attempt = match orb.local_server(&ep.host, ep.port, ep.tls) {
            Some(server) => match local_call(orb, &ior, &ep, kind, deadline, disposition) {
                Ok(call) => {
                    let (tx, rx) = tokio::sync::oneshot::channel();
                    let on_pool = Arc::clone(&server);
                    server.submit(Box::new(move || {
                        let _ = tx.send(call.run(&on_pool));
                    }));
                    // The pool is gone when the ORB is being destroyed.
                    let result = rx.await.unwrap_or_else(|_| {
                        Err(sys(SystemExceptionKind::TRANSIENT, 0, CompletionStatus::No))
                    });
                    local_outcome(orb, result)
                }
                Err(done) => done,
            },
            None => attempt(orb, &ior, &ep, kind, deadline, disposition, false).await,
        };
        if let Some(result) = walk.record(attempt) {
            return result;
        }
    }
}

/// How long a `CancelRequest` after a timeout may take to go out.
const CANCEL_SEND_TIME: Duration = Duration::from_millis(100);

/// Sends the request to one endpoint. A `CloseConnection` before the reply
/// means the server did not process it, so it is sent once more on a new
/// connection.
///
/// With `own_thread`, the caller is a thread of its own (a blocking call),
/// which may block reading the reply.
async fn attempt<A: WriteArgs + ?Sized>(
    orb: &Orb,
    ior: &Ior,
    ep: &Endpoint,
    kind: &Kind<'_, A>,
    deadline: Option<Instant>,
    disposition: AddressingDisposition,
    own_thread: bool,
) -> Attempt {
    let mut result = Attempt::NotSent(sys(SystemExceptionKind::TRANSIENT, 0, CompletionStatus::No));
    for _ in 0..2 {
        let conn = match orb.connection(&ep.host, ep.port, ep.tls, deadline).await {
            Ok(conn) => conn,
            // Out of time: the other addresses are not tried.
            Err(e) if e.is(SystemExceptionKind::TIMEOUT) => return Attempt::Done(Err(e.into())),
            Err(e) => return Attempt::NotSent(e),
        };
        match attempt_on(orb, &conn, ior, ep, kind, deadline, disposition, own_thread).await {
            Ok(done) => return done,
            Err(Closed::Orderly) => {
                orb.discard(&conn);
                result =
                    Attempt::NotSent(sys(SystemExceptionKind::TRANSIENT, 0, CompletionStatus::No));
            }
            Err(Closed::Failed(_)) => {
                orb.discard(&conn);
                return Attempt::Done(Err(sys(
                    SystemExceptionKind::COMM_FAILURE,
                    0,
                    CompletionStatus::Maybe,
                )
                .into()));
            }
        }
    }
    result
}

/// `Err(Closed::Orderly)` asks for a retry on a new connection;
/// `Err(Closed::Failed)` after the request was sent is `COMM_FAILURE`.
#[allow(clippy::too_many_arguments)]
async fn attempt_on<A: WriteArgs + ?Sized>(
    orb: &Orb,
    conn: &Arc<Connection>,
    ior: &Ior,
    ep: &Endpoint,
    kind: &Kind<'_, A>,
    deadline: Option<Instant>,
    disposition: AddressingDisposition,
    own_thread: bool,
) -> Result<Attempt, Closed> {
    let options = &orb.inner.options;
    let order = options.byte_order;
    let version = ep.version.min(options.max_giop_version);
    let target = target_address(ior, ep, version, disposition);

    // Code sets: negotiated from the IOR for GIOP 1.1 and later, fixed for
    // the connection by its first request, and announced in every request.
    let (code_sets, context) = match (&ep.code_sets, version >= Version::V1_1) {
        (Some(info), true) => {
            let proposed = match conn.code_sets() {
                Some(cs) => cs,
                None => match CodeSets::negotiate(info) {
                    Ok(cs) => cs,
                    Err(e) => return Ok(Attempt::Done(Err(e.into()))),
                },
            };
            let cs = conn.claim_code_sets(proposed);
            (cs, vec![cs.context().to_service_context(order)])
        }
        _ => (CodeSets::DEFAULT, Vec::new()),
    };

    let request_id = conn.next_request_id();
    let response = match kind {
        Kind::Request { response, .. } => *response,
        Kind::Locate => true,
    };
    let msg = match kind {
        Kind::Request {
            operation,
            args,
            response,
        } => {
            let header = request_header(request_id, *response, target, operation, context);
            match encode_request(version, order, &header, *args, code_sets) {
                Ok(msg) => msg,
                Err(e) => return Ok(Attempt::Done(Err(e))),
            }
        }
        Kind::Locate => match MessageEncoder::locate_request(
            version,
            order,
            &LocateRequestHeader { request_id, target },
        ) {
            Ok(msg) => msg.into(),
            Err(_) => {
                return Ok(Attempt::Done(Err(sys(
                    SystemExceptionKind::MARSHAL,
                    0,
                    CompletionStatus::No,
                )
                .into())));
            }
        },
    };

    let rx = if response {
        match conn.expect_reply(request_id) {
            Ok(rx) => Some(rx),
            Err(Closed::Orderly) => return Err(Closed::Orderly),
            // The connection broke before anything was sent: not processed.
            Err(Closed::Failed(_)) => {
                orb.discard(conn);
                return Ok(Attempt::NotSent(sys(
                    SystemExceptionKind::COMM_FAILURE,
                    0,
                    CompletionStatus::No,
                )));
            }
        }
    } else {
        None
    };
    match conn.send(&msg, deadline).await {
        Ok(()) => {}
        // Out of time before the whole request went out (a peer that
        // stopped reading, say): the server did not get it.
        Err(SendError::TimedOut) => {
            conn.forget(request_id);
            if !conn.is_usable() {
                orb.discard(conn);
            }
            return Ok(Attempt::Done(Err(sys(
                SystemExceptionKind::TIMEOUT,
                0,
                CompletionStatus::No,
            )
            .into())));
        }
        Err(SendError::Failed) => {
            conn.forget(request_id);
            orb.discard(conn);
            return Ok(Attempt::NotSent(sys(
                SystemExceptionKind::COMM_FAILURE,
                0,
                CompletionStatus::No,
            )));
        }
    }
    let Some(rx) = rx else {
        return Ok(Attempt::Done(Ok(Outcome::Reply(None))));
    };

    // A blocking call runs on the caller's own thread (`Orb::block_on`),
    // which may block reading the reply.
    let here = if own_thread {
        conn.read_reply_here(request_id, deadline)
    } else {
        None
    };
    let delivered = match here {
        Some(Some(message)) => Some(Ok(Ok(message))),
        Some(None) => None,
        None => match deadline {
            Some(at) => tokio::time::timeout_at(at.into(), rx).await.ok(),
            None => Some(rx.await),
        },
    };
    let delivered = match delivered {
        Some(delivered) => delivered,
        None => {
            conn.forget(request_id);
            if let Ok(cancel) =
                MessageEncoder::cancel_request(version, order, &CancelRequestHeader { request_id })
            {
                // The caller is out of time already, and the connection
                // may be the reason: a short wait at most.
                let at = Instant::now() + CANCEL_SEND_TIME;
                let _ = conn.send(&cancel.into(), Some(at)).await;
            }
            return Ok(Attempt::Done(Err(sys(
                SystemExceptionKind::TIMEOUT,
                0,
                CompletionStatus::Maybe,
            )
            .into())));
        }
    };
    let message = match delivered {
        Ok(Ok(message)) => message,
        Ok(Err(closed)) => return Err(closed),
        Err(_) => return Err(Closed::Failed("reply channel dropped".into())),
    };
    Ok(match message {
        Message::Reply { header, body } => {
            if !matches!(kind, Kind::Request { .. }) {
                return Err(Closed::Failed("Reply to a LocateRequest".into()));
            }
            reply_outcome(orb, header.reply_status, body, code_sets)
        }
        Message::LocateReply { header, body } => {
            if !matches!(kind, Kind::Locate) {
                return Err(Closed::Failed("LocateReply to a Request".into()));
            }
            locate_outcome(header.locate_status, &body)
        }
        _ => return Err(Closed::Failed("unexpected message".into())),
    })
}

/// How a request names its target, in the addressing disposition the
/// server asked for (GIOP 1.2), or by object key.
fn target_address(
    ior: &Ior,
    ep: &Endpoint,
    version: Version,
    disposition: AddressingDisposition,
) -> TargetAddress {
    if version < Version::V1_2 {
        return TargetAddress::Key(ep.object_key.clone());
    }
    match disposition {
        AddressingDisposition::Key => TargetAddress::Key(ep.object_key.clone()),
        AddressingDisposition::Profile => {
            TargetAddress::Profile(ior.profiles[ep.profile_index].clone())
        }
        AddressingDisposition::Reference => TargetAddress::Reference {
            selected_profile_index: ep.profile_index as u32,
            ior: ior.clone(),
        },
    }
}

fn request_header(
    request_id: u32,
    response: bool,
    target: TargetAddress,
    operation: &str,
    service_context: Vec<rustorb_giop::ServiceContext>,
) -> RequestHeader {
    RequestHeader {
        request_id,
        response_flags: if response {
            response_flags::SYNC_WITH_TARGET
        } else {
            response_flags::NONE
        },
        target,
        operation: operation.to_owned(),
        service_context,
        requesting_principal: Vec::new(),
    }
}

/// The whole `Request` message, with the arguments `args` writes.
fn encode_request<A: WriteArgs + ?Sized>(
    version: Version,
    order: rustorb_cdr::ByteOrder,
    header: &RequestHeader,
    args: Option<&A>,
    code_sets: CodeSets,
) -> Result<OutMessage, Exception> {
    let marshal = || sys(SystemExceptionKind::MARSHAL, 0, CompletionStatus::No).into();
    let mut m = MessageEncoder::request(version, order, header).map_err(|_| marshal())?;
    if let Some(args) = args {
        let mut out = OutStream {
            enc: m.body(),
            code_sets,
            orb: None,
        };
        args.write_args(&mut out)?;
    }
    m.finish_shared().map_err(|_| marshal())
}

/// A request to an object of the ORB's own server side, without the
/// network: marshalled as for the server, which runs it on the calling
/// thread (or queues it, as its thread policy says) and answers with a
/// whole reply message.
enum LocalCall {
    Locate(TargetAddress),
    Request {
        version: Version,
        header: RequestHeader,
        body: Body,
        code_sets: CodeSets,
        timeout: Option<Duration>,
    },
}

/// The answer of the server side to a [`LocalCall`].
enum LocalAnswer {
    Located(bool),
    /// The reply message (none for a oneway request).
    Reply(Option<Vec<u8>>, CodeSets),
}

/// The [`LocalCall`] for `kind`, or the end of the attempt.
fn local_call<A: WriteArgs + ?Sized>(
    orb: &Orb,
    ior: &Ior,
    ep: &Endpoint,
    kind: &Kind<'_, A>,
    deadline: Option<Instant>,
    disposition: AddressingDisposition,
) -> Result<LocalCall, Attempt> {
    let options = &orb.inner.options;
    let order = options.byte_order;
    let version = ep.version.min(options.max_giop_version);
    let target = target_address(ior, ep, version, disposition);
    let (code_sets, context) = match (&ep.code_sets, version >= Version::V1_1) {
        (Some(info), true) => match CodeSets::negotiate(info) {
            Ok(cs) => (cs, vec![cs.context().to_service_context(order)]),
            Err(e) => return Err(Attempt::Done(Err(e.into()))),
        },
        _ => (CodeSets::DEFAULT, Vec::new()),
    };
    let (operation, args, response) = match kind {
        Kind::Locate => return Ok(LocalCall::Locate(target)),
        Kind::Request {
            operation,
            args,
            response,
        } => (*operation, *args, *response),
    };
    let header = request_header(0, response, target, operation, context);
    let msg = match encode_request(version, order, &header, args, code_sets) {
        Ok(msg) => msg.into_vec(),
        Err(e) => return Err(Attempt::Done(Err(e))),
    };
    let Ok((_, Message::Request { header, body })) = decode_message(&msg) else {
        return Err(Attempt::Done(Err(local_internal())));
    };
    let timeout = deadline.map(|at| at.saturating_duration_since(Instant::now()));
    Ok(LocalCall::Request {
        version,
        header,
        body,
        code_sets,
        timeout,
    })
}

impl LocalCall {
    /// Runs the call on the calling thread.
    fn run(self, server: &Arc<Server>) -> Result<LocalAnswer, SystemException> {
        match self {
            LocalCall::Locate(target) => server.locate_local(&target).map(LocalAnswer::Located),
            LocalCall::Request {
                version,
                header,
                body,
                code_sets,
                timeout,
            } => server
                .call_local(version, header, body, code_sets, timeout)
                .map(|reply| LocalAnswer::Reply(reply, code_sets)),
        }
    }
}

fn local_outcome(orb: &Orb, answer: Result<LocalAnswer, SystemException>) -> Attempt {
    match answer {
        Ok(LocalAnswer::Located(here)) => Attempt::Done(Ok(Outcome::Located(here))),
        Ok(LocalAnswer::Reply(None, _)) => Attempt::Done(Ok(Outcome::Reply(None))),
        Ok(LocalAnswer::Reply(Some(reply), code_sets)) => match decode_message(&reply) {
            Ok((_, Message::Reply { header, body })) => {
                reply_outcome(orb, header.reply_status, body, code_sets)
            }
            _ => Attempt::Done(Err(local_internal())),
        },
        Err(e) => Attempt::Done(Err(e.into())),
    }
}

fn local_internal() -> Exception {
    sys(SystemExceptionKind::INTERNAL, 0, CompletionStatus::Maybe).into()
}

fn reply_outcome(orb: &Orb, status: ReplyStatus, body: Body, code_sets: CodeSets) -> Attempt {
    match status {
        ReplyStatus::NoException => Attempt::Done(Ok(Outcome::Reply(Some(Reply {
            body,
            code_sets,
            orb: orb.clone(),
        })))),
        ReplyStatus::UserException => {
            let reply = Reply {
                body,
                code_sets,
                orb: orb.clone(),
            };
            let id = reply.input().read_string();
            Attempt::Done(Err(match id {
                Ok(repository_id) => Exception::User(UserException {
                    repository_id,
                    reply,
                }),
                Err(e) => Exception::System(e),
            }))
        }
        ReplyStatus::SystemException => {
            let mut dec = body.decoder();
            Attempt::Done(Err(match SystemExceptionBody::decode(&mut dec) {
                Ok(e) => Exception::System(e.into()),
                Err(e) => Exception::System(marshal(e)),
            }))
        }
        ReplyStatus::LocationForward | ReplyStatus::LocationForwardPerm => forward(&body),
        ReplyStatus::NeedsAddressingMode => addressing(&body),
    }
}

fn locate_outcome(status: LocateStatus, body: &Body) -> Attempt {
    match status {
        LocateStatus::ObjectHere => Attempt::Done(Ok(Outcome::Located(true))),
        LocateStatus::UnknownObject => Attempt::Done(Ok(Outcome::Located(false))),
        LocateStatus::ObjectForward | LocateStatus::ObjectForwardPerm => forward(body),
        LocateStatus::LocSystemException => {
            let mut dec = body.decoder();
            Attempt::Done(Err(match SystemExceptionBody::decode(&mut dec) {
                Ok(e) => Exception::System(e.into()),
                Err(e) => Exception::System(marshal(e)),
            }))
        }
        LocateStatus::LocNeedsAddressingMode => addressing(body),
    }
}

fn forward(body: &Body) -> Attempt {
    let mut dec = body.decoder();
    match Ior::decode(&mut dec) {
        Ok(ior) if !ior.is_nil() => Attempt::Forward(ior),
        Ok(_) => Attempt::Done(Err(sys(
            SystemExceptionKind::INV_OBJREF,
            0,
            CompletionStatus::No,
        )
        .into())),
        Err(e) => Attempt::Done(Err(Exception::System(marshal(e)))),
    }
}

fn addressing(body: &Body) -> Attempt {
    let mut dec = body.decoder();
    match dec
        .read_i16()
        .ok()
        .and_then(|d| AddressingDisposition::from_i16(d).ok())
    {
        Some(d) => Attempt::Addressing(d),
        None => Attempt::Done(Err(sys(
            SystemExceptionKind::MARSHAL,
            0,
            CompletionStatus::No,
        )
        .into())),
    }
}

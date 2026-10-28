use std::fmt;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rustorb_cdr::CdrDecode;
use rustorb_giop::{
    AddressingDisposition, AlternateIiopAddress, Body, CancelRequestHeader, CodeSetComponentInfo,
    Ior, LocateRequestHeader, LocateStatus, Message, MessageEncoder, ReplyStatus, RequestHeader,
    SystemExceptionBody, TAG_INTERNET_IOP, TAG_MULTIPLE_COMPONENTS, TargetAddress, Version,
    component_tag, decode_message, response_flags,
};

use crate::connection::{Closed, Connection};
use crate::server::Server;
use crate::stream::{InStream, OutStream, marshal};
use crate::{
    CodeSets, CompletionStatus, Exception, Orb, SystemException, SystemExceptionKind,
    UserException, minor,
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
}

impl fmt::Debug for ObjectRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ObjectRef")
            .field("type_id", &self.ior.type_id)
            .field("profiles", &self.ior.profiles.len())
            .finish_non_exhaustive()
    }
}

type ArgsFn<'a> = dyn Fn(&mut OutStream<'_>) -> Result<(), SystemException> + 'a;

impl ObjectRef {
    pub(crate) fn new(orb: Orb, ior: Ior) -> ObjectRef {
        ObjectRef {
            orb,
            ior: Arc::new(ior),
            forward: Arc::new(Mutex::new(None)),
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
            timeout: self.orb.inner.options.request_timeout,
        }
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

    /// Sends a `LocateRequest`, following forwards. Returns whether the
    /// server has the object (`OBJECT_HERE`) or not (`UNKNOWN_OBJECT`).
    pub fn locate(&self) -> Result<bool, Exception> {
        let timeout = self.orb.inner.options.request_timeout;
        let outcome = invoke(self, Kind::Locate, timeout)?;
        match outcome {
            Outcome::Located(here) => Ok(here),
            Outcome::Reply(_) => unreachable!("a locate request gives a locate outcome"),
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
    timeout: Option<Duration>,
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

    /// Overrides [`OrbOptions::request_timeout`](crate::OrbOptions::request_timeout)
    /// for this request.
    pub fn timeout(mut self, timeout: Option<Duration>) -> Request<'a> {
        self.timeout = timeout;
        self
    }

    /// Sends the request and waits for the reply.
    pub fn invoke(self) -> Result<Reply, Exception> {
        let kind = Kind::Request {
            operation: &self.operation,
            args: self.args.as_deref(),
            response: true,
        };
        match invoke(self.obj, kind, self.timeout)? {
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
        match invoke(self.obj, kind, self.timeout) {
            Ok(_) => Ok(()),
            Err(Exception::System(e)) => Err(e),
            Err(Exception::User(_)) => unreachable!("a oneway request has no reply"),
        }
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
        }
    }

    /// The raw reply body.
    pub fn body(&self) -> &Body {
        &self.body
    }
}

enum Kind<'k> {
    Request {
        operation: &'k str,
        args: Option<&'k ArgsFn<'k>>,
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
    object_key: Vec<u8>,
    code_sets: Option<CodeSetComponentInfo>,
}

/// Every IIOP address in `ior`, in order: each profile's own address and
/// then its alternate addresses.
fn endpoints(ior: &Ior) -> Vec<Endpoint> {
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
        let mut addrs = vec![(iiop.host.clone(), iiop.port)];
        addrs.extend(
            components
                .iter()
                .filter(|c| c.tag == component_tag::ALTERNATE_IIOP_ADDRESS)
                .filter_map(|c| AlternateIiopAddress::from_component(c).ok())
                .map(|a| (a.host, a.port)),
        );
        for (host, port) in addrs {
            out.push(Endpoint {
                profile_index: index,
                version: iiop.version,
                host,
                port,
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

fn invoke(
    obj: &ObjectRef,
    kind: Kind<'_>,
    timeout: Option<Duration>,
) -> Result<Outcome, Exception> {
    let orb = &obj.orb;
    orb.check_running()?;
    let max_hops = orb.inner.options.max_forwards;
    let mut hops = 0;
    let mut ior = obj.current_target();
    let mut disposition = AddressingDisposition::Key;
    'target: loop {
        if ior.is_nil() {
            return Err(sys(SystemExceptionKind::INV_OBJREF, 0, CompletionStatus::No).into());
        }
        let mut last = sys(
            SystemExceptionKind::TRANSIENT,
            minor::NO_USABLE_PROFILE,
            CompletionStatus::No,
        );
        for ep in endpoints(&ior) {
            let result = match orb.local_server(&ep.host, ep.port) {
                Some(server) => attempt_local(orb, &server, &ior, &ep, &kind, timeout, disposition),
                None => orb.block_on(attempt(orb, &ior, &ep, &kind, timeout, disposition))?,
            };
            match result {
                Attempt::Done(result) => return result,
                Attempt::NotSent(e) => last = e,
                Attempt::Forward(next) => {
                    hops += 1;
                    if hops > max_hops {
                        return Err(
                            sys(SystemExceptionKind::TRANSIENT, 0, CompletionStatus::No).into()
                        );
                    }
                    let next = Arc::new(next);
                    obj.set_forward(Some(Arc::clone(&next)));
                    ior = next;
                    disposition = AddressingDisposition::Key;
                    continue 'target;
                }
                Attempt::Addressing(wanted) => {
                    hops += 1;
                    if hops > max_hops || wanted == disposition {
                        return Err(
                            sys(SystemExceptionKind::MARSHAL, 0, CompletionStatus::No).into()
                        );
                    }
                    disposition = wanted;
                    continue 'target;
                }
            }
        }
        // Nothing could be reached. A forwarded target falls back to the
        // original reference.
        if !Arc::ptr_eq(&ior, &obj.ior) {
            hops += 1;
            if hops <= max_hops {
                obj.set_forward(None);
                ior = Arc::clone(&obj.ior);
                disposition = AddressingDisposition::Key;
                continue 'target;
            }
        }
        return Err(last.into());
    }
}

/// Sends the request to one endpoint. A `CloseConnection` before the reply
/// means the server did not process it, so it is sent once more on a new
/// connection.
async fn attempt(
    orb: &Orb,
    ior: &Ior,
    ep: &Endpoint,
    kind: &Kind<'_>,
    timeout: Option<Duration>,
    disposition: AddressingDisposition,
) -> Attempt {
    let mut result = Attempt::NotSent(sys(SystemExceptionKind::TRANSIENT, 0, CompletionStatus::No));
    for _ in 0..2 {
        let conn = match orb.connection(&ep.host, ep.port).await {
            Ok(conn) => conn,
            Err(e) => return Attempt::NotSent(e),
        };
        match attempt_on(orb, &conn, ior, ep, kind, timeout, disposition).await {
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
async fn attempt_on(
    orb: &Orb,
    conn: &Arc<Connection>,
    ior: &Ior,
    ep: &Endpoint,
    kind: &Kind<'_>,
    timeout: Option<Duration>,
    disposition: AddressingDisposition,
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
            Ok(msg) => msg,
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
    if conn.send(&msg).await.is_err() {
        conn.forget(request_id);
        orb.discard(conn);
        return Ok(Attempt::NotSent(sys(
            SystemExceptionKind::COMM_FAILURE,
            0,
            CompletionStatus::No,
        )));
    }
    let Some(rx) = rx else {
        return Ok(Attempt::Done(Ok(Outcome::Reply(None))));
    };

    let delivered = match timeout {
        Some(limit) => match tokio::time::timeout(limit, rx).await {
            Ok(delivered) => delivered,
            Err(_) => {
                conn.forget(request_id);
                if let Ok(cancel) = MessageEncoder::cancel_request(
                    version,
                    order,
                    &CancelRequestHeader { request_id },
                ) {
                    let _ = conn.send(&cancel).await;
                }
                return Ok(Attempt::Done(Err(sys(
                    SystemExceptionKind::TIMEOUT,
                    0,
                    CompletionStatus::Maybe,
                )
                .into())));
            }
        },
        None => rx.await,
    };
    let whole = match delivered {
        Ok(Ok(whole)) => whole,
        Ok(Err(closed)) => return Err(closed),
        Err(_) => return Err(Closed::Failed("reply channel dropped".into())),
    };
    let Ok((_, message)) = whole.decode() else {
        return Err(Closed::Failed("undecodable reply".into()));
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
fn encode_request(
    version: Version,
    order: rustorb_cdr::ByteOrder,
    header: &RequestHeader,
    args: Option<&ArgsFn<'_>>,
    code_sets: CodeSets,
) -> Result<Vec<u8>, Exception> {
    let marshal = || sys(SystemExceptionKind::MARSHAL, 0, CompletionStatus::No).into();
    let mut m = MessageEncoder::request(version, order, header).map_err(|_| marshal())?;
    if let Some(args) = args {
        let mut out = OutStream {
            enc: m.body(),
            code_sets,
        };
        args(&mut out)?;
    }
    m.finish().map_err(|_| marshal())
}

/// Makes a request to an object of the ORB's own server side, without the
/// network: the request is marshalled as for the server, which runs it on
/// this thread (or queues it, as its thread policy says) and answers with
/// a whole reply message.
fn attempt_local(
    orb: &Orb,
    server: &Arc<Server>,
    ior: &Ior,
    ep: &Endpoint,
    kind: &Kind<'_>,
    timeout: Option<Duration>,
    disposition: AddressingDisposition,
) -> Attempt {
    let options = &orb.inner.options;
    let order = options.byte_order;
    let version = ep.version.min(options.max_giop_version);
    let target = target_address(ior, ep, version, disposition);
    let (code_sets, context) = match (&ep.code_sets, version >= Version::V1_1) {
        (Some(info), true) => match CodeSets::negotiate(info) {
            Ok(cs) => (cs, vec![cs.context().to_service_context(order)]),
            Err(e) => return Attempt::Done(Err(e.into())),
        },
        _ => (CodeSets::DEFAULT, Vec::new()),
    };
    let (operation, args, response) = match kind {
        Kind::Locate => {
            return Attempt::Done(
                server
                    .locate_local(&target)
                    .map(Outcome::Located)
                    .map_err(Exception::from),
            );
        }
        Kind::Request {
            operation,
            args,
            response,
        } => (*operation, *args, *response),
    };
    let header = request_header(0, response, target, operation, context);
    let msg = match encode_request(version, order, &header, args, code_sets) {
        Ok(msg) => msg,
        Err(e) => return Attempt::Done(Err(e)),
    };
    let internal = || sys(SystemExceptionKind::INTERNAL, 0, CompletionStatus::Maybe).into();
    let Ok((_, Message::Request { header, body })) = decode_message(&msg) else {
        return Attempt::Done(Err(internal()));
    };
    match server.call_local(version, header, body, code_sets, timeout) {
        Ok(None) => Attempt::Done(Ok(Outcome::Reply(None))),
        Ok(Some(reply)) => match decode_message(&reply) {
            Ok((_, Message::Reply { header, body })) => {
                reply_outcome(orb, header.reply_status, body, code_sets)
            }
            _ => Attempt::Done(Err(internal())),
        },
        Err(e) => Attempt::Done(Err(e.into())),
    }
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

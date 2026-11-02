//! The Dynamic Invocation Interface (DII) and the Dynamic Skeleton
//! Interface (DSI): calls and servants whose operations are described at
//! run time, by TypeCodes, instead of by generated code.
//!
//! - [`DiiRequest`] (`CORBA::Request`): a call built from [`NamedValue`]s,
//!   made by [`ObjectRef::create_request`]. It can be invoked, sent
//!   oneway, or sent deferred and its response collected later.
//! - [`DynamicImplementation`] (`PortableServer::DynamicImplementation`):
//!   a servant that gets each request as a [`ServerRequest`], reads the
//!   arguments with the TypeCodes it gives, and sets the result or a user
//!   exception. [`DynamicServant`] makes it a [`Servant`].
//!
//! Contexts (`Context`, `ctx`) are not supported. The values are
//! [`Any`]s, so an argument's type is the TypeCode of its Any.

use std::fmt;
use std::sync::mpsc;
use std::time::Duration;

use crate::{
    Any, CompletionStatus, Exception, InStream, ObjectRef, Orb, Reply, Servant, ServerReply,
    SystemException, SystemExceptionKind, TCKind, TypeCode, UserException, Value,
};

/// The direction of an argument (`CORBA::ARG_IN`, `ARG_OUT`, `ARG_INOUT`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArgMode {
    In,
    Out,
    InOut,
}

impl ArgMode {
    /// The `CORBA::Flags` value: 1, 2 or 3.
    pub fn flags(self) -> u32 {
        match self {
            ArgMode::In => 1,
            ArgMode::Out => 2,
            ArgMode::InOut => 3,
        }
    }

    /// The mode of a `CORBA::Flags` value (other flags ignored).
    pub fn from_flags(flags: u32) -> Option<ArgMode> {
        match flags & 3 {
            1 => Some(ArgMode::In),
            2 => Some(ArgMode::Out),
            3 => Some(ArgMode::InOut),
            _ => None,
        }
    }

    pub(crate) fn is_in(self) -> bool {
        self != ArgMode::Out
    }

    pub(crate) fn is_out(self) -> bool {
        self != ArgMode::In
    }
}

/// An argument (`CORBA::NamedValue`): a name, a value and a mode. The
/// value of an `out` argument is only its type until the value arrives:
/// an Any of that TypeCode holding [`Value::Void`].
#[derive(Debug, Clone, PartialEq)]
pub struct NamedValue {
    pub name: String,
    pub value: Any,
    pub mode: ArgMode,
}

impl NamedValue {
    pub fn new(name: impl Into<String>, value: Any, mode: ArgMode) -> NamedValue {
        NamedValue {
            name: name.into(),
            value,
            mode,
        }
    }

    /// An `out` argument of type `tc`, still without a value.
    pub fn out(name: impl Into<String>, tc: TypeCode) -> NamedValue {
        NamedValue::new(name, Any::new(tc, Value::Void), ArgMode::Out)
    }
}

fn sys(kind: SystemExceptionKind, completed: CompletionStatus) -> SystemException {
    SystemException::new(kind, 0, completed)
}

fn bad_inv_order() -> SystemException {
    sys(SystemExceptionKind::BAD_INV_ORDER, CompletionStatus::No)
}

// --- the client side ---------------------------------------------------------

/// The result of a call sent deferred.
type Pending = mpsc::Receiver<Result<Reply, Exception>>;

/// `CORBA::Request`: a call whose operation, arguments, result type and
/// user exceptions are given at run time.
///
/// After a call the `out` and `inout` arguments hold the values the server
/// sent and [`result`](DiiRequest::result) the return value. A user
/// exception comes back as [`Exception::User`];
/// [`exception_value`](DiiRequest::exception_value) decodes it with the
/// TypeCodes given by [`add_exception`](DiiRequest::add_exception).
pub struct DiiRequest {
    target: ObjectRef,
    operation: String,
    arguments: Vec<NamedValue>,
    result: Any,
    exceptions: Vec<TypeCode>,
    timeout: Option<Option<Duration>>,
    pending: Option<Pending>,
}

impl fmt::Debug for DiiRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DiiRequest")
            .field("operation", &self.operation)
            .field("arguments", &self.arguments)
            .field("result", &self.result)
            .field("pending", &self.pending.is_some())
            .finish_non_exhaustive()
    }
}

impl ObjectRef {
    /// `CORBA::Object::_request`: a [`DiiRequest`] for `operation`, with
    /// no arguments and a `void` result.
    pub fn create_request(&self, operation: &str) -> DiiRequest {
        DiiRequest {
            target: self.clone(),
            operation: operation.to_owned(),
            arguments: Vec::new(),
            result: Any::new(TypeCode::void(), Value::Void),
            exceptions: Vec::new(),
            timeout: None,
            pending: None,
        }
    }
}

impl DiiRequest {
    pub fn target(&self) -> &ObjectRef {
        &self.target
    }

    pub fn operation(&self) -> &str {
        &self.operation
    }

    /// The arguments (`CORBA::NVList`), in order.
    pub fn arguments(&self) -> &[NamedValue] {
        &self.arguments
    }

    /// The arguments, to change or add to before a call.
    pub fn arguments_mut(&mut self) -> &mut Vec<NamedValue> {
        &mut self.arguments
    }

    pub fn add_in_arg(&mut self, name: &str, value: Any) -> &mut DiiRequest {
        self.arguments
            .push(NamedValue::new(name, value, ArgMode::In));
        self
    }

    pub fn add_inout_arg(&mut self, name: &str, value: Any) -> &mut DiiRequest {
        self.arguments
            .push(NamedValue::new(name, value, ArgMode::InOut));
        self
    }

    pub fn add_out_arg(&mut self, name: &str, tc: TypeCode) -> &mut DiiRequest {
        self.arguments.push(NamedValue::out(name, tc));
        self
    }

    /// The type of the return value (`void` until set).
    pub fn set_return_type(&mut self, tc: TypeCode) -> &mut DiiRequest {
        self.result = Any::new(tc, Value::Void);
        self
    }

    /// The return value after a call (its type before).
    pub fn result(&self) -> &Any {
        &self.result
    }

    /// Adds a user exception the operation may raise (`CORBA::ExceptionList`).
    pub fn add_exception(&mut self, tc: TypeCode) -> &mut DiiRequest {
        self.exceptions.push(tc);
        self
    }

    pub fn exceptions(&self) -> &[TypeCode] {
        &self.exceptions
    }

    /// [`Request::timeout`](crate::Request::timeout) for the calls of this
    /// request.
    pub fn set_timeout(&mut self, timeout: Option<Duration>) -> &mut DiiRequest {
        self.timeout = Some(timeout);
        self
    }

    /// The `in` and `inout` values, as the arguments function writes them.
    fn in_values(&self) -> Vec<(TypeCode, Value)> {
        self.arguments
            .iter()
            .filter(|nv| nv.mode.is_in())
            .map(|nv| (nv.value.typecode().clone(), nv.value.value().clone()))
            .collect()
    }

    /// What the client interceptors see of the request, if the ORB has
    /// any.
    fn pi_params(&self) -> Option<crate::pi::DiiParams> {
        let pi = self.target.orb().interceptors()?;
        pi.has_client().then(|| crate::pi::DiiParams {
            arguments: self.arguments.clone(),
            result: self.result.typecode().clone(),
            exceptions: self.exceptions.clone(),
        })
    }

    fn check_idle(&self) -> Result<(), SystemException> {
        match self.pending {
            Some(_) => Err(bad_inv_order()),
            None => Ok(()),
        }
    }

    /// Sends the request and waits for the reply.
    pub fn invoke(&mut self) -> Result<(), Exception> {
        self.check_idle()?;
        let values = self.in_values();
        let mut request = self
            .target
            .request(&self.operation)
            .args(move |out| write_values(out, &values));
        if let Some(t) = self.timeout {
            request = request.timeout(t);
        }
        if let Some(params) = self.pi_params() {
            request = request.dii(params);
        }
        let reply = request.invoke()?;
        self.take_reply(&reply)
    }

    /// [`invoke`](DiiRequest::invoke) as a future (see
    /// [`AsyncRequest`](crate::AsyncRequest)).
    pub async fn invoke_async(&mut self) -> Result<(), Exception> {
        self.check_idle()?;
        let values = self.in_values();
        let mut request = self
            .target
            .request_async(&self.operation)
            .args(move |out| write_values(out, &values));
        if let Some(t) = self.timeout {
            request = request.timeout(t);
        }
        if let Some(params) = self.pi_params() {
            request = request.dii(params);
        }
        let reply = request.invoke().await?;
        self.take_reply(&reply)
    }

    /// Sends the request as a oneway call.
    pub fn send_oneway(&self) -> Result<(), SystemException> {
        self.check_idle()?;
        let values = self.in_values();
        let mut request = self
            .target
            .request(&self.operation)
            .args(move |out| write_values(out, &values));
        if let Some(t) = self.timeout {
            request = request.timeout(t);
        }
        if let Some(params) = self.pi_params() {
            request = request.dii(params);
        }
        request.send_oneway()
    }

    /// Sends the request without waiting: the reply is collected by
    /// [`poll_response`](DiiRequest::poll_response) and
    /// [`get_response`](DiiRequest::get_response). The arguments are taken
    /// now. A request still waiting for its response cannot be sent again
    /// (`BAD_INV_ORDER`).
    pub fn send_deferred(&mut self) -> Result<(), SystemException> {
        self.check_idle()?;
        let orb: Orb = self.target.orb().clone();
        orb.check_running()?;
        let values = self.in_values();
        let target = self.target.clone();
        let operation = self.operation.clone();
        let timeout = self.timeout;
        let (tx, rx) = mpsc::channel();
        // The thread's timeout is that of the thread that sends.
        let timeout = timeout.unwrap_or_else(|| target.effective_timeout());
        let params = self.pi_params();
        // The request scope slots are those of the thread that sends.
        let slots = target.request_slots();
        orb.runtime_handle().spawn(async move {
            let mut request = target
                .request_async(&operation)
                .args(move |out| write_values(out, &values))
                .timeout(timeout)
                .slots(slots);
            if let Some(params) = params {
                request = request.dii(params);
            }
            let result = request.invoke().await;
            let _ = tx.send(result);
        });
        self.pending = Some(rx);
        Ok(())
    }

    /// Whether the response of [`send_deferred`](DiiRequest::send_deferred)
    /// has arrived. `BAD_INV_ORDER` when nothing was sent deferred.
    pub fn poll_response(&mut self) -> Result<bool, SystemException> {
        let Some(rx) = &self.pending else {
            return Err(bad_inv_order());
        };
        match rx.try_recv() {
            Ok(result) => {
                // Kept for get_response: put back as a ready channel.
                let (tx, ready) = mpsc::channel();
                let _ = tx.send(result);
                self.pending = Some(ready);
                Ok(true)
            }
            Err(mpsc::TryRecvError::Empty) => Ok(false),
            Err(mpsc::TryRecvError::Disconnected) => Ok(true),
        }
    }

    /// Waits for the response of [`send_deferred`](DiiRequest::send_deferred)
    /// and takes it like [`invoke`](DiiRequest::invoke). `BAD_INV_ORDER`
    /// when nothing was sent deferred.
    pub fn get_response(&mut self) -> Result<(), Exception> {
        let Some(rx) = self.pending.take() else {
            return Err(bad_inv_order().into());
        };
        // A dropped sender: the runtime went away with the call.
        let result = rx.recv().unwrap_or_else(|_| {
            Err(sys(SystemExceptionKind::COMM_FAILURE, CompletionStatus::Maybe).into())
        });
        self.take_reply(&result?)
    }

    /// Reads the return value and the `out` and `inout` arguments.
    fn take_reply(&mut self, reply: &Reply) -> Result<(), Exception> {
        let mut input = reply.input();
        let tc = self.result.typecode().clone();
        let value = input.read_value(&tc)?;
        self.result = Any::new(tc, value);
        for nv in self.arguments.iter_mut().filter(|nv| nv.mode.is_out()) {
            let tc = nv.value.typecode().clone();
            let value = input.read_value(&tc)?;
            nv.value = Any::new(tc, value);
        }
        Ok(())
    }

    /// The user exception `e` of a call of this request as an Any, with the
    /// TypeCode of [`exceptions`](DiiRequest::exceptions) of the same
    /// repository id. An exception that is not listed is `UNKNOWN`,
    /// completed yes.
    pub fn exception_value(&self, e: &UserException) -> Result<Any, SystemException> {
        exception_any(&self.exceptions, e)
    }
}

/// The user exception `e` as an Any, with the TypeCode of `exceptions`
/// of the same repository id (`UNKNOWN` if none).
pub(crate) fn exception_any(
    exceptions: &[TypeCode],
    e: &UserException,
) -> Result<Any, SystemException> {
    let tc = exceptions
        .iter()
        .find(|tc| tc.unaliased().id().ok() == Some(e.repository_id()))
        .ok_or_else(|| sys(SystemExceptionKind::UNKNOWN, CompletionStatus::Yes))?;
    let mut input = e.reply.input();
    let value = input.read_value(tc)?;
    Ok(Any::new(tc.clone(), value))
}

fn write_values(
    out: &mut crate::OutStream<'_>,
    values: &[(TypeCode, Value)],
) -> Result<(), SystemException> {
    for (tc, v) in values {
        out.write_value(tc, v)?;
    }
    Ok(())
}

// --- the server side ---------------------------------------------------------

/// `PortableServer::DynamicImplementation`: a servant that handles every
/// operation itself, from a [`ServerRequest`]. Wrap it in
/// [`DynamicServant`] to activate it.
pub trait DynamicImplementation: Send + Sync {
    /// `_primary_interface`: the repository id of the most derived
    /// interface the servant implements, the type id of its references.
    fn primary_interface(&self) -> &str;

    /// Whether the servant implements `repository_id`; the default only
    /// knows the primary interface.
    fn is_a(&self, repository_id: &str) -> bool {
        repository_id == self.primary_interface()
    }

    /// Runs `request`: calls [`ServerRequest::arguments`] once with the
    /// types and modes of the parameters, and then
    /// [`set_result`](ServerRequest::set_result) or
    /// [`set_exception`](ServerRequest::set_exception), or returns a system
    /// exception. An unknown operation is `BAD_OPERATION` with minor
    /// [`UNKNOWN_OPERATION`](crate::minor::UNKNOWN_OPERATION).
    fn invoke(&self, request: &mut ServerRequest) -> Result<(), SystemException>;
}

/// A [`DynamicImplementation`] as a [`Servant`].
#[derive(Debug)]
pub struct DynamicServant<T>(pub T);

impl<T: DynamicImplementation> Servant for DynamicServant<T> {
    fn repository_id(&self) -> &str {
        self.0.primary_interface()
    }

    fn is_a(&self, repository_id: &str) -> bool {
        self.0.is_a(repository_id)
    }

    fn dispatch(
        &self,
        operation: &str,
        input: &mut InStream<'_>,
        reply: &mut ServerReply,
    ) -> Result<(), SystemException> {
        let mut request = ServerRequest::new(operation, input)?;
        self.0.invoke(&mut request)?;
        if crate::pi::wants_dsi_params() {
            crate::pi::leave_dsi_params(crate::pi::DsiParams {
                arguments: request.arguments.clone(),
                result: request.result.clone(),
                exception: request.exception.clone(),
            });
        }
        request.write_reply(reply)
    }
}

/// `CORBA::ServerRequest`: a request to a [`DynamicImplementation`].
///
/// It owns what it needs of the request, so a binding can keep it while it
/// calls into its language.
pub struct ServerRequest {
    operation: String,
    input: Option<crate::stream::DetachedInput>,
    arguments: Option<Vec<NamedValue>>,
    result: Option<Any>,
    exception: Option<Any>,
}

impl fmt::Debug for ServerRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ServerRequest")
            .field("operation", &self.operation)
            .field("arguments", &self.arguments)
            .field("result", &self.result)
            .field("exception", &self.exception)
            .finish_non_exhaustive()
    }
}

impl ServerRequest {
    /// The request for `operation` whose arguments `input` (the stream a
    /// servant's [`dispatch`](Servant::dispatch) gets, nothing read from it
    /// yet) holds.
    pub fn new(operation: &str, input: &InStream<'_>) -> Result<ServerRequest, SystemException> {
        let input = input
            .detach()
            .ok_or_else(|| sys(SystemExceptionKind::INTERNAL, CompletionStatus::No))?;
        Ok(ServerRequest {
            operation: operation.to_owned(),
            input: Some(input),
            arguments: None,
            result: None,
            exception: None,
        })
    }

    pub fn operation(&self) -> &str {
        &self.operation
    }

    /// Reads the `in` and `inout` arguments into `parameters` (whose
    /// TypeCodes and modes describe the operation's parameters, in order)
    /// and keeps them; the servant then sets the `out` and `inout` values
    /// in the returned slice (or later in
    /// [`parameters_mut`](ServerRequest::parameters_mut)). Called once:
    /// again is `BAD_INV_ORDER`. Arguments that do not match the types are
    /// `MARSHAL`, completed no.
    pub fn arguments(
        &mut self,
        mut parameters: Vec<NamedValue>,
    ) -> Result<&mut [NamedValue], SystemException> {
        let Some(input) = self.input.take() else {
            return Err(bad_inv_order());
        };
        {
            let mut stream = input.stream();
            for nv in parameters.iter_mut().filter(|nv| nv.mode.is_in()) {
                let tc = nv.value.typecode().clone();
                let value = stream.read_value(&tc)?;
                nv.value = Any::new(tc, value);
            }
        }
        Ok(self.arguments.insert(parameters))
    }

    /// The parameters given to [`arguments`](ServerRequest::arguments).
    pub fn parameters_mut(&mut self) -> Option<&mut [NamedValue]> {
        self.arguments.as_deref_mut()
    }

    /// Sets the return value (for an operation with a `void` result, an
    /// Any of type `void`, or none at all). `BAD_INV_ORDER` before
    /// [`arguments`](ServerRequest::arguments) or after
    /// [`set_exception`](ServerRequest::set_exception).
    pub fn set_result(&mut self, value: Any) -> Result<(), SystemException> {
        if self.arguments.is_none() || self.exception.is_some() {
            return Err(bad_inv_order());
        }
        self.result = Some(value);
        Ok(())
    }

    /// Makes the reply the user exception `value` (an Any of a `tk_except`
    /// TypeCode; another is `BAD_PARAM`). A system exception is returned
    /// from [`DynamicImplementation::invoke`] instead.
    pub fn set_exception(&mut self, value: Any) -> Result<(), SystemException> {
        if value.typecode().unaliased().kind() != TCKind::Except {
            return Err(sys(SystemExceptionKind::BAD_PARAM, CompletionStatus::No));
        }
        self.exception = Some(value);
        Ok(())
    }

    /// Writes the reply: the user exception, or the return value and the
    /// `out` and `inout` arguments in order. A value that does not fit its
    /// type (an `out` argument left without a value, say) is `BAD_PARAM`.
    pub fn write_reply(self, reply: &mut ServerReply) -> Result<(), SystemException> {
        if let Some(exception) = &self.exception {
            let tc = exception.typecode().unaliased();
            let id = tc.id().unwrap_or_default().to_owned();
            let Value::List(members) = exception.value() else {
                return Err(sys(SystemExceptionKind::BAD_PARAM, CompletionStatus::Maybe));
            };
            let mut out = reply.user_exception(&id)?;
            for (i, member) in members.iter().enumerate() {
                let member_tc = tc
                    .member_type(i as u32)
                    .map_err(|_| sys(SystemExceptionKind::BAD_PARAM, CompletionStatus::Maybe))?;
                out.write_value(&member_tc, member)?;
            }
            return Ok(());
        }
        let mut out = reply.output();
        if let Some(result) = &self.result {
            out.write_value(result.typecode(), result.value())?;
        }
        for nv in self
            .arguments
            .iter()
            .flatten()
            .filter(|nv| nv.mode.is_out())
        {
            out.write_value(nv.value.typecode(), nv.value.value())?;
        }
        Ok(())
    }
}

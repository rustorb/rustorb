//! Servants, the replies they write, servant managers and adapter
//! activators.

use std::any::Any;
use std::fmt;
use std::sync::Arc;

use rustorb_cdr::{ByteOrder, CdrEncode};
use rustorb_giop::{
    Ior, MessageEncoder, OutMessage, ReplyHeader, ReplyStatus, SystemExceptionBody, Version,
};

use crate::stream::{InStream, OutStream};
use crate::{CodeSets, CompletionStatus, ObjectRef, Poa, SystemException, SystemExceptionKind};

/// The repository id every object is (`CORBA::Object`).
pub(crate) const OBJECT_ID: &str = "IDL:omg.org/CORBA/Object:1.0";

/// The implementation of a CORBA object, called by a POA.
///
/// Generated skeletons (and the adapters of the language bindings)
/// implement it. Servants are called on the ORB's upcall threads, possibly
/// on several at once, so they must be `Send + Sync`.
pub trait Servant: Send + Sync {
    /// Repository id of the most derived interface, used as the type id of
    /// the servant's references and for `_is_a`.
    fn repository_id(&self) -> &str;

    /// Whether the servant implements the interface `repository_id`. The
    /// default only knows [`repository_id`](Servant::repository_id);
    /// `IDL:omg.org/CORBA/Object:1.0` is answered by the ORB.
    fn is_a(&self, repository_id: &str) -> bool {
        repository_id == self.repository_id()
    }

    /// Runs `operation`: reads the `in` and `inout` arguments from `input`
    /// and writes the result to `reply`. An operation the servant does not
    /// know should return `BAD_OPERATION` with minor
    /// [`UNKNOWN_OPERATION`](crate::minor::UNKNOWN_OPERATION), and a user
    /// exception is written with [`ServerReply::user_exception`].
    ///
    /// `_is_a`, `_non_existent`, `_interface` and `_repository_id` are
    /// answered by the ORB and do not reach the servant.
    fn dispatch(
        &self,
        operation: &str,
        input: &mut InStream<'_>,
        reply: &mut ServerReply,
    ) -> Result<(), SystemException>;
}

/// What a servant manager raises instead of providing a servant.
#[derive(Debug, Clone)]
pub enum ServantManagerError {
    /// `PortableServer::ForwardRequest`: the client is sent to this object
    /// with a `LOCATION_FORWARD` reply.
    ForwardRequest(ObjectRef),
    /// A system exception, returned to the client as it is.
    System(SystemException),
}

impl From<SystemException> for ServantManagerError {
    fn from(e: SystemException) -> Self {
        ServantManagerError::System(e)
    }
}

impl fmt::Display for ServantManagerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ServantManagerError::ForwardRequest(_) => f.write_str("PortableServer::ForwardRequest"),
            ServantManagerError::System(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for ServantManagerError {}

/// `PortableServer::ServantActivator`: the servant manager of a POA with
/// the RETAIN and USE_SERVANT_MANAGER policies. It is asked for a servant
/// the first time an object that is not in the active object map gets a
/// request, and the servant stays in the map until the object is
/// deactivated.
///
/// Both methods are called on the ORB's upcall threads.
pub trait ServantActivator: Send + Sync {
    /// The servant for the object `id` of `adapter`. Called once per
    /// activation: requests for the same object that arrive meanwhile wait
    /// for the result.
    fn incarnate(&self, id: &[u8], adapter: &Poa) -> Result<Arc<dyn Servant>, ServantManagerError>;

    /// The object `id` was deactivated, and its last request finished.
    /// `cleanup_in_progress` is true when the POA is destroyed or its
    /// manager deactivated; `remaining_activations` whether `servant` is
    /// still active under other ObjectIds.
    fn etherealize(
        &self,
        id: &[u8],
        adapter: &Poa,
        servant: Arc<dyn Servant>,
        cleanup_in_progress: bool,
        remaining_activations: bool,
    ) {
        let _ = (
            id,
            adapter,
            servant,
            cleanup_in_progress,
            remaining_activations,
        );
    }
}

/// The cookie a [`ServantLocator`] passes from `preinvoke` to `postinvoke`.
pub type Cookie = Box<dyn Any + Send>;

/// `PortableServer::ServantLocator`: the servant manager of a POA with the
/// NON_RETAIN and USE_SERVANT_MANAGER policies, asked for a servant for
/// every request.
///
/// Both methods are called on the thread that runs the request.
pub trait ServantLocator: Send + Sync {
    /// The servant to run `operation` on the object `id` of `adapter`, and
    /// a cookie for [`postinvoke`](ServantLocator::postinvoke).
    fn preinvoke(
        &self,
        id: &[u8],
        adapter: &Poa,
        operation: &str,
    ) -> Result<(Arc<dyn Servant>, Cookie), ServantManagerError>;

    /// The request that `preinvoke` provided `servant` for has run, whatever
    /// its outcome. Not called when `preinvoke` failed.
    fn postinvoke(
        &self,
        id: &[u8],
        adapter: &Poa,
        operation: &str,
        cookie: Cookie,
        servant: Arc<dyn Servant>,
    ) {
        let _ = (id, adapter, operation, cookie, servant);
    }
}

/// `PortableServer::ServantManager`: one of the two kinds.
#[derive(Clone)]
pub enum ServantManager {
    /// For a POA with RETAIN.
    Activator(Arc<dyn ServantActivator>),
    /// For a POA with NON_RETAIN.
    Locator(Arc<dyn ServantLocator>),
}

impl fmt::Debug for ServantManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ServantManager::Activator(_) => f.write_str("ServantManager::Activator"),
            ServantManager::Locator(_) => f.write_str("ServantManager::Locator"),
        }
    }
}

/// `PortableServer::AdapterActivator`: creates child POAs on demand, when
/// a request or [`Poa::find_poa`] names a child that does not exist.
pub trait AdapterActivator: Send + Sync {
    /// Creates the child `name` of `parent` (with [`Poa::create_poa`]) and
    /// returns true, or returns false if there is no such POA. Called on an
    /// upcall thread; calls for the same name are made one at a time, and
    /// requests for it wait for the result.
    fn unknown_adapter(&self, parent: &Poa, name: &str) -> bool;
}

/// The reply a servant writes: the return value and the `out` and `inout`
/// arguments, in order, or a user exception.
#[derive(Debug)]
pub struct ServerReply {
    version: Version,
    order: ByteOrder,
    request_id: u32,
    code_sets: CodeSets,
    msg: MessageEncoder,
    status: ReplyStatus,
}

impl ServerReply {
    pub(crate) fn new(
        version: Version,
        order: ByteOrder,
        request_id: u32,
        code_sets: CodeSets,
    ) -> Result<ServerReply, SystemException> {
        Ok(ServerReply {
            version,
            order,
            request_id,
            code_sets,
            msg: start(version, order, request_id, ReplyStatus::NoException)?,
            status: ReplyStatus::NoException,
        })
    }

    /// The stream for the result. Each call continues where the last one
    /// stopped.
    pub fn output(&mut self) -> OutStream<'_> {
        OutStream {
            enc: self.msg.body(),
            code_sets: self.code_sets,
            orb: None,
        }
    }

    /// Replaces the reply with the user exception `repository_id`, and
    /// returns the stream its members are written to.
    pub fn user_exception(
        &mut self,
        repository_id: &str,
    ) -> Result<OutStream<'_>, SystemException> {
        self.msg = start(
            self.version,
            self.order,
            self.request_id,
            ReplyStatus::UserException,
        )?;
        self.status = ReplyStatus::UserException;
        let mut out = self.output();
        out.write_string(repository_id)?;
        Ok(out)
    }

    /// The status the reply has so far.
    pub fn status(&self) -> ReplyStatus {
        self.status
    }

    /// The whole reply; large octet sequences written with
    /// `write_octet_seq_shared` are not copied into it.
    pub(crate) fn finish(self) -> Result<OutMessage, SystemException> {
        self.msg.finish_shared().map_err(|_| internal())
    }
}

fn start(
    version: Version,
    order: ByteOrder,
    request_id: u32,
    reply_status: ReplyStatus,
) -> Result<MessageEncoder, SystemException> {
    let header = ReplyHeader {
        request_id,
        reply_status,
        service_context: Vec::new(),
    };
    MessageEncoder::reply(version, order, &header).map_err(|_| internal())
}

fn internal() -> SystemException {
    SystemException::new(SystemExceptionKind::INTERNAL, 0, CompletionStatus::Maybe)
}

/// A whole `Reply` sending the client to `ior` (`LOCATION_FORWARD`).
pub(crate) fn location_forward_reply(
    version: Version,
    order: ByteOrder,
    request_id: u32,
    ior: &Ior,
) -> Option<Vec<u8>> {
    let mut m = start(version, order, request_id, ReplyStatus::LocationForward).ok()?;
    ior.encode(m.body()).ok()?;
    m.finish().ok()
}

/// A whole `Reply` carrying the system exception `e`.
pub(crate) fn system_exception_reply(
    version: Version,
    order: ByteOrder,
    request_id: u32,
    e: &SystemException,
) -> Option<Vec<u8>> {
    let mut m = start(version, order, request_id, ReplyStatus::SystemException).ok()?;
    SystemExceptionBody::from(e).encode(m.body()).ok()?;
    m.finish().ok()
}

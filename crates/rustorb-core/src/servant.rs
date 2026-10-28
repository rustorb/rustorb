//! Servants and the replies they write.

use rustorb_cdr::{ByteOrder, CdrEncode};
use rustorb_giop::{MessageEncoder, ReplyHeader, ReplyStatus, SystemExceptionBody, Version};

use crate::stream::{InStream, OutStream};
use crate::{CodeSets, CompletionStatus, SystemException, SystemExceptionKind};

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

    pub(crate) fn finish(self) -> Result<Vec<u8>, SystemException> {
        self.msg.finish().map_err(|_| internal())
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

use std::fmt;

use rustorb_core::name::{self, InvalidName, Name};
use rustorb_core::{
    CompletionStatus, Exception, InStream, ServerReply, SystemException, SystemExceptionKind,
    UserException, minor,
};

use crate::client::NamingContext;
use crate::repository_id;

/// `CosNaming::NamingContext::NotFoundReason`: why a name was not found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NotFoundReason {
    /// `missing_node`: the first component of the rest of the name is not
    /// bound.
    MissingNode,
    /// `not_context`: the first component of the rest of the name is bound
    /// to an object, where a context was needed.
    NotContext,
    /// `not_object`: the first component of the rest of the name is bound
    /// to a context, where an object was needed.
    NotObject,
}

impl NotFoundReason {
    /// The IDL name, such as `missing_node`.
    pub fn name(self) -> &'static str {
        match self {
            NotFoundReason::MissingNode => "missing_node",
            NotFoundReason::NotContext => "not_context",
            NotFoundReason::NotObject => "not_object",
        }
    }

    fn read(input: &mut InStream<'_>) -> Result<NotFoundReason, SystemException> {
        match input.read::<u32>()? {
            0 => Ok(NotFoundReason::MissingNode),
            1 => Ok(NotFoundReason::NotContext),
            2 => Ok(NotFoundReason::NotObject),
            _ => Err(SystemException::new(
                SystemExceptionKind::MARSHAL,
                0,
                CompletionStatus::Yes,
            )),
        }
    }
}

impl fmt::Display for NotFoundReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// How a naming operation failed: one of the CosNaming user exceptions, or
/// a system exception.
///
/// Which user exceptions an operation can raise is listed on it. A user
/// exception the operation does not declare becomes `UNKNOWN` (minor
/// `UNLISTED_USER_EXCEPTION`), as for any IDL stub.
#[derive(Debug, Clone)]
pub enum NamingError {
    /// `NamingContext::NotFound`: resolving `rest_of_name` failed for
    /// `why`.
    NotFound {
        /// Why the name was not found.
        why: NotFoundReason,
        /// The part of the name not resolved, from the component that
        /// failed.
        rest_of_name: Name,
    },
    /// `NamingContext::CannotProceed`: the naming service gave up on
    /// `rest_of_name`, which the client may resolve in `context` itself.
    CannotProceed {
        /// The context to continue in.
        context: Option<NamingContext>,
        /// The part of the name not resolved.
        rest_of_name: Name,
    },
    /// `NamingContext::InvalidName`: the name is empty or a component is
    /// not valid. Also raised locally for a malformed stringified name.
    InvalidName,
    /// `NamingContext::AlreadyBound`: `bind` on a name that is bound.
    AlreadyBound,
    /// `NamingContext::NotEmpty`: `destroy` on a context with bindings.
    NotEmpty,
    /// `NamingContextExt::InvalidAddress`: `to_url` with an address that is
    /// not a valid `corbaloc` address list.
    InvalidAddress,
    /// A system exception.
    System(SystemException),
}

impl NamingError {
    /// The repository id of the exception.
    pub fn repository_id(&self) -> &str {
        match self {
            NamingError::NotFound { .. } => repository_id::NOT_FOUND,
            NamingError::CannotProceed { .. } => repository_id::CANNOT_PROCEED,
            NamingError::InvalidName => repository_id::INVALID_NAME,
            NamingError::AlreadyBound => repository_id::ALREADY_BOUND,
            NamingError::NotEmpty => repository_id::NOT_EMPTY,
            NamingError::InvalidAddress => repository_id::INVALID_ADDRESS,
            NamingError::System(e) => &e.repository_id,
        }
    }

    /// The system exception, if this is one.
    pub fn as_system(&self) -> Option<&SystemException> {
        match self {
            NamingError::System(e) => Some(e),
            _ => None,
        }
    }

    /// Decodes an exception raised by an operation that declares the user
    /// exceptions in `declared` (repository ids).
    pub(crate) fn from_exception(e: Exception, declared: &[&str]) -> NamingError {
        match e {
            Exception::System(e) => NamingError::System(e),
            Exception::User(e) if declared.contains(&e.repository_id()) => {
                NamingError::from_user(&e).unwrap_or_else(NamingError::System)
            }
            Exception::User(_) => NamingError::System(SystemException::new(
                SystemExceptionKind::UNKNOWN,
                minor::UNLISTED_USER_EXCEPTION,
                CompletionStatus::Yes,
            )),
        }
    }

    /// Writes a user exception as the reply of a servant; a system
    /// exception is returned instead.
    pub(crate) fn write(&self, reply: &mut ServerReply) -> Result<(), SystemException> {
        if let NamingError::System(e) = self {
            return Err(e.clone());
        }
        let mut out = reply.user_exception(self.repository_id())?;
        match self {
            NamingError::NotFound { why, rest_of_name } => {
                out.write_u32(match why {
                    NotFoundReason::MissingNode => 0,
                    NotFoundReason::NotContext => 1,
                    NotFoundReason::NotObject => 2,
                });
                name::write_name(&mut out, rest_of_name)
            }
            NamingError::CannotProceed {
                context,
                rest_of_name,
            } => {
                out.write_object(context.as_ref().map(NamingContext::object))?;
                name::write_name(&mut out, rest_of_name)
            }
            _ => Ok(()),
        }
    }

    fn from_user(e: &UserException) -> Result<NamingError, SystemException> {
        let mut input = e.input()?;
        Ok(match e.repository_id() {
            repository_id::NOT_FOUND => NamingError::NotFound {
                why: NotFoundReason::read(&mut input)?,
                rest_of_name: name::read_name(&mut input)?,
            },
            repository_id::CANNOT_PROCEED => NamingError::CannotProceed {
                context: input.read_object()?.map(NamingContext::unchecked_narrow),
                rest_of_name: name::read_name(&mut input)?,
            },
            repository_id::INVALID_NAME => NamingError::InvalidName,
            repository_id::ALREADY_BOUND => NamingError::AlreadyBound,
            repository_id::NOT_EMPTY => NamingError::NotEmpty,
            repository_id::INVALID_ADDRESS => NamingError::InvalidAddress,
            _ => unreachable!("only declared CosNaming exceptions are decoded"),
        })
    }
}

impl From<SystemException> for NamingError {
    fn from(e: SystemException) -> Self {
        NamingError::System(e)
    }
}

impl From<InvalidName> for NamingError {
    fn from(_: InvalidName) -> Self {
        NamingError::InvalidName
    }
}

impl fmt::Display for NamingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let rest = |name: &Name| name::to_string(name).unwrap_or_else(|_| "<empty>".to_owned());
        match self {
            NamingError::NotFound { why, rest_of_name } => {
                write!(
                    f,
                    "CosNaming::NamingContext::NotFound ({why}, rest {:?})",
                    rest(rest_of_name)
                )
            }
            NamingError::CannotProceed { rest_of_name, .. } => {
                write!(
                    f,
                    "CosNaming::NamingContext::CannotProceed (rest {:?})",
                    rest(rest_of_name)
                )
            }
            NamingError::InvalidName => f.write_str("CosNaming::NamingContext::InvalidName"),
            NamingError::AlreadyBound => f.write_str("CosNaming::NamingContext::AlreadyBound"),
            NamingError::NotEmpty => f.write_str("CosNaming::NamingContext::NotEmpty"),
            NamingError::InvalidAddress => {
                f.write_str("CosNaming::NamingContextExt::InvalidAddress")
            }
            NamingError::System(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for NamingError {}

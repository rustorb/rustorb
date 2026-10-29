use std::fmt;

use rustorb_core::name::{self, InvalidName, Name};
use rustorb_core::{CompletionStatus, SystemException, SystemExceptionKind, minor};

use crate::client::{NamingContext, from_idl_name, to_idl_name};
use crate::idl::cos_naming::{self as idl, naming_context as nc, naming_context_ext as nce};
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
}

// --- the exceptions of the generated code (crate::idl) ----------------------

impl From<nc::NotFoundReason> for NotFoundReason {
    fn from(why: nc::NotFoundReason) -> NotFoundReason {
        match why {
            nc::NotFoundReason::MissingNode => NotFoundReason::MissingNode,
            nc::NotFoundReason::NotContext => NotFoundReason::NotContext,
            nc::NotFoundReason::NotObject => NotFoundReason::NotObject,
        }
    }
}

impl From<NotFoundReason> for nc::NotFoundReason {
    fn from(why: NotFoundReason) -> nc::NotFoundReason {
        match why {
            NotFoundReason::MissingNode => nc::NotFoundReason::MissingNode,
            NotFoundReason::NotContext => nc::NotFoundReason::NotContext,
            NotFoundReason::NotObject => nc::NotFoundReason::NotObject,
        }
    }
}

impl From<nc::NotFound> for NamingError {
    fn from(e: nc::NotFound) -> NamingError {
        NamingError::NotFound {
            why: e.why.into(),
            rest_of_name: from_idl_name(e.rest_of_name),
        }
    }
}

impl From<nc::CannotProceed> for NamingError {
    fn from(e: nc::CannotProceed) -> NamingError {
        NamingError::CannotProceed {
            context: e
                .cxt
                .map(|c| NamingContext::unchecked_narrow(c.into_object())),
            rest_of_name: from_idl_name(e.rest_of_name),
        }
    }
}

impl From<nc::InvalidName> for NamingError {
    fn from(_: nc::InvalidName) -> NamingError {
        NamingError::InvalidName
    }
}

impl From<nc::AlreadyBound> for NamingError {
    fn from(_: nc::AlreadyBound) -> NamingError {
        NamingError::AlreadyBound
    }
}

impl From<nc::NotEmpty> for NamingError {
    fn from(_: nc::NotEmpty) -> NamingError {
        NamingError::NotEmpty
    }
}

impl From<nce::InvalidAddress> for NamingError {
    fn from(_: nce::InvalidAddress) -> NamingError {
        NamingError::InvalidAddress
    }
}

/// A [`NamingError`] as the user exception of the generated code, for the
/// servants.
enum User {
    NotFound(nc::NotFound),
    CannotProceed(nc::CannotProceed),
    InvalidName(nc::InvalidName),
    AlreadyBound(nc::AlreadyBound),
    NotEmpty(nc::NotEmpty),
    InvalidAddress(nce::InvalidAddress),
}

impl NamingError {
    fn into_user(self) -> Result<User, SystemException> {
        Ok(match self {
            NamingError::NotFound { why, rest_of_name } => User::NotFound(nc::NotFound {
                why: why.into(),
                rest_of_name: to_idl_name(&rest_of_name),
            }),
            NamingError::CannotProceed {
                context,
                rest_of_name,
            } => User::CannotProceed(nc::CannotProceed {
                cxt: context.map(|c| idl::NamingContext::unchecked_narrow(c.into_object())),
                rest_of_name: to_idl_name(&rest_of_name),
            }),
            NamingError::InvalidName => User::InvalidName(nc::InvalidName {}),
            NamingError::AlreadyBound => User::AlreadyBound(nc::AlreadyBound {}),
            NamingError::NotEmpty => User::NotEmpty(nc::NotEmpty {}),
            NamingError::InvalidAddress => User::InvalidAddress(nce::InvalidAddress {}),
            NamingError::System(e) => return Err(e),
        })
    }
}

/// Converts between [`NamingError`] and the error enum of an operation of
/// the generated code, whose user exceptions are `$user`. A user exception
/// the operation does not declare becomes `UNKNOWN`, as a skeleton sends it.
macro_rules! errors {
    ($($ty:ty { $($user:ident),* })*) => {
        $(
            impl From<$ty> for NamingError {
                fn from(e: $ty) -> NamingError {
                    type E = $ty;
                    match e {
                        $(E::$user(e) => e.into(),)*
                        E::System(e) => NamingError::System(e),
                    }
                }
            }

            impl From<NamingError> for $ty {
                fn from(e: NamingError) -> $ty {
                    type E = $ty;
                    match e.into_user() {
                        $(Ok(User::$user(e)) => E::$user(e),)*
                        #[allow(unreachable_patterns)]
                        Ok(_) => E::System(SystemException::new(
                            SystemExceptionKind::UNKNOWN,
                            minor::UNLISTED_USER_EXCEPTION,
                            CompletionStatus::Maybe,
                        )),
                        Err(e) => E::System(e),
                    }
                }
            }
        )*
    };
}

errors! {
    nc::BindError { NotFound, CannotProceed, InvalidName, AlreadyBound }
    nc::RebindError { NotFound, CannotProceed, InvalidName }
    nc::BindContextError { NotFound, CannotProceed, InvalidName, AlreadyBound }
    nc::RebindContextError { NotFound, CannotProceed, InvalidName }
    nc::ResolveError { NotFound, CannotProceed, InvalidName }
    nc::UnbindError { NotFound, CannotProceed, InvalidName }
    nc::BindNewContextError { NotFound, CannotProceed, InvalidName, AlreadyBound }
    nc::DestroyError { NotEmpty }
    nce::ToStringError { InvalidName }
    nce::ToNameError { InvalidName }
    nce::ToUrlError { InvalidAddress, InvalidName }
    nce::ResolveStrError { NotFound, CannotProceed, InvalidName }
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

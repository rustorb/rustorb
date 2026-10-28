use std::fmt;

use rustorb_giop::{CompletionStatus, SystemExceptionBody};

use crate::stream::InStream;

/// The vendor minor code set id of the OMG (`"OM\0\0"`). Minor codes the
/// CORBA specification defines are `OMG_VMCID | n`.
pub const OMG_VMCID: u32 = 0x4f4d_0000;

macro_rules! system_exceptions {
    ($($variant:ident => $name:literal,)*) => {
        /// The standard system exceptions of CORBA 3.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        #[allow(non_camel_case_types, clippy::upper_case_acronyms)]
        pub enum SystemExceptionKind {
            $(
                #[doc = concat!("`CORBA::", $name, "`.")]
                $variant,
            )*
        }

        impl SystemExceptionKind {
            /// The IDL name, such as `TRANSIENT`.
            pub fn name(self) -> &'static str {
                match self {
                    $(SystemExceptionKind::$variant => $name,)*
                }
            }

            /// The repository id, such as `IDL:omg.org/CORBA/TRANSIENT:1.0`.
            pub fn repository_id(self) -> &'static str {
                match self {
                    $(SystemExceptionKind::$variant => concat!("IDL:omg.org/CORBA/", $name, ":1.0"),)*
                }
            }

            /// Looks a repository id up.
            pub fn from_repository_id(id: &str) -> Option<SystemExceptionKind> {
                match id {
                    $(concat!("IDL:omg.org/CORBA/", $name, ":1.0") => Some(SystemExceptionKind::$variant),)*
                    _ => None,
                }
            }
        }
    };
}

system_exceptions! {
    UNKNOWN => "UNKNOWN",
    BAD_PARAM => "BAD_PARAM",
    NO_MEMORY => "NO_MEMORY",
    IMP_LIMIT => "IMP_LIMIT",
    COMM_FAILURE => "COMM_FAILURE",
    INV_OBJREF => "INV_OBJREF",
    NO_PERMISSION => "NO_PERMISSION",
    INTERNAL => "INTERNAL",
    MARSHAL => "MARSHAL",
    INITIALIZE => "INITIALIZE",
    NO_IMPLEMENT => "NO_IMPLEMENT",
    BAD_TYPECODE => "BAD_TYPECODE",
    BAD_OPERATION => "BAD_OPERATION",
    NO_RESOURCES => "NO_RESOURCES",
    NO_RESPONSE => "NO_RESPONSE",
    PERSIST_STORE => "PERSIST_STORE",
    BAD_INV_ORDER => "BAD_INV_ORDER",
    TRANSIENT => "TRANSIENT",
    FREE_MEM => "FREE_MEM",
    INV_IDENT => "INV_IDENT",
    INV_FLAG => "INV_FLAG",
    INTF_REPOS => "INTF_REPOS",
    BAD_CONTEXT => "BAD_CONTEXT",
    OBJ_ADAPTER => "OBJ_ADAPTER",
    DATA_CONVERSION => "DATA_CONVERSION",
    OBJECT_NOT_EXIST => "OBJECT_NOT_EXIST",
    TRANSACTION_REQUIRED => "TRANSACTION_REQUIRED",
    TRANSACTION_ROLLEDBACK => "TRANSACTION_ROLLEDBACK",
    INVALID_TRANSACTION => "INVALID_TRANSACTION",
    INV_POLICY => "INV_POLICY",
    CODESET_INCOMPATIBLE => "CODESET_INCOMPATIBLE",
    REBIND => "REBIND",
    TIMEOUT => "TIMEOUT",
    TRANSACTION_UNAVAILABLE => "TRANSACTION_UNAVAILABLE",
    TRANSACTION_MODE => "TRANSACTION_MODE",
    BAD_QOS => "BAD_QOS",
    INVALID_ACTIVITY => "INVALID_ACTIVITY",
    ACTIVITY_COMPLETED => "ACTIVITY_COMPLETED",
    ACTIVITY_REQUIRED => "ACTIVITY_REQUIRED",
    THREAD_CANCELLED => "THREAD_CANCELLED",
}

impl fmt::Display for SystemExceptionKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Minor codes defined by the CORBA specification (already including
/// [`OMG_VMCID`]), for the exceptions rustorb raises itself.
pub mod minor {
    use super::OMG_VMCID;

    /// `BAD_PARAM`: `string_to_object` failed because of a bad scheme name.
    pub const BAD_SCHEME_NAME: u32 = OMG_VMCID | 7;
    /// `BAD_PARAM`: `string_to_object` failed because of a bad address.
    pub const BAD_ADDRESS: u32 = OMG_VMCID | 8;
    /// `BAD_PARAM`: `string_to_object` failed because of a bad scheme
    /// specific part.
    pub const BAD_SCHEME_SPECIFIC_PART: u32 = OMG_VMCID | 9;
    /// `BAD_PARAM`: `string_to_object` failed for another reason.
    pub const STRING_TO_OBJECT_OTHER: u32 = OMG_VMCID | 10;
    /// `BAD_PARAM`: a `wchar` or `wstring` was used, but no `wchar`
    /// transmission code set is known for the connection.
    pub const WCHAR_TCS_NOT_KNOWN: u32 = OMG_VMCID | 23;
    /// `UNKNOWN`: the target raised a user exception the operation does
    /// not declare.
    pub const UNLISTED_USER_EXCEPTION: u32 = OMG_VMCID | 1;
    /// `BAD_INV_ORDER`: the ORB has shut down.
    pub const ORB_SHUT_DOWN: u32 = OMG_VMCID | 4;
    /// `TRANSIENT`: no usable profile in the IOR.
    pub const NO_USABLE_PROFILE: u32 = OMG_VMCID | 2;
    /// `TRANSIENT`: the request was cancelled.
    pub const REQUEST_CANCELLED: u32 = OMG_VMCID | 3;
}

/// A CORBA system exception: one of the standard exceptions (or another
/// ORB's own), with its minor code and completion status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemException {
    /// The repository id, such as `IDL:omg.org/CORBA/TRANSIENT:1.0`.
    pub repository_id: String,
    /// The minor code.
    pub minor: u32,
    /// Whether the operation completed.
    pub completed: CompletionStatus,
}

impl SystemException {
    /// Creates a standard system exception.
    pub fn new(kind: SystemExceptionKind, minor: u32, completed: CompletionStatus) -> Self {
        SystemException {
            repository_id: kind.repository_id().to_owned(),
            minor,
            completed,
        }
    }

    /// The standard exception this is, if it is one.
    pub fn kind(&self) -> Option<SystemExceptionKind> {
        SystemExceptionKind::from_repository_id(&self.repository_id)
    }

    /// Whether this is the standard exception `kind`.
    pub fn is(&self, kind: SystemExceptionKind) -> bool {
        self.kind() == Some(kind)
    }
}

impl From<SystemExceptionBody> for SystemException {
    fn from(body: SystemExceptionBody) -> Self {
        SystemException {
            repository_id: body.exception_id,
            minor: body.minor_code_value,
            completed: body.completion_status,
        }
    }
}

impl From<&SystemException> for SystemExceptionBody {
    fn from(e: &SystemException) -> Self {
        SystemExceptionBody {
            exception_id: e.repository_id.clone(),
            minor_code_value: e.minor,
            completion_status: e.completed,
        }
    }
}

/// Errors while marshalling arguments are `MARSHAL`, not completed.
impl From<rustorb_cdr::Error> for SystemException {
    fn from(_: rustorb_cdr::Error) -> Self {
        SystemException::new(SystemExceptionKind::MARSHAL, 0, CompletionStatus::No)
    }
}

impl fmt::Display for SystemException {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self.kind() {
            Some(kind) => kind.name(),
            None => &self.repository_id,
        };
        let completed = match self.completed {
            CompletionStatus::Yes => "COMPLETED_YES",
            CompletionStatus::No => "COMPLETED_NO",
            CompletionStatus::Maybe => "COMPLETED_MAYBE",
        };
        write!(f, "CORBA::{name} (minor {:#010x}, {completed})", self.minor)
    }
}

impl std::error::Error for SystemException {}

/// A user exception raised by the target: its repository id, and the
/// members, which the caller decodes with [`input`](UserException::input).
#[derive(Debug, Clone)]
pub struct UserException {
    pub(crate) repository_id: String,
    pub(crate) reply: crate::Reply,
}

impl UserException {
    /// The repository id of the exception.
    pub fn repository_id(&self) -> &str {
        &self.repository_id
    }

    /// A stream positioned at the first member, after the repository id.
    pub fn input(&self) -> Result<InStream<'_>, SystemException> {
        let mut input = self.reply.input();
        input.read_string()?;
        Ok(input)
    }
}

impl fmt::Display for UserException {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "user exception {}", self.repository_id)
    }
}

/// The outcome of an invocation that did not return normally.
#[derive(Debug, Clone)]
pub enum Exception {
    /// A system exception, raised by the target or by rustorb.
    System(SystemException),
    /// A user exception raised by the target.
    User(UserException),
}

impl Exception {
    /// The system exception, if this is one.
    pub fn as_system(&self) -> Option<&SystemException> {
        match self {
            Exception::System(e) => Some(e),
            Exception::User(_) => None,
        }
    }

    /// The user exception, if this is one.
    pub fn as_user(&self) -> Option<&UserException> {
        match self {
            Exception::User(e) => Some(e),
            Exception::System(_) => None,
        }
    }

    /// The repository id of the exception.
    pub fn repository_id(&self) -> &str {
        match self {
            Exception::System(e) => &e.repository_id,
            Exception::User(e) => &e.repository_id,
        }
    }
}

impl From<SystemException> for Exception {
    fn from(e: SystemException) -> Self {
        Exception::System(e)
    }
}

impl fmt::Display for Exception {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Exception::System(e) => e.fmt(f),
            Exception::User(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for Exception {}

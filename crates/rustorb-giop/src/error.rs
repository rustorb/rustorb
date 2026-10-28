use std::fmt;

use crate::Version;

/// Result type of GIOP operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Errors raised while reading or writing GIOP messages.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// A CDR error in a header or body.
    Cdr(rustorb_cdr::Error),
    /// The message did not start with `GIOP`.
    BadMagic([u8; 4]),
    /// The GIOP version is not 1.0, 1.1 or 1.2.
    UnsupportedVersion(Version),
    /// The message type is unknown or not valid for the version.
    UnknownMessageType(u8),
    /// The GIOP 1.0 byte order octet was neither `0` nor `1`.
    InvalidFlags(u8),
    /// Fewer bytes than the header and message size require.
    Truncated {
        /// Bytes the message needs.
        expected: usize,
        /// Bytes available.
        actual: usize,
    },
    /// Bytes left over after the message.
    TrailingBytes(usize),
    /// The message does not fit in the 32-bit message size.
    MessageTooLarge(usize),
    /// A `TargetAddress` had an unknown discriminator.
    InvalidAddressingDisposition(i16),
    /// GIOP 1.0 and 1.1 can only address an object by key.
    TargetNotKey,
    /// A `ReplyStatusType` value was unknown.
    InvalidReplyStatus(u32),
    /// A `LocateStatusType` value was unknown.
    InvalidLocateStatus(u32),
    /// A fragment arrived with no message to continue.
    UnexpectedFragment,
    /// A GIOP 1.1 message started fragmenting while another was in progress,
    /// or a GIOP 1.2 request id was reused while in progress.
    FragmentInProgress,
    /// A fragment's version or byte order differs from the first message.
    FragmentMismatch,
    /// The reassembled message would exceed the configured limit.
    FragmentTooLarge(usize),
    /// GIOP 1.0 has no fragmentation.
    FragmentNotSupported,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Cdr(e) => write!(f, "CDR error: {e}"),
            Error::BadMagic(m) => write!(f, "bad GIOP magic {m:02x?}"),
            Error::UnsupportedVersion(v) => write!(f, "unsupported GIOP version {v}"),
            Error::UnknownMessageType(t) => write!(f, "unknown GIOP message type {t}"),
            Error::InvalidFlags(b) => write!(f, "invalid GIOP 1.0 byte order {b:#04x}"),
            Error::Truncated { expected, actual } => {
                write!(
                    f,
                    "GIOP message truncated: need {expected} bytes, got {actual}"
                )
            }
            Error::TrailingBytes(n) => write!(f, "{n} bytes after the GIOP message"),
            Error::MessageTooLarge(n) => write!(f, "GIOP message of {n} bytes is too large"),
            Error::InvalidAddressingDisposition(d) => {
                write!(f, "invalid addressing disposition {d}")
            }
            Error::TargetNotKey => f.write_str("GIOP 1.0/1.1 requires an object key target"),
            Error::InvalidReplyStatus(v) => write!(f, "invalid reply status {v}"),
            Error::InvalidLocateStatus(v) => write!(f, "invalid locate status {v}"),
            Error::UnexpectedFragment => f.write_str("fragment without a message to continue"),
            Error::FragmentInProgress => f.write_str("a fragmented message is already in progress"),
            Error::FragmentMismatch => {
                f.write_str("fragment version or byte order differs from the first message")
            }
            Error::FragmentTooLarge(n) => {
                write!(f, "reassembled message of {n} bytes exceeds the limit")
            }
            Error::FragmentNotSupported => f.write_str("GIOP 1.0 does not support fragments"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Cdr(e) => Some(e),
            _ => None,
        }
    }
}

impl From<rustorb_cdr::Error> for Error {
    fn from(e: rustorb_cdr::Error) -> Error {
        Error::Cdr(e)
    }
}

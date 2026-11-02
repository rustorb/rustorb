use std::fmt;

use rustorb_cdr::xcdr::XcdrFormat;

/// Result type of the type support.
pub type Result<T> = std::result::Result<T, Error>;

/// Errors raised while writing or reading a value.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// An error of the CDR stream.
    Cdr(rustorb_cdr::Error),
    /// A bounded string or sequence was longer than its bound.
    BoundExceeded {
        /// The bound.
        bound: usize,
        /// The length of the value.
        len: usize,
    },
    /// A `char` that does not fit in an IDL `char` (more than 0xFF) or
    /// `wchar` (outside the Basic Multilingual Plane).
    InvalidChar(u32),
    /// An enum value that is not one of the enumerators.
    InvalidEnumValue(i64),
    /// A union discriminator that selects no member of a union without a
    /// default, or a discriminator that does not match the member of the
    /// value being written.
    InvalidDiscriminator,
    /// A member that must be understood (`@key`, `@must_understand`) whose
    /// ID the reader does not know.
    UnknownMustUnderstand(u32),
    /// A member of a mutable type that was not received, has no default
    /// and is not optional.
    MissingMember(u32),
    /// The encapsulation of a payload is not the one of the type: a final
    /// type in a PL_CDR payload, for example.
    FormatMismatch(XcdrFormat),
    /// A payload was asked for with an encoding that is not XCDR.
    NotXcdr,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Cdr(e) => e.fmt(f),
            Error::BoundExceeded { bound, len } => {
                write!(f, "length {len} is larger than the bound {bound}")
            }
            Error::InvalidChar(c) => write!(f, "character U+{c:04X} does not fit the IDL type"),
            Error::InvalidEnumValue(v) => write!(f, "{v} is not a value of the enum"),
            Error::InvalidDiscriminator => f.write_str("invalid union discriminator"),
            Error::UnknownMustUnderstand(id) => {
                write!(f, "unknown member {id:#x} that must be understood")
            }
            Error::MissingMember(id) => write!(f, "member {id:#x} is missing"),
            Error::FormatMismatch(format) => {
                write!(f, "payload format {format:?} does not match the type")
            }
            Error::NotXcdr => f.write_str("payloads use XCDR1 or XCDR2"),
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

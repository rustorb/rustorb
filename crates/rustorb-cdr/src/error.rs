use std::fmt;

/// Result type of CDR operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Errors raised while encoding or decoding CDR.
///
/// A decoding error corresponds to `CORBA::MARSHAL` at the ORB level.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// The stream ended before a value was complete.
    UnexpectedEof {
        /// Bytes the value needed.
        needed: usize,
        /// Bytes left in the stream.
        remaining: usize,
    },
    /// A boolean octet was neither `0` nor `1`.
    InvalidBoolean(u8),
    /// A byte order flag was neither `0` nor `1`.
    InvalidByteOrder(u8),
    /// A `string` did not end with a NUL octet.
    StringNotTerminated,
    /// A `string` was not valid UTF-8.
    InvalidUtf8,
    /// A `wchar` or `wstring` was not valid UTF-16.
    InvalidUtf16,
    /// A GIOP 1.2 `wchar` had an octet length other than 2 or 4 (with BOM).
    InvalidWcharLength(u8),
    /// A GIOP 1.2 `wstring` had an odd octet length.
    OddWstringLength(u32),
    /// `wchar` and `wstring` cannot be used with GIOP 1.0.
    WcharNotSupported,
    /// A sequence or string length is larger than the bytes left.
    LengthTooLarge {
        /// The declared length.
        len: u32,
        /// Bytes left in the stream.
        remaining: usize,
    },
    /// A length does not fit in an unsigned long.
    LengthOverflow(usize),
    /// An encapsulation had no byte order octet.
    EmptyEncapsulation,
    /// An enum or union discriminator had an unknown value.
    InvalidEnumValue(u32),
    /// A TypeCode was malformed: an unknown kind, an indirection that
    /// points nowhere, invalid parameters, or nesting too deep. On the
    /// writing side, a TypeCode that cannot be written (an unresolved
    /// recursive placeholder).
    InvalidTypeCode(&'static str),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::UnexpectedEof { needed, remaining } => write!(
                f,
                "unexpected end of CDR stream: needed {needed} bytes, {remaining} left"
            ),
            Error::InvalidBoolean(v) => write!(f, "invalid boolean octet {v:#04x}"),
            Error::InvalidByteOrder(v) => write!(f, "invalid byte order flag {v:#04x}"),
            Error::StringNotTerminated => f.write_str("string is not NUL-terminated"),
            Error::InvalidUtf8 => f.write_str("string is not valid UTF-8"),
            Error::InvalidUtf16 => f.write_str("wide string is not valid UTF-16"),
            Error::InvalidWcharLength(n) => write!(f, "invalid wchar octet length {n}"),
            Error::OddWstringLength(n) => write!(f, "odd wstring octet length {n}"),
            Error::WcharNotSupported => f.write_str("wchar is not supported by GIOP 1.0"),
            Error::LengthTooLarge { len, remaining } => write!(
                f,
                "length {len} is larger than the {remaining} bytes left in the stream"
            ),
            Error::LengthOverflow(len) => {
                write!(f, "length {len} does not fit in an unsigned long")
            }
            Error::EmptyEncapsulation => f.write_str("encapsulation has no byte order octet"),
            Error::InvalidEnumValue(v) => write!(f, "invalid enum value {v}"),
            Error::InvalidTypeCode(why) => write!(f, "invalid TypeCode: {why}"),
        }
    }
}

impl std::error::Error for Error {}

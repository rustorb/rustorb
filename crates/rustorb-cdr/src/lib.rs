//! CDR (Common Data Representation) encoder and decoder.
//!
//! Handles primitive and constructed IDL types with GIOP alignment rules
//! and both byte orders.
//!
//! - [`Encoder`] and [`Decoder`] read and write primitives, strings, wide
//!   strings, sequences and encapsulations. Alignment is computed from the
//!   absolute stream offset, so a body that starts in the middle of a GIOP
//!   message keeps the alignment of the whole message.
//! - [`CdrEncode`] and [`CdrDecode`] map Rust types to IDL types: integers
//!   and floats to their IDL counterparts, [`String`] to `string`, [`Vec`] to
//!   `sequence`, and fixed-size arrays to IDL arrays.
//!
//! - [`Octets`] is a `sequence<octet>` that shares its bytes: read from a
//!   message without copying ([`Decoder::read_octet_seq_shared`]) and
//!   written without copying into the message
//!   ([`Encoder::write_octet_seq_shared`], when the encoder allows it).
//!
//! Code set conversion is out of scope: `string` is read and written as
//! bytes (UTF-8 when converted to [`String`]), and `wchar`/`wstring` use
//! UTF-16. Negotiating other code sets is left to the ORB core.
//! `any`, `TypeCode` and `fixed` are in the ORB core, which knows the
//! types. `long double` is [`LongDouble`], the IEEE 754 binary128 bits.
//!
//! Valuetypes are written and read in pieces: [`Encoder::begin_value`] /
//! [`Encoder::end_value`] and [`Decoder::read_value_header`] /
//! [`Decoder::end_value`] handle the value tags, sharing (indirections)
//! and chunking, and the caller writes or reads the state members in
//! between.

/// Octet sequences at least this long are shared rather than copied, by
/// [`Decoder::read_octet_seq_shared`] and
/// [`Encoder::write_octet_seq_shared`]. Shorter ones are cheaper to copy,
/// and a short slice should not keep a large message buffer alive.
pub const SHARE_MIN: usize = 4096;

mod decoder;
mod encoder;
mod error;
mod long_double;
mod octets;
mod traits;
mod value;
pub mod xcdr;

pub use decoder::{Decoder, Segment};
pub use encoder::Encoder;
pub use error::{Error, Result};
pub use long_double::LongDouble;
pub use octets::Octets;
pub use traits::{CdrDecode, CdrEncode};
pub use value::ValueHeader;

/// Byte order of a CDR stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ByteOrder {
    /// Big endian, flagged as `0` in GIOP headers and encapsulations.
    Big,
    /// Little endian, flagged as `1` in GIOP headers and encapsulations.
    Little,
}

impl ByteOrder {
    /// The byte order of the host.
    pub const NATIVE: ByteOrder = if cfg!(target_endian = "little") {
        ByteOrder::Little
    } else {
        ByteOrder::Big
    };

    /// The value of the GIOP byte order flag (`0` big, `1` little).
    pub fn flag(self) -> u8 {
        match self {
            ByteOrder::Big => 0,
            ByteOrder::Little => 1,
        }
    }

    /// Converts a GIOP byte order flag. Only `0` and `1` are valid.
    pub fn from_flag(flag: u8) -> Result<ByteOrder> {
        match flag {
            0 => Ok(ByteOrder::Big),
            1 => Ok(ByteOrder::Little),
            other => Err(Error::InvalidByteOrder(other)),
        }
    }
}

/// The encoding rules a stream follows.
///
/// The GIOP versions differ only in how `wchar` and `wstring` are encoded:
/// GIOP 1.0 has no wide characters, GIOP 1.1 encodes them as aligned
/// fixed-width code units, and GIOP 1.2 encodes them as octet sequences.
///
/// [`Xcdr1`](Encoding::Xcdr1) and [`Xcdr2`](Encoding::Xcdr2) are the
/// Extended CDR encodings of DDS-XTypes, versions 1 and 2. XCDR2 aligns
/// no primitive to more than 4 octets ([`max_align`](Encoding::max_align)).
/// The headers they add (the payload header, DHEADER, EMHEADER, parameter
/// headers) are in [`xcdr`]; which of them a type needs is up to the code
/// that knows the type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Encoding {
    /// GIOP 1.0.
    Giop1_0,
    /// GIOP 1.1.
    Giop1_1,
    /// GIOP 1.2 and later.
    Giop1_2,
    /// DDS-XTypes Extended CDR, encoding version 1.
    Xcdr1,
    /// DDS-XTypes Extended CDR, encoding version 2.
    Xcdr2,
}

impl Encoding {
    /// Whether this is one of the GIOP encodings.
    pub fn is_giop(self) -> bool {
        matches!(
            self,
            Encoding::Giop1_0 | Encoding::Giop1_1 | Encoding::Giop1_2
        )
    }

    /// The largest alignment a value gets: 8 octets, or 4 for XCDR2.
    ///
    /// [`Encoder::align`] and [`Decoder::align`] use no more than this.
    pub fn max_align(self) -> usize {
        match self {
            Encoding::Xcdr2 => 4,
            _ => 8,
        }
    }
}

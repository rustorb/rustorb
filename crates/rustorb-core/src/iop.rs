//! `IOP::CodecFactory` and `IOP::Codec`: CDR encapsulations of anys and
//! values, the usual form of service context and tagged component data
//! (see [`pi`](crate::pi) and docs/portable-interceptors.md).

use std::fmt;
use std::sync::Weak;

use rustorb_cdr::{ByteOrder, CdrVersion, Decoder, Encoder};

pub use rustorb_giop::{ServiceContext, TaggedComponent, TaggedProfile, component_tag, service_id};

use crate::orb::OrbInner;
use crate::stream::{InStream, OutStream};
use crate::{Any, Orb, TypeCode};

/// `IOP::ENCODING_CDR_ENCAPS`.
pub const ENCODING_CDR_ENCAPS: i16 = 0;

/// `IOP::Encoding`: a format and its version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Encoding {
    /// The format, such as [`ENCODING_CDR_ENCAPS`].
    pub format: i16,
    /// Major version.
    pub major_version: u8,
    /// Minor version.
    pub minor_version: u8,
}

impl Encoding {
    /// `ENCODING_CDR_ENCAPS` of GIOP `major.minor`.
    pub fn cdr(major_version: u8, minor_version: u8) -> Encoding {
        Encoding {
            format: ENCODING_CDR_ENCAPS,
            major_version,
            minor_version,
        }
    }
}

/// Why a [`Codec`] failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodecError {
    /// `Codec::InvalidTypeForEncoding`: the value cannot be written in
    /// this encoding (or does not match its TypeCode).
    InvalidTypeForEncoding,
    /// `Codec::FormatMismatch`: the data is not a valid encapsulation of
    /// this encoding.
    FormatMismatch,
    /// `Codec::TypeMismatch`: the data does not hold a value of the
    /// TypeCode given.
    TypeMismatch,
}

impl fmt::Display for CodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            CodecError::InvalidTypeForEncoding => "IOP::Codec::InvalidTypeForEncoding",
            CodecError::FormatMismatch => "IOP::Codec::FormatMismatch",
            CodecError::TypeMismatch => "IOP::Codec::TypeMismatch",
        })
    }
}

impl std::error::Error for CodecError {}

/// `IOP::CodecFactory::UnknownEncoding`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnknownEncoding;

impl fmt::Display for UnknownEncoding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("IOP::CodecFactory::UnknownEncoding")
    }
}

impl std::error::Error for UnknownEncoding {}

/// `IOP::CodecFactory` of an ORB ([`Orb::codec_factory`]).
#[derive(Clone)]
pub struct CodecFactory {
    orb: Weak<OrbInner>,
    order: ByteOrder,
}

impl fmt::Debug for CodecFactory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CodecFactory").finish_non_exhaustive()
    }
}

impl CodecFactory {
    pub(crate) fn new(orb: &Orb) -> CodecFactory {
        CodecFactory {
            orb: std::sync::Arc::downgrade(&orb.inner),
            order: orb.inner.options.byte_order,
        }
    }

    /// `create_codec`: a codec for `ENCODING_CDR_ENCAPS` 1.0 to 1.2;
    /// others are `UnknownEncoding`.
    pub fn create_codec(&self, encoding: Encoding) -> Result<Codec, UnknownEncoding> {
        let version = match (
            encoding.format,
            encoding.major_version,
            encoding.minor_version,
        ) {
            (ENCODING_CDR_ENCAPS, 1, 0) => CdrVersion::Giop1_0,
            (ENCODING_CDR_ENCAPS, 1, 1) => CdrVersion::Giop1_1,
            (ENCODING_CDR_ENCAPS, 1, 2) => CdrVersion::Giop1_2,
            _ => return Err(UnknownEncoding),
        };
        Ok(Codec {
            orb: self.orb.clone(),
            order: self.order,
            version,
        })
    }
}

/// `IOP::Codec`: CDR encapsulations, with the native code sets (UTF-8
/// and UTF-16). Object references read from data belong to the ORB of the
/// factory.
#[derive(Clone)]
pub struct Codec {
    orb: Weak<OrbInner>,
    order: ByteOrder,
    version: CdrVersion,
}

impl fmt::Debug for Codec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Codec")
            .field("version", &self.version)
            .finish_non_exhaustive()
    }
}

impl Codec {
    /// `encode`: the TypeCode and the value of `data`.
    pub fn encode(&self, data: &Any) -> Result<Vec<u8>, CodecError> {
        self.write(|out| out.write_any(data))
    }

    /// `decode`: an any written by [`encode`](Codec::encode).
    pub fn decode(&self, data: &[u8]) -> Result<Any, CodecError> {
        self.read(data, |input| input.read_any())
            .map_err(|_| CodecError::FormatMismatch)
    }

    /// `encode_value`: the value of `data` only.
    pub fn encode_value(&self, data: &Any) -> Result<Vec<u8>, CodecError> {
        self.write(|out| out.write_value(data.typecode(), data.value()))
    }

    /// `decode_value`: a value of type `tc` written by
    /// [`encode_value`](Codec::encode_value).
    pub fn decode_value(&self, data: &[u8], tc: &TypeCode) -> Result<Any, CodecError> {
        let value = self
            .read(data, |input| input.read_value(tc))
            .map_err(|e| match e {
                ReadError::Format => CodecError::FormatMismatch,
                ReadError::Value => CodecError::TypeMismatch,
            })?;
        Ok(Any::new(tc.clone(), value))
    }

    fn write(
        &self,
        f: impl FnOnce(&mut OutStream<'_>) -> Result<(), crate::SystemException>,
    ) -> Result<Vec<u8>, CodecError> {
        let mut enc = Encoder::encapsulation(self.order, self.version);
        {
            let mut out = OutStream::standalone(&mut enc);
            if self.version == CdrVersion::Giop1_0 {
                // GIOP 1.0 has no wchar.
                out.set_code_sets(crate::CodeSets::NATIVE_NO_WCHAR);
            }
            f(&mut out).map_err(|_| CodecError::InvalidTypeForEncoding)?;
        }
        Ok(enc.into_parts().0)
    }

    fn read<T>(
        &self,
        data: &[u8],
        f: impl FnOnce(&mut InStream<'_>) -> Result<T, crate::SystemException>,
    ) -> Result<T, ReadError> {
        let dec = Decoder::encapsulation(data, self.version).map_err(|_| ReadError::Format)?;
        let orb = self.orb.upgrade().map(|inner| Orb { inner });
        let mut input = InStream::standalone(dec, orb.as_ref());
        if self.version == CdrVersion::Giop1_0 {
            input.set_code_sets(crate::CodeSets::NATIVE_NO_WCHAR);
        }
        f(&mut input).map_err(|_| ReadError::Value)
    }
}

enum ReadError {
    Format,
    Value,
}

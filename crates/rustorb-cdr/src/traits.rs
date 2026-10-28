use crate::{Decoder, Encoder, Result};

/// A type that can be written as CDR.
///
/// [`Vec<T>`] and `[T]` are written as an IDL `sequence` (with a length);
/// `[T; N]` is written as an IDL array (without a length).
pub trait CdrEncode {
    /// Writes `self`.
    fn encode(&self, enc: &mut Encoder) -> Result<()>;

    /// Writes the elements of a slice, without a length. Overridden for
    /// `u8` to copy the bytes in one go.
    fn encode_slice(items: &[Self], enc: &mut Encoder) -> Result<()>
    where
        Self: Sized,
    {
        items.iter().try_for_each(|item| item.encode(enc))
    }
}

/// A type that can be read from CDR.
pub trait CdrDecode: Sized {
    /// Reads a value.
    fn decode(dec: &mut Decoder<'_>) -> Result<Self>;

    /// Reads `len` elements. Overridden for `u8` to copy the bytes in one go.
    fn decode_vec(len: usize, dec: &mut Decoder<'_>) -> Result<Vec<Self>> {
        // Cap the up-front allocation; the length was only checked against
        // a minimum element size of one byte.
        let mut items = Vec::with_capacity(len.min(4096));
        for _ in 0..len {
            items.push(Self::decode(dec)?);
        }
        Ok(items)
    }
}

macro_rules! impl_number {
    ($($ty:ty => $write:ident, $read:ident;)*) => {
        $(
            impl CdrEncode for $ty {
                fn encode(&self, enc: &mut Encoder) -> Result<()> {
                    enc.$write(*self);
                    Ok(())
                }
            }

            impl CdrDecode for $ty {
                fn decode(dec: &mut Decoder<'_>) -> Result<Self> {
                    dec.$read()
                }
            }
        )*
    };
}

impl_number! {
    bool => write_bool, read_bool;
    i16 => write_i16, read_i16;
    u16 => write_u16, read_u16;
    i32 => write_i32, read_i32;
    u32 => write_u32, read_u32;
    i64 => write_i64, read_i64;
    u64 => write_u64, read_u64;
    f32 => write_f32, read_f32;
    f64 => write_f64, read_f64;
}

impl CdrEncode for u8 {
    fn encode(&self, enc: &mut Encoder) -> Result<()> {
        enc.write_octet(*self);
        Ok(())
    }

    fn encode_slice(items: &[Self], enc: &mut Encoder) -> Result<()> {
        enc.write_octets(items);
        Ok(())
    }
}

impl CdrDecode for u8 {
    fn decode(dec: &mut Decoder<'_>) -> Result<Self> {
        dec.read_octet()
    }

    fn decode_vec(len: usize, dec: &mut Decoder<'_>) -> Result<Vec<Self>> {
        Ok(dec.read_octets(len)?.to_vec())
    }
}

/// IDL4 `int8`, an octet-sized signed integer.
impl CdrEncode for i8 {
    fn encode(&self, enc: &mut Encoder) -> Result<()> {
        enc.write_octet(*self as u8);
        Ok(())
    }
}

impl CdrDecode for i8 {
    fn decode(dec: &mut Decoder<'_>) -> Result<Self> {
        Ok(dec.read_octet()? as i8)
    }
}

impl CdrEncode for str {
    fn encode(&self, enc: &mut Encoder) -> Result<()> {
        enc.write_string(self)
    }
}

impl CdrEncode for String {
    fn encode(&self, enc: &mut Encoder) -> Result<()> {
        enc.write_string(self)
    }
}

impl CdrDecode for String {
    fn decode(dec: &mut Decoder<'_>) -> Result<Self> {
        dec.read_string()
    }
}

impl<T: CdrEncode> CdrEncode for [T] {
    fn encode(&self, enc: &mut Encoder) -> Result<()> {
        enc.write_len(self.len())?;
        T::encode_slice(self, enc)
    }
}

impl<T: CdrEncode> CdrEncode for Vec<T> {
    fn encode(&self, enc: &mut Encoder) -> Result<()> {
        self.as_slice().encode(enc)
    }
}

impl<T: CdrDecode> CdrDecode for Vec<T> {
    fn decode(dec: &mut Decoder<'_>) -> Result<Self> {
        let len = dec.read_len(1)?;
        T::decode_vec(len, dec)
    }
}

impl<T: CdrEncode, const N: usize> CdrEncode for [T; N] {
    fn encode(&self, enc: &mut Encoder) -> Result<()> {
        T::encode_slice(self, enc)
    }
}

impl<T: CdrDecode, const N: usize> CdrDecode for [T; N] {
    fn decode(dec: &mut Decoder<'_>) -> Result<Self> {
        let items = T::decode_vec(N, dec)?;
        Ok(items
            .try_into()
            .unwrap_or_else(|_| unreachable!("decode_vec returned N items")))
    }
}

impl<T: CdrEncode + ?Sized> CdrEncode for &T {
    fn encode(&self, enc: &mut Encoder) -> Result<()> {
        (**self).encode(enc)
    }
}

impl<T: CdrEncode + ?Sized> CdrEncode for Box<T> {
    fn encode(&self, enc: &mut Encoder) -> Result<()> {
        (**self).encode(enc)
    }
}

impl<T: CdrDecode> CdrDecode for Box<T> {
    fn decode(dec: &mut Decoder<'_>) -> Result<Self> {
        T::decode(dec).map(Box::new)
    }
}

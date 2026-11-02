//! [`TypeSupport`] of the primitive types and the descriptors of the IDL
//! types that share a Rust type.

use std::marker::PhantomData;

use rustorb_cdr::{Decoder, Encoder, Encoding, LongDouble};

use crate::{Error, Result, TypeSupport, align_up};

fn is_v2(enc: &Encoder) -> bool {
    enc.encoding() == Encoding::Xcdr2
}

fn is_v2_in(dec: &Decoder<'_>) -> bool {
    dec.encoding() == Encoding::Xcdr2
}

macro_rules! primitive {
    ($($ty:ty, $size:expr => $write:ident, $read:ident;)*) => {
        $(
            impl TypeSupport for $ty {
                type Rust = $ty;
                const PRIMITIVE: bool = true;

                fn write(v: &$ty, enc: &mut Encoder) -> Result<()> {
                    enc.$write(*v);
                    Ok(())
                }

                fn read(dec: &mut Decoder<'_>) -> Result<$ty> {
                    Ok(dec.$read()?)
                }

                fn max_key_end(offset: usize) -> Option<usize> {
                    Some(align_up(offset, $size) + $size)
                }
            }
        )*
    };
}

primitive! {
    bool, 1 => write_bool, read_bool;
    i16, 2 => write_i16, read_i16;
    u16, 2 => write_u16, read_u16;
    i32, 4 => write_i32, read_i32;
    u32, 4 => write_u32, read_u32;
    i64, 8 => write_i64, read_i64;
    u64, 8 => write_u64, read_u64;
    f32, 4 => write_f32, read_f32;
    f64, 8 => write_f64, read_f64;
    LongDouble, 16 => write_long_double, read_long_double;
}

/// IDL `octet` (`uint8`).
impl TypeSupport for u8 {
    type Rust = u8;
    const PRIMITIVE: bool = true;

    fn write(v: &u8, enc: &mut Encoder) -> Result<()> {
        enc.write_octet(*v);
        Ok(())
    }

    fn read(dec: &mut Decoder<'_>) -> Result<u8> {
        Ok(dec.read_octet()?)
    }

    fn max_key_end(offset: usize) -> Option<usize> {
        Some(offset + 1)
    }

    fn write_slice(items: &[u8], enc: &mut Encoder) -> Result<()> {
        enc.write_octets(items);
        Ok(())
    }

    fn read_vec(len: usize, dec: &mut Decoder<'_>) -> Result<Vec<u8>> {
        Ok(dec.read_octets(len)?.to_vec())
    }
}

/// IDL `int8`.
impl TypeSupport for i8 {
    type Rust = i8;
    const PRIMITIVE: bool = true;

    fn write(v: &i8, enc: &mut Encoder) -> Result<()> {
        enc.write_octet(*v as u8);
        Ok(())
    }

    fn read(dec: &mut Decoder<'_>) -> Result<i8> {
        Ok(dec.read_octet()? as i8)
    }

    fn max_key_end(offset: usize) -> Option<usize> {
        Some(offset + 1)
    }
}

/// IDL `char` (DDS-XTypes `Char8`): a Rust `char` up to U+00FF, written as
/// one octet.
#[derive(Debug)]
pub struct Char8;

impl TypeSupport for Char8 {
    type Rust = char;
    const PRIMITIVE: bool = true;

    fn write(v: &char, enc: &mut Encoder) -> Result<()> {
        let c = u32::from(*v);
        let b = u8::try_from(c).map_err(|_| Error::InvalidChar(c))?;
        enc.write_char(b);
        Ok(())
    }

    fn read(dec: &mut Decoder<'_>) -> Result<char> {
        Ok(char::from(dec.read_char()?))
    }

    fn max_key_end(offset: usize) -> Option<usize> {
        Some(offset + 1)
    }
}

/// IDL `wchar` (DDS-XTypes `Char16`): a Rust `char` of the Basic
/// Multilingual Plane, written as one UTF-16 code unit.
#[derive(Debug)]
pub struct Char16;

impl TypeSupport for Char16 {
    type Rust = char;
    const PRIMITIVE: bool = true;

    fn write(v: &char, enc: &mut Encoder) -> Result<()> {
        let c = u32::from(*v);
        let unit = u16::try_from(c).map_err(|_| Error::InvalidChar(c))?;
        enc.write_wchar(unit)?;
        Ok(())
    }

    fn read(dec: &mut Decoder<'_>) -> Result<char> {
        let unit = dec.read_wchar()?;
        char::from_u32(u32::from(unit)).ok_or(Error::InvalidChar(u32::from(unit)))
    }

    fn max_key_end(offset: usize) -> Option<usize> {
        Some(align_up(offset, 2) + 2)
    }
}

fn check_bound(bound: usize, len: usize) -> Result<()> {
    if len > bound {
        return Err(Error::BoundExceeded { bound, len });
    }
    Ok(())
}

/// IDL `string`: a `String`, written as UTF-8.
#[derive(Debug)]
pub struct Str;

impl TypeSupport for Str {
    type Rust = String;

    fn write(v: &String, enc: &mut Encoder) -> Result<()> {
        Ok(enc.write_string(v)?)
    }

    fn read(dec: &mut Decoder<'_>) -> Result<String> {
        Ok(dec.read_string()?)
    }

    fn max_key_end(_offset: usize) -> Option<usize> {
        None
    }
}

/// IDL `string<N>`: a `String` of at most `N` octets of UTF-8.
#[derive(Debug)]
pub struct BoundedStr<const N: usize>;

impl<const N: usize> TypeSupport for BoundedStr<N> {
    type Rust = String;

    fn write(v: &String, enc: &mut Encoder) -> Result<()> {
        check_bound(N, v.len())?;
        Ok(enc.write_string(v)?)
    }

    fn read(dec: &mut Decoder<'_>) -> Result<String> {
        let s = dec.read_string()?;
        check_bound(N, s.len())?;
        Ok(s)
    }

    fn max_key_end(offset: usize) -> Option<usize> {
        // The length, the characters and the NUL.
        Some(align_up(offset, 4) + 4 + N + 1)
    }
}

/// IDL `wstring`: a `String`, written as UTF-16.
#[derive(Debug)]
pub struct WStr;

impl TypeSupport for WStr {
    type Rust = String;

    fn write(v: &String, enc: &mut Encoder) -> Result<()> {
        Ok(enc.write_wstring(v)?)
    }

    fn read(dec: &mut Decoder<'_>) -> Result<String> {
        Ok(dec.read_wstring()?)
    }

    fn max_key_end(_offset: usize) -> Option<usize> {
        None
    }
}

/// IDL `wstring<N>`: a `String` of at most `N` UTF-16 code units.
#[derive(Debug)]
pub struct BoundedWStr<const N: usize>;

impl<const N: usize> TypeSupport for BoundedWStr<N> {
    type Rust = String;

    fn write(v: &String, enc: &mut Encoder) -> Result<()> {
        check_bound(N, v.encode_utf16().count())?;
        Ok(enc.write_wstring(v)?)
    }

    fn read(dec: &mut Decoder<'_>) -> Result<String> {
        let s = dec.read_wstring()?;
        check_bound(N, s.encode_utf16().count())?;
        Ok(s)
    }

    fn max_key_end(offset: usize) -> Option<usize> {
        // The length in octets and the code units, without a NUL.
        Some(align_up(offset, 4) + 4 + 2 * N)
    }
}

fn write_seq<D: TypeSupport>(v: &[D::Rust], enc: &mut Encoder, key: bool) -> Result<()> {
    let mark = (is_v2(enc) && !D::PRIMITIVE).then(|| enc.begin_dheader());
    enc.write_len(v.len())?;
    if key {
        D::write_key_slice(v, enc)?;
    } else {
        D::write_slice(v, enc)?;
    }
    if let Some(mark) = mark {
        enc.end_dheader(mark)?;
    }
    Ok(())
}

fn read_seq<D: TypeSupport>(dec: &mut Decoder<'_>, bound: Option<usize>) -> Result<Vec<D::Rust>> {
    let delimited = if is_v2_in(dec) && !D::PRIMITIVE {
        Some(dec.read_dheader()?)
    } else {
        None
    };
    let len = dec.read_len(1)?;
    if let Some(bound) = bound {
        check_bound(bound, len)?;
    }
    let items = D::read_vec(len, dec)?;
    if let Some(delimited) = delimited {
        dec.end_dheader(delimited)?;
    }
    Ok(items)
}

/// The offset after `n` elements of `D` written at `offset`.
fn max_elements_end<D: TypeSupport>(mut offset: usize, n: usize) -> Option<usize> {
    if n == 0 {
        return Some(offset);
    }
    if D::ARRAY_ELEMENT_PRIMITIVE && D::PRIMITIVE {
        // After the first, the elements follow each other without padding.
        let first = D::max_key_end_in_array(offset)?;
        let size = D::max_key_end_in_array(0)?;
        return first.checked_add(size.checked_mul(n - 1)?);
    }
    for _ in 0..n {
        offset = D::max_key_end_in_array(offset)?;
    }
    Some(offset)
}

/// IDL `sequence<T>`: a `Vec`, with a DHEADER in XCDR2 when `T` is not
/// primitive (DDS-XTypes 7.4.3.5.3 rules 11 to 13).
#[derive(Debug)]
pub struct Seq<D>(PhantomData<D>);

impl<D: TypeSupport> TypeSupport for Seq<D> {
    type Rust = Vec<D::Rust>;

    fn write(v: &Vec<D::Rust>, enc: &mut Encoder) -> Result<()> {
        write_seq::<D>(v, enc, false)
    }

    fn write_key(v: &Vec<D::Rust>, enc: &mut Encoder) -> Result<()> {
        write_seq::<D>(v, enc, true)
    }

    fn read(dec: &mut Decoder<'_>) -> Result<Vec<D::Rust>> {
        read_seq::<D>(dec, None)
    }

    fn max_key_end(_offset: usize) -> Option<usize> {
        None
    }
}

/// IDL `sequence<T, N>`: a `Vec` of at most `N` elements.
#[derive(Debug)]
pub struct BoundedSeq<D, const N: usize>(PhantomData<D>);

impl<D: TypeSupport, const N: usize> TypeSupport for BoundedSeq<D, N> {
    type Rust = Vec<D::Rust>;

    fn write(v: &Vec<D::Rust>, enc: &mut Encoder) -> Result<()> {
        check_bound(N, v.len())?;
        write_seq::<D>(v, enc, false)
    }

    fn write_key(v: &Vec<D::Rust>, enc: &mut Encoder) -> Result<()> {
        check_bound(N, v.len())?;
        write_seq::<D>(v, enc, true)
    }

    fn read(dec: &mut Decoder<'_>) -> Result<Vec<D::Rust>> {
        read_seq::<D>(dec, Some(N))
    }

    fn max_key_end(offset: usize) -> Option<usize> {
        let dheader = if D::PRIMITIVE { 0 } else { 4 };
        let start = align_up(offset, 4) + dheader + 4;
        if D::PRIMITIVE {
            return max_elements_end::<D>(start, N);
        }
        let mut offset = start;
        for _ in 0..N {
            offset = D::max_key_end(offset)?;
        }
        Some(offset)
    }
}

fn write_array<D: TypeSupport, const N: usize>(
    v: &[D::Rust; N],
    enc: &mut Encoder,
    key: bool,
) -> Result<()> {
    let mark = (is_v2(enc) && !D::ARRAY_ELEMENT_PRIMITIVE).then(|| enc.begin_dheader());
    <Array<D, N> as TypeSupport>::write_in_array(v, enc, key)?;
    if let Some(mark) = mark {
        enc.end_dheader(mark)?;
    }
    Ok(())
}

/// IDL array `T[N]` (arrays of arrays are arrays of this): a Rust array,
/// with a DHEADER in XCDR2 when the innermost element type is not
/// primitive (DDS-XTypes 7.4.3.5.3 rules 8 to 10).
#[derive(Debug)]
pub struct Array<D, const N: usize>(PhantomData<D>);

impl<D: TypeSupport, const N: usize> TypeSupport for Array<D, N> {
    type Rust = [D::Rust; N];
    const ARRAY_ELEMENT_PRIMITIVE: bool = D::ARRAY_ELEMENT_PRIMITIVE;

    fn write(v: &[D::Rust; N], enc: &mut Encoder) -> Result<()> {
        write_array::<D, N>(v, enc, false)
    }

    fn write_key(v: &[D::Rust; N], enc: &mut Encoder) -> Result<()> {
        write_array::<D, N>(v, enc, true)
    }

    fn read(dec: &mut Decoder<'_>) -> Result<[D::Rust; N]> {
        let delimited = if is_v2_in(dec) && !Self::ARRAY_ELEMENT_PRIMITIVE {
            Some(dec.read_dheader()?)
        } else {
            None
        };
        let v = Self::read_in_array(dec)?;
        if let Some(delimited) = delimited {
            dec.end_dheader(delimited)?;
        }
        Ok(v)
    }

    fn max_key_end(offset: usize) -> Option<usize> {
        let start = if Self::ARRAY_ELEMENT_PRIMITIVE {
            offset
        } else {
            align_up(offset, 4) + 4
        };
        Self::max_key_end_in_array(start)
    }

    fn write_in_array(v: &[D::Rust; N], enc: &mut Encoder, key: bool) -> Result<()> {
        v.iter()
            .try_for_each(|item| D::write_in_array(item, enc, key))
    }

    fn read_in_array(dec: &mut Decoder<'_>) -> Result<[D::Rust; N]> {
        let mut items = Vec::with_capacity(N);
        for _ in 0..N {
            items.push(D::read_in_array(dec)?);
        }
        Ok(items
            .try_into()
            .unwrap_or_else(|_| unreachable!("N elements were read")))
    }

    fn max_key_end_in_array(offset: usize) -> Option<usize> {
        max_elements_end::<D>(offset, N)
    }
}

/// An `@external` member: a `Box`, written as its content (DDS-XTypes
/// 7.3.1.2.1.4).
#[derive(Debug)]
pub struct External<D>(PhantomData<D>);

impl<D: TypeSupport> TypeSupport for External<D> {
    type Rust = Box<D::Rust>;
    const PRIMITIVE: bool = D::PRIMITIVE;
    const ARRAY_ELEMENT_PRIMITIVE: bool = D::ARRAY_ELEMENT_PRIMITIVE;

    fn write(v: &Box<D::Rust>, enc: &mut Encoder) -> Result<()> {
        D::write(v, enc)
    }

    fn read(dec: &mut Decoder<'_>) -> Result<Box<D::Rust>> {
        D::read(dec).map(Box::new)
    }

    fn write_key(v: &Box<D::Rust>, enc: &mut Encoder) -> Result<()> {
        D::write_key(v, enc)
    }

    fn max_key_end(offset: usize) -> Option<usize> {
        D::max_key_end(offset)
    }
}

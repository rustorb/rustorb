//! The runtime of the code `rustorb-idl -brust` generates (see
//! `docs/rust-mapping.md`).
//!
//! Every IDL type has a descriptor, a type that implements [`IdlType`]:
//! it names the Rust type of the values ([`IdlType::Rust`]), makes the
//! TypeCode and marshals the values. The types the generator makes for
//! structs, unions, enums, exceptions, interfaces and valuetypes are their
//! own descriptors; the descriptors of the basic and anonymous types are
//! here ([`Long`], [`WString`], [`Sequence`], ...), and those of typedefs
//! are in the `alias` module the generator makes next to them.
//!
//! The same Rust type can stand for several IDL types (`String` is both
//! `string` and `wstring`, `char` both `char` and `wchar`); the descriptor
//! says which. [`Any::insert`](crate::Any::insert) and
//! [`Any::extract`](crate::Any::extract) take one.

use std::borrow::Borrow;
use std::marker::PhantomData;

use rustorb_cdr::{ByteOrder, CdrEncode, CdrVersion, Decoder, Encoder};

use crate::{
    CompletionStatus, Exception, InStream, ObjectRef, OutStream, SystemException,
    SystemExceptionKind, TCKind, TypeCode, minor,
};

mod abstract_iface;
mod value;

pub use abstract_iface::{
    AbstractValue, SupportedFn, Upcast, read_abstract, register_supported, supported,
    write_abstract_object, write_abstract_value,
};
pub use value::{
    CustomMarshal, DataInputStream, DataOutputStream, ValueHandle, ValueInfo, ValueState, read_box,
    register_value, write_box,
};

/// Referred to by every generated file: a generator that writes code for
/// another version of this runtime fails to compile.
pub const GENERATED_CODE_VERSION_1: () = ();

/// The result of marshalling.
pub type Sys<T> = Result<T, SystemException>;

/// The descriptor of an IDL type.
pub trait IdlType: 'static {
    /// The Rust type of the values.
    type Rust: Borrow<Self::Borrowed> + 'static;
    /// What is written: `str` for strings, a slice for sequences, else
    /// [`Rust`](IdlType::Rust) itself.
    type Borrowed: ?Sized;
    /// The smallest size of a value on the wire, to check the lengths of
    /// sequences before reading them.
    const MIN_SIZE: usize;

    /// The TypeCode of the type.
    fn type_code() -> TypeCode;

    /// Writes a value.
    fn write(v: &Self::Borrowed, out: &mut OutStream<'_>) -> Sys<()>;

    /// Reads a value.
    fn read(input: &mut InStream<'_>) -> Sys<Self::Rust>;

    /// Writes the elements of a sequence or an array.
    fn write_items(items: &[Self::Rust], out: &mut OutStream<'_>) -> Sys<()> {
        items.iter().try_for_each(|v| Self::write(v.borrow(), out))
    }

    /// Reads `len` elements of a sequence or an array.
    fn read_items(len: usize, input: &mut InStream<'_>) -> Sys<Vec<Self::Rust>> {
        let mut items = Vec::with_capacity(len.min(4096));
        for _ in 0..len {
            items.push(Self::read(input)?);
        }
        Ok(items)
    }
}

// --- errors ---------------------------------------------------------------

/// `BAD_PARAM`, not completed: a value that does not fit its type.
pub fn bad_param() -> SystemException {
    SystemException::new(SystemExceptionKind::BAD_PARAM, 0, CompletionStatus::No)
}

/// `DATA_CONVERSION`, not completed: a character or a number the type
/// cannot hold.
pub fn data_conversion() -> SystemException {
    SystemException::new(
        SystemExceptionKind::DATA_CONVERSION,
        0,
        CompletionStatus::No,
    )
}

/// `BAD_OPERATION` with minor `UNKNOWN_OPERATION`: what a skeleton
/// returns for an operation the interface does not have.
pub fn unknown_operation() -> SystemException {
    SystemException::new(
        SystemExceptionKind::BAD_OPERATION,
        minor::UNKNOWN_OPERATION,
        CompletionStatus::No,
    )
}

/// The system exception a call that raises no user exceptions fails
/// with: a user exception is `UNKNOWN` (minor `UNLISTED_USER_EXCEPTION`).
pub fn system_exception(e: Exception) -> SystemException {
    match e {
        Exception::System(e) => e,
        Exception::User(_) => unlisted_user_exception(),
    }
}

/// `UNKNOWN` with minor `UNLISTED_USER_EXCEPTION`, for a user exception
/// the operation does not raise.
pub fn unlisted_user_exception() -> SystemException {
    SystemException::new(
        SystemExceptionKind::UNKNOWN,
        minor::UNLISTED_USER_EXCEPTION,
        CompletionStatus::Maybe,
    )
}

/// Whether `obj` supports the interface `repository_id`: from the type id
/// of its IOR when it is that one, else by asking the object (`_is_a`).
pub fn is_a(obj: &ObjectRef, repository_id: &str) -> Sys<bool> {
    if obj.type_id() == repository_id {
        return Ok(true);
    }
    obj.is_a(repository_id).map_err(system_exception)
}

/// The TypeCode of a basic type.
pub fn simple_tc(kind: TCKind) -> TypeCode {
    TypeCode::simple(kind).expect("a simple kind")
}

/// Unwraps the result of making a TypeCode the generator checked.
pub fn tc(r: Result<TypeCode, crate::TypeCodeError>) -> TypeCode {
    r.expect("a TypeCode checked by rustorb-idl")
}

// --- basic types ----------------------------------------------------------

macro_rules! numeric {
    ($($(#[$doc:meta])* $name:ident: $ty:ty, $kind:ident, $size:expr;)*) => {
        $(
            $(#[$doc])*
            #[derive(Debug)]
            pub struct $name;

            impl IdlType for $name {
                type Rust = $ty;
                type Borrowed = $ty;
                const MIN_SIZE: usize = $size;

                fn type_code() -> TypeCode {
                    simple_tc(TCKind::$kind)
                }

                fn write(v: &$ty, out: &mut OutStream<'_>) -> Sys<()> {
                    Ok(v.encode(out)?)
                }

                fn read(input: &mut InStream<'_>) -> Sys<$ty> {
                    input.read::<$ty>()
                }
            }
        )*
    };
}

numeric! {
    /// `boolean`: `bool`.
    Boolean: bool, Boolean, 1;
    /// `short`: `i16`.
    Short: i16, Short, 2;
    /// `unsigned short`: `u16`.
    UShort: u16, UShort, 2;
    /// `long`: `i32`.
    Long: i32, Long, 4;
    /// `unsigned long`: `u32`.
    ULong: u32, ULong, 4;
    /// `long long`: `i64`.
    LongLong: i64, LongLong, 8;
    /// `unsigned long long`: `u64`.
    ULongLong: u64, ULongLong, 8;
    /// `float`: `f32`.
    Float: f32, Float, 4;
    /// `double`: `f64`.
    Double: f64, Double, 8;
    /// `long double`: [`LongDouble`](crate::LongDouble), the binary128 bits.
    LongDouble: crate::LongDouble, LongDouble, 16;
}

/// `octet`: `u8`. Sequences and arrays of octets are copied at once.
#[derive(Debug)]
pub struct Octet;

impl IdlType for Octet {
    type Rust = u8;
    type Borrowed = u8;
    const MIN_SIZE: usize = 1;

    fn type_code() -> TypeCode {
        simple_tc(TCKind::Octet)
    }

    fn write(v: &u8, out: &mut OutStream<'_>) -> Sys<()> {
        out.write_octet(*v);
        Ok(())
    }

    fn read(input: &mut InStream<'_>) -> Sys<u8> {
        input.read::<u8>()
    }

    fn write_items(items: &[u8], out: &mut OutStream<'_>) -> Sys<()> {
        out.write_octets(items);
        Ok(())
    }

    fn read_items(len: usize, input: &mut InStream<'_>) -> Sys<Vec<u8>> {
        let e = input.marshal_error();
        Ok(input.read_octets(len).map_err(|_| e)?.to_vec())
    }
}

/// `char`: `char`, one octet in the negotiated `char` code set; a
/// character that is not is `DATA_CONVERSION`.
#[derive(Debug)]
pub struct Char;

impl IdlType for Char {
    type Rust = char;
    type Borrowed = char;
    const MIN_SIZE: usize = 1;

    fn type_code() -> TypeCode {
        simple_tc(TCKind::Char)
    }

    fn write(v: &char, out: &mut OutStream<'_>) -> Sys<()> {
        let bytes = out.code_sets().encode_string(v.encode_utf8(&mut [0; 4]))?;
        match bytes[..] {
            [b] => {
                out.write_char(b);
                Ok(())
            }
            _ => Err(data_conversion()),
        }
    }

    fn read(input: &mut InStream<'_>) -> Sys<char> {
        input.read_char_value()
    }
}

/// `wchar`: `char`, one UTF-16 code unit; a character outside the Basic
/// Multilingual Plane is `DATA_CONVERSION`, as is a lone surrogate
/// received.
#[derive(Debug)]
pub struct WChar;

impl IdlType for WChar {
    type Rust = char;
    type Borrowed = char;
    const MIN_SIZE: usize = 1;

    fn type_code() -> TypeCode {
        simple_tc(TCKind::WChar)
    }

    fn write(v: &char, out: &mut OutStream<'_>) -> Sys<()> {
        let unit = u16::try_from(u32::from(*v)).map_err(|_| data_conversion())?;
        out.write_wchar(unit)
    }

    fn read(input: &mut InStream<'_>) -> Sys<char> {
        let unit = input.read_wchar()?;
        char::from_u32(u32::from(unit)).ok_or_else(|| {
            SystemException::new(
                SystemExceptionKind::DATA_CONVERSION,
                0,
                input.completion_status(),
            )
        })
    }
}

/// `string`: `String`.
#[derive(Debug)]
pub struct String;

impl IdlType for String {
    type Rust = std::string::String;
    type Borrowed = str;
    const MIN_SIZE: usize = 4;

    fn type_code() -> TypeCode {
        TypeCode::string(0)
    }

    fn write(v: &str, out: &mut OutStream<'_>) -> Sys<()> {
        out.write_string(v)
    }

    fn read(input: &mut InStream<'_>) -> Sys<std::string::String> {
        input.read_string()
    }
}

/// `wstring`: `String`.
#[derive(Debug)]
pub struct WString;

impl IdlType for WString {
    type Rust = std::string::String;
    type Borrowed = str;
    const MIN_SIZE: usize = 4;

    fn type_code() -> TypeCode {
        TypeCode::wstring(0)
    }

    fn write(v: &str, out: &mut OutStream<'_>) -> Sys<()> {
        out.write_wstring(v)
    }

    fn read(input: &mut InStream<'_>) -> Sys<std::string::String> {
        input.read_wstring()
    }
}

fn check_len_out(len: usize, bound: u32) -> Sys<()> {
    if len > bound as usize {
        return Err(bad_param());
    }
    Ok(())
}

fn check_len_in(len: usize, bound: u32, input: &InStream<'_>) -> Sys<()> {
    if len > bound as usize {
        return Err(input.marshal_error());
    }
    Ok(())
}

/// `string<N>`: `String` of at most `N` characters. A longer one is
/// `BAD_PARAM` when sent and `MARSHAL` when received.
#[derive(Debug)]
pub struct BoundedString<const N: u32>;

impl<const N: u32> IdlType for BoundedString<N> {
    type Rust = std::string::String;
    type Borrowed = str;
    const MIN_SIZE: usize = 4;

    fn type_code() -> TypeCode {
        TypeCode::string(N)
    }

    fn write(v: &str, out: &mut OutStream<'_>) -> Sys<()> {
        check_len_out(v.chars().count(), N)?;
        out.write_string(v)
    }

    fn read(input: &mut InStream<'_>) -> Sys<std::string::String> {
        let s = input.read_string()?;
        check_len_in(s.chars().count(), N, input)?;
        Ok(s)
    }
}

/// `wstring<N>`: `String` of at most `N` characters.
#[derive(Debug)]
pub struct BoundedWString<const N: u32>;

impl<const N: u32> IdlType for BoundedWString<N> {
    type Rust = std::string::String;
    type Borrowed = str;
    const MIN_SIZE: usize = 4;

    fn type_code() -> TypeCode {
        TypeCode::wstring(N)
    }

    fn write(v: &str, out: &mut OutStream<'_>) -> Sys<()> {
        check_len_out(v.chars().count(), N)?;
        out.write_wstring(v)
    }

    fn read(input: &mut InStream<'_>) -> Sys<std::string::String> {
        let s = input.read_wstring()?;
        check_len_in(s.chars().count(), N, input)?;
        Ok(s)
    }
}

/// `sequence<T>`: `Vec`.
#[derive(Debug)]
pub struct Sequence<T>(PhantomData<T>);

impl<T: IdlType> IdlType for Sequence<T> {
    type Rust = Vec<T::Rust>;
    type Borrowed = [T::Rust];
    const MIN_SIZE: usize = 4;

    fn type_code() -> TypeCode {
        tc(TypeCode::sequence(&T::type_code(), 0))
    }

    fn write(v: &[T::Rust], out: &mut OutStream<'_>) -> Sys<()> {
        out.write_len(v.len())?;
        T::write_items(v, out)
    }

    fn read(input: &mut InStream<'_>) -> Sys<Vec<T::Rust>> {
        let len = read_len(input, T::MIN_SIZE)?;
        T::read_items(len, input)
    }
}

/// `sequence<T, N>`: `Vec` of at most `N` elements. A longer one is
/// `BAD_PARAM` when sent and `MARSHAL` when received.
#[derive(Debug)]
pub struct BoundedSequence<T, const N: u32>(PhantomData<T>);

impl<T: IdlType, const N: u32> IdlType for BoundedSequence<T, N> {
    type Rust = Vec<T::Rust>;
    type Borrowed = [T::Rust];
    const MIN_SIZE: usize = 4;

    fn type_code() -> TypeCode {
        tc(TypeCode::sequence(&T::type_code(), N))
    }

    fn write(v: &[T::Rust], out: &mut OutStream<'_>) -> Sys<()> {
        check_len_out(v.len(), N)?;
        out.write_len(v.len())?;
        T::write_items(v, out)
    }

    fn read(input: &mut InStream<'_>) -> Sys<Vec<T::Rust>> {
        let len = read_len(input, T::MIN_SIZE)?;
        check_len_in(len, N, input)?;
        T::read_items(len, input)
    }
}

/// Reads the length of a sequence whose elements take at least
/// `min_size` octets.
pub fn read_len(input: &mut InStream<'_>, min_size: usize) -> Sys<usize> {
    let e = input.marshal_error();
    input.read_len(min_size.max(1)).map_err(|_| e)
}

/// `T x[N]`: `[T; N]`. A multidimensional array is an array of arrays.
#[derive(Debug)]
pub struct Array<T, const N: usize>(PhantomData<T>);

impl<T: IdlType, const N: usize> IdlType for Array<T, N> {
    type Rust = [T::Rust; N];
    type Borrowed = [T::Rust; N];
    const MIN_SIZE: usize = T::MIN_SIZE.saturating_mul(N);

    fn type_code() -> TypeCode {
        let n = u32::try_from(N).expect("array length checked by rustorb-idl");
        tc(TypeCode::array(&T::type_code(), n))
    }

    fn write(v: &[T::Rust; N], out: &mut OutStream<'_>) -> Sys<()> {
        T::write_items(v, out)
    }

    fn read(input: &mut InStream<'_>) -> Sys<[T::Rust; N]> {
        let items = T::read_items(N, input)?;
        items.try_into().map_err(|_| input.marshal_error())
    }
}

/// `Object`: `Option<ObjectRef>`, `None` for a nil reference.
#[derive(Debug)]
pub struct Object;

impl IdlType for Object {
    type Rust = Option<ObjectRef>;
    type Borrowed = Option<ObjectRef>;
    const MIN_SIZE: usize = 4;

    fn type_code() -> TypeCode {
        TypeCode::object()
    }

    fn write(v: &Option<ObjectRef>, out: &mut OutStream<'_>) -> Sys<()> {
        out.write_object(v.as_ref())
    }

    fn read(input: &mut InStream<'_>) -> Sys<Option<ObjectRef>> {
        input.read_object()
    }
}

/// `any`: [`crate::Any`].
#[derive(Debug)]
pub struct Any;

impl IdlType for Any {
    type Rust = crate::Any;
    type Borrowed = crate::Any;
    const MIN_SIZE: usize = 4;

    fn type_code() -> TypeCode {
        simple_tc(TCKind::Any)
    }

    fn write(v: &crate::Any, out: &mut OutStream<'_>) -> Sys<()> {
        out.write_any(v)
    }

    fn read(input: &mut InStream<'_>) -> Sys<crate::Any> {
        input.read_any()
    }
}

/// `TypeCode`: [`crate::TypeCode`].
#[derive(Debug)]
pub struct TypeCodeType;

impl IdlType for TypeCodeType {
    type Rust = TypeCode;
    type Borrowed = TypeCode;
    const MIN_SIZE: usize = 4;

    fn type_code() -> TypeCode {
        simple_tc(TCKind::TypeCode)
    }

    fn write(v: &TypeCode, out: &mut OutStream<'_>) -> Sys<()> {
        out.write_typecode(v)
    }

    fn read(input: &mut InStream<'_>) -> Sys<TypeCode> {
        input.read_typecode()
    }
}

/// `fixed<D, S>`: [`crate::Fixed`]. A value is sent with digits after `S`
/// cut off; an integer part longer than `D - S` digits is
/// `DATA_CONVERSION`.
#[derive(Debug)]
pub struct Fixed<const D: u16, const S: u16>;

impl<const D: u16, const S: u16> IdlType for Fixed<D, S> {
    type Rust = crate::Fixed;
    type Borrowed = crate::Fixed;
    const MIN_SIZE: usize = (D as usize + 2) / 2;

    fn type_code() -> TypeCode {
        let scale = i16::try_from(S).expect("scale checked by rustorb-idl");
        tc(TypeCode::fixed(D, scale))
    }

    fn write(v: &crate::Fixed, out: &mut OutStream<'_>) -> Sys<()> {
        let digits = v.to_digits(D, S).ok_or_else(data_conversion)?;
        out.write_fixed(&digits, v.is_negative())
    }

    fn read(input: &mut InStream<'_>) -> Sys<crate::Fixed> {
        let (digits, negative) = input.read_fixed(D as usize)?;
        crate::Fixed::from_digits(&digits, S, negative).ok_or_else(|| input.marshal_error())
    }
}

// --- Any ------------------------------------------------------------------

impl crate::Any {
    /// An Any holding `v` as a value of the IDL type `T` (a descriptor):
    /// `Any::insert::<Point>(&p)`, `Any::insert::<idl::WString>("w")`.
    /// A value that does not fit the type is `BAD_PARAM` (or
    /// `DATA_CONVERSION`).
    pub fn insert<T: IdlType>(v: &T::Borrowed) -> Sys<crate::Any> {
        let mut enc = Encoder::new(ByteOrder::Big, CdrVersion::Giop1_2);
        let mut out = OutStream::standalone(&mut enc);
        T::write(v, &mut out)?;
        let orb = out.orb.take();
        let bytes = enc.into_bytes();
        let dec = Decoder::new(&bytes, ByteOrder::Big, CdrVersion::Giop1_2);
        let mut input = InStream::standalone(dec, orb.as_ref());
        let tc = T::type_code();
        let value = input.read_value(&tc)?;
        Ok(crate::Any::new(tc, value))
    }

    /// The value of the Any as a value of the IDL type `T`, if the Any
    /// holds one: its TypeCode is equivalent to `T`'s.
    pub fn extract<T: IdlType>(&self) -> Option<T::Rust> {
        if !self.typecode().equivalent(&T::type_code()) {
            return None;
        }
        let mut enc = Encoder::new(ByteOrder::Big, CdrVersion::Giop1_2);
        let mut out = OutStream::standalone(&mut enc);
        out.write_value(self.typecode(), self.value()).ok()?;
        let orb = out.orb.take();
        let bytes = enc.into_bytes();
        let dec = Decoder::new(&bytes, ByteOrder::Big, CdrVersion::Giop1_2);
        let mut input = InStream::standalone(dec, orb.as_ref());
        T::read(&mut input).ok()
    }
}

#[cfg(test)]
mod tests;

//! Values of any IDL type, described by a [`TypeCode`], and [`Any`].
//!
//! [`Value`] is the form the language bindings marshal through: they
//! convert their own values to it and the ORB writes it as the TypeCode
//! says, and the other way when reading. An [`Any`] is a TypeCode with a
//! value of that type.

use rustorb_cdr::CdrEncode;

use crate::typecode::{TCKind, TypeCode};
use crate::{
    CompletionStatus, InStream, ObjectRef, OutStream, SystemException, SystemExceptionKind,
};

/// How deep values may nest when read, against stack exhaustion with
/// recursive types.
const MAX_DEPTH: usize = 256;

/// A value of an IDL type. Which variant goes with which type:
///
/// | Type | Value |
/// | --- | --- |
/// | `void`, `null` | `Void` |
/// | integers, `float`, `double`, `boolean`, `octet` | the variant of the same name |
/// | `char`, `wchar` | `Char` (a character of the `char` code set), `WChar` (a UTF-16 code unit) |
/// | `string`, `wstring` | `String` |
/// | `sequence` and array of `octet`, `Principal` | `Octets` (or `List` of `Octet`) |
/// | `sequence` and array of `char` | `Chars` (or `List` of `Char`) |
/// | other `sequence`s and arrays, struct, exception | `List`: the elements or the members in order (an exception is written with its repository id first, as in a reply) |
/// | union | `Union`: the discriminator, and the index of the TypeCode member with its value, `None` when no member is selected |
/// | enum | `Enum`: the position of the enumerator |
/// | `fixed` | `Fixed`: the digits (as many as the type has, most significant first) and the sign |
/// | object reference | `Objref`, `None` for nil |
/// | `TypeCode` | `TypeCode` |
/// | `any` | `Any` |
///
/// Aliases take the value of the type they stand for. Valuetypes, abstract
/// interfaces and `long double` are not supported (`NO_IMPLEMENT`).
#[derive(Debug, Clone)]
#[allow(missing_docs)]
pub enum Value {
    Void,
    Bool(bool),
    Octet(u8),
    Short(i16),
    UShort(u16),
    Long(i32),
    ULong(u32),
    LongLong(i64),
    ULongLong(u64),
    Float(f32),
    Double(f64),
    Char(char),
    WChar(u16),
    String(String),
    Octets(Vec<u8>),
    Chars(String),
    List(Vec<Value>),
    Union {
        disc: Box<Value>,
        member: Option<(u32, Box<Value>)>,
    },
    Enum(u32),
    Fixed {
        digits: Vec<u8>,
        negative: bool,
    },
    Objref(Option<ObjectRef>),
    TypeCode(TypeCode),
    Any(Box<Any>),
}

impl Value {
    /// The number a union discriminator value stands for, as in union
    /// labels ([`TypeCode::union_tc`]): integers as they are, `boolean` as
    /// 0 or 1, `char` and `wchar` as their code, an enumerator as its
    /// position. `None` for values that cannot be discriminators.
    pub fn label_key(&self) -> Option<i128> {
        Some(match *self {
            Value::Bool(v) => i128::from(v),
            Value::Octet(v) => i128::from(v),
            Value::Short(v) => i128::from(v),
            Value::UShort(v) => i128::from(v),
            Value::Long(v) => i128::from(v),
            Value::ULong(v) => i128::from(v),
            Value::LongLong(v) => i128::from(v),
            Value::ULongLong(v) => i128::from(v),
            Value::Char(c) => i128::from(u32::from(c)),
            Value::WChar(c) => i128::from(c),
            Value::Enum(v) => i128::from(v),
            _ => return None,
        })
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Value) -> bool {
        use Value::*;
        match (self, other) {
            (Void, Void) => true,
            (Bool(a), Bool(b)) => a == b,
            (Octet(a), Octet(b)) => a == b,
            (Short(a), Short(b)) => a == b,
            (UShort(a), UShort(b)) => a == b,
            (Long(a), Long(b)) => a == b,
            (ULong(a), ULong(b)) => a == b,
            (LongLong(a), LongLong(b)) => a == b,
            (ULongLong(a), ULongLong(b)) => a == b,
            (Float(a), Float(b)) => a == b,
            (Double(a), Double(b)) => a == b,
            (Char(a), Char(b)) => a == b,
            (WChar(a), WChar(b)) => a == b,
            (String(a), String(b)) | (Chars(a), Chars(b)) => a == b,
            (Octets(a), Octets(b)) => a == b,
            (Octets(a), List(b)) | (List(b), Octets(a)) => {
                a.len() == b.len() && a.iter().zip(b).all(|(x, y)| *y == Octet(*x))
            }
            (Chars(a), List(b)) | (List(b), Chars(a)) => {
                a.chars().count() == b.len() && a.chars().zip(b).all(|(x, y)| *y == Char(x))
            }
            (List(a), List(b)) => a == b,
            (
                Union {
                    disc: d1,
                    member: m1,
                },
                Union {
                    disc: d2,
                    member: m2,
                },
            ) => d1 == d2 && m1 == m2,
            (Enum(a), Enum(b)) => a == b,
            (
                Fixed {
                    digits: d1,
                    negative: n1,
                },
                Fixed {
                    digits: d2,
                    negative: n2,
                },
            ) => d1 == d2 && n1 == n2,
            (Objref(a), Objref(b)) => match (a, b) {
                (None, None) => true,
                (Some(a), Some(b)) => a.is_equivalent(b),
                _ => false,
            },
            (TypeCode(a), TypeCode(b)) => a.equal(b),
            (Any(a), Any(b)) => a == b,
            _ => false,
        }
    }
}

/// A value with its type (`CORBA::Any`). The default is an empty Any, of
/// type `null`.
///
/// `Any::new` does not check that the value fits the type; writing an Any
/// that does not fails with `BAD_PARAM`.
#[derive(Debug, Clone, PartialEq)]
pub struct Any {
    tc: TypeCode,
    value: Value,
}

impl Default for Any {
    fn default() -> Any {
        Any::new(TypeCode::null(), Value::Void)
    }
}

impl Any {
    /// An Any of type `tc` holding `value`.
    pub fn new(tc: TypeCode, value: Value) -> Any {
        Any { tc, value }
    }

    /// The type of the value.
    pub fn typecode(&self) -> &TypeCode {
        &self.tc
    }

    /// The value.
    pub fn value(&self) -> &Value {
        &self.value
    }

    /// The type and the value.
    pub fn into_parts(self) -> (TypeCode, Value) {
        (self.tc, self.value)
    }
}

macro_rules! any_from {
    ($($ty:ty => $kind:ident, $variant:ident;)*) => {
        $(
            impl From<$ty> for Any {
                fn from(v: $ty) -> Any {
                    Any::new(TypeCode::simple(TCKind::$kind).expect("simple kind"), Value::$variant(v))
                }
            }
        )*
    };
}

any_from! {
    bool => Boolean, Bool;
    u8 => Octet, Octet;
    i16 => Short, Short;
    u16 => UShort, UShort;
    i32 => Long, Long;
    u32 => ULong, ULong;
    i64 => LongLong, LongLong;
    u64 => ULongLong, ULongLong;
    f32 => Float, Float;
    f64 => Double, Double;
    char => Char, Char;
}

impl From<&str> for Any {
    fn from(v: &str) -> Any {
        Any::new(TypeCode::string(0), Value::String(v.to_owned()))
    }
}

impl From<String> for Any {
    fn from(v: String) -> Any {
        Any::new(TypeCode::string(0), Value::String(v))
    }
}

impl From<TypeCode> for Any {
    fn from(v: TypeCode) -> Any {
        Any::new(
            TypeCode::simple(TCKind::TypeCode).expect("simple kind"),
            Value::TypeCode(v),
        )
    }
}

fn sys(kind: SystemExceptionKind, completed: CompletionStatus) -> SystemException {
    SystemException::new(kind, 0, completed)
}

fn bad_param() -> SystemException {
    sys(SystemExceptionKind::BAD_PARAM, CompletionStatus::No)
}

// --- writing ---------------------------------------------------------------------

/// Writes `value` of type `tc`. A value that does not fit the type is
/// `BAD_PARAM`, a character the code set cannot represent
/// `DATA_CONVERSION`.
pub(crate) fn write(
    out: &mut OutStream<'_>,
    tc: &TypeCode,
    value: &Value,
) -> Result<(), SystemException> {
    let tc = tc.unaliased();
    let kind = tc.kind();
    match (kind, value) {
        (TCKind::Null | TCKind::Void, _) => {}
        (TCKind::Short, Value::Short(v)) => v.encode(out)?,
        (TCKind::Long, Value::Long(v)) => v.encode(out)?,
        (TCKind::UShort, Value::UShort(v)) => v.encode(out)?,
        (TCKind::ULong, Value::ULong(v)) => v.encode(out)?,
        (TCKind::LongLong, Value::LongLong(v)) => v.encode(out)?,
        (TCKind::ULongLong, Value::ULongLong(v)) => v.encode(out)?,
        (TCKind::Float, Value::Float(v)) => v.encode(out)?,
        (TCKind::Double, Value::Double(v)) => v.encode(out)?,
        (TCKind::Boolean, Value::Bool(v)) => v.encode(out)?,
        (TCKind::Octet, Value::Octet(v)) => out.write_octet(*v),
        (TCKind::Char, Value::Char(c)) => write_char(out, *c)?,
        (TCKind::WChar, Value::WChar(c)) => out.write_wchar(*c)?,
        (TCKind::String, Value::String(s)) => {
            check_bound(s.chars().count(), tc.length().unwrap_or(0))?;
            out.write_string(s)?;
        }
        (TCKind::WString, Value::String(s)) => {
            check_bound(s.chars().count(), tc.length().unwrap_or(0))?;
            out.write_wstring(s)?;
        }
        (TCKind::Principal, Value::Octets(b)) => out.write_octet_seq(b)?,
        (TCKind::Sequence | TCKind::Array, _) => write_items(out, &tc, value)?,
        (TCKind::Struct | TCKind::Except, Value::List(items)) => {
            let count = tc.member_count().unwrap_or(0);
            if items.len() != count as usize {
                return Err(bad_param());
            }
            if kind == TCKind::Except {
                // An exception starts with its repository id, as in a
                // reply.
                out.write_string(tc.id().unwrap_or_default())?;
            }
            for (i, item) in items.iter().enumerate() {
                write(out, &member(&tc, i as u32), item)?;
            }
        }
        (TCKind::Union, Value::Union { disc, member: m }) => {
            let disc_tc = tc.discriminator_type().map_err(|_| bad_param())?;
            let selected = tc.union_member_for(disc).map_err(|_| bad_param())?;
            if disc.label_key().is_none() || selected != m.as_ref().map(|(i, _)| *i) {
                return Err(bad_param());
            }
            write(out, &disc_tc, disc)?;
            if let Some((i, v)) = m {
                write(out, &member(&tc, *i), v)?;
            }
        }
        (TCKind::Enum, Value::Enum(v)) => {
            if *v >= tc.member_count().unwrap_or(0) {
                return Err(bad_param());
            }
            v.encode(out)?;
        }
        (TCKind::Fixed, Value::Fixed { digits, negative }) => {
            let n = tc.fixed_digits().unwrap_or(0) as usize;
            if digits.len() != n || digits.iter().any(|&d| d > 9) {
                return Err(bad_param());
            }
            // Packed decimal: two digits per octet, a leading zero digit
            // when the count is even, and the sign in the last half octet.
            let mut nibbles = Vec::with_capacity(n + 2);
            if n % 2 == 0 {
                nibbles.push(0);
            }
            nibbles.extend_from_slice(digits);
            nibbles.push(if *negative { 0xd } else { 0xc });
            let bytes: Vec<u8> = nibbles.chunks(2).map(|p| (p[0] << 4) | p[1]).collect();
            out.write_octets(&bytes);
        }
        (TCKind::Objref | TCKind::Component | TCKind::Home, Value::Objref(obj)) => {
            out.write_object(obj.as_ref())?
        }
        (TCKind::TypeCode, Value::TypeCode(t)) => out.write_typecode(t)?,
        (TCKind::Any, Value::Any(a)) => out.write_any(a)?,
        (
            TCKind::Value
            | TCKind::ValueBox
            | TCKind::Event
            | TCKind::AbstractInterface
            | TCKind::LongDouble
            | TCKind::Native,
            _,
        ) => {
            return Err(sys(SystemExceptionKind::NO_IMPLEMENT, CompletionStatus::No));
        }
        _ => return Err(bad_param()),
    }
    Ok(())
}

fn member(tc: &TypeCode, i: u32) -> TypeCode {
    tc.member_type(i)
        .expect("index checked against the TypeCode")
}

fn write_char(out: &mut OutStream<'_>, c: char) -> Result<(), SystemException> {
    let bytes = out.code_sets().encode_string(c.encode_utf8(&mut [0; 4]))?;
    match bytes[..] {
        [b] => {
            out.write_char(b);
            Ok(())
        }
        _ => Err(sys(
            SystemExceptionKind::DATA_CONVERSION,
            CompletionStatus::No,
        )),
    }
}

fn check_bound(len: usize, bound: u32) -> Result<(), SystemException> {
    if bound != 0 && len > bound as usize {
        return Err(bad_param());
    }
    Ok(())
}

fn write_items(
    out: &mut OutStream<'_>,
    tc: &TypeCode,
    value: &Value,
) -> Result<(), SystemException> {
    let is_seq = tc.kind() == TCKind::Sequence;
    let n = tc.length().unwrap_or(0);
    let check = |len: usize| {
        if is_seq {
            check_bound(len, n)
        } else if len != n as usize {
            Err(bad_param())
        } else {
            Ok(())
        }
    };
    let content = tc.content_type().map_err(|_| bad_param())?;
    let content_kind = content.unaliased().kind();
    match (content_kind, value) {
        (TCKind::Octet, Value::Octets(b)) => {
            check(b.len())?;
            if is_seq {
                out.write_len(b.len())?;
            }
            out.write_octets(b);
        }
        (TCKind::Char, Value::Chars(s)) => {
            check(s.chars().count())?;
            let bytes = out.code_sets().encode_string(s)?;
            if bytes.len() != s.chars().count() {
                return Err(sys(
                    SystemExceptionKind::DATA_CONVERSION,
                    CompletionStatus::No,
                ));
            }
            if is_seq {
                out.write_len(bytes.len())?;
            }
            out.write_octets(&bytes);
        }
        (_, Value::List(items)) => {
            check(items.len())?;
            if is_seq {
                out.write_len(items.len())?;
            }
            for item in items {
                write(out, &content, item)?;
            }
        }
        _ => return Err(bad_param()),
    }
    Ok(())
}

// --- reading ---------------------------------------------------------------------

/// Reads a value of type `tc`. Errors are `MARSHAL` with the completion
/// status of the stream.
pub(crate) fn read(input: &mut InStream<'_>, tc: &TypeCode) -> Result<Value, SystemException> {
    Reader { depth: 0 }.read(input, tc)
}

struct Reader {
    depth: usize,
}

impl Reader {
    fn read(&mut self, input: &mut InStream<'_>, tc: &TypeCode) -> Result<Value, SystemException> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err(input.marshal_error());
        }
        let value = self.read_value(input, tc);
        self.depth -= 1;
        value
    }

    fn read_value(
        &mut self,
        input: &mut InStream<'_>,
        tc: &TypeCode,
    ) -> Result<Value, SystemException> {
        let tc = tc.unaliased();
        Ok(match tc.kind() {
            TCKind::Null | TCKind::Void => Value::Void,
            TCKind::Short => Value::Short(input.read()?),
            TCKind::Long => Value::Long(input.read()?),
            TCKind::UShort => Value::UShort(input.read()?),
            TCKind::ULong => Value::ULong(input.read()?),
            TCKind::LongLong => Value::LongLong(input.read()?),
            TCKind::ULongLong => Value::ULongLong(input.read()?),
            TCKind::Float => Value::Float(input.read()?),
            TCKind::Double => Value::Double(input.read()?),
            TCKind::Boolean => Value::Bool(input.read()?),
            TCKind::Octet => Value::Octet(input.read()?),
            TCKind::Char => Value::Char(input.read_char_value()?),
            TCKind::WChar => Value::WChar(input.read_wchar()?),
            TCKind::String => {
                let s = input.read_string()?;
                self.check_bound(input, s.chars().count(), &tc)?;
                Value::String(s)
            }
            TCKind::WString => {
                let s = input.read_wstring()?;
                self.check_bound(input, s.chars().count(), &tc)?;
                Value::String(s)
            }
            TCKind::Principal => Value::Octets(input.read()?),
            TCKind::Sequence => {
                let content = tc.content_type().map_err(|_| input.marshal_error())?;
                let min = min_size(&content, 0).max(1);
                let len = input.read_len(min).map_err(|_| input.marshal_error())?;
                self.check_bound(input, len, &tc)?;
                self.read_items(input, &content, len)?
            }
            TCKind::Array => {
                let content = tc.content_type().map_err(|_| input.marshal_error())?;
                let len = tc.length().unwrap_or(0) as usize;
                self.read_items(input, &content, len)?
            }
            TCKind::Struct | TCKind::Except => {
                if tc.kind() == TCKind::Except {
                    input.read_string()?;
                }
                let count = tc.member_count().unwrap_or(0);
                let mut items = Vec::with_capacity(count as usize);
                for i in 0..count {
                    items.push(self.read(input, &member(&tc, i))?);
                }
                Value::List(items)
            }
            TCKind::Union => {
                let disc_tc = tc.discriminator_type().map_err(|_| input.marshal_error())?;
                let disc = self.read(input, &disc_tc)?;
                let selected = tc
                    .union_member_for(&disc)
                    .map_err(|_| input.marshal_error())?;
                let m = match selected {
                    Some(i) => Some((i, Box::new(self.read(input, &member(&tc, i))?))),
                    None => None,
                };
                Value::Union {
                    disc: Box::new(disc),
                    member: m,
                }
            }
            TCKind::Enum => {
                let v: u32 = input.read()?;
                if v >= tc.member_count().unwrap_or(0) {
                    return Err(input.marshal_error());
                }
                Value::Enum(v)
            }
            TCKind::Fixed => {
                let n = tc.fixed_digits().unwrap_or(0) as usize;
                let bytes = input
                    .read_octets(n / 2 + 1)
                    .map_err(|_| input.marshal_error())?;
                let mut nibbles: Vec<u8> = bytes.iter().flat_map(|b| [b >> 4, b & 0xf]).collect();
                let sign = nibbles.pop().unwrap_or(0xc);
                if n % 2 == 0 {
                    nibbles.remove(0);
                }
                if nibbles.iter().any(|&d| d > 9) || !matches!(sign, 0xc | 0xd | 0xf) {
                    return Err(input.marshal_error());
                }
                Value::Fixed {
                    digits: nibbles,
                    negative: sign == 0xd,
                }
            }
            TCKind::Objref | TCKind::Component | TCKind::Home => {
                Value::Objref(input.read_object()?)
            }
            TCKind::TypeCode => Value::TypeCode(input.read_typecode()?),
            TCKind::Any => {
                let inner = input.read_typecode()?;
                let v = self.read(input, &inner)?;
                Value::Any(Box::new(Any::new(inner, v)))
            }
            TCKind::LocalInterface => return Err(input.marshal_error()),
            _ => {
                return Err(sys(SystemExceptionKind::NO_IMPLEMENT, input.completed));
            }
        })
    }

    fn check_bound(
        &self,
        input: &InStream<'_>,
        len: usize,
        tc: &TypeCode,
    ) -> Result<(), SystemException> {
        let bound = tc.length().unwrap_or(0);
        if bound != 0 && len > bound as usize {
            return Err(input.marshal_error());
        }
        Ok(())
    }

    fn read_items(
        &mut self,
        input: &mut InStream<'_>,
        content: &TypeCode,
        len: usize,
    ) -> Result<Value, SystemException> {
        Ok(match content.unaliased().kind() {
            TCKind::Octet => Value::Octets(
                input
                    .read_octets(len)
                    .map_err(|_| input.marshal_error())?
                    .to_vec(),
            ),
            TCKind::Char => {
                let bytes = input.read_octets(len).map_err(|_| input.marshal_error())?;
                let completed = input.completed;
                Value::Chars(input.code_sets().decode_string(bytes).map_err(|mut e| {
                    e.completed = completed;
                    e
                })?)
            }
            _ => {
                let mut items = Vec::with_capacity(len.min(4096));
                for _ in 0..len {
                    items.push(self.read(input, content)?);
                }
                Value::List(items)
            }
        })
    }
}

/// The smallest size of a value on the wire, to check sequence lengths
/// before allocating.
fn min_size(tc: &TypeCode, depth: usize) -> usize {
    if depth > 8 {
        return 0;
    }
    let tc = tc.unaliased();
    match tc.kind() {
        TCKind::Boolean | TCKind::Char | TCKind::Octet | TCKind::WChar => 1,
        TCKind::Short | TCKind::UShort => 2,
        TCKind::Long
        | TCKind::ULong
        | TCKind::Float
        | TCKind::Enum
        | TCKind::String
        | TCKind::WString
        | TCKind::Sequence
        | TCKind::Objref
        | TCKind::TypeCode
        | TCKind::Any
        | TCKind::Principal => 4,
        TCKind::LongLong | TCKind::ULongLong | TCKind::Double => 8,
        TCKind::Fixed => (tc.fixed_digits().unwrap_or(0) as usize + 2) / 2,
        TCKind::Array => {
            let content = tc.content_type().map(|c| min_size(&c, depth + 1));
            content.unwrap_or(0) * tc.length().unwrap_or(0) as usize
        }
        TCKind::Struct | TCKind::Except => (0..tc.member_count().unwrap_or(0))
            .map(|i| min_size(&member(&tc, i), depth + 1))
            .fold(0usize, usize::saturating_add),
        _ => 0,
    }
}

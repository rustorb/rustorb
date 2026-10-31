use std::ops::{Deref, DerefMut};

use rustorb_cdr::{CdrDecode, CdrEncode, Decoder, Encoder};
use rustorb_giop::{Body, Ior};

use crate::value::{self, Any, Value};
use crate::{
    CodeSets, CompletionStatus, ObjectRef, Orb, SystemException, SystemExceptionKind, TypeCode,
};

/// The stream request arguments are written to.
///
/// It dereferences to the CDR [`Encoder`] of the message body, and adds
/// the operations that depend on the connection: `string` in the
/// negotiated code set, `wchar` and `wstring` only when a wide code set was
/// negotiated, and object references.
#[derive(Debug)]
pub struct OutStream<'a> {
    pub(crate) enc: &'a mut Encoder,
    pub(crate) code_sets: CodeSets,
    /// The ORB of the first object reference written, so that a standalone
    /// stream can be read back with it (`Any::insert`).
    pub(crate) orb: Option<Orb>,
}

impl<'a> OutStream<'a> {
    /// A stream over `enc` outside any message, with the native code
    /// sets (UTF-8 and UTF-16). Bindings use it to marshal the value of an
    /// `any` on its own, to read it back with [`InStream::standalone`].
    pub fn standalone(enc: &'a mut Encoder) -> OutStream<'a> {
        OutStream {
            enc,
            code_sets: CodeSets::NATIVE,
            orb: None,
        }
    }

    /// The transmission code sets of the connection.
    pub fn code_sets(&self) -> CodeSets {
        self.code_sets
    }

    /// Uses `code_sets` for the `char` and `wchar` data that follows, as
    /// on a standalone stream that must match another ORB's native code
    /// sets.
    pub fn set_code_sets(&mut self, code_sets: CodeSets) {
        self.code_sets = code_sets;
    }

    /// Writes a `string` in the negotiated `char` code set.
    pub fn write_string(&mut self, s: &str) -> Result<(), SystemException> {
        let bytes = self.code_sets.encode_string(s)?;
        Ok(self.enc.write_string_bytes(&bytes)?)
    }

    /// Writes a `wchar` (one UTF-16 code unit).
    pub fn write_wchar(&mut self, c: u16) -> Result<(), SystemException> {
        self.check_wchar()?;
        Ok(self.enc.write_wchar(c)?)
    }

    /// Writes a `wstring`.
    pub fn write_wstring(&mut self, s: &str) -> Result<(), SystemException> {
        self.check_wchar()?;
        Ok(self.enc.write_wstring(s)?)
    }

    /// Writes an object reference; `None` is a nil reference.
    pub fn write_object(&mut self, obj: Option<&ObjectRef>) -> Result<(), SystemException> {
        match obj {
            Some(obj) => {
                if self.orb.is_none() {
                    self.orb = Some(obj.orb().clone());
                }
                obj.ior().encode(self.enc)?
            }
            None => Ior::nil().encode(self.enc)?,
        }
        Ok(())
    }

    /// Writes a TypeCode. An unresolved recursive placeholder is
    /// `BAD_TYPECODE`.
    pub fn write_typecode(&mut self, tc: &TypeCode) -> Result<(), SystemException> {
        tc.encode(self.enc).map_err(|e| match e {
            rustorb_cdr::Error::InvalidTypeCode(_) => {
                SystemException::new(SystemExceptionKind::BAD_TYPECODE, 0, CompletionStatus::No)
            }
            e => e.into(),
        })
    }

    /// Writes a value of type `tc`. A value that does not fit the type is
    /// `BAD_PARAM`.
    pub fn write_value(&mut self, tc: &TypeCode, v: &Value) -> Result<(), SystemException> {
        value::write(self, tc, v)
    }

    /// Writes a `fixed` of `digits.len()` digits (each 0 to 9, most
    /// significant first) as packed decimal. A digit over 9 or more than
    /// 31 digits is `BAD_PARAM`.
    pub fn write_fixed(&mut self, digits: &[u8], negative: bool) -> Result<(), SystemException> {
        let n = digits.len();
        if n > 31 || digits.iter().any(|&d| d > 9) {
            return Err(SystemException::new(
                SystemExceptionKind::BAD_PARAM,
                0,
                CompletionStatus::No,
            ));
        }
        // Two digits per octet, a leading zero digit when the count is
        // even, and the sign in the last half octet.
        let mut nibbles = Vec::with_capacity(n + 2);
        if n % 2 == 0 {
            nibbles.push(0);
        }
        nibbles.extend_from_slice(digits);
        nibbles.push(if negative { 0xd } else { 0xc });
        let bytes: Vec<u8> = nibbles.chunks(2).map(|p| (p[0] << 4) | p[1]).collect();
        self.enc.write_octets(&bytes);
        Ok(())
    }

    /// Writes an `any`: its TypeCode, then its value.
    pub fn write_any(&mut self, any: &Any) -> Result<(), SystemException> {
        self.write_typecode(any.typecode())?;
        value::write(self, any.typecode(), any.value())
    }

    fn check_wchar(&self) -> Result<(), SystemException> {
        if self.code_sets.wchar_data.is_none() {
            return Err(SystemException::new(
                SystemExceptionKind::BAD_PARAM,
                crate::minor::WCHAR_TCS_NOT_KNOWN,
                CompletionStatus::No,
            ));
        }
        Ok(())
    }
}

impl Deref for OutStream<'_> {
    type Target = Encoder;

    fn deref(&self) -> &Encoder {
        self.enc
    }
}

impl DerefMut for OutStream<'_> {
    fn deref_mut(&mut self) -> &mut Encoder {
        self.enc
    }
}

/// The stream a reply, a user exception or the arguments of a request
/// are read from.
///
/// It dereferences to the CDR [`Decoder`] of the message body, and adds
/// the operations that depend on the connection, like [`OutStream`].
/// Errors are `MARSHAL` (or `DATA_CONVERSION`), completed yes when reading
/// a reply and completed no when a servant reads its arguments.
#[derive(Debug)]
pub struct InStream<'a> {
    pub(crate) dec: Decoder<'a>,
    pub(crate) code_sets: CodeSets,
    /// The ORB of the object references read; `None` for a standalone
    /// stream without one, which reads only nil references.
    pub(crate) orb: Option<&'a Orb>,
    pub(crate) completed: CompletionStatus,
    /// Whether `wchar` data needs a negotiated `wchar` code set, as for
    /// request arguments (`BAD_PARAM`, `WCHAR_TCS_NOT_KNOWN`).
    pub(crate) wchar_needs_code_set: bool,
    /// The valuetypes read, by their slot in the stream, for
    /// indirections.
    pub(crate) values: Vec<Option<crate::ValueRef>>,
    /// The valuetypes and value boxes of generated code read, by their
    /// slot in the stream (see `idl::ValueHandle`).
    pub(crate) typed_values: Vec<Option<Box<dyn std::any::Any>>>,
    /// The message body `dec` reads, when it reads one: for
    /// [`detach`](InStream::detach).
    pub(crate) body: Option<&'a Body>,
}

/// The rest of an [`InStream`] over a message body, owned, to be read
/// later ([`ServerRequest`](crate::dii::ServerRequest)).
pub(crate) struct DetachedInput {
    body: Body,
    skip: usize,
    code_sets: CodeSets,
    orb: Option<Orb>,
    completed: CompletionStatus,
    wchar_needs_code_set: bool,
}

impl DetachedInput {
    /// A stream at the place the detached one was.
    pub(crate) fn stream(&self) -> InStream<'_> {
        let mut dec = self.body.decoder();
        // Within the body: it was read that far.
        let _ = dec.skip(self.skip);
        InStream {
            dec,
            code_sets: self.code_sets,
            orb: self.orb.as_ref(),
            completed: self.completed,
            wchar_needs_code_set: self.wchar_needs_code_set,
            values: Vec::new(),
            typed_values: Vec::new(),
            body: Some(&self.body),
        }
    }
}

impl<'a> InStream<'a> {
    /// A stream over `dec` outside any message, with the native code sets,
    /// as [`OutStream::standalone`] writes. Object references read belong
    /// to `orb`; without one, only nil references can be read
    /// (`BAD_INV_ORDER`). Errors are completed no.
    pub fn standalone(dec: Decoder<'a>, orb: Option<&'a Orb>) -> InStream<'a> {
        InStream {
            dec,
            code_sets: CodeSets::NATIVE,
            orb,
            completed: CompletionStatus::No,
            wchar_needs_code_set: false,
            values: Vec::new(),
            typed_values: Vec::new(),
            body: None,
        }
    }

    /// The rest of the stream, owned, if it reads a message body and no
    /// valuetype has been read (later ones could refer to it).
    pub(crate) fn detach(&self) -> Option<DetachedInput> {
        let body = self.body?;
        if !self.values.is_empty() || !self.typed_values.is_empty() {
            return None;
        }
        Some(DetachedInput {
            body: body.clone(),
            skip: self.dec.position() - body.decoder().position(),
            code_sets: self.code_sets,
            orb: self.orb.cloned(),
            completed: self.completed,
            wchar_needs_code_set: self.wchar_needs_code_set,
        })
    }

    /// The transmission code sets of the connection.
    pub fn code_sets(&self) -> CodeSets {
        self.code_sets
    }

    /// Uses `code_sets` for the `char` and `wchar` data that follows, as
    /// on a standalone stream that must match another ORB's native code
    /// sets.
    pub fn set_code_sets(&mut self, code_sets: CodeSets) {
        self.code_sets = code_sets;
    }

    /// Reads a value with its [`CdrDecode`] implementation.
    pub fn read<T: CdrDecode>(&mut self) -> Result<T, SystemException> {
        let completed = self.completed;
        T::decode(&mut self.dec).map_err(|_| marshal_with(completed))
    }

    /// Reads a `string` in the negotiated `char` code set.
    pub fn read_string(&mut self) -> Result<String, SystemException> {
        let completed = self.completed;
        let bytes = self
            .dec
            .read_string_bytes()
            .map_err(|_| marshal_with(completed))?;
        self.code_sets.decode_string(bytes).map_err(|mut e| {
            e.completed = completed;
            e
        })
    }

    /// Reads a `wchar` (one UTF-16 code unit).
    pub fn read_wchar(&mut self) -> Result<u16, SystemException> {
        self.check_wchar()?;
        let completed = self.completed;
        self.dec.read_wchar().map_err(|_| marshal_with(completed))
    }

    /// Reads a `wstring`.
    pub fn read_wstring(&mut self) -> Result<String, SystemException> {
        self.check_wchar()?;
        let completed = self.completed;
        self.dec.read_wstring().map_err(|_| marshal_with(completed))
    }

    fn check_wchar(&self) -> Result<(), SystemException> {
        if self.wchar_needs_code_set && self.code_sets.wchar_data.is_none() {
            return Err(SystemException::new(
                SystemExceptionKind::BAD_PARAM,
                crate::minor::WCHAR_TCS_NOT_KNOWN,
                self.completed,
            ));
        }
        Ok(())
    }

    /// Reads an object reference; a nil reference is `None`.
    pub fn read_object(&mut self) -> Result<Option<ObjectRef>, SystemException> {
        let completed = self.completed;
        let ior = Ior::decode(&mut self.dec).map_err(|_| marshal_with(completed))?;
        if ior.is_nil() {
            return Ok(None);
        }
        let orb = self.orb.ok_or_else(|| {
            SystemException::new(SystemExceptionKind::BAD_INV_ORDER, 0, completed)
        })?;
        Ok(Some(ObjectRef::new(orb.clone(), ior)))
    }

    /// Reads a `char` in the negotiated `char` code set.
    pub fn read_char_value(&mut self) -> Result<char, SystemException> {
        let completed = self.completed;
        let b = self.dec.read_char().map_err(|_| marshal_with(completed))?;
        let s = self.code_sets.decode_string(&[b]).map_err(|mut e| {
            e.completed = completed;
            e
        })?;
        s.chars().next().ok_or_else(|| marshal_with(completed))
    }

    /// Reads a TypeCode.
    pub fn read_typecode(&mut self) -> Result<TypeCode, SystemException> {
        let completed = self.completed;
        TypeCode::decode(&mut self.dec).map_err(|_| marshal_with(completed))
    }

    /// Reads a value of type `tc`.
    pub fn read_value(&mut self, tc: &TypeCode) -> Result<Value, SystemException> {
        value::read(self, tc)
    }

    /// Reads a `fixed` of `n` digits: the digits, most significant first,
    /// and whether it is negative. A malformed digit or sign is `MARSHAL`.
    pub fn read_fixed(&mut self, n: usize) -> Result<(Vec<u8>, bool), SystemException> {
        let e = self.marshal_error();
        let bytes = self.dec.read_octets(n / 2 + 1).map_err(|_| e.clone())?;
        let mut nibbles: Vec<u8> = bytes.iter().flat_map(|b| [b >> 4, b & 0xf]).collect();
        let sign = nibbles.pop().unwrap_or(0xc);
        if n % 2 == 0 {
            nibbles.remove(0);
        }
        if nibbles.iter().any(|&d| d > 9) || !matches!(sign, 0xc | 0xd | 0xf) {
            return Err(e);
        }
        Ok((nibbles, sign == 0xd))
    }

    /// Reads an `any`.
    pub fn read_any(&mut self) -> Result<Any, SystemException> {
        let tc = self.read_typecode()?;
        let v = value::read(self, &tc)?;
        Ok(Any::new(tc, v))
    }

    /// `MARSHAL` with the completion status of the stream.
    pub fn marshal_error(&self) -> SystemException {
        marshal_with(self.completed)
    }

    /// The completion status errors of this stream carry: yes for a
    /// reply, no for request arguments.
    pub fn completion_status(&self) -> CompletionStatus {
        self.completed
    }

    /// Whether every byte of the body was read.
    pub fn is_empty(&self) -> bool {
        self.dec.is_empty()
    }
}

impl<'a> Deref for InStream<'a> {
    type Target = Decoder<'a>;

    fn deref(&self) -> &Decoder<'a> {
        &self.dec
    }
}

impl DerefMut for InStream<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.dec
    }
}

pub(crate) fn marshal(_: rustorb_cdr::Error) -> SystemException {
    marshal_with(CompletionStatus::Yes)
}

fn marshal_with(completed: CompletionStatus) -> SystemException {
    SystemException::new(SystemExceptionKind::MARSHAL, 0, completed)
}

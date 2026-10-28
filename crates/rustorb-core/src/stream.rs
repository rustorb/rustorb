use std::ops::{Deref, DerefMut};

use rustorb_cdr::{CdrDecode, CdrEncode, Decoder, Encoder};
use rustorb_giop::Ior;

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
}

impl<'a> OutStream<'a> {
    /// A stream over `enc` outside any message, with the native code
    /// sets (UTF-8 and UTF-16). Bindings use it to marshal the value of an
    /// `any` on its own, to read it back with [`InStream::standalone`].
    pub fn standalone(enc: &'a mut Encoder) -> OutStream<'a> {
        OutStream {
            enc,
            code_sets: CodeSets::NATIVE,
        }
    }

    /// The transmission code sets of the connection.
    pub fn code_sets(&self) -> CodeSets {
        self.code_sets
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
            Some(obj) => obj.ior().encode(self.enc)?,
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
        }
    }

    /// The transmission code sets of the connection.
    pub fn code_sets(&self) -> CodeSets {
        self.code_sets
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

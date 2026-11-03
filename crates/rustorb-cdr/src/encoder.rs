use crate::{ByteOrder, Encoding, Error, LongDouble, Octets, Result, SHARE_MIN};

/// Writes CDR into a growable buffer.
///
/// Alignment is relative to the stream offset of the first byte, which is
/// `0` unless the encoder was created with [`Encoder::with_offset`].
///
/// An encoder that [allows shared octets](Encoder::allow_shared) keeps
/// large octet sequences written with
/// [`write_octet_seq_shared`](Encoder::write_octet_seq_shared) apart from
/// its buffer: [`into_parts`](Encoder::into_parts) gives the buffer and
/// where each goes, for a vectored write, and
/// [`into_bytes`](Encoder::into_bytes) puts them in place.
#[derive(Debug, Clone)]
pub struct Encoder {
    pub(crate) buf: Vec<u8>,
    /// Shared octet sequences, each with the buffer length at which it
    /// goes.
    shared: Vec<(usize, Octets)>,
    /// Their total length.
    shared_len: usize,
    allow_shared: bool,
    pub(crate) order: ByteOrder,
    encoding: Encoding,
    base: usize,
    /// The stream offset alignment counts from (`XCDR.origin`); 0 except
    /// inside an XCDR stream.
    pub(crate) origin: usize,
    wstring_bom: bool,
    /// The valuetypes written so far (for indirections) and the chunk
    /// being written; made with the first value.
    pub(crate) values: Option<Box<crate::value::WriteState>>,
    /// The 8-byte alignment points written since
    /// [`track_align8`](Encoder::track_align8), each as the buffer length
    /// before its padding and the padding written; `None` when not
    /// tracked, or when something was written whose bytes depend on
    /// stream offsets (see [`take_align8`](Encoder::take_align8)).
    align8: Option<Vec<(usize, usize)>>,
}

macro_rules! write_number {
    ($(#[$doc:meta] $name:ident: $ty:ty;)*) => {
        $(
            #[$doc]
            pub fn $name(&mut self, value: $ty) {
                self.align(size_of::<$ty>());
                match self.order {
                    ByteOrder::Big => self.buf.extend_from_slice(&value.to_be_bytes()),
                    ByteOrder::Little => self.buf.extend_from_slice(&value.to_le_bytes()),
                }
            }
        )*
    };
}

impl Encoder {
    /// Creates an encoder whose first byte is at stream offset 0.
    pub fn new(order: ByteOrder, encoding: Encoding) -> Encoder {
        Encoder::with_offset(order, encoding, 0)
    }

    /// Creates an encoder whose first byte is at stream offset `base`.
    pub fn with_offset(order: ByteOrder, encoding: Encoding, base: usize) -> Encoder {
        Encoder {
            buf: Vec::new(),
            shared: Vec::new(),
            shared_len: 0,
            allow_shared: false,
            order,
            encoding,
            base,
            origin: 0,
            wstring_bom: true,
            values: None,
            align8: None,
        }
    }

    /// Starts an encapsulation: offset 0, with the byte order octet written.
    pub fn encapsulation(order: ByteOrder, encoding: Encoding) -> Encoder {
        let mut enc = Encoder::new(order, encoding);
        enc.write_octet(order.flag());
        enc
    }

    /// Chooses how a GIOP 1.2 `wstring` is written: with a BOM followed by
    /// code units in the stream byte order (the default, as omniORB does),
    /// or, when `false`, big endian without a BOM (the form the
    /// specification defines for a missing BOM). Readers that follow
    /// CORBA 2.4 or later accept both.
    pub fn set_wstring_bom(&mut self, bom: bool) {
        self.wstring_bom = bom;
    }

    /// The byte order of the stream.
    pub fn byte_order(&self) -> ByteOrder {
        self.order
    }

    /// The encoding whose rules apply.
    pub fn encoding(&self) -> Encoding {
        self.encoding
    }

    /// The stream offset of the next byte.
    ///
    /// A caller that writes bytes depending on it (an offset or an
    /// indirection) calls [`pin`](Encoder::pin) as well.
    pub fn position(&self) -> usize {
        self.offset()
    }

    /// The stream offset of the next byte.
    pub(crate) fn offset(&self) -> usize {
        self.base + self.len()
    }

    /// Starts recording where the stream is aligned to 8 bytes, so that
    /// the bytes written from here on can be moved by 4 bytes with
    /// [`take_align8`](Encoder::take_align8). Only for GIOP encodings;
    /// recording stops for good when something is written whose bytes
    /// depend on stream offsets: a valuetype, a call to
    /// [`pin`](Encoder::pin), [`set_origin`](Encoder::set_origin),
    /// [`as_bytes_mut`](Encoder::as_bytes_mut) or
    /// [`truncate`](Encoder::truncate).
    pub fn track_align8(&mut self) {
        self.align8 = self.encoding.is_giop().then(Vec::new);
    }

    /// Stops recording, and returns the 8-byte alignment points written
    /// since [`track_align8`](Encoder::track_align8): each is the buffer
    /// length before its padding and the length of that padding. Data
    /// whose alignment is at most 4 does not depend on the stream offset
    /// modulo 8, so the bytes written since can be moved by 4 bytes by
    /// recomputing the padding at these points alone. `None` if not
    /// recording, or if recording stopped.
    pub fn take_align8(&mut self) -> Option<Vec<(usize, usize)>> {
        self.align8.take()
    }

    /// Says that bytes written from here on depend on stream offsets, as
    /// an indirection does, so the stream cannot be moved: stops
    /// [`track_align8`](Encoder::track_align8) for good.
    pub fn pin(&mut self) {
        self.align8 = None;
    }

    /// Bytes written so far, shared octets included.
    pub fn len(&self) -> usize {
        self.buf.len() + self.shared_len
    }

    /// Whether nothing has been written.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The bytes written into the buffer so far: all of them, unless
    /// shared octets were kept apart.
    pub fn as_bytes(&self) -> &[u8] {
        &self.buf
    }

    /// Mutable access to the buffer, for patching lengths. Indexes are
    /// buffer indexes, which differ from stream offsets after shared
    /// octets.
    pub fn as_bytes_mut(&mut self) -> &mut [u8] {
        self.align8 = None;
        &mut self.buf
    }

    /// Drops bytes past `len`, which must not be before shared octets.
    pub fn truncate(&mut self, len: usize) {
        debug_assert!(self.shared.last().is_none_or(|(at, _)| *at <= len));
        self.align8 = None;
        self.buf.truncate(len);
    }

    /// Consumes the encoder and returns the bytes, shared octets copied in
    /// place.
    pub fn into_bytes(self) -> Vec<u8> {
        if self.shared.is_empty() {
            return self.buf;
        }
        let mut out = Vec::with_capacity(self.len());
        let mut from = 0;
        for (at, octets) in &self.shared {
            out.extend_from_slice(&self.buf[from..*at]);
            out.extend_from_slice(octets);
            from = *at;
        }
        out.extend_from_slice(&self.buf[from..]);
        out
    }

    /// Consumes the encoder and returns the buffer and the shared octets,
    /// each with the buffer length at which it goes.
    pub fn into_parts(self) -> (Vec<u8>, Vec<(usize, Octets)>) {
        (self.buf, self.shared)
    }

    /// Makes room for at least `additional` more bytes in the buffer.
    pub fn reserve(&mut self, additional: usize) {
        self.buf.reserve(additional);
    }

    /// Lets [`write_octet_seq_shared`](Encoder::write_octet_seq_shared)
    /// keep large octet sequences apart from the buffer. Only for an
    /// encoder whose result goes out through
    /// [`into_parts`](Encoder::into_parts) or
    /// [`into_bytes`](Encoder::into_bytes), not one read through
    /// [`as_bytes`](Encoder::as_bytes).
    pub fn allow_shared(&mut self, allow: bool) {
        self.allow_shared = allow;
    }

    /// Pads with zero octets until the stream offset is a multiple of
    /// `align`, or of the encoding's [`max_align`](Encoding::max_align)
    /// when that is smaller.
    ///
    /// In an XCDR stream the offset counts from the alignment origin (see
    /// [`set_origin`](Encoder::set_origin)).
    pub fn align(&mut self, align: usize) {
        let align = align.min(self.encoding.max_align());
        let rem = (self.offset() - self.origin) % align;
        let pad = if rem == 0 { 0 } else { align - rem };
        if align == 8 {
            if let Some(points) = self.align8.as_mut() {
                points.push((self.buf.len(), pad));
            }
        }
        if pad != 0 {
            self.buf.resize(self.buf.len() + pad, 0);
        }
    }

    /// The stream offset alignment counts from: 0 unless changed with
    /// [`set_origin`](Encoder::set_origin).
    pub fn origin(&self) -> usize {
        self.origin
    }

    /// Makes alignment count from stream offset `origin`, which must not
    /// be past [`position`](Encoder::position). This is the XCDR
    /// `PUSH(ORIGIN)`; the parameter headers of
    /// [`begin_parameter`](Encoder::begin_parameter) do it themselves.
    pub fn set_origin(&mut self, origin: usize) {
        debug_assert!(origin <= self.offset());
        self.align8 = None;
        self.origin = origin;
    }

    /// Writes an `octet`.
    pub fn write_octet(&mut self, value: u8) {
        self.buf.push(value);
    }

    /// Writes a `boolean`.
    pub fn write_bool(&mut self, value: bool) {
        self.buf.push(u8::from(value));
    }

    /// Writes a `char` as a single octet in the transmission code set.
    pub fn write_char(&mut self, value: u8) {
        self.buf.push(value);
    }

    write_number! {
        /// Writes a `short`.
        write_i16: i16;
        /// Writes an `unsigned short`.
        write_u16: u16;
        /// Writes a `long`.
        write_i32: i32;
        /// Writes an `unsigned long`.
        write_u32: u32;
        /// Writes a `long long`.
        write_i64: i64;
        /// Writes an `unsigned long long`.
        write_u64: u64;
        /// Writes a `float`.
        write_f32: f32;
        /// Writes a `double`.
        write_f64: f64;
    }

    /// Writes a `long double`: 16 octets on an 8 octet boundary.
    pub fn write_long_double(&mut self, value: LongDouble) {
        self.align(8);
        let bits = value.to_bits();
        self.buf.extend_from_slice(&match self.order {
            ByteOrder::Big => bits.to_be_bytes(),
            ByteOrder::Little => bits.to_le_bytes(),
        });
    }

    /// Writes raw octets without alignment or a length.
    pub fn write_octets(&mut self, bytes: &[u8]) {
        self.buf.extend_from_slice(bytes);
    }

    /// Writes a sequence length.
    pub fn write_len(&mut self, len: usize) -> Result<()> {
        let len = u32::try_from(len).map_err(|_| Error::LengthOverflow(len))?;
        self.write_u32(len);
        Ok(())
    }

    /// Writes a `sequence<octet>`.
    pub fn write_octet_seq(&mut self, bytes: &[u8]) -> Result<()> {
        self.write_len(bytes.len())?;
        self.write_octets(bytes);
        Ok(())
    }

    /// Writes a `sequence<octet>` without copying a large one, if the
    /// encoder [allows it](Encoder::allow_shared) and no valuetype is
    /// being written (whose chunk lengths count the buffer), and the
    /// encoding is a GIOP one (XCDR headers are patched in the buffer). Otherwise
    /// like [`write_octet_seq`](Encoder::write_octet_seq).
    pub fn write_octet_seq_shared(&mut self, octets: &Octets) -> Result<()> {
        if !self.allow_shared
            || octets.len() < SHARE_MIN
            || self.values.is_some()
            || !self.encoding.is_giop()
        {
            return self.write_octet_seq(octets);
        }
        self.write_len(octets.len())?;
        self.shared.push((self.buf.len(), octets.clone()));
        self.shared_len += octets.len();
        Ok(())
    }

    /// Writes a `string` from bytes already in the transmission code set.
    ///
    /// The length includes the terminating NUL, which is appended here.
    pub fn write_string_bytes(&mut self, bytes: &[u8]) -> Result<()> {
        self.write_len(bytes.len() + 1)?;
        self.write_octets(bytes);
        self.write_octet(0);
        Ok(())
    }

    /// Writes a `string` as UTF-8.
    pub fn write_string(&mut self, value: &str) -> Result<()> {
        self.write_string_bytes(value.as_bytes())
    }

    /// Writes a `wchar` holding one UTF-16 code unit.
    ///
    /// GIOP 1.1 and XCDR (`Char16`, DDS-XTypes 7.4.1.1.1) write an aligned
    /// `unsigned short` in the stream byte order. GIOP 1.2 writes an octet length of 2 followed by the code unit in big
    /// endian without a BOM, as omniORB does.
    pub fn write_wchar(&mut self, value: u16) -> Result<()> {
        match self.encoding {
            Encoding::Giop1_0 => return Err(Error::WcharNotSupported),
            Encoding::Giop1_1 | Encoding::Xcdr1 | Encoding::Xcdr2 => self.write_u16(value),
            Encoding::Giop1_2 => {
                self.write_octet(2);
                self.write_octets(&value.to_be_bytes());
            }
        }
        Ok(())
    }

    /// Writes a `wstring` encoded as UTF-16.
    ///
    /// GIOP 1.1 writes the number of code units including a terminating
    /// NUL, then aligned code units in the stream byte order. GIOP 1.2
    /// writes the octet length, then the code units without a terminator:
    /// by default a BOM followed by the stream byte order, or big endian
    /// without a BOM (see [`Encoder::set_wstring_bom`]). An empty string is
    /// just a zero length. XCDR (DDS-XTypes 7.4.1.1.2) writes the length in
    /// octets, then the code units in the stream byte order, without a BOM
    /// or a terminator.
    pub fn write_wstring(&mut self, value: &str) -> Result<()> {
        match self.encoding {
            Encoding::Giop1_0 => Err(Error::WcharNotSupported),
            Encoding::Xcdr1 | Encoding::Xcdr2 => {
                let units: Vec<u16> = value.encode_utf16().collect();
                self.write_len(units.len().saturating_mul(2))?;
                for unit in units {
                    self.write_u16(unit);
                }
                Ok(())
            }
            Encoding::Giop1_1 => {
                let units: Vec<u16> = value.encode_utf16().collect();
                self.write_len(units.len() + 1)?;
                for unit in units {
                    self.write_u16(unit);
                }
                self.write_u16(0);
                Ok(())
            }
            Encoding::Giop1_2 => {
                let order = if self.wstring_bom {
                    self.order
                } else {
                    ByteOrder::Big
                };
                let bom = (self.wstring_bom && !value.is_empty()).then_some(0xFEFF);
                let bytes: Vec<u8> = bom
                    .into_iter()
                    .chain(value.encode_utf16())
                    .flat_map(|unit| match order {
                        ByteOrder::Big => unit.to_be_bytes(),
                        ByteOrder::Little => unit.to_le_bytes(),
                    })
                    .collect();
                self.write_octet_seq(&bytes)
            }
        }
    }

    /// Writes an encapsulation as a `sequence<octet>`.
    ///
    /// `f` writes the contents into a fresh encoder whose alignment starts
    /// at the byte order octet. The encapsulation uses this stream's byte
    /// order and version.
    pub fn write_encapsulation<F>(&mut self, f: F) -> Result<()>
    where
        F: FnOnce(&mut Encoder) -> Result<()>,
    {
        let mut inner = Encoder::encapsulation(self.order, self.encoding);
        inner.wstring_bom = self.wstring_bom;
        f(&mut inner)?;
        self.write_octet_seq(&inner.buf)
    }
}

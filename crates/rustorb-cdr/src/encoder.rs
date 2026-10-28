use crate::{ByteOrder, CdrVersion, Error, Result};

/// Writes CDR into a growable buffer.
///
/// Alignment is relative to the stream offset of the first byte, which is
/// `0` unless the encoder was created with [`Encoder::with_offset`].
#[derive(Debug, Clone)]
pub struct Encoder {
    buf: Vec<u8>,
    order: ByteOrder,
    version: CdrVersion,
    base: usize,
    wstring_bom: bool,
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
    pub fn new(order: ByteOrder, version: CdrVersion) -> Encoder {
        Encoder::with_offset(order, version, 0)
    }

    /// Creates an encoder whose first byte is at stream offset `base`.
    pub fn with_offset(order: ByteOrder, version: CdrVersion, base: usize) -> Encoder {
        Encoder {
            buf: Vec::new(),
            order,
            version,
            base,
            wstring_bom: true,
        }
    }

    /// Starts an encapsulation: offset 0, with the byte order octet written.
    pub fn encapsulation(order: ByteOrder, version: CdrVersion) -> Encoder {
        let mut enc = Encoder::new(order, version);
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

    /// The GIOP version whose CDR rules apply.
    pub fn version(&self) -> CdrVersion {
        self.version
    }

    /// The stream offset of the next byte.
    pub fn position(&self) -> usize {
        self.base + self.buf.len()
    }

    /// Bytes written so far.
    pub fn len(&self) -> usize {
        self.buf.len()
    }

    /// Whether nothing has been written.
    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    /// The bytes written so far.
    pub fn as_bytes(&self) -> &[u8] {
        &self.buf
    }

    /// Mutable access to the bytes written so far, for patching lengths.
    pub fn as_bytes_mut(&mut self) -> &mut [u8] {
        &mut self.buf
    }

    /// Drops bytes past `len`.
    pub fn truncate(&mut self, len: usize) {
        self.buf.truncate(len);
    }

    /// Consumes the encoder and returns the bytes.
    pub fn into_bytes(self) -> Vec<u8> {
        self.buf
    }

    /// Pads with zero octets until the stream offset is a multiple of `align`.
    pub fn align(&mut self, align: usize) {
        let rem = self.position() % align;
        if rem != 0 {
            self.buf.resize(self.buf.len() + align - rem, 0);
        }
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
    /// GIOP 1.1 writes an aligned `unsigned short` in the stream byte order.
    /// GIOP 1.2 writes an octet length of 2 followed by the code unit in big
    /// endian without a BOM, as omniORB does.
    pub fn write_wchar(&mut self, value: u16) -> Result<()> {
        match self.version {
            CdrVersion::Giop1_0 => return Err(Error::WcharNotSupported),
            CdrVersion::Giop1_1 => self.write_u16(value),
            CdrVersion::Giop1_2 => {
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
    /// just a zero length.
    pub fn write_wstring(&mut self, value: &str) -> Result<()> {
        match self.version {
            CdrVersion::Giop1_0 => Err(Error::WcharNotSupported),
            CdrVersion::Giop1_1 => {
                let units: Vec<u16> = value.encode_utf16().collect();
                self.write_len(units.len() + 1)?;
                for unit in units {
                    self.write_u16(unit);
                }
                self.write_u16(0);
                Ok(())
            }
            CdrVersion::Giop1_2 => {
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
        let mut inner = Encoder::encapsulation(self.order, self.version);
        inner.wstring_bom = self.wstring_bom;
        f(&mut inner)?;
        self.write_octet_seq(&inner.buf)
    }
}

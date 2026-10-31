use std::sync::Arc;

use crate::{ByteOrder, CdrVersion, Error, LongDouble, Octets, Result, SHARE_MIN};

/// Reads CDR from a byte slice.
///
/// Alignment is relative to the stream offset of the first byte, which is
/// `0` unless the decoder was created with [`Decoder::with_offset`].
/// [`Decoder::set_segments`] restarts the stream offset at given bytes,
/// for data joined from several messages.
#[derive(Debug, Clone)]
pub struct Decoder<'a> {
    pub(crate) buf: &'a [u8],
    /// The buffer `buf` lies in, when it is shared.
    shared: Option<&'a Arc<Vec<u8>>>,
    pub(crate) pos: usize,
    pub(crate) order: ByteOrder,
    version: CdrVersion,
    base: usize,
    segments: &'a [Segment],
    wstring_default_order: ByteOrder,
    /// What the valuetypes read so far need (indirections, chunks);
    /// made with the first value.
    pub(crate) values: Option<Box<crate::value::ReadState>>,
}

/// A point in a decoder's buffer from which the stream offset restarts.
///
/// The byte at index `start` of the buffer is at stream offset `offset`,
/// and the bytes after it follow on, until the next segment. This
/// describes a message joined from fragments whose contents are aligned
/// relative to each fragment rather than to the joined message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Segment {
    /// Index of the first byte of the segment in the buffer.
    pub start: usize,
    /// Stream offset of that byte.
    pub offset: usize,
}

macro_rules! read_number {
    ($(#[$doc:meta] $name:ident: $ty:ty;)*) => {
        $(
            #[$doc]
            pub fn $name(&mut self) -> Result<$ty> {
                self.align(size_of::<$ty>())?;
                let bytes = self.read_array()?;
                Ok(match self.order {
                    ByteOrder::Big => <$ty>::from_be_bytes(bytes),
                    ByteOrder::Little => <$ty>::from_le_bytes(bytes),
                })
            }
        )*
    };
}

impl<'a> Decoder<'a> {
    /// Creates a decoder whose first byte is at stream offset 0.
    pub fn new(buf: &'a [u8], order: ByteOrder, version: CdrVersion) -> Decoder<'a> {
        Decoder::with_offset(buf, order, version, 0)
    }

    /// Creates a decoder whose first byte is at stream offset `base`.
    pub fn with_offset(
        buf: &'a [u8],
        order: ByteOrder,
        version: CdrVersion,
        base: usize,
    ) -> Decoder<'a> {
        Decoder {
            buf,
            shared: None,
            pos: 0,
            order,
            version,
            base,
            segments: &[],
            wstring_default_order: ByteOrder::Big,
            values: None,
        }
    }

    /// Restarts the stream offset at the given buffer indexes. `segments`
    /// must be sorted by `start`; bytes before the first segment keep the
    /// offset given when the decoder was created.
    pub fn set_segments(&mut self, segments: &'a [Segment]) {
        debug_assert!(segments.windows(2).all(|w| w[0].start < w[1].start));
        self.segments = segments;
    }

    /// Opens an encapsulation: reads the byte order octet and returns a
    /// decoder positioned after it, with alignment starting at that octet.
    pub fn encapsulation(buf: &'a [u8], version: CdrVersion) -> Result<Decoder<'a>> {
        let (&flag, _) = buf.split_first().ok_or(Error::EmptyEncapsulation)?;
        let order = ByteOrder::from_flag(flag)?;
        let mut dec = Decoder::new(buf, order, version);
        dec.pos = 1;
        Ok(dec)
    }

    /// Sets the byte order assumed for a GIOP 1.2 `wstring` or `wchar`
    /// without a BOM. The specification says big endian, which is the
    /// default; some ORBs use the stream byte order instead.
    pub fn set_wstring_default_order(&mut self, order: ByteOrder) {
        self.wstring_default_order = order;
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
        let i = self.segments.partition_point(|s| s.start <= self.pos);
        match i.checked_sub(1) {
            Some(i) => {
                let segment = self.segments[i];
                segment.offset + (self.pos - segment.start)
            }
            None => self.base + self.pos,
        }
    }

    /// Bytes left to read.
    pub fn remaining(&self) -> usize {
        self.buf.len() - self.pos
    }

    /// Whether every byte has been read.
    pub fn is_empty(&self) -> bool {
        self.remaining() == 0
    }

    /// The bytes left to read, without consuming them.
    pub fn remaining_bytes(&self) -> &'a [u8] {
        &self.buf[self.pos..]
    }

    /// Skips padding until the stream offset is a multiple of `align`.
    ///
    /// Inside a chunked valuetype, a chunk that ends here is followed by
    /// the next one: its length is read and the data aligned in it.
    pub fn align(&mut self, align: usize) -> Result<()> {
        loop {
            self.chunk_boundary()?;
            let rem = self.position() % align;
            if rem == 0 {
                return Ok(());
            }
            // Padding that runs past the end of a chunk continues in the
            // next one, after its length.
            let mut pad = align - rem;
            if let Some(end) = self.chunk_end() {
                pad = pad.min(end - self.pos);
            }
            self.read_octets_raw(pad)?;
        }
    }

    /// Skips `n` bytes.
    pub fn skip(&mut self, n: usize) -> Result<()> {
        self.read_octets(n).map(|_| ())
    }

    /// Reads `n` raw octets without alignment.
    pub fn read_octets(&mut self, n: usize) -> Result<&'a [u8]> {
        if n > 0 {
            self.chunk_boundary()?;
            self.check_in_chunk(n)?;
        }
        self.read_octets_raw(n)
    }

    /// Skips padding, ignoring chunks.
    pub(crate) fn align_raw(&mut self, align: usize) -> Result<()> {
        let rem = self.position() % align;
        if rem != 0 {
            self.read_octets_raw(align - rem)?;
        }
        Ok(())
    }

    /// Reads an aligned `unsigned long`, ignoring chunks.
    pub(crate) fn read_u32_raw(&mut self) -> Result<u32> {
        self.align_raw(4)?;
        let bytes: [u8; 4] = self
            .read_octets_raw(4)?
            .try_into()
            .expect("read_octets_raw returned 4 bytes");
        Ok(match self.order {
            ByteOrder::Big => u32::from_be_bytes(bytes),
            ByteOrder::Little => u32::from_le_bytes(bytes),
        })
    }

    /// Reads `n` octets, ignoring chunks.
    pub(crate) fn read_octets_raw(&mut self, n: usize) -> Result<&'a [u8]> {
        let remaining = self.remaining();
        if n > remaining {
            return Err(Error::UnexpectedEof {
                needed: n,
                remaining,
            });
        }
        let bytes = &self.buf[self.pos..self.pos + n];
        self.pos += n;
        Ok(bytes)
    }

    fn read_array<const N: usize>(&mut self) -> Result<[u8; N]> {
        let bytes = self.read_octets(N)?;
        Ok(bytes.try_into().expect("read_octets returned N bytes"))
    }

    /// Reads an `octet`.
    pub fn read_octet(&mut self) -> Result<u8> {
        Ok(self.read_array::<1>()?[0])
    }

    /// Reads a `boolean`. Octets other than `0` and `1` are rejected.
    pub fn read_bool(&mut self) -> Result<bool> {
        match self.read_octet()? {
            0 => Ok(false),
            1 => Ok(true),
            other => Err(Error::InvalidBoolean(other)),
        }
    }

    /// Reads a `char` as a single octet in the transmission code set.
    pub fn read_char(&mut self) -> Result<u8> {
        self.read_octet()
    }

    read_number! {
        /// Reads a `short`.
        read_i16: i16;
        /// Reads an `unsigned short`.
        read_u16: u16;
        /// Reads a `long`.
        read_i32: i32;
        /// Reads an `unsigned long`.
        read_u32: u32;
        /// Reads a `long long`.
        read_i64: i64;
        /// Reads an `unsigned long long`.
        read_u64: u64;
        /// Reads a `float`.
        read_f32: f32;
        /// Reads a `double`.
        read_f64: f64;
    }

    /// Reads a `long double`: 16 octets on an 8 octet boundary.
    pub fn read_long_double(&mut self) -> Result<LongDouble> {
        self.align(8)?;
        let bytes = self.read_array()?;
        Ok(LongDouble::from_bits(match self.order {
            ByteOrder::Big => u128::from_be_bytes(bytes),
            ByteOrder::Little => u128::from_le_bytes(bytes),
        }))
    }

    /// Reads a sequence length and checks that `len * min_element_size`
    /// bytes are left, so a corrupt length cannot cause a huge allocation.
    pub fn read_len(&mut self, min_element_size: usize) -> Result<usize> {
        let len = self.read_u32()?;
        let remaining = self.remaining();
        let needed = (len as usize).saturating_mul(min_element_size);
        if needed > remaining {
            return Err(Error::LengthTooLarge { len, remaining });
        }
        Ok(len as usize)
    }

    /// Reads a `sequence<octet>` without copying.
    pub fn read_octet_seq(&mut self) -> Result<&'a [u8]> {
        let len = self.read_len(1)?;
        self.read_octets(len)
    }

    /// Tells the decoder that its bytes lie in `buf`, so that
    /// [`read_octet_seq_shared`](Decoder::read_octet_seq_shared) can share
    /// them.
    pub fn set_shared(&mut self, buf: &'a Arc<Vec<u8>>) {
        self.shared = Some(buf);
    }

    /// Reads a `sequence<octet>` that keeps pointing into the decoder's
    /// buffer, when that is shared (see
    /// [`set_shared`](Decoder::set_shared)) and the sequence is not short;
    /// otherwise a copy.
    pub fn read_octet_seq_shared(&mut self) -> Result<Octets> {
        let len = self.read_len(1)?;
        self.read_octets_shared(len)
    }

    /// Reads `len` raw octets like
    /// [`read_octet_seq_shared`](Decoder::read_octet_seq_shared) reads a
    /// sequence's.
    pub fn read_octets_shared(&mut self, len: usize) -> Result<Octets> {
        let bytes = self.read_octets(len)?;
        if let Some(buf) = self.shared.filter(|_| bytes.len() >= SHARE_MIN) {
            let base = buf.as_ptr() as usize;
            let at = bytes.as_ptr() as usize;
            if at >= base && at + bytes.len() <= base + buf.len() {
                let start = at - base;
                return Ok(Octets::slice_of(
                    Arc::clone(buf),
                    start..start + bytes.len(),
                ));
            }
        }
        Ok(Octets::from(bytes))
    }

    /// Reads a `string` as bytes in the transmission code set, without the
    /// terminating NUL.
    ///
    /// A length of 0 is not allowed by the specification but is accepted as
    /// an empty string, since some ORBs send it.
    pub fn read_string_bytes(&mut self) -> Result<&'a [u8]> {
        let bytes = self.read_octet_seq()?;
        match bytes.split_last() {
            None => Ok(bytes),
            Some((0, rest)) => Ok(rest),
            Some(_) => Err(Error::StringNotTerminated),
        }
    }

    /// Reads a `string` as UTF-8.
    pub fn read_string(&mut self) -> Result<String> {
        let bytes = self.read_string_bytes()?;
        String::from_utf8(bytes.to_vec()).map_err(|_| Error::InvalidUtf8)
    }

    /// Reads a `wchar` holding one UTF-16 code unit.
    pub fn read_wchar(&mut self) -> Result<u16> {
        match self.version {
            CdrVersion::Giop1_0 => Err(Error::WcharNotSupported),
            CdrVersion::Giop1_1 => self.read_u16(),
            CdrVersion::Giop1_2 => {
                let len = self.read_octet()?;
                if len != 2 && len != 4 {
                    return Err(Error::InvalidWcharLength(len));
                }
                let bytes = self.read_octets(len as usize)?;
                let units = self.utf16_units(bytes);
                match units.as_slice() {
                    [unit] => Ok(*unit),
                    _ => Err(Error::InvalidWcharLength(len)),
                }
            }
        }
    }

    /// Reads a `wstring` encoded as UTF-16.
    ///
    /// For GIOP 1.2 a leading BOM selects the byte order; without one the
    /// order set by [`Decoder::set_wstring_default_order`] is used.
    pub fn read_wstring(&mut self) -> Result<String> {
        match self.version {
            CdrVersion::Giop1_0 => Err(Error::WcharNotSupported),
            CdrVersion::Giop1_1 => {
                let len = self.read_len(2)?;
                let mut units = Vec::with_capacity(len);
                for _ in 0..len {
                    units.push(self.read_u16()?);
                }
                // The length counts a terminating NUL; tolerate its absence.
                if units.last() == Some(&0) {
                    units.pop();
                }
                String::from_utf16(&units).map_err(|_| Error::InvalidUtf16)
            }
            CdrVersion::Giop1_2 => {
                let bytes = self.read_octet_seq()?;
                if bytes.len() % 2 != 0 {
                    return Err(Error::OddWstringLength(bytes.len() as u32));
                }
                String::from_utf16(&self.utf16_units(bytes)).map_err(|_| Error::InvalidUtf16)
            }
        }
    }

    /// Splits GIOP 1.2 wide character octets into code units, honouring and
    /// removing a leading BOM.
    fn utf16_units(&self, bytes: &[u8]) -> Vec<u16> {
        let (order, data) = match bytes {
            [0xFE, 0xFF, rest @ ..] => (ByteOrder::Big, rest),
            [0xFF, 0xFE, rest @ ..] => (ByteOrder::Little, rest),
            _ => (self.wstring_default_order, bytes),
        };
        data.chunks_exact(2)
            .map(|c| match order {
                ByteOrder::Big => u16::from_be_bytes([c[0], c[1]]),
                ByteOrder::Little => u16::from_le_bytes([c[0], c[1]]),
            })
            .collect()
    }

    /// Reads an encapsulation and returns a decoder over its contents.
    pub fn read_encapsulation(&mut self) -> Result<Decoder<'a>> {
        let bytes = self.read_octet_seq()?;
        let mut inner = Decoder::encapsulation(bytes, self.version)?;
        inner.wstring_default_order = self.wstring_default_order;
        Ok(inner)
    }
}

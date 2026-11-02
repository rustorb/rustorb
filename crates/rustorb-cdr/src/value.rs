//! Valuetype tags, sharing and chunking (CORBA 3.4 part 2, 9.3.4).
//!
//! A value is written as a value tag (`0x7fffff00` plus flags), an
//! optional codebase URL, optional repository ids and its state. `null`
//! is the tag `0`, and a value written before in the same stream is the
//! tag `0xffffffff` followed by the offset back to its value tag.
//!
//! A chunked value splits its state into chunks, each a positive length
//! followed by that many octets, and ends with an end tag: the negated
//! nesting depth. A reader that does not know a truncatable value's type
//! skips the chunks and nested values it cannot read, up to the end tag.
//!
//! Positions are buffer indexes, so a message joined from fragments keeps
//! its indirections even when the stream offset restarts.

use std::collections::HashMap;

use crate::{Decoder, Encoder, Error, Result};

const VALUE_TAG: u32 = 0x7fff_ff00;
const CODEBASE: u32 = 0x01;
const SINGLE_ID: u32 = 0x02;
const ID_LIST: u32 = 0x06;
const CHUNKED: u32 = 0x08;
const INDIRECTION: u32 = 0xffff_ffff;

/// What [`Decoder::read_value_header`] found where a value was expected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValueHeader {
    /// A `null` value.
    Null,
    /// A value read earlier in the stream, by its slot.
    Indirection(usize),
    /// A new value, whose state follows. The caller reads the state and
    /// then calls [`Decoder::end_value`].
    Value {
        /// The number of this value in the stream, counting from 0, which
        /// later indirections refer to.
        slot: usize,
        /// The repository ids sent, most derived first; empty when the
        /// sender left the type to the receiver.
        ids: Vec<String>,
        /// Whether the state is chunked.
        chunked: bool,
    },
}

/// Writing state kept in an [`Encoder`] once a value has been written.
#[derive(Debug, Clone, Default)]
pub(crate) struct WriteState {
    /// Buffer index of the value tag of each value written, by key.
    written: HashMap<u64, usize>,
    /// Whether each value being written is chunked, outermost first.
    frames: Vec<bool>,
    /// Buffer index of the length of the open chunk.
    chunk: Option<usize>,
}

/// Reading state kept in a [`Decoder`] once a value has been read.
#[derive(Debug, Clone, Default)]
pub(crate) struct ReadState {
    /// Slot of each value tag read, by buffer index.
    tags: HashMap<usize, usize>,
    /// Repository ids and codebase URLs read, by buffer index of their
    /// length, for indirections to them.
    strings: HashMap<usize, String>,
    /// Lists of repository ids read, by buffer index of their count.
    lists: HashMap<usize, Vec<String>>,
    /// Values read so far.
    slots: usize,
    /// The values being read, outermost first.
    frames: Vec<Frame>,
    /// Buffer index of the end of the chunk being read.
    chunk_end: Option<usize>,
    /// The depth of the values an end tag has already ended, or `0`.
    ended: usize,
}

#[derive(Debug, Clone, Copy)]
struct Frame {
    chunked: bool,
    /// The nesting depth, from 1.
    depth: usize,
}

impl Encoder {
    /// Writes a `null` value.
    pub fn write_null_value(&mut self) {
        self.write_u32(0);
    }

    /// Starts writing a value.
    ///
    /// `key` identifies the value: if a value with the same key was
    /// written before in this stream, an indirection to it is written and
    /// `false` is returned. Otherwise the value tag and `ids` (repository
    /// ids, most derived first; none leaves the type to the receiver) are
    /// written and `true` is returned: the caller writes the state and
    /// then calls [`Encoder::end_value`].
    ///
    /// A value nested in a chunked value is chunked as well.
    pub fn begin_value(&mut self, key: u64, ids: &[&str], chunked: bool) -> Result<bool> {
        let state = self.values.get_or_insert_with(Default::default);
        if let Some(&tag_at) = state.written.get(&key) {
            self.write_u32(INDIRECTION);
            let at = self.buf.len();
            let offset = i32::try_from(tag_at as i64 - at as i64)
                .map_err(|_| Error::LengthOverflow(at - tag_at))?;
            self.write_i32(offset);
            return Ok(false);
        }
        let chunked = chunked || state.frames.last() == Some(&true);
        self.close_chunk();
        let mut tag = VALUE_TAG;
        match ids.len() {
            0 => {}
            1 => tag |= SINGLE_ID,
            _ => tag |= ID_LIST,
        }
        if chunked {
            tag |= CHUNKED;
        }
        self.align(4);
        let tag_at = self.buf.len();
        self.write_u32(tag);
        if ids.len() > 1 {
            self.write_len(ids.len())?;
        }
        for id in ids {
            self.write_string(id)?;
        }
        let state = self.values.as_mut().expect("made above");
        state.written.insert(key, tag_at);
        state.frames.push(chunked);
        if chunked {
            self.open_chunk();
        }
        Ok(true)
    }

    /// Ends the value started by the last [`Encoder::begin_value`] that
    /// returned `true`.
    pub fn end_value(&mut self) -> Result<()> {
        let state = self
            .values
            .as_mut()
            .ok_or(Error::InvalidValue("end_value without begin_value"))?;
        let depth = state.frames.len();
        let chunked = state
            .frames
            .pop()
            .ok_or(Error::InvalidValue("end_value without begin_value"))?;
        let outer_chunked = state.frames.last() == Some(&true);
        if chunked {
            self.close_chunk();
            let depth = i32::try_from(depth).map_err(|_| Error::LengthOverflow(depth))?;
            self.write_i32(-depth);
            if outer_chunked {
                self.open_chunk();
            }
        }
        Ok(())
    }

    /// Starts a chunk, whose length is filled in when it is closed.
    fn open_chunk(&mut self) {
        self.align(4);
        let at = self.buf.len();
        self.write_u32(0);
        if let Some(state) = self.values.as_mut() {
            state.chunk = Some(at);
        }
    }

    /// Fills in the length of the open chunk, or drops it if it is empty.
    fn close_chunk(&mut self) {
        let Some(at) = self.values.as_mut().and_then(|s| s.chunk.take()) else {
            return;
        };
        let len = self.buf.len() - (at + 4);
        if len == 0 {
            self.buf.truncate(at);
        } else {
            let len = len as u32;
            let bytes = match self.order {
                crate::ByteOrder::Big => len.to_be_bytes(),
                crate::ByteOrder::Little => len.to_le_bytes(),
            };
            self.buf[at..at + 4].copy_from_slice(&bytes);
        }
    }
}

impl Decoder<'_> {
    /// Reads what stands where a value is expected: `null`, an indirection
    /// to a value read before, or the header of a new value.
    ///
    /// After [`ValueHeader::Value`], the caller reads the state and then
    /// calls [`Decoder::end_value`].
    pub fn read_value_header(&mut self) -> Result<ValueHeader> {
        let state = self.values.get_or_insert_with(Default::default);
        let outer_chunked = state.frames.last().is_some_and(|f| f.chunked);
        let (at, tag, in_chunk) = self.read_value_tag(outer_chunked)?;
        match tag {
            0 => Ok(ValueHeader::Null),
            INDIRECTION => {
                let offset_at = self.pos;
                let offset = if in_chunk {
                    self.read_i32()?
                } else {
                    self.read_u32_raw()? as i32
                };
                let target = offset_at as i64 + i64::from(offset);
                let slot = usize::try_from(target)
                    .ok()
                    .and_then(|target| self.state().tags.get(&target).copied())
                    .ok_or(Error::InvalidValue("indirection to no value"))?;
                Ok(ValueHeader::Indirection(slot))
            }
            tag if tag & 0xffff_fff0 == VALUE_TAG => {
                if in_chunk {
                    return Err(Error::InvalidValue("value tag inside a chunk"));
                }
                let chunked = tag & CHUNKED != 0;
                if outer_chunked && !chunked {
                    return Err(Error::InvalidValue("unchunked value in a chunked value"));
                }
                if tag & CODEBASE != 0 {
                    self.read_shared_string()?;
                }
                let ids = match tag & ID_LIST {
                    0 => Vec::new(),
                    SINGLE_ID => vec![self.read_shared_string()?],
                    ID_LIST => self.read_id_list()?,
                    _ => return Err(Error::InvalidValue("invalid value tag")),
                };
                let state = self.state();
                let slot = state.slots;
                state.slots += 1;
                state.tags.insert(at, slot);
                let depth = state.frames.len() + 1;
                state.frames.push(Frame { chunked, depth });
                state.chunk_end = None;
                Ok(ValueHeader::Value { slot, ids, chunked })
            }
            _ => Err(Error::InvalidValue("invalid value tag")),
        }
    }

    /// Ends the value whose header was read last and is not yet ended.
    ///
    /// For a chunked value, the rest of its state (chunks and nested
    /// values a truncating reader did not read) is skipped up to its end
    /// tag.
    pub fn end_value(&mut self) -> Result<()> {
        let frame = *self
            .values
            .as_ref()
            .and_then(|s| s.frames.last())
            .ok_or(Error::InvalidValue("end_value without a value"))?;
        if frame.chunked {
            let ended = self.state().ended;
            if ended == 0 || ended > frame.depth {
                self.skip_to_end_tag(frame.depth)?;
            }
        }
        let state = self.state();
        state.frames.pop();
        state.chunk_end = None;
        if state.frames.last().is_none_or(|f| f.depth < state.ended) {
            state.ended = 0;
        }
        Ok(())
    }

    /// Skips the rest of a chunked value at `depth` and reads its end tag.
    fn skip_to_end_tag(&mut self, depth: usize) -> Result<()> {
        loop {
            if let Some(end) = self.state().chunk_end.take() {
                if end > self.pos {
                    self.pos = end;
                }
            }
            self.align_raw(4)?;
            let at = self.pos;
            let tag = self.read_u32_raw()?;
            match tag {
                // A null value between chunks.
                0 => {}
                tag if tag >= 0x8000_0000 => {
                    // An end tag ends this value and the values nested in
                    // it; some writers count only chunked values, so a
                    // deeper tag still ends this one.
                    let ended = (tag as i32).unsigned_abs() as usize;
                    self.state().ended = ended.min(depth);
                    return Ok(());
                }
                tag if tag < VALUE_TAG => self.enter_chunk(tag)?,
                _ => {
                    self.pos = at;
                    if let ValueHeader::Value { .. } = self.read_value_header()? {
                        self.end_value()?;
                    }
                }
            }
        }
    }

    /// Reads a value tag, entering chunks before it. Returns the buffer
    /// index of the tag, the tag, and whether it was inside a chunk.
    fn read_value_tag(&mut self, outer_chunked: bool) -> Result<(usize, u32, bool)> {
        loop {
            if outer_chunked {
                if let Some(end) = self.chunk_end() {
                    let rem = self.position() % 4;
                    if rem != 0 {
                        let pad = (4 - rem).min(end - self.pos);
                        self.read_octets_raw(pad)?;
                    }
                    if self.pos < end {
                        self.check_in_chunk(4)?;
                        let at = self.pos;
                        return Ok((at, self.read_u32_raw()?, true));
                    }
                    self.state().chunk_end = None;
                }
            }
            self.align_raw(4)?;
            let at = self.pos;
            let tag = self.read_u32_raw()?;
            if outer_chunked && tag != 0 && tag < VALUE_TAG {
                self.enter_chunk(tag)?;
                continue;
            }
            return Ok((at, tag, false));
        }
    }

    /// Starts a chunk of `len` octets at the current position.
    fn enter_chunk(&mut self, len: u32) -> Result<()> {
        let remaining = self.remaining();
        if len as usize > remaining {
            return Err(Error::LengthTooLarge { len, remaining });
        }
        let end = self.pos + len as usize;
        self.state().chunk_end = Some(end);
        Ok(())
    }

    /// Reads a repository id or codebase URL, which may be an indirection
    /// to the same string read before.
    fn read_shared_string(&mut self) -> Result<String> {
        self.align_raw(4)?;
        let at = self.pos;
        let len = self.read_u32_raw()?;
        if len == INDIRECTION {
            let target = self.read_offset()?;
            return self
                .state()
                .strings
                .get(&target)
                .cloned()
                .ok_or(Error::InvalidValue("indirection to no repository id"));
        }
        let remaining = self.remaining();
        if len as usize > remaining {
            return Err(Error::LengthTooLarge { len, remaining });
        }
        let bytes = self.read_octets_raw(len as usize)?;
        let bytes = match bytes.split_last() {
            None => bytes,
            Some((0, rest)) => rest,
            Some(_) => return Err(Error::StringNotTerminated),
        };
        let s = String::from_utf8(bytes.to_vec()).map_err(|_| Error::InvalidUtf8)?;
        self.state().strings.insert(at, s.clone());
        Ok(s)
    }

    /// Reads a list of repository ids, which may be an indirection to the
    /// same list read before.
    fn read_id_list(&mut self) -> Result<Vec<String>> {
        self.align_raw(4)?;
        let at = self.pos;
        let count = self.read_u32_raw()?;
        if count == INDIRECTION {
            let target = self.read_offset()?;
            return self
                .state()
                .lists
                .get(&target)
                .cloned()
                .ok_or(Error::InvalidValue("indirection to no repository id list"));
        }
        let remaining = self.remaining();
        if (count as usize).saturating_mul(4) > remaining {
            return Err(Error::LengthTooLarge {
                len: count,
                remaining,
            });
        }
        let ids = (0..count)
            .map(|_| self.read_shared_string())
            .collect::<Result<Vec<_>>>()?;
        self.state().lists.insert(at, ids.clone());
        Ok(ids)
    }

    /// Reads an indirection offset outside chunks and returns the buffer
    /// index it points to.
    fn read_offset(&mut self) -> Result<usize> {
        let at = self.pos;
        let offset = self.read_u32_raw()? as i32;
        usize::try_from(at as i64 + i64::from(offset))
            .map_err(|_| Error::InvalidValue("indirection before the stream"))
    }

    /// Before data is read in a chunked value: if no chunk is open, or the
    /// open one ended, reads the length of the next chunk.
    pub(crate) fn chunk_boundary(&mut self) -> Result<()> {
        if !self.in_chunked_value() {
            return Ok(());
        }
        loop {
            if self.chunk_end().is_some_and(|end| self.pos < end) {
                return Ok(());
            }
            let len = self.read_u32_raw()?;
            if len >= VALUE_TAG {
                return Err(Error::InvalidValue("chunk expected"));
            }
            self.enter_chunk(len)?;
        }
    }

    /// Fails if `n` octets do not fit in the open chunk.
    pub(crate) fn check_in_chunk(&self, n: usize) -> Result<()> {
        match self.chunk_end() {
            Some(end) if self.pos + n > end => Err(Error::InvalidValue("data crosses a chunk end")),
            _ => Ok(()),
        }
    }

    /// The buffer index where the open chunk ends, inside a chunked value.
    pub(crate) fn chunk_end(&self) -> Option<usize> {
        if self.in_chunked_value() {
            self.values.as_ref().and_then(|s| s.chunk_end)
        } else {
            None
        }
    }

    fn in_chunked_value(&self) -> bool {
        self.values
            .as_ref()
            .and_then(|s| s.frames.last())
            .is_some_and(|f| f.chunked)
    }

    fn state(&mut self) -> &mut ReadState {
        self.values.get_or_insert_with(Default::default)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ByteOrder, Encoding};

    fn encoder(order: ByteOrder) -> Encoder {
        Encoder::new(order, Encoding::Giop1_2)
    }

    fn decoder(bytes: &[u8], order: ByteOrder) -> Decoder<'_> {
        Decoder::new(bytes, order, Encoding::Giop1_2)
    }

    #[test]
    fn plain_value_and_sharing() {
        for order in [ByteOrder::Big, ByteOrder::Little] {
            let mut enc = encoder(order);
            enc.write_octet(7);
            assert!(enc.begin_value(1, &["IDL:V:1.0"], false).unwrap());
            enc.write_i32(42);
            enc.write_f64(1.5);
            enc.end_value().unwrap();
            assert!(!enc.begin_value(1, &["IDL:V:1.0"], false).unwrap());
            enc.write_null_value();
            enc.write_u16(9);
            let bytes = enc.into_bytes();
            assert_eq!(&bytes[4..8], &tag_bytes(0x7fffff02, order));

            let mut dec = decoder(&bytes, order);
            assert_eq!(dec.read_octet().unwrap(), 7);
            assert_eq!(
                dec.read_value_header().unwrap(),
                ValueHeader::Value {
                    slot: 0,
                    ids: vec!["IDL:V:1.0".into()],
                    chunked: false
                }
            );
            assert_eq!(dec.read_i32().unwrap(), 42);
            assert_eq!(dec.read_f64().unwrap(), 1.5);
            dec.end_value().unwrap();
            assert_eq!(
                dec.read_value_header().unwrap(),
                ValueHeader::Indirection(0)
            );
            assert_eq!(dec.read_value_header().unwrap(), ValueHeader::Null);
            assert_eq!(dec.read_u16().unwrap(), 9);
            assert!(dec.is_empty());
        }
    }

    fn tag_bytes(tag: u32, order: ByteOrder) -> [u8; 4] {
        match order {
            ByteOrder::Big => tag.to_be_bytes(),
            ByteOrder::Little => tag.to_le_bytes(),
        }
    }

    #[test]
    fn chunked_values_nest() {
        let order = ByteOrder::Big;
        let mut enc = encoder(order);
        assert!(
            enc.begin_value(1, &["IDL:D:1.0", "IDL:B:1.0"], true)
                .unwrap()
        );
        enc.write_i32(1);
        // An unchunked request inside a chunked value is chunked.
        assert!(enc.begin_value(2, &["IDL:N:1.0"], false).unwrap());
        enc.write_f64(2.0);
        enc.write_null_value();
        assert!(!enc.begin_value(1, &[], false).unwrap());
        enc.end_value().unwrap();
        enc.write_octet(3);
        enc.end_value().unwrap();
        enc.write_i32(4);
        let bytes = enc.into_bytes();

        let mut dec = decoder(&bytes, order);
        assert_eq!(
            dec.read_value_header().unwrap(),
            ValueHeader::Value {
                slot: 0,
                ids: vec!["IDL:D:1.0".into(), "IDL:B:1.0".into()],
                chunked: true
            }
        );
        assert_eq!(dec.read_i32().unwrap(), 1);
        assert_eq!(
            dec.read_value_header().unwrap(),
            ValueHeader::Value {
                slot: 1,
                ids: vec!["IDL:N:1.0".into()],
                chunked: true
            }
        );
        assert_eq!(dec.read_f64().unwrap(), 2.0);
        assert_eq!(dec.read_value_header().unwrap(), ValueHeader::Null);
        assert_eq!(
            dec.read_value_header().unwrap(),
            ValueHeader::Indirection(0)
        );
        dec.end_value().unwrap();
        assert_eq!(dec.read_octet().unwrap(), 3);
        dec.end_value().unwrap();
        assert_eq!(dec.read_i32().unwrap(), 4);
        assert!(dec.is_empty());
    }

    #[test]
    fn truncation_skips_unread_state() {
        let order = ByteOrder::Little;
        let mut enc = encoder(order);
        assert!(
            enc.begin_value(1, &["IDL:D:1.0", "IDL:B:1.0"], true)
                .unwrap()
        );
        enc.write_i32(1);
        // State of the derived type, with a nested value, that a reader
        // knowing only the base type skips.
        enc.write_string("derived").unwrap();
        assert!(enc.begin_value(2, &["IDL:N:1.0"], false).unwrap());
        enc.write_f64(2.0);
        enc.end_value().unwrap();
        enc.write_octet(5);
        enc.end_value().unwrap();
        enc.write_i32(4);
        let bytes = enc.into_bytes();

        let mut dec = decoder(&bytes, order);
        assert!(matches!(
            dec.read_value_header().unwrap(),
            ValueHeader::Value { chunked: true, .. }
        ));
        assert_eq!(dec.read_i32().unwrap(), 1);
        dec.end_value().unwrap();
        assert_eq!(dec.read_i32().unwrap(), 4);
        assert!(dec.is_empty());
    }

    /// A stream written by hand: data split across chunks in the middle
    /// of padding, a merged end tag, and repository ids by indirection.
    #[test]
    fn reads_other_writers_forms() {
        let mut b: Vec<u8> = Vec::new();
        let long = |b: &mut Vec<u8>, v: u32| b.extend_from_slice(&v.to_be_bytes());
        // Outer chunked value with one id.
        long(&mut b, 0x7fffff0a);
        long(&mut b, 4);
        b.extend_from_slice(b"IDL\0");
        let id_at = 4;
        // A chunk holding an octet only.
        long(&mut b, 1);
        b.push(1);
        b.extend_from_slice(&[0; 3]);
        // The next chunk holds the long that follows the octet.
        long(&mut b, 4);
        long(&mut b, 2);
        // Nested chunked value whose id is an indirection.
        long(&mut b, 0x7fffff0a);
        long(&mut b, 0xffffffff);
        let at = b.len();
        long(&mut b, (id_at as i64 - at as i64) as i32 as u32);
        long(&mut b, 2);
        b.extend_from_slice(&[0x12, 0x34]);
        // One end tag ends both values.
        b.extend_from_slice(&[0; 2]);
        long(&mut b, -1i32 as u32);
        long(&mut b, 9);

        let mut dec = decoder(&b, ByteOrder::Big);
        assert!(matches!(
            dec.read_value_header().unwrap(),
            ValueHeader::Value { ref ids, .. } if ids == &["IDL"]
        ));
        assert_eq!(dec.read_octet().unwrap(), 1);
        assert_eq!(dec.read_i32().unwrap(), 2);
        assert!(matches!(
            dec.read_value_header().unwrap(),
            ValueHeader::Value { slot: 1, ref ids, .. } if ids == &["IDL"]
        ));
        assert_eq!(dec.read_u16().unwrap(), 0x1234);
        dec.end_value().unwrap();
        dec.end_value().unwrap();
        assert_eq!(dec.read_u32().unwrap(), 9);
        assert!(dec.is_empty());
    }

    #[test]
    fn rejects_bad_streams() {
        let mut dec = decoder(&[0x7f, 0xff, 0xff, 0x10], ByteOrder::Big);
        assert!(dec.read_value_header().is_err());
        let mut dec = decoder(&[0xff, 0xff, 0xff, 0xff, 0, 0, 0, 0], ByteOrder::Big);
        assert!(dec.read_value_header().is_err());
        // Data past the end of the last chunk.
        let mut b = Vec::new();
        b.extend_from_slice(&0x7fffff08u32.to_be_bytes());
        b.extend_from_slice(&2u32.to_be_bytes());
        b.extend_from_slice(&[0, 0]);
        b.extend_from_slice(&[0, 0]);
        b.extend_from_slice(&(-1i32).to_be_bytes());
        let mut dec = decoder(&b, ByteOrder::Big);
        dec.read_value_header().unwrap();
        assert!(dec.read_i32().is_err());
        let mut enc = encoder(ByteOrder::Big);
        assert!(enc.end_value().is_err());
    }
}

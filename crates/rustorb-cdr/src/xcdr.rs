//! The headers of DDS-XTypes Extended CDR (XCDR), clause 7.4 of
//! DDS-XTypes 1.3, and the RTPS SerializedPayload header around them
//! (DDSI-RTPS 2.5 clause 10).
//!
//! The encoder and decoder write and read values with the alignment of
//! [`Encoding::Xcdr1`] or [`Encoding::Xcdr2`]; what is here adds the
//! pieces that only XCDR has:
//!
//! - the encapsulation header of a payload
//!   ([`Encoder::xcdr_payload`], [`Decoder::xcdr_payload`]), after which
//!   alignment restarts, and whose options record the final padding;
//! - the delimiter header (DHEADER) of XCDR2 appendable and mutable types,
//!   and of XCDR2 collections of non-primitive elements
//!   ([`Encoder::begin_dheader`], [`Decoder::read_dheader`]);
//! - the member header (EMHEADER) of XCDR2 mutable types
//!   ([`Encoder::begin_member`], [`Decoder::read_member_header`]);
//! - the parameter headers of XCDR1 mutable types and optional members
//!   ([`Encoder::begin_parameter`], [`Decoder::read_parameter`]).
//!
//! Which of them a type uses depends on its extensibility and the
//! encoding version (Table 35, [`XcdrFormat::new`]); that choice belongs
//! to the code that knows the type.

use crate::{ByteOrder, Decoder, Encoder, Encoding, Error, Result};

/// The extensibility kind of a type (DDS-XTypes 7.2.2.4.4.4.6, `@final`,
/// `@appendable`, `@mutable`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Extensibility {
    /// `@final`.
    Final,
    /// `@appendable`, the default.
    Appendable,
    /// `@mutable`.
    Mutable,
}

/// The format of an XCDR payload: which encoding rules its top-level
/// object follows (DDS-XTypes Table 35).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum XcdrFormat {
    /// PLAIN_CDR: final or appendable types, encoding version 1.
    PlainCdr,
    /// PL_CDR: mutable types, encoding version 1.
    PlCdr,
    /// PLAIN_CDR2: final types, encoding version 2.
    PlainCdr2,
    /// DELIMITED_CDR: appendable types, encoding version 2.
    DelimitedCdr2,
    /// PL_CDR2: mutable types, encoding version 2.
    PlCdr2,
}

impl XcdrFormat {
    /// The format of a type of extensibility `kind` written with
    /// `encoding`, or `None` when that is not an XCDR encoding.
    pub fn new(encoding: Encoding, kind: Extensibility) -> Option<XcdrFormat> {
        use Extensibility::*;
        Some(match (encoding, kind) {
            (Encoding::Xcdr1, Final | Appendable) => XcdrFormat::PlainCdr,
            (Encoding::Xcdr1, Mutable) => XcdrFormat::PlCdr,
            (Encoding::Xcdr2, Final) => XcdrFormat::PlainCdr2,
            (Encoding::Xcdr2, Appendable) => XcdrFormat::DelimitedCdr2,
            (Encoding::Xcdr2, Mutable) => XcdrFormat::PlCdr2,
            _ => return None,
        })
    }

    /// The encoding version: [`Encoding::Xcdr1`] or [`Encoding::Xcdr2`].
    pub fn encoding(self) -> Encoding {
        match self {
            XcdrFormat::PlainCdr | XcdrFormat::PlCdr => Encoding::Xcdr1,
            _ => Encoding::Xcdr2,
        }
    }
}

/// The encapsulation identifier at the start of a payload: a format and a
/// byte order (DDS-XTypes Table 39, DDSI-RTPS 2.5 Table 10.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EncapsulationId {
    /// The format of the top-level object.
    pub format: XcdrFormat,
    /// The byte order of the payload.
    pub order: ByteOrder,
}

impl EncapsulationId {
    /// The two octets of the identifier.
    ///
    /// The encoding version 2 values are those of DDS-XTypes Table 39 and
    /// DDSI-RTPS 2.5 Table 10.3 (`0x10` to `0x15`). DDS-XTypes Table 60
    /// lists other values (`0x06` to `0x0b`) for the same formats; they
    /// are accepted by [`from_bytes`](EncapsulationId::from_bytes) but
    /// never written.
    pub fn to_bytes(self) -> [u8; 2] {
        let base = match self.format {
            XcdrFormat::PlainCdr => 0x00,
            XcdrFormat::PlCdr => 0x02,
            XcdrFormat::PlainCdr2 => 0x10,
            XcdrFormat::PlCdr2 => 0x12,
            XcdrFormat::DelimitedCdr2 => 0x14,
        };
        [0x00, base | self.order.flag()]
    }

    /// Reads the two octets of an identifier. XML (`0x0004`) and unknown
    /// identifiers are errors.
    pub fn from_bytes(bytes: [u8; 2]) -> Result<EncapsulationId> {
        let [0x00, id] = bytes else {
            return Err(Error::InvalidXcdr("unknown encapsulation identifier"));
        };
        let format = match id & !1 {
            0x00 => XcdrFormat::PlainCdr,
            0x02 => XcdrFormat::PlCdr,
            0x10 | 0x06 => XcdrFormat::PlainCdr2,
            0x12 | 0x0a => XcdrFormat::PlCdr2,
            0x14 | 0x08 => XcdrFormat::DelimitedCdr2,
            _ => return Err(Error::InvalidXcdr("unknown encapsulation identifier")),
        };
        // In every identifier the low bit is the byte order.
        let order = if id & 1 == 0 {
            ByteOrder::Big
        } else {
            ByteOrder::Little
        };
        Ok(EncapsulationId { format, order })
    }
}

/// The header of a payload read by [`Decoder::xcdr_payload`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PayloadHeader {
    /// The encapsulation identifier.
    pub id: EncapsulationId,
    /// The options octets as received. The low two bits of the second are
    /// the padding after the data, which the decoder leaves out.
    pub options: [u8; 2],
}

/// Parameter ID of an extended parameter header (DDS-XTypes Table 34),
/// with `FLAG_MUST_UNDERSTAND` set as the table requires.
pub const PID_EXTENDED: u16 = 0x7F01;
/// Parameter ID that ends an XCDR1 parameter list (DDS-XTypes Table 34),
/// with `FLAG_MUST_UNDERSTAND` set as the table requires.
pub const PID_LIST_END: u16 = 0x7F02;
/// Parameter ID of a parameter every reader ignores (DDS-XTypes Table 34).
pub const PID_IGNORE: u16 = 0x3F03;
/// Parameter ID that ends the parameter lists of the RTPS built-in topic
/// types (DDSI-RTPS 9.4.2.11).
pub const PID_SENTINEL: u16 = 0x0001;
/// The implementation-specific flag of a parameter ID.
pub const FLAG_IMPL_EXTENSION: u16 = 0x8000;
/// The must-understand flag of a parameter ID.
pub const FLAG_MUST_UNDERSTAND: u16 = 0x4000;

/// The largest member ID (28 bits), in an EMHEADER or an extended
/// parameter header.
pub const MAX_MEMBER_ID: u32 = 0x0FFF_FFFF;
/// The largest member ID a short parameter header holds (DDS-XTypes
/// 7.4.1.2.2).
pub const MAX_SHORT_PID: u32 = 0x3F00;

/// What ends an XCDR1 parameter list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ListEnd {
    /// [`PID_LIST_END`], for user types.
    Xtypes,
    /// [`PID_SENTINEL`], for the RTPS built-in topic types (whose member
    /// ID 1 is therefore never used). A reader also accepts
    /// [`PID_LIST_END`].
    Rtps,
}

impl ListEnd {
    fn pid(self) -> u16 {
        match self {
            ListEnd::Xtypes => PID_LIST_END,
            ListEnd::Rtps => PID_SENTINEL,
        }
    }
}

/// A DHEADER being written; see [`Encoder::begin_dheader`].
#[derive(Debug)]
#[must_use = "a DHEADER is written by end_dheader"]
pub struct DHeaderMark {
    at: usize,
    start: usize,
}

/// A member with an EMHEADER being written; see [`Encoder::begin_member`].
#[derive(Debug)]
#[must_use = "a member header is written by end_member"]
pub struct MemberMark {
    at: usize,
    start: usize,
    header: u32,
}

/// A parameter being written; see [`Encoder::begin_parameter`].
#[derive(Debug)]
#[must_use = "a parameter header is written by end_parameter"]
pub struct ParameterMark {
    at: usize,
    start: usize,
    id: u32,
    must_understand: bool,
    extended: bool,
    origin: usize,
}

/// An object read after its DHEADER; see [`Decoder::read_dheader`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use = "end_dheader skips what was not read"]
pub struct Delimited {
    end: usize,
}

impl Delimited {
    /// Whether `dec` has not reached the end of the object yet: more
    /// members of a mutable type, or more of an appendable one that the
    /// reader may not know.
    pub fn has_more(&self, dec: &Decoder<'_>) -> bool {
        dec.pos < self.end
    }

    /// Bytes of the object left after the position of `dec`.
    pub fn remaining(&self, dec: &Decoder<'_>) -> usize {
        self.end.saturating_sub(dec.pos)
    }
}

/// A member header read by [`Decoder::read_member_header`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use = "end_member skips what was not read"]
pub struct MemberHeader {
    /// The member ID.
    pub id: u32,
    /// Whether the member must be understood (`M_FLAG`).
    pub must_understand: bool,
    /// The length of the serialized member.
    pub len: usize,
    end: usize,
}

/// A parameter header read by [`Decoder::read_parameter`] or
/// [`Decoder::read_parameter_header`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use = "end_parameter skips what was not read and restores the alignment origin"]
pub struct Parameter {
    /// The member ID (the low 14 bits of a short parameter ID, or the 28
    /// bits of an extended header).
    pub id: u32,
    /// `FLAG_MUST_UNDERSTAND`.
    pub must_understand: bool,
    /// `FLAG_IMPL_EXTENSION`.
    pub impl_extension: bool,
    /// The length of the serialized member: 0 for an absent optional
    /// member.
    pub len: usize,
    end: usize,
    origin: usize,
}

impl Encoder {
    /// Starts an RTPS SerializedPayload: the encapsulation identifier of
    /// `format` and `order`, the options, and alignment counted from the
    /// byte after them (DDSI-RTPS 2.5 10.2, DDS-XTypes 7.4.3.5.3 rule 1).
    /// End it with [`finish_xcdr_payload`](Encoder::finish_xcdr_payload).
    pub fn xcdr_payload(format: XcdrFormat, order: ByteOrder) -> Encoder {
        let mut enc = Encoder::new(order, format.encoding());
        enc.write_octets(&EncapsulationId { format, order }.to_bytes());
        enc.write_octets(&[0, 0]);
        enc.origin = 4;
        enc
    }

    /// Ends a payload started with
    /// [`xcdr_payload`](Encoder::xcdr_payload): zero padding up to a
    /// multiple of 4 octets, its length recorded in the options
    /// (DDS-XTypes 7.6.3.1.2), and the bytes.
    pub fn finish_xcdr_payload(mut self) -> Vec<u8> {
        let pad = (4 - self.len() % 4) % 4;
        self.buf.resize(self.buf.len() + pad, 0);
        if let Some(options) = self.buf.get_mut(3) {
            *options = (*options & !3) | pad as u8;
        }
        self.into_bytes()
    }

    fn patch_u32(&mut self, at: usize, value: u32) {
        let bytes = match self.order {
            ByteOrder::Big => value.to_be_bytes(),
            ByteOrder::Little => value.to_le_bytes(),
        };
        self.buf[at..at + 4].copy_from_slice(&bytes);
    }

    fn patch_u16(&mut self, at: usize, value: u16) {
        let bytes = match self.order {
            ByteOrder::Big => value.to_be_bytes(),
            ByteOrder::Little => value.to_le_bytes(),
        };
        self.buf[at..at + 2].copy_from_slice(&bytes);
    }

    fn u16_bytes(&self, value: u16) -> [u8; 2] {
        match self.order {
            ByteOrder::Big => value.to_be_bytes(),
            ByteOrder::Little => value.to_le_bytes(),
        }
    }

    fn u32_bytes(&self, value: u32) -> [u8; 4] {
        match self.order {
            ByteOrder::Big => value.to_be_bytes(),
            ByteOrder::Little => value.to_le_bytes(),
        }
    }

    /// Writes a DHEADER to be filled in by
    /// [`end_dheader`](Encoder::end_dheader) with the length of what is
    /// written in between (DDS-XTypes 7.4.3.4.1).
    ///
    /// Marks of [`begin_dheader`](Encoder::begin_dheader),
    /// [`begin_member`](Encoder::begin_member) and
    /// [`begin_parameter`](Encoder::begin_parameter) must be ended
    /// innermost first.
    pub fn begin_dheader(&mut self) -> DHeaderMark {
        self.align(4);
        let at = self.buf.len();
        self.write_u32(0);
        DHeaderMark {
            at,
            start: self.len(),
        }
    }

    /// Fills in the DHEADER of `mark`.
    pub fn end_dheader(&mut self, mark: DHeaderMark) -> Result<()> {
        let size = self.len() - mark.start;
        let size = u32::try_from(size).map_err(|_| Error::LengthOverflow(size))?;
        self.patch_u32(mark.at, size);
        Ok(())
    }

    /// Starts a member of an XCDR2 mutable type: an EMHEADER for member
    /// `id`, filled in by [`end_member`](Encoder::end_member) with the
    /// length of what is written in between (DDS-XTypes 7.4.3.4.2, rule
    /// 22). Only for [`Encoding::Xcdr2`].
    pub fn begin_member(&mut self, id: u32, must_understand: bool) -> Result<MemberMark> {
        if self.encoding() != Encoding::Xcdr2 {
            return Err(Error::InvalidXcdr("EMHEADER outside XCDR2"));
        }
        if id > MAX_MEMBER_ID {
            return Err(Error::InvalidXcdr("member ID larger than 28 bits"));
        }
        self.align(4);
        let at = self.buf.len();
        // EMHEADER1 and NEXTINT, which end_member drops when a length code
        // of 0 to 3 is enough.
        self.write_u32(0);
        self.write_u32(0);
        Ok(MemberMark {
            at,
            start: self.len(),
            header: (u32::from(must_understand) << 31) | id,
        })
    }

    /// Fills in the EMHEADER of `mark`. A member of 1, 2, 4 or 8 octets
    /// gets a length code of 0 to 3 and no NEXTINT; any other gets length
    /// code 4 and its length in NEXTINT. (Length codes 5 to 7, which share
    /// NEXTINT with the member, are read but not written.)
    pub fn end_member(&mut self, mark: MemberMark) -> Result<()> {
        let size = self.len() - mark.start;
        let lc = match size {
            1 => Some(0),
            2 => Some(1),
            4 => Some(2),
            8 => Some(3),
            _ => None,
        };
        match lc {
            // Dropping NEXTINT moves the member back by 4 octets, which
            // keeps its alignment: XCDR2 aligns to no more than 4.
            Some(lc) => {
                self.buf.drain(mark.at + 4..mark.at + 8);
                self.patch_u32(mark.at, mark.header | (lc << 28));
            }
            None => {
                let size = u32::try_from(size).map_err(|_| Error::LengthOverflow(size))?;
                self.patch_u32(mark.at, mark.header | (4 << 28));
                self.patch_u32(mark.at + 4, size);
            }
        }
        Ok(())
    }

    /// Starts a parameter of an XCDR1 parameter list, or the header of an
    /// XCDR1 optional member: a parameter header for member `id`, filled
    /// in by [`end_parameter`](Encoder::end_parameter) with the length of
    /// what is written in between (DDS-XTypes 7.4.1.2, rules 24 and 25).
    ///
    /// The header is a short one (16-bit ID and length) when `id` is at
    /// most [`MAX_SHORT_PID`], and an extended one ([`PID_EXTENDED`])
    /// otherwise or when the member turns out longer than 65535 octets.
    /// Alignment counts from the byte after the header until
    /// `end_parameter`. An absent optional member is a parameter with
    /// nothing written in it.
    pub fn begin_parameter(&mut self, id: u32, must_understand: bool) -> Result<ParameterMark> {
        if id > MAX_MEMBER_ID {
            return Err(Error::InvalidXcdr("member ID larger than 28 bits"));
        }
        self.align(4);
        let at = self.buf.len();
        let extended = id > MAX_SHORT_PID;
        if extended {
            self.write_u16(PID_EXTENDED);
            self.write_u16(8);
            self.write_u32(extended_header(id, must_understand));
            self.write_u32(0);
        } else {
            let flag = if must_understand {
                FLAG_MUST_UNDERSTAND
            } else {
                0
            };
            self.write_u16(flag | id as u16);
            self.write_u16(0);
        }
        let mark = ParameterMark {
            at,
            start: self.len(),
            id,
            must_understand,
            extended,
            origin: self.origin,
        };
        self.origin = self.position();
        Ok(mark)
    }

    /// Fills in the parameter header of `mark` and restores the alignment
    /// origin. The length is that of the member, without padding
    /// (DDS-XTypes 7.4.1.2).
    pub fn end_parameter(&mut self, mark: ParameterMark) -> Result<()> {
        let size = self.len() - mark.start;
        let size32 = u32::try_from(size).map_err(|_| Error::LengthOverflow(size))?;
        if mark.extended {
            self.patch_u32(mark.at + 8, size32);
        } else if let Ok(short) = u16::try_from(size) {
            self.patch_u16(mark.at + 2, short);
        } else {
            // Too long for a short header: make it an extended one. The
            // member moves with its alignment origin, so its padding stays
            // right.
            let mut header = Vec::with_capacity(12);
            header.extend_from_slice(&self.u16_bytes(PID_EXTENDED));
            header.extend_from_slice(&self.u16_bytes(8));
            header
                .extend_from_slice(&self.u32_bytes(extended_header(mark.id, mark.must_understand)));
            header.extend_from_slice(&self.u32_bytes(size32));
            self.buf.splice(mark.at..mark.at + 4, header);
        }
        self.origin = mark.origin;
        Ok(())
    }

    /// Ends an XCDR1 parameter list with `end` and a zero length (rule
    /// 23).
    pub fn end_parameter_list(&mut self, end: ListEnd) {
        self.align(4);
        self.write_u16(end.pid());
        self.write_u16(0);
    }
}

/// The `eMemberHeader` of an extended parameter header: the
/// must-understand flag (`FLAG_2`) and the member ID.
fn extended_header(id: u32, must_understand: bool) -> u32 {
    (u32::from(must_understand) << 30) | id
}

impl<'a> Decoder<'a> {
    /// Opens an RTPS SerializedPayload: reads the encapsulation header and
    /// returns a decoder positioned after it, with alignment counted from
    /// there, and the header. Padding the options say follows the data is
    /// left out of the decoder.
    pub fn xcdr_payload(buf: &'a [u8]) -> Result<(Decoder<'a>, PayloadHeader)> {
        let [a, b, o1, o2, ..] = *buf else {
            return Err(Error::UnexpectedEof {
                needed: 4,
                remaining: buf.len(),
            });
        };
        let id = EncapsulationId::from_bytes([a, b])?;
        let pad = usize::from(o2 & 3);
        let end = buf
            .len()
            .checked_sub(pad)
            .filter(|&end| end >= 4)
            .ok_or(Error::InvalidXcdr("payload padding longer than the data"))?;
        let mut dec = Decoder::new(&buf[..end], id.order, id.format.encoding());
        dec.pos = 4;
        dec.origin = 4;
        Ok((
            dec,
            PayloadHeader {
                id,
                options: [o1, o2],
            },
        ))
    }

    /// Reads a DHEADER (DDS-XTypes 7.4.3.4.1). Read the object, then call
    /// [`end_dheader`](Decoder::end_dheader), which skips what was not
    /// read: members that a newer version of an appendable type added.
    pub fn read_dheader(&mut self) -> Result<Delimited> {
        let size = self.read_u32()? as usize;
        if size > self.remaining() {
            return Err(Error::InvalidXcdr("DHEADER longer than the data"));
        }
        Ok(Delimited {
            end: self.pos + size,
        })
    }

    /// Moves to the end of the object of `delimited`. Reading past it is
    /// an error.
    pub fn end_dheader(&mut self, delimited: Delimited) -> Result<()> {
        if self.pos > delimited.end {
            return Err(Error::InvalidXcdr("object longer than its DHEADER"));
        }
        self.pos = delimited.end;
        Ok(())
    }

    /// Reads the EMHEADER of a member of an XCDR2 mutable type
    /// (DDS-XTypes 7.4.3.4.2) and leaves the decoder at the member. Read
    /// it or not, then call [`end_member`](Decoder::end_member).
    ///
    /// With length codes 5 to 7, NEXTINT is also the first 4 octets of the
    /// member (rule 22), so the decoder is left on it, and the length
    /// counts it: 4 + NEXTINT, 4 + 4 × NEXTINT or 4 + 8 × NEXTINT octets.
    pub fn read_member_header(&mut self) -> Result<MemberHeader> {
        let header = self.read_u32()?;
        let must_understand = header >> 31 == 1;
        let lc = (header >> 28) & 7;
        let id = header & MAX_MEMBER_ID;
        let len = match lc {
            0 => 1,
            1 => 2,
            2 => 4,
            3 => 8,
            _ => {
                let next = self.read_u32()? as usize;
                if lc == 4 {
                    next
                } else {
                    self.pos -= 4;
                    let unit = match lc {
                        5 => 1,
                        6 => 4,
                        _ => 8,
                    };
                    next.checked_mul(unit)
                        .and_then(|n| n.checked_add(4))
                        .ok_or(Error::InvalidXcdr("member longer than the data"))?
                }
            }
        };
        if len > self.remaining() {
            return Err(Error::InvalidXcdr("member longer than the data"));
        }
        Ok(MemberHeader {
            id,
            must_understand,
            len,
            end: self.pos + len,
        })
    }

    /// Moves to the end of the member of `header`. Reading past it is an
    /// error.
    pub fn end_member(&mut self, header: MemberHeader) -> Result<()> {
        if self.pos > header.end {
            return Err(Error::InvalidXcdr("member longer than its EMHEADER"));
        }
        self.pos = header.end;
        Ok(())
    }

    /// Reads a parameter header of XCDR1 (DDS-XTypes 7.4.1.2), short or
    /// extended, and makes alignment count from the byte after it. Read
    /// the member or not, then call
    /// [`end_parameter`](Decoder::end_parameter).
    ///
    /// This reads the header in front of an optional member of a final or
    /// appendable type, where a length of 0 means the member is absent.
    /// In a parameter list use [`read_parameter`](Decoder::read_parameter),
    /// which knows its end.
    pub fn read_parameter_header(&mut self) -> Result<Parameter> {
        self.align(4)?;
        let pid = self.read_u16()?;
        let short_len = self.read_u16()?;
        let (id, must_understand, impl_extension, len) =
            if pid & !(FLAG_MUST_UNDERSTAND | FLAG_IMPL_EXTENSION) == PID_EXTENDED & 0x3FFF
                && pid & FLAG_IMPL_EXTENSION == 0
            {
                if short_len != 8 {
                    return Err(Error::InvalidXcdr("PID_EXTENDED length is not 8"));
                }
                let header = self.read_u32()?;
                let len = self.read_u32()? as usize;
                (
                    header & MAX_MEMBER_ID,
                    header & (1 << 30) != 0,
                    header & (1 << 31) != 0,
                    len,
                )
            } else {
                (
                    u32::from(pid & 0x3FFF),
                    pid & FLAG_MUST_UNDERSTAND != 0,
                    pid & FLAG_IMPL_EXTENSION != 0,
                    usize::from(short_len),
                )
            };
        if len > self.remaining() {
            return Err(Error::InvalidXcdr("parameter longer than the data"));
        }
        let origin = self.origin;
        self.origin = self.position();
        Ok(Parameter {
            id,
            must_understand,
            impl_extension,
            len,
            end: self.pos + len,
            origin,
        })
    }

    /// Reads the next parameter header of an XCDR1 parameter list, or
    /// `None` at its end. Parameters with [`PID_IGNORE`] are skipped. The
    /// list ends with [`PID_LIST_END`] (with any flags), or for
    /// [`ListEnd::Rtps`] also with [`PID_SENTINEL`].
    pub fn read_parameter(&mut self, list_end: ListEnd) -> Result<Option<Parameter>> {
        loop {
            // The end of the list has a length that is not looked at.
            self.align(4)?;
            if let [a, b, ..] = *self.remaining_bytes() {
                let pid = match self.order {
                    ByteOrder::Big => u16::from_be_bytes([a, b]),
                    ByteOrder::Little => u16::from_le_bytes([a, b]),
                };
                if pid & !FLAG_MUST_UNDERSTAND == PID_LIST_END & 0x3FFF
                    || list_end == ListEnd::Rtps && pid == PID_SENTINEL
                {
                    self.skip(4)?;
                    return Ok(None);
                }
            }
            let param = self.read_parameter_header()?;
            if !param.impl_extension && param.id == u32::from(PID_IGNORE) {
                self.end_parameter(param)?;
                continue;
            }
            return Ok(Some(param));
        }
    }

    /// Moves to the end of the parameter `param` and restores the
    /// alignment origin. Reading past it is an error.
    pub fn end_parameter(&mut self, param: Parameter) -> Result<()> {
        if self.pos > param.end {
            return Err(Error::InvalidXcdr("member longer than its parameter"));
        }
        self.pos = param.end;
        self.origin = param.origin;
        Ok(())
    }
}

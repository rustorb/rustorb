//! TypeCodes in CDR (CORBA 3.0, 15.3.5.1).
//!
//! A TypeCode is its kind as an `unsigned long`, followed by its
//! parameters: none for the simple kinds, a bound for strings, the digits
//! and scale for `fixed`, and an encapsulation for the others. A TypeCode
//! nested in another one may instead be an indirection, the kind
//! `0xffffffff` followed by a `long` offset from that `long` to the kind of
//! an enclosing TypeCode. Offsets count octets of the whole stream, across
//! encapsulation boundaries.
//!
//! Writing uses an indirection only for a recursive reference, and writes
//! other repeated types in full. Reading accepts an indirection to any
//! TypeCode that started earlier in the same outermost TypeCode.

use std::collections::HashMap;
use std::sync::Arc;

use rustorb_cdr::{Decoder, Encoder, Error, Result};

use super::{Arena, Node, TCKind, TypeCode};

/// The kind value of an indirection.
const INDIRECTION: u32 = 0xffff_ffff;

/// How deep TypeCodes may nest when read, against stack exhaustion.
const MAX_DEPTH: usize = 128;

impl TypeCode {
    /// Writes the TypeCode. Fails with [`Error::InvalidTypeCode`] for an
    /// unresolved recursive placeholder.
    pub fn encode(&self, enc: &mut Encoder) -> Result<()> {
        Writer {
            arena: &self.arena,
            open: HashMap::new(),
        }
        .write(enc, 0, self.index)
    }

    /// Reads a TypeCode.
    pub fn decode(dec: &mut Decoder<'_>) -> Result<TypeCode> {
        let mut r = Reader {
            nodes: Vec::new(),
            starts: HashMap::new(),
            depth: 0,
        };
        let index = r.read(dec)?;
        Ok(TypeCode {
            arena: Arc::new(Arena { nodes: r.nodes }),
            index,
        })
    }
}

impl rustorb_cdr::CdrEncode for TypeCode {
    fn encode(&self, enc: &mut Encoder) -> Result<()> {
        TypeCode::encode(self, enc)
    }
}

impl rustorb_cdr::CdrDecode for TypeCode {
    fn decode(dec: &mut Decoder<'_>) -> Result<TypeCode> {
        TypeCode::decode(dec)
    }
}

fn invalid(why: &'static str) -> Error {
    Error::InvalidTypeCode(why)
}

struct Writer<'a> {
    arena: &'a Arena,
    /// The nodes being written, with the stream offset of their kind.
    open: HashMap<u32, isize>,
}

impl Writer<'_> {
    /// Writes node `index` to `enc`, whose offset 0 is at stream offset
    /// `base`.
    fn write(&mut self, enc: &mut Encoder, base: isize, index: u32) -> Result<()> {
        enc.align(4);
        let here = base + enc.position() as isize;
        if let Some(&start) = self.open.get(&index) {
            enc.write_u32(INDIRECTION);
            // The offset is relative to the offset field itself.
            let offset = start - (here + 4);
            enc.write_i32(i32::try_from(offset).map_err(|_| invalid("indirection too far"))?);
            return Ok(());
        }
        let node = &self.arena.nodes[index as usize];
        if let Node::Recursive { .. } = node {
            return Err(invalid("unresolved recursive TypeCode"));
        }
        enc.write_u32(node.kind().as_u32());
        match node {
            Node::Simple(_) => {}
            Node::String { bound, .. } => enc.write_u32(*bound),
            Node::Fixed { digits, scale } => {
                enc.write_u16(*digits);
                enc.write_i16(*scale);
            }
            _ => {
                self.open.insert(index, here);
                // The encapsulation's length is aligned to 4 and its
                // contents follow it.
                enc.align(4);
                let inner_base = base + enc.position() as isize + 4;
                let mut inner = Encoder::encapsulation(enc.byte_order(), enc.encoding());
                self.params(&mut inner, inner_base, node)?;
                enc.write_octet_seq(inner.as_bytes())?;
                self.open.remove(&index);
            }
        }
        Ok(())
    }

    fn params(&mut self, enc: &mut Encoder, base: isize, node: &Node) -> Result<()> {
        if let Some((id, name)) = node.id_name() {
            enc.write_string(id)?;
            enc.write_string(name)?;
        }
        match node {
            Node::Struct { members, .. } => {
                enc.write_len(members.len())?;
                for (name, tc) in members {
                    enc.write_string(name)?;
                    self.write(enc, base, *tc)?;
                }
            }
            Node::Union {
                disc,
                default,
                members,
                ..
            } => {
                self.write(enc, base, *disc)?;
                enc.write_i32(*default);
                enc.write_len(members.len())?;
                let disc_kind = unaliased_kind(self.arena, *disc);
                for (label, name, tc) in members {
                    match label {
                        // The default member's label is the octet 0.
                        None => enc.write_octet(0),
                        Some(l) => write_label(enc, disc_kind, *l)?,
                    }
                    enc.write_string(name)?;
                    self.write(enc, base, *tc)?;
                }
            }
            Node::Enum { members, .. } => {
                enc.write_len(members.len())?;
                for name in members {
                    enc.write_string(name)?;
                }
            }
            Node::Sequence { content, bound } => {
                self.write(enc, base, *content)?;
                enc.write_u32(*bound);
            }
            Node::Array { content, length } => {
                self.write(enc, base, *content)?;
                enc.write_u32(*length);
            }
            Node::Alias { content, .. } => self.write(enc, base, *content)?,
            Node::Value {
                modifier,
                base: concrete,
                members,
                ..
            } => {
                enc.write_i16(*modifier);
                match concrete {
                    Some(b) => self.write(enc, base, *b)?,
                    None => {
                        enc.align(4);
                        enc.write_u32(TCKind::Null.as_u32());
                    }
                }
                enc.write_len(members.len())?;
                for (name, tc, visibility) in members {
                    enc.write_string(name)?;
                    self.write(enc, base, *tc)?;
                    enc.write_i16(*visibility);
                }
            }
            _ => {}
        }
        Ok(())
    }
}

fn unaliased_kind(arena: &Arena, mut index: u32) -> TCKind {
    loop {
        match &arena.nodes[index as usize] {
            Node::Alias {
                kind: TCKind::Alias,
                content,
                ..
            } => index = *content,
            n => return n.kind(),
        }
    }
}

fn write_label(enc: &mut Encoder, kind: TCKind, l: i128) -> Result<()> {
    match kind {
        TCKind::Short => enc.write_i16(l as i16),
        TCKind::UShort => enc.write_u16(l as u16),
        TCKind::Long => enc.write_i32(l as i32),
        TCKind::ULong | TCKind::Enum => enc.write_u32(l as u32),
        TCKind::LongLong => enc.write_i64(l as i64),
        TCKind::ULongLong => enc.write_u64(l as u64),
        TCKind::Boolean => enc.write_bool(l != 0),
        TCKind::Char => enc.write_char(l as u8),
        TCKind::WChar => enc.write_wchar(l as u16)?,
        _ => return Err(invalid("bad discriminator type")),
    }
    Ok(())
}

fn read_label(dec: &mut Decoder<'_>, kind: TCKind) -> Result<i128> {
    Ok(match kind {
        TCKind::Short => dec.read_i16()?.into(),
        TCKind::UShort => dec.read_u16()?.into(),
        TCKind::Long => dec.read_i32()?.into(),
        TCKind::ULong | TCKind::Enum => dec.read_u32()?.into(),
        TCKind::LongLong => dec.read_i64()?.into(),
        TCKind::ULongLong => dec.read_u64()?.into(),
        TCKind::Boolean => dec.read_bool()?.into(),
        TCKind::Char => dec.read_char()?.into(),
        TCKind::WChar => dec.read_wchar()?.into(),
        _ => return Err(invalid("bad discriminator type")),
    })
}

struct Reader {
    nodes: Vec<Node>,
    /// Nodes by the address of their kind in memory. The encapsulations
    /// are slices of the outer buffer, so addresses measure stream offsets
    /// across encapsulations.
    starts: HashMap<usize, u32>,
    depth: usize,
}

impl Reader {
    fn read(&mut self, dec: &mut Decoder<'_>) -> Result<u32> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err(invalid("nested too deep"));
        }
        let index = self.read_node(dec);
        self.depth -= 1;
        index
    }

    fn read_node(&mut self, dec: &mut Decoder<'_>) -> Result<u32> {
        dec.align(4)?;
        let start = dec.remaining_bytes().as_ptr() as usize;
        let k = dec.read_u32()?;
        if k == INDIRECTION {
            let at = dec.remaining_bytes().as_ptr() as usize;
            let offset = dec.read_i32()?;
            let target = (at as isize).wrapping_add(offset as isize) as usize;
            return self
                .starts
                .get(&target)
                .copied()
                .ok_or(invalid("indirection to no TypeCode"));
        }
        let kind = TCKind::from_u32(k).ok_or(invalid("unknown kind"))?;
        if kind.is_simple() {
            // Simple TypeCodes are not indirection targets worth sharing,
            // but an indirection to one is still valid.
            let index = self.push(Node::Simple(kind));
            self.starts.insert(start, index);
            return Ok(index);
        }
        let index = self.push(Node::Simple(TCKind::Null));
        self.starts.insert(start, index);
        let node = match kind {
            TCKind::String | TCKind::WString => Node::String {
                kind,
                bound: dec.read_u32()?,
            },
            TCKind::Fixed => {
                let digits = dec.read_u16()?;
                let scale = dec.read_i16()?;
                if !(1..=31).contains(&digits) || scale < 0 || scale as u16 > digits {
                    return Err(invalid("bad fixed digits or scale"));
                }
                Node::Fixed { digits, scale }
            }
            _ => {
                let mut inner = dec.read_encapsulation()?;
                self.params(&mut inner, kind)?
            }
        };
        self.nodes[index as usize] = node;
        Ok(index)
    }

    fn push(&mut self, node: Node) -> u32 {
        self.nodes.push(node);
        (self.nodes.len() - 1) as u32
    }

    fn kind_of(&self, mut index: u32) -> TCKind {
        loop {
            match &self.nodes[index as usize] {
                Node::Alias {
                    kind: TCKind::Alias,
                    content,
                    ..
                } => index = *content,
                n => return n.kind(),
            }
        }
    }

    fn params(&mut self, dec: &mut Decoder<'_>, kind: TCKind) -> Result<Node> {
        let id_name = |dec: &mut Decoder<'_>| -> Result<(String, String)> {
            Ok((dec.read_string()?, dec.read_string()?))
        };
        Ok(match kind {
            TCKind::Objref
            | TCKind::Native
            | TCKind::AbstractInterface
            | TCKind::LocalInterface
            | TCKind::Component
            | TCKind::Home => {
                let (id, name) = id_name(dec)?;
                Node::Named { kind, id, name }
            }
            TCKind::Struct | TCKind::Except => {
                let (id, name) = id_name(dec)?;
                let count = dec.read_len(8)?;
                let mut members = Vec::with_capacity(count);
                for _ in 0..count {
                    let name = dec.read_string()?;
                    members.push((name, self.read(dec)?));
                }
                Node::Struct {
                    kind,
                    id,
                    name,
                    members,
                }
            }
            TCKind::Union => {
                let (id, name) = id_name(dec)?;
                let disc = self.read(dec)?;
                let disc_kind = self.kind_of(disc);
                if !disc_kind.is_discriminator() {
                    return Err(invalid("bad discriminator type"));
                }
                let default = dec.read_i32()?;
                let count = dec.read_len(9)?;
                if default < -1 || (default >= 0 && default as usize >= count) {
                    return Err(invalid("bad default index"));
                }
                let mut members = Vec::with_capacity(count);
                for i in 0..count {
                    let label = if i as i32 == default {
                        dec.read_octet()?;
                        None
                    } else {
                        Some(read_label(dec, disc_kind)?)
                    };
                    let name = dec.read_string()?;
                    members.push((label, name, self.read(dec)?));
                }
                Node::Union {
                    id,
                    name,
                    disc,
                    default,
                    members,
                }
            }
            TCKind::Enum => {
                let (id, name) = id_name(dec)?;
                let count = dec.read_len(5)?;
                let mut members = Vec::with_capacity(count);
                for _ in 0..count {
                    members.push(dec.read_string()?);
                }
                Node::Enum { id, name, members }
            }
            TCKind::Sequence | TCKind::Array => {
                let content = self.read(dec)?;
                let n = dec.read_u32()?;
                if kind == TCKind::Sequence {
                    Node::Sequence { content, bound: n }
                } else {
                    Node::Array { content, length: n }
                }
            }
            TCKind::Alias | TCKind::ValueBox => {
                let (id, name) = id_name(dec)?;
                let content = self.read(dec)?;
                Node::Alias {
                    kind,
                    id,
                    name,
                    content,
                }
            }
            TCKind::Value | TCKind::Event => {
                let (id, name) = id_name(dec)?;
                let modifier = dec.read_i16()?;
                let base = self.read(dec)?;
                let base = match self.nodes[base as usize] {
                    Node::Simple(TCKind::Null) => None,
                    _ => Some(base),
                };
                let count = dec.read_len(12)?;
                let mut members = Vec::with_capacity(count);
                for _ in 0..count {
                    let name = dec.read_string()?;
                    let tc = self.read(dec)?;
                    members.push((name, tc, dec.read_i16()?));
                }
                Node::Value {
                    kind,
                    id,
                    name,
                    modifier,
                    base,
                    members,
                }
            }
            _ => return Err(invalid("unknown kind")),
        })
    }
}

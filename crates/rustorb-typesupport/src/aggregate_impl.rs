//! The headers of aggregated types (structs and unions), by extensibility
//! kind and encoding version (DDS-XTypes 1.3, 7.4.3.5.3 rules 17 to 30).
//!
//! A generated `write` is
//!
//! ```text
//! let s = aggregate::begin(enc, Extensibility::Mutable);
//! aggregate::write_member::<D>(&v.a, ID_A, must_understand_a, enc)?;  // mutable
//! <D as TypeSupport>::write(&v.b, enc)?;                               // final, appendable
//! aggregate::write_optional::<D>(v.c.as_ref(), ID_C, false, enc)?;     // final, appendable
//! aggregate::end(enc, s)
//! ```
//!
//! and a generated `read` of a mutable type loops over the members it
//! receives with [`next_member`].

use rustorb_cdr::xcdr::{DHeaderMark, Delimited, Extensibility, ListEnd, MemberHeader, Parameter};
use rustorb_cdr::{Decoder, Encoder, Encoding};

use crate::{Error, Result, TypeSupport};

/// An aggregated value being written; see [`begin`].
#[derive(Debug)]
#[must_use = "an aggregated value is ended by aggregate::end"]
pub struct Out {
    dheader: Option<DHeaderMark>,
    parameter_list: bool,
}

/// Starts an aggregated value of extensibility `kind`: a DHEADER in XCDR2
/// for appendable and mutable types (rules 21 and 30).
pub fn begin(enc: &mut Encoder, kind: Extensibility) -> Out {
    let v2 = enc.encoding() == Encoding::Xcdr2;
    Out {
        dheader: (v2 && kind != Extensibility::Final).then(|| enc.begin_dheader()),
        parameter_list: !v2 && kind == Extensibility::Mutable,
    }
}

/// Ends an aggregated value: the length of its DHEADER (XCDR2), or the end
/// of the parameter list of a mutable type (XCDR1, rule 23).
pub fn end(enc: &mut Encoder, out: Out) -> Result<()> {
    if out.parameter_list {
        enc.end_parameter_list(ListEnd::Xtypes);
    }
    if let Some(mark) = out.dheader {
        enc.end_dheader(mark)?;
    }
    Ok(())
}

/// Writes member `id` of a mutable type: its EMHEADER (XCDR2, rule 22) or
/// parameter header (XCDR1, rules 24 and 25), then `v`. An absent optional
/// member is not written at all.
pub fn write_member<D: TypeSupport>(
    v: &D::Rust,
    id: u32,
    must_understand: bool,
    enc: &mut Encoder,
) -> Result<()> {
    if enc.encoding() == Encoding::Xcdr2 {
        let mark = enc.begin_member(id, must_understand)?;
        D::write(v, enc)?;
        enc.end_member(mark)?;
    } else {
        let mark = enc.begin_parameter(id, must_understand)?;
        D::write(v, enc)?;
        enc.end_parameter(mark)?;
    }
    Ok(())
}

/// Writes the optional member `id` of a final or appendable type: a
/// boolean that says whether it is present (XCDR2, rule 20), or a
/// parameter header with a zero length when it is absent (XCDR1, rule 19
/// and 7.4.1.1.5.2), then `v` when present.
pub fn write_optional<D: TypeSupport>(
    v: Option<&D::Rust>,
    id: u32,
    must_understand: bool,
    enc: &mut Encoder,
) -> Result<()> {
    if enc.encoding() == Encoding::Xcdr2 {
        enc.write_bool(v.is_some());
        if let Some(v) = v {
            D::write(v, enc)?;
        }
    } else {
        let mark = enc.begin_parameter(id, must_understand)?;
        if let Some(v) = v {
            D::write(v, enc)?;
        }
        enc.end_parameter(mark)?;
    }
    Ok(())
}

/// Writes an optional member of a key holder, which is a final type in
/// PLAIN_CDR2: whether it is present, then its own key holder (rule 20).
pub fn write_optional_key<D: TypeSupport>(v: Option<&D::Rust>, enc: &mut Encoder) -> Result<()> {
    enc.write_bool(v.is_some());
    if let Some(v) = v {
        D::write_key(v, enc)?;
    }
    Ok(())
}

/// Reads the optional member of a final or appendable type written by
/// [`write_optional`].
pub fn read_optional<D: TypeSupport>(dec: &mut Decoder<'_>) -> Result<Option<D::Rust>> {
    if dec.encoding() == Encoding::Xcdr2 {
        return Ok(if dec.read_bool()? {
            Some(D::read(dec)?)
        } else {
            None
        });
    }
    let param = dec.read_parameter_header()?;
    let v = if param.len == 0 {
        None
    } else {
        Some(D::read(dec)?)
    };
    dec.end_parameter(param)?;
    Ok(v)
}

/// An aggregated value being read; see [`begin_read`].
#[derive(Debug)]
#[must_use = "an aggregated value is ended by aggregate::end_read"]
pub struct In {
    delimited: Option<Delimited>,
}

/// Starts reading an aggregated value of extensibility `kind`: its
/// DHEADER in XCDR2 for appendable and mutable types.
pub fn begin_read(dec: &mut Decoder<'_>, kind: Extensibility) -> Result<In> {
    let delimited = if dec.encoding() == Encoding::Xcdr2 && kind != Extensibility::Final {
        Some(dec.read_dheader()?)
    } else {
        None
    };
    Ok(In { delimited })
}

/// Whether an appendable value has more after what was read: false at
/// the end of its DHEADER (XCDR2), when the writer had an older version of
/// the type without the members that follow, which then take their
/// default values. Always true in XCDR1, which has no length to tell.
pub fn has_more(dec: &Decoder<'_>, input: &In) -> bool {
    input.delimited.is_none_or(|d| d.has_more(dec))
}

/// Ends reading an aggregated value: what an appendable value has after
/// the members the reader knows is skipped.
pub fn end_read(dec: &mut Decoder<'_>, input: In) -> Result<()> {
    if let Some(delimited) = input.delimited {
        dec.end_dheader(delimited)?;
    }
    Ok(())
}

/// A member of a mutable type being read; see [`next_member`].
#[derive(Debug)]
#[must_use = "a member is ended by aggregate::end_member"]
pub enum Member {
    /// An XCDR2 member, after its EMHEADER.
    V2(MemberHeader),
    /// An XCDR1 member, after its parameter header.
    V1(Parameter),
}

impl Member {
    /// The member ID.
    pub fn id(&self) -> u32 {
        match self {
            Member::V2(h) => h.id,
            Member::V1(p) => p.id,
        }
    }

    /// Whether the member must be understood.
    pub fn must_understand(&self) -> bool {
        match self {
            Member::V2(h) => h.must_understand,
            Member::V1(p) => p.must_understand,
        }
    }

    /// What a reader does with a member it does not know: skips it, or
    /// fails when it must be understood (DDS-XTypes 7.2.2.4.4.4.6).
    pub fn unknown(&self) -> Result<()> {
        if self.must_understand() {
            return Err(Error::UnknownMustUnderstand(self.id()));
        }
        Ok(())
    }
}

/// The next member of a mutable value, or `None` after the last: the end
/// of its DHEADER (XCDR2) or of its parameter list (XCDR1). Read the
/// member or not, then call [`end_member`].
pub fn next_member(dec: &mut Decoder<'_>, input: &In) -> Result<Option<Member>> {
    match &input.delimited {
        Some(delimited) => {
            if !delimited.has_more(dec) {
                return Ok(None);
            }
            Ok(Some(Member::V2(dec.read_member_header()?)))
        }
        None => Ok(dec.read_parameter(ListEnd::Xtypes)?.map(Member::V1)),
    }
}

/// Moves to the end of `member`.
pub fn end_member(dec: &mut Decoder<'_>, member: Member) -> Result<()> {
    match member {
        Member::V2(h) => dec.end_member(h)?,
        Member::V1(p) => dec.end_parameter(p)?,
    }
    Ok(())
}

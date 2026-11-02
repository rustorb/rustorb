use std::fmt;
use std::str::FromStr;

use rustorb_cdr::{ByteOrder, CdrDecode, CdrEncode, Decoder, Encoder, Encoding};

use crate::{Error, IiopProfile};

/// `IOP::TAG_INTERNET_IOP`, an IIOP profile.
pub const TAG_INTERNET_IOP: u32 = 0;
/// `IOP::TAG_MULTIPLE_COMPONENTS`.
pub const TAG_MULTIPLE_COMPONENTS: u32 = 1;

/// `IOP::TaggedProfile`, with the profile body left as an encapsulation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaggedProfile {
    /// Profile tag such as [`TAG_INTERNET_IOP`].
    pub tag: u32,
    /// Encapsulated profile body.
    pub profile_data: Vec<u8>,
}

impl CdrEncode for TaggedProfile {
    fn encode(&self, enc: &mut Encoder) -> rustorb_cdr::Result<()> {
        enc.write_u32(self.tag);
        enc.write_octet_seq(&self.profile_data)
    }
}

impl CdrDecode for TaggedProfile {
    fn decode(dec: &mut Decoder<'_>) -> rustorb_cdr::Result<Self> {
        Ok(TaggedProfile {
            tag: dec.read_u32()?,
            profile_data: dec.read_octet_seq()?.to_vec(),
        })
    }
}

/// `IOP::IOR`, an interoperable object reference.
///
/// Profiles are kept raw; [`Ior::iiop_profiles`] interprets the IIOP ones.
/// The stringified form (`IOR:` and the hex encoded encapsulation) is
/// read with [`Ior::from_ior_string`] or [`FromStr`] and written with
/// [`Ior::to_ior_string`] or [`Display`](fmt::Display), which uses the host
/// byte order like omniORB.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ior {
    /// Repository id of the most derived interface, or empty.
    pub type_id: String,
    /// Profiles describing how to reach the object.
    pub profiles: Vec<TaggedProfile>,
}

impl Ior {
    /// The nil reference.
    pub fn nil() -> Ior {
        Ior {
            type_id: String::new(),
            profiles: Vec::new(),
        }
    }

    /// Whether this is a nil reference (no type id and no profiles).
    pub fn is_nil(&self) -> bool {
        self.type_id.is_empty() && self.profiles.is_empty()
    }

    /// The IIOP profiles, in order. Other profiles are skipped.
    pub fn iiop_profiles(&self) -> impl Iterator<Item = crate::Result<IiopProfile>> {
        self.profiles
            .iter()
            .filter(|p| p.tag == TAG_INTERNET_IOP)
            .map(IiopProfile::from_tagged_profile)
    }

    /// Reads an IOR from an encapsulation, the binary form behind a
    /// stringified IOR. Bytes after the IOR are ignored.
    pub fn from_encapsulation(data: &[u8]) -> crate::Result<Ior> {
        let mut dec = Decoder::encapsulation(data, Encoding::Giop1_2)?;
        Ok(Ior::decode(&mut dec)?)
    }

    /// Writes the IOR as an encapsulation in byte order `order`.
    pub fn to_encapsulation(&self, order: ByteOrder) -> crate::Result<Vec<u8>> {
        let mut enc = Encoder::encapsulation(order, Encoding::Giop1_2);
        self.encode(&mut enc)?;
        Ok(enc.into_bytes())
    }

    /// Parses a stringified IOR: `IOR:` followed by an even number of hex
    /// digits. The prefix and the digits may be in either case, and
    /// whitespace around the string (such as a trailing newline in a file)
    /// is ignored.
    pub fn from_ior_string(s: &str) -> crate::Result<Ior> {
        let s = s.trim_ascii();
        let hex = match s.get(..4) {
            Some(prefix) if prefix.eq_ignore_ascii_case("IOR:") => &s.as_bytes()[4..],
            _ => return Err(Error::InvalidIorString("missing IOR: prefix")),
        };
        if hex.len() % 2 != 0 {
            return Err(Error::InvalidIorString("odd number of hex digits"));
        }
        let bytes = hex
            .chunks_exact(2)
            .map(|pair| Some(hex_value(pair[0])? << 4 | hex_value(pair[1])?))
            .collect::<Option<Vec<u8>>>()
            .ok_or(Error::InvalidIorString("invalid hex digit"))?;
        Ior::from_encapsulation(&bytes)
    }

    /// Writes the stringified IOR with lower case hex digits, encoding the
    /// IOR in byte order `order`.
    pub fn to_ior_string(&self, order: ByteOrder) -> crate::Result<String> {
        const DIGITS: &[u8; 16] = b"0123456789abcdef";
        let bytes = self.to_encapsulation(order)?;
        let mut s = String::with_capacity(4 + bytes.len() * 2);
        s.push_str("IOR:");
        for b in bytes {
            s.push(DIGITS[usize::from(b >> 4)] as char);
            s.push(DIGITS[usize::from(b & 0xf)] as char);
        }
        Ok(s)
    }
}

fn hex_value(digit: u8) -> Option<u8> {
    (digit as char).to_digit(16).map(|v| v as u8)
}

impl FromStr for Ior {
    type Err = Error;

    fn from_str(s: &str) -> crate::Result<Ior> {
        Ior::from_ior_string(s)
    }
}

impl fmt::Display for Ior {
    /// Writes the stringified IOR in the host byte order.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = self
            .to_ior_string(ByteOrder::NATIVE)
            .map_err(|_| fmt::Error)?;
        f.write_str(&s)
    }
}

impl CdrEncode for Ior {
    fn encode(&self, enc: &mut Encoder) -> rustorb_cdr::Result<()> {
        enc.write_string(&self.type_id)?;
        self.profiles.encode(enc)
    }
}

impl CdrDecode for Ior {
    fn decode(dec: &mut Decoder<'_>) -> rustorb_cdr::Result<Self> {
        Ok(Ior {
            type_id: dec.read_string()?,
            profiles: Vec::decode(dec)?,
        })
    }
}

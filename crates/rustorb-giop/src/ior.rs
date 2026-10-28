use rustorb_cdr::{CdrDecode, CdrEncode, Decoder, Encoder, Result};

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
    fn encode(&self, enc: &mut Encoder) -> Result<()> {
        enc.write_u32(self.tag);
        enc.write_octet_seq(&self.profile_data)
    }
}

impl CdrDecode for TaggedProfile {
    fn decode(dec: &mut Decoder<'_>) -> Result<Self> {
        Ok(TaggedProfile {
            tag: dec.read_u32()?,
            profile_data: dec.read_octet_seq()?.to_vec(),
        })
    }
}

/// `IOP::IOR`, an interoperable object reference.
///
/// Profiles are kept raw; interpreting IIOP profiles comes with IOR
/// handling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ior {
    /// Repository id of the most derived interface, or empty.
    pub type_id: String,
    /// Profiles describing how to reach the object.
    pub profiles: Vec<TaggedProfile>,
}

impl Ior {
    /// Whether this is a nil reference (no type id and no profiles).
    pub fn is_nil(&self) -> bool {
        self.type_id.is_empty() && self.profiles.is_empty()
    }
}

impl CdrEncode for Ior {
    fn encode(&self, enc: &mut Encoder) -> Result<()> {
        enc.write_string(&self.type_id)?;
        self.profiles.encode(enc)
    }
}

impl CdrDecode for Ior {
    fn decode(dec: &mut Decoder<'_>) -> Result<Self> {
        Ok(Ior {
            type_id: dec.read_string()?,
            profiles: Vec::decode(dec)?,
        })
    }
}

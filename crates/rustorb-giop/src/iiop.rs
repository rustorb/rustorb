use rustorb_cdr::{ByteOrder, CdrDecode, CdrEncode, Decoder, Encoder, Encoding};

use crate::{
    AlternateIiopAddress, CodeSetComponentInfo, Error, Result, TAG_INTERNET_IOP, TaggedComponent,
    TaggedProfile, Version, component_tag,
};

/// `IIOP::ProfileBody`, the body of a [`TAG_INTERNET_IOP`] profile.
///
/// IIOP 1.0 profiles have no components; 1.1 and later carry a component
/// list. Minor versions above 2 are read like 1.2, and bytes after the
/// components are ignored, as the specification asks for forward
/// compatibility.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IiopProfile {
    /// IIOP version, which bounds the GIOP version a client may use.
    pub version: Version,
    /// Host name or IP address.
    pub host: String,
    /// TCP port.
    pub port: u16,
    /// Object key sent in requests.
    pub object_key: Vec<u8>,
    /// Tagged components (IIOP 1.1 and later).
    pub components: Vec<TaggedComponent>,
}

impl IiopProfile {
    /// Reads the profile from a [`TAG_INTERNET_IOP`] tagged profile.
    pub fn from_tagged_profile(profile: &TaggedProfile) -> Result<IiopProfile> {
        if profile.tag != TAG_INTERNET_IOP {
            return Err(Error::UnexpectedProfileTag(profile.tag));
        }
        IiopProfile::from_encapsulation(&profile.profile_data)
    }

    /// Reads the profile from the encapsulated profile body.
    pub fn from_encapsulation(data: &[u8]) -> Result<IiopProfile> {
        let mut dec = Decoder::encapsulation(data, Encoding::Giop1_2)?;
        let version = Version {
            major: dec.read_octet()?,
            minor: dec.read_octet()?,
        };
        if version.major != 1 {
            return Err(Error::UnsupportedIiopVersion(version));
        }
        let host = dec.read_string()?;
        let port = dec.read_u16()?;
        let object_key = dec.read_octet_seq()?.to_vec();
        let components = if version.minor == 0 {
            Vec::new()
        } else {
            Vec::decode(&mut dec)?
        };
        Ok(IiopProfile {
            version,
            host,
            port,
            object_key,
            components,
        })
    }

    /// Writes the profile as a [`TAG_INTERNET_IOP`] tagged profile whose
    /// encapsulation uses `order`.
    pub fn to_tagged_profile(&self, order: ByteOrder) -> Result<TaggedProfile> {
        Ok(TaggedProfile {
            tag: TAG_INTERNET_IOP,
            profile_data: self.to_encapsulation(order)?,
        })
    }

    /// Writes the encapsulated profile body.
    ///
    /// Fails with [`Error::ComponentsInIiop1_0`] if an IIOP 1.0 profile has
    /// components, since 1.0 cannot carry them.
    pub fn to_encapsulation(&self, order: ByteOrder) -> Result<Vec<u8>> {
        if self.version.major != 1 {
            return Err(Error::UnsupportedIiopVersion(self.version));
        }
        if self.version.minor == 0 && !self.components.is_empty() {
            return Err(Error::ComponentsInIiop1_0);
        }
        let mut enc = Encoder::encapsulation(order, Encoding::Giop1_2);
        enc.write_octet(self.version.major);
        enc.write_octet(self.version.minor);
        enc.write_string(&self.host)?;
        enc.write_u16(self.port);
        enc.write_octet_seq(&self.object_key)?;
        if self.version.minor != 0 {
            self.components.encode(&mut enc)?;
        }
        Ok(enc.into_bytes())
    }

    /// The components with tag `tag`, in order.
    pub fn components_with_tag(&self, tag: u32) -> impl Iterator<Item = &TaggedComponent> {
        self.components.iter().filter(move |c| c.tag == tag)
    }

    /// The ORB type from the first [`component_tag::ORB_TYPE`] component,
    /// such as [`orb_type::OMNIORB`](crate::orb_type::OMNIORB).
    pub fn orb_type(&self) -> Option<Result<u32>> {
        self.components_with_tag(component_tag::ORB_TYPE)
            .next()
            .map(|c| Ok(c.decoder()?.read_u32()?))
    }

    /// The code sets from the first [`component_tag::CODE_SETS`] component.
    pub fn code_sets(&self) -> Option<Result<CodeSetComponentInfo>> {
        self.components_with_tag(component_tag::CODE_SETS)
            .next()
            .map(CodeSetComponentInfo::from_component)
    }

    /// The addresses from every [`component_tag::ALTERNATE_IIOP_ADDRESS`]
    /// component.
    pub fn alternate_addresses(&self) -> Result<Vec<AlternateIiopAddress>> {
        self.components_with_tag(component_tag::ALTERNATE_IIOP_ADDRESS)
            .map(AlternateIiopAddress::from_component)
            .collect()
    }
}

impl TaggedProfile {
    /// Reads this profile as an IIOP profile.
    pub fn iiop(&self) -> Result<IiopProfile> {
        IiopProfile::from_tagged_profile(self)
    }

    /// Reads the components of a [`TAG_MULTIPLE_COMPONENTS`] profile.
    ///
    /// [`TAG_MULTIPLE_COMPONENTS`]: crate::TAG_MULTIPLE_COMPONENTS
    pub fn multiple_components(&self) -> Result<Vec<TaggedComponent>> {
        if self.tag != crate::TAG_MULTIPLE_COMPONENTS {
            return Err(Error::UnexpectedProfileTag(self.tag));
        }
        let mut dec = Decoder::encapsulation(&self.profile_data, Encoding::Giop1_2)?;
        Ok(Vec::decode(&mut dec)?)
    }

    /// Builds a [`TAG_MULTIPLE_COMPONENTS`] profile.
    ///
    /// [`TAG_MULTIPLE_COMPONENTS`]: crate::TAG_MULTIPLE_COMPONENTS
    pub fn multiple_components_profile(
        components: &[TaggedComponent],
        order: ByteOrder,
    ) -> Result<TaggedProfile> {
        let mut enc = Encoder::encapsulation(order, Encoding::Giop1_2);
        components.encode(&mut enc)?;
        Ok(TaggedProfile {
            tag: crate::TAG_MULTIPLE_COMPONENTS,
            profile_data: enc.into_bytes(),
        })
    }
}

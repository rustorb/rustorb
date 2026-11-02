use rustorb_cdr::{ByteOrder, CdrDecode, CdrEncode, Decoder, Encoder, Encoding};

use crate::Result;

/// `IOP::TaggedComponent`, with the component data left as an
/// encapsulation (or raw octets for the few components that are not).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaggedComponent {
    /// Component tag such as [`component_tag::CODE_SETS`].
    pub tag: u32,
    /// Component data.
    pub component_data: Vec<u8>,
}

impl CdrEncode for TaggedComponent {
    fn encode(&self, enc: &mut Encoder) -> rustorb_cdr::Result<()> {
        enc.write_u32(self.tag);
        enc.write_octet_seq(&self.component_data)
    }
}

impl CdrDecode for TaggedComponent {
    fn decode(dec: &mut Decoder<'_>) -> rustorb_cdr::Result<Self> {
        Ok(TaggedComponent {
            tag: dec.read_u32()?,
            component_data: dec.read_octet_seq()?.to_vec(),
        })
    }
}

impl TaggedComponent {
    /// Builds a component whose data is an encapsulation written by `f`.
    pub fn encapsulated<F>(tag: u32, order: ByteOrder, f: F) -> Result<TaggedComponent>
    where
        F: FnOnce(&mut Encoder) -> rustorb_cdr::Result<()>,
    {
        let mut enc = Encoder::encapsulation(order, Encoding::Giop1_2);
        f(&mut enc)?;
        Ok(TaggedComponent {
            tag,
            component_data: enc.into_bytes(),
        })
    }

    /// Opens the component data as an encapsulation.
    pub fn decoder(&self) -> Result<Decoder<'_>> {
        Ok(Decoder::encapsulation(
            &self.component_data,
            Encoding::Giop1_2,
        )?)
    }
}

/// Standard `IOP::ComponentId` values, and the omniORB specific ones.
pub mod component_tag {
    /// `TAG_ORB_TYPE`, an `unsigned long` naming the ORB vendor.
    pub const ORB_TYPE: u32 = 0;
    /// `TAG_CODE_SETS`, a [`CodeSetComponentInfo`](crate::CodeSetComponentInfo).
    pub const CODE_SETS: u32 = 1;
    /// `TAG_POLICIES`.
    pub const POLICIES: u32 = 2;
    /// `TAG_ALTERNATE_IIOP_ADDRESS`, an [`AlternateIiopAddress`](crate::AlternateIiopAddress).
    pub const ALTERNATE_IIOP_ADDRESS: u32 = 3;
    /// `TAG_COMPLETE_OBJECT_KEY`.
    pub const COMPLETE_OBJECT_KEY: u32 = 5;
    /// `TAG_ENDPOINT_ID_POSITION`.
    pub const ENDPOINT_ID_POSITION: u32 = 6;
    /// `TAG_LOCATION_POLICY`.
    pub const LOCATION_POLICY: u32 = 12;
    /// `TAG_ASSOCIATION_OPTIONS`.
    pub const ASSOCIATION_OPTIONS: u32 = 13;
    /// `TAG_SEC_NAME`.
    pub const SEC_NAME: u32 = 14;
    /// `TAG_SSL_SEC_TRANS`, an [`SslSecTrans`](crate::SslSecTrans).
    pub const SSL_SEC_TRANS: u32 = 20;
    /// `TAG_JAVA_CODEBASE`.
    pub const JAVA_CODEBASE: u32 = 25;
    /// `TAG_CSI_SEC_MECH_LIST`.
    pub const CSI_SEC_MECH_LIST: u32 = 33;
    /// `TAG_NULL_TAG`.
    pub const NULL_TAG: u32 = 34;
    /// `TAG_TLS_SEC_TRANS`.
    pub const TLS_SEC_TRANS: u32 = 36;
    /// omniORB `TAG_OMNIORB_BIDIR`.
    pub const OMNIORB_BIDIR: u32 = 0x4154_5401;
    /// omniORB `TAG_OMNIORB_UNIX_TRANS`.
    pub const OMNIORB_UNIX_TRANS: u32 = 0x4154_5402;
    /// omniORB `TAG_OMNIORB_PERSISTENT_ID`.
    pub const OMNIORB_PERSISTENT_ID: u32 = 0x4154_5403;
    /// omniORB `TAG_OMNIORB_RESTRICTED_CONNECTION`.
    pub const OMNIORB_RESTRICTED_CONNECTION: u32 = 0x4154_5404;
    /// omniORB `TAG_OMNIORB_HTTP_TRANS`.
    pub const OMNIORB_HTTP_TRANS: u32 = 0x4154_5405;
}

/// Values of the [`component_tag::ORB_TYPE`] component.
pub mod orb_type {
    /// omniORB (`"ATT\0"`).
    pub const OMNIORB: u32 = 0x4154_5400;
}

/// `CONV_FRAME::CodeSetComponent`: the native code set of one character
/// type and the code sets it can convert to.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CodeSetComponent {
    /// Native code set.
    pub native_code_set: u32,
    /// Code sets the server can convert to and from.
    pub conversion_code_sets: Vec<u32>,
}

impl CdrEncode for CodeSetComponent {
    fn encode(&self, enc: &mut Encoder) -> rustorb_cdr::Result<()> {
        enc.write_u32(self.native_code_set);
        self.conversion_code_sets.encode(enc)
    }
}

impl CdrDecode for CodeSetComponent {
    fn decode(dec: &mut Decoder<'_>) -> rustorb_cdr::Result<Self> {
        Ok(CodeSetComponent {
            native_code_set: dec.read_u32()?,
            conversion_code_sets: Vec::decode(dec)?,
        })
    }
}

/// `CONV_FRAME::CodeSetComponentInfo`, the data of the
/// [`component_tag::CODE_SETS`] component.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CodeSetComponentInfo {
    /// Code sets for `char` and `string`.
    pub for_char_data: CodeSetComponent,
    /// Code sets for `wchar` and `wstring`.
    pub for_wchar_data: CodeSetComponent,
}

impl CodeSetComponentInfo {
    /// Wraps the info in a [`component_tag::CODE_SETS`] component.
    pub fn to_component(&self, order: ByteOrder) -> Result<TaggedComponent> {
        TaggedComponent::encapsulated(component_tag::CODE_SETS, order, |enc| {
            self.for_char_data.encode(enc)?;
            self.for_wchar_data.encode(enc)
        })
    }

    /// Reads the info from a component's data.
    pub fn from_component(component: &TaggedComponent) -> Result<CodeSetComponentInfo> {
        let mut dec = component.decoder()?;
        Ok(CodeSetComponentInfo {
            for_char_data: CodeSetComponent::decode(&mut dec)?,
            for_wchar_data: CodeSetComponent::decode(&mut dec)?,
        })
    }
}

/// The data of the [`component_tag::ALTERNATE_IIOP_ADDRESS`] component: one
/// more address the object can be reached at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlternateIiopAddress {
    /// Host name or IP address.
    pub host: String,
    /// TCP port.
    pub port: u16,
}

impl AlternateIiopAddress {
    /// Wraps the address in a [`component_tag::ALTERNATE_IIOP_ADDRESS`]
    /// component.
    pub fn to_component(&self, order: ByteOrder) -> Result<TaggedComponent> {
        TaggedComponent::encapsulated(component_tag::ALTERNATE_IIOP_ADDRESS, order, |enc| {
            enc.write_string(&self.host)?;
            enc.write_u16(self.port);
            Ok(())
        })
    }

    /// Reads the address from a component's data.
    pub fn from_component(component: &TaggedComponent) -> Result<AlternateIiopAddress> {
        let mut dec = component.decoder()?;
        Ok(AlternateIiopAddress {
            host: dec.read_string()?,
            port: dec.read_u16()?,
        })
    }
}

/// `Security::AssociationOptions` bits, as in the
/// [`SslSecTrans`] component.
pub mod association_options {
    /// `NoProtection`: unprotected IIOP.
    pub const NO_PROTECTION: u16 = 1;
    /// `Integrity`.
    pub const INTEGRITY: u16 = 2;
    /// `Confidentiality`.
    pub const CONFIDENTIALITY: u16 = 4;
    /// `DetectReplay`.
    pub const DETECT_REPLAY: u16 = 8;
    /// `DetectMisordering`.
    pub const DETECT_MISORDERING: u16 = 16;
    /// `EstablishTrustInTarget`: the server authenticates itself.
    pub const ESTABLISH_TRUST_IN_TARGET: u16 = 32;
    /// `EstablishTrustInClient`: the client authenticates itself.
    pub const ESTABLISH_TRUST_IN_CLIENT: u16 = 64;
}

/// `SSLIOP::SSL`, the data of a [`component_tag::SSL_SEC_TRANS`]
/// component: the port of the TLS endpoint at the host of the IIOP
/// profile, and the association options of the target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SslSecTrans {
    /// What the target supports ([`association_options`] bits).
    pub target_supports: u16,
    /// What the target requires of clients.
    pub target_requires: u16,
    /// TCP port of the TLS endpoint.
    pub port: u16,
}

impl SslSecTrans {
    /// Wraps the data in a [`component_tag::SSL_SEC_TRANS`] component.
    pub fn to_component(&self, order: ByteOrder) -> Result<TaggedComponent> {
        TaggedComponent::encapsulated(component_tag::SSL_SEC_TRANS, order, |enc| {
            enc.write_u16(self.target_supports);
            enc.write_u16(self.target_requires);
            enc.write_u16(self.port);
            Ok(())
        })
    }

    /// Reads the data from a component's data.
    pub fn from_component(component: &TaggedComponent) -> Result<SslSecTrans> {
        let mut dec = component.decoder()?;
        Ok(SslSecTrans {
            target_supports: dec.read_u16()?,
            target_requires: dec.read_u16()?,
            port: dec.read_u16()?,
        })
    }
}

/// Builds a [`component_tag::ORB_TYPE`] component.
pub fn orb_type_component(orb_type: u32, order: ByteOrder) -> Result<TaggedComponent> {
    TaggedComponent::encapsulated(component_tag::ORB_TYPE, order, |enc| {
        enc.write_u32(orb_type);
        Ok(())
    })
}

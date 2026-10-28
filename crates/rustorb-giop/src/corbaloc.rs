//! `corbaloc:` and `corbaname:` object URLs (CORBA 3.0, 13.6.10).

use std::fmt;
use std::str::FromStr;

use rustorb_cdr::ByteOrder;

use crate::{AlternateIiopAddress, Error, IiopProfile, Ior, Result, TaggedProfile, Version};

/// Port used when a corbaloc IIOP address has none.
pub const DEFAULT_CORBALOC_PORT: u16 = 2809;

/// Object key used by `corbaname:` URLs that have none.
pub const DEFAULT_CORBANAME_KEY: &[u8] = b"NameService";

/// One address of a corbaloc URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObjAddr {
    /// `rir:`, which looks the key up with `resolve_initial_references`.
    /// It must be the only address.
    Rir,
    /// `iiop:` or the empty protocol (`:`).
    Iiop(IiopAddr),
    /// Any other protocol, such as omniORB's `ssliop:` or `omniunix:`, kept
    /// as text. The address ends at the first `,`, `/` or `#`.
    Other {
        /// Protocol token without the colon.
        protocol: String,
        /// Address after the colon.
        address: String,
    },
}

/// An IIOP address in a corbaloc URL: `[major.minor@]host[:port]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IiopAddr {
    /// IIOP version, 1.0 when not given.
    pub version: Version,
    /// Host name or IP address. IPv6 addresses are stored without the
    /// brackets they have in the URL.
    pub host: String,
    /// TCP port, [`DEFAULT_CORBALOC_PORT`] when not given.
    pub port: u16,
}

impl IiopAddr {
    /// An IIOP 1.0 address.
    pub fn new(host: impl Into<String>, port: u16) -> IiopAddr {
        IiopAddr {
            version: Version::V1_0,
            host: host.into(),
            port,
        }
    }

    /// The IIOP profile for this address and `object_key`, with no
    /// components.
    pub fn to_profile(&self, object_key: &[u8]) -> IiopProfile {
        IiopProfile {
            version: self.version,
            host: self.host.clone(),
            port: self.port,
            object_key: object_key.to_vec(),
            components: Vec::new(),
        }
    }
}

/// A `corbaloc:` URL: a list of addresses and an object key.
///
/// Parsing follows the specification and omniORB:
///
/// - an empty IIOP address means `localhost` on the default port,
/// - a missing port, or `:` with no digits, means the default port,
/// - the key is URL-unescaped (`%xx`) into bytes; a URL without `/` has an
///   empty key,
/// - scheme and protocol names are matched without regard to case.
///
/// [`Display`](fmt::Display) writes `iiop:` addresses with an explicit port
/// and escapes key bytes outside the characters the specification allows,
/// so parsing the output gives the same value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Corbaloc {
    /// Addresses, at least one.
    pub addresses: Vec<ObjAddr>,
    /// Object key.
    pub key: Vec<u8>,
}

impl Corbaloc {
    /// A corbaloc URL with one IIOP 1.0 address.
    pub fn iiop(host: impl Into<String>, port: u16, key: impl Into<Vec<u8>>) -> Corbaloc {
        Corbaloc {
            addresses: vec![ObjAddr::Iiop(IiopAddr::new(host, port))],
            key: key.into(),
        }
    }

    /// Parses a `corbaloc:` URL.
    pub fn parse(s: &str) -> Result<Corbaloc> {
        let rest = strip_scheme(s.trim_ascii(), "corbaloc:")
            .ok_or(Error::InvalidObjectUrl("missing corbaloc: scheme"))?;
        let mut parser = Parser { rest };
        let loc = parser.corbaloc(None)?;
        if !parser.rest.is_empty() {
            return Err(Error::InvalidObjectUrl("unexpected text after the key"));
        }
        Ok(loc)
    }

    /// Whether this URL is `rir:`.
    pub fn is_rir(&self) -> bool {
        matches!(self.addresses.as_slice(), [ObjAddr::Rir])
    }

    /// The IIOP addresses, in order.
    pub fn iiop_addresses(&self) -> impl Iterator<Item = &IiopAddr> {
        self.addresses.iter().filter_map(|a| match a {
            ObjAddr::Iiop(addr) => Some(addr),
            _ => None,
        })
    }

    /// Builds the IOR this URL stands for, the way omniORB does: no type
    /// id, and one IIOP profile for the first IIOP address with the lowest
    /// version of all of them. The other IIOP addresses become
    /// [`ALTERNATE_IIOP_ADDRESS`](crate::component_tag::ALTERNATE_IIOP_ADDRESS)
    /// components, which go in the IIOP profile from IIOP 1.1 on and in a
    /// separate [`TAG_MULTIPLE_COMPONENTS`] profile for IIOP 1.0. Other
    /// protocols are skipped.
    ///
    /// Fails for `rir:` and when there is no IIOP address.
    ///
    /// [`TAG_MULTIPLE_COMPONENTS`]: crate::TAG_MULTIPLE_COMPONENTS
    pub fn to_ior(&self, order: ByteOrder) -> Result<Ior> {
        let mut addrs = self.iiop_addresses();
        let first = addrs
            .next()
            .ok_or(Error::InvalidObjectUrl("no IIOP address"))?;
        let mut profile = first.to_profile(&self.key);
        let mut alternates = Vec::new();
        for addr in addrs {
            profile.version = profile.version.min(addr.version);
            let alternate = AlternateIiopAddress {
                host: addr.host.clone(),
                port: addr.port,
            };
            alternates.push(alternate.to_component(order)?);
        }
        let mut profiles = Vec::with_capacity(2);
        if profile.version.minor == 0 {
            profiles.push(profile.to_tagged_profile(order)?);
            if !alternates.is_empty() {
                profiles.push(TaggedProfile::multiple_components_profile(
                    &alternates,
                    order,
                )?);
            }
        } else {
            profile.components = alternates;
            profiles.push(profile.to_tagged_profile(order)?);
        }
        Ok(Ior {
            type_id: String::new(),
            profiles,
        })
    }

    fn write_location(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, addr) in self.addresses.iter().enumerate() {
            if i > 0 {
                f.write_str(",")?;
            }
            write!(f, "{addr}")?;
        }
        f.write_str("/")?;
        write_escaped(f, &self.key)
    }
}

impl FromStr for Corbaloc {
    type Err = Error;

    fn from_str(s: &str) -> Result<Corbaloc> {
        Corbaloc::parse(s)
    }
}

impl fmt::Display for Corbaloc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("corbaloc:")?;
        self.write_location(f)
    }
}

impl fmt::Display for ObjAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ObjAddr::Rir => f.write_str("rir:"),
            ObjAddr::Iiop(addr) => write!(f, "iiop:{addr}"),
            ObjAddr::Other { protocol, address } => write!(f, "{protocol}:{address}"),
        }
    }
}

impl fmt::Display for IiopAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.version != Version::V1_0 {
            write!(f, "{}.{}@", self.version.major, self.version.minor)?;
        }
        if self.host.contains(':') {
            write!(f, "[{}]:{}", self.host, self.port)
        } else {
            write!(f, "{}:{}", self.host, self.port)
        }
    }
}

/// A `corbaname:` URL: a naming context given as a corbaloc location, and
/// a stringified name to resolve in it.
///
/// The key defaults to [`DEFAULT_CORBANAME_KEY`] when the URL has no `/`.
/// The name is the URL-unescaped text after `#`, in the stringified name
/// syntax of `CosNaming::NamingContextExt` (`a.kind/b`); splitting it into
/// name components is left to the naming service code. An empty name
/// stands for the naming context itself, and is written without `#`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorbaName {
    /// The naming context.
    pub context: Corbaloc,
    /// Stringified name to resolve in the context, or empty.
    pub name: String,
}

impl CorbaName {
    /// Parses a `corbaname:` URL.
    pub fn parse(s: &str) -> Result<CorbaName> {
        let rest = strip_scheme(s.trim_ascii(), "corbaname:")
            .ok_or(Error::InvalidObjectUrl("missing corbaname: scheme"))?;
        let mut parser = Parser { rest };
        let context = parser.corbaloc(Some(DEFAULT_CORBANAME_KEY))?;
        let name = match parser.rest.strip_prefix('#') {
            Some(escaped) => String::from_utf8(unescape(escaped)?)
                .map_err(|_| Error::InvalidObjectUrl("name is not UTF-8"))?,
            None if parser.rest.is_empty() => String::new(),
            None => return Err(Error::InvalidObjectUrl("unexpected text after the key")),
        };
        Ok(CorbaName { context, name })
    }
}

impl FromStr for CorbaName {
    type Err = Error;

    fn from_str(s: &str) -> Result<CorbaName> {
        CorbaName::parse(s)
    }
}

impl fmt::Display for CorbaName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("corbaname:")?;
        self.context.write_location(f)?;
        if !self.name.is_empty() {
            f.write_str("#")?;
            write_escaped(f, self.name.as_bytes())?;
        }
        Ok(())
    }
}

/// Any string `CORBA::ORB::string_to_object` accepts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObjectUrl {
    /// A stringified IOR.
    Ior(Ior),
    /// A `corbaloc:` URL.
    Corbaloc(Corbaloc),
    /// A `corbaname:` URL.
    CorbaName(CorbaName),
}

impl ObjectUrl {
    /// Parses a stringified IOR, `corbaloc:` or `corbaname:` URL, choosing
    /// by the scheme (in any case).
    pub fn parse(s: &str) -> Result<ObjectUrl> {
        let s = s.trim_ascii();
        if strip_scheme(s, "IOR:").is_some() {
            Ior::from_ior_string(s).map(ObjectUrl::Ior)
        } else if strip_scheme(s, "corbaloc:").is_some() {
            Corbaloc::parse(s).map(ObjectUrl::Corbaloc)
        } else if strip_scheme(s, "corbaname:").is_some() {
            CorbaName::parse(s).map(ObjectUrl::CorbaName)
        } else {
            Err(Error::InvalidObjectUrl("unknown scheme"))
        }
    }
}

impl FromStr for ObjectUrl {
    type Err = Error;

    fn from_str(s: &str) -> Result<ObjectUrl> {
        ObjectUrl::parse(s)
    }
}

impl fmt::Display for ObjectUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ObjectUrl::Ior(ior) => write!(f, "{ior}"),
            ObjectUrl::Corbaloc(loc) => write!(f, "{loc}"),
            ObjectUrl::CorbaName(name) => write!(f, "{name}"),
        }
    }
}

/// Strips `scheme` from the start of `s`, ignoring case.
fn strip_scheme<'a>(s: &'a str, scheme: &str) -> Option<&'a str> {
    let head = s.get(..scheme.len())?;
    head.eq_ignore_ascii_case(scheme)
        .then(|| &s[scheme.len()..])
}

/// Whether `c` ends an address.
fn is_addr_end(c: char) -> bool {
    matches!(c, ',' | '/' | '#')
}

struct Parser<'a> {
    rest: &'a str,
}

impl<'a> Parser<'a> {
    /// Parses `<obj_addr_list>["/"<key_string>]`, leaving `#` and what
    /// follows it in `rest`.
    fn corbaloc(&mut self, default_key: Option<&[u8]>) -> Result<Corbaloc> {
        let mut addresses = vec![self.obj_addr()?];
        while let Some(rest) = self.rest.strip_prefix(',') {
            self.rest = rest;
            addresses.push(self.obj_addr()?);
        }
        if addresses.len() > 1 && addresses.contains(&ObjAddr::Rir) {
            return Err(Error::InvalidObjectUrl("rir: must be the only address"));
        }
        let key = match self.rest.strip_prefix('/') {
            Some(rest) => {
                let end = rest.find('#').unwrap_or(rest.len());
                self.rest = &rest[end..];
                unescape(&rest[..end])?
            }
            None => default_key.unwrap_or_default().to_vec(),
        };
        Ok(Corbaloc { addresses, key })
    }

    fn obj_addr(&mut self) -> Result<ObjAddr> {
        let colon = self
            .rest
            .find(|c: char| c == ':' || is_addr_end(c))
            .filter(|&i| self.rest[i..].starts_with(':'))
            .ok_or(Error::InvalidObjectUrl("address without a protocol"))?;
        let protocol = &self.rest[..colon];
        self.rest = &self.rest[colon + 1..];
        if protocol.is_empty() || protocol.eq_ignore_ascii_case("iiop") {
            return self.iiop_addr().map(ObjAddr::Iiop);
        }
        if !protocol
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.' | '_'))
        {
            return Err(Error::InvalidObjectUrl("invalid protocol name"));
        }
        let end = self.rest.find(is_addr_end).unwrap_or(self.rest.len());
        let address = &self.rest[..end];
        self.rest = &self.rest[end..];
        if protocol.eq_ignore_ascii_case("rir") {
            if !address.is_empty() {
                return Err(Error::InvalidObjectUrl("rir: takes no address"));
            }
            return Ok(ObjAddr::Rir);
        }
        Ok(ObjAddr::Other {
            protocol: protocol.to_owned(),
            address: address.to_owned(),
        })
    }

    /// Parses `[<version>]<host>[":"<port>]` after `iiop:`.
    fn iiop_addr(&mut self) -> Result<IiopAddr> {
        if self.rest.is_empty() || self.rest.starts_with(is_addr_end) {
            return Ok(IiopAddr::new("localhost", DEFAULT_CORBALOC_PORT));
        }
        let version = self.version()?;
        let bracketed = self.rest.starts_with('[');
        let host = if let Some(rest) = self.rest.strip_prefix('[') {
            let end = rest
                .find(']')
                .ok_or(Error::InvalidObjectUrl("unterminated IPv6 address"))?;
            self.rest = &rest[end + 1..];
            &rest[..end]
        } else {
            let end = self
                .rest
                .find(|c: char| c == ':' || is_addr_end(c))
                .unwrap_or(self.rest.len());
            let host = &self.rest[..end];
            self.rest = &self.rest[end..];
            host
        };
        if host.is_empty() {
            return Err(Error::InvalidObjectUrl("empty host"));
        }
        if !host
            .bytes()
            .all(|b| is_host_char(b) || (bracketed && b == b':'))
        {
            return Err(Error::InvalidObjectUrl("invalid character in host"));
        }
        let port = match self.rest.strip_prefix(':') {
            Some(rest) => {
                let end = rest.find(is_addr_end).unwrap_or(rest.len());
                let digits = &rest[..end];
                self.rest = &rest[end..];
                if digits.is_empty() {
                    DEFAULT_CORBALOC_PORT
                } else if digits.bytes().all(|b| b.is_ascii_digit()) {
                    digits
                        .parse()
                        .map_err(|_| Error::InvalidObjectUrl("port out of range"))?
                } else {
                    return Err(Error::InvalidObjectUrl("invalid port"));
                }
            }
            None if self.rest.is_empty() || self.rest.starts_with(is_addr_end) => {
                DEFAULT_CORBALOC_PORT
            }
            None => return Err(Error::InvalidObjectUrl("invalid address")),
        };
        Ok(IiopAddr {
            version,
            host: host.to_owned(),
            port,
        })
    }

    /// Parses an optional `<major>.<minor>@`.
    fn version(&mut self) -> Result<Version> {
        let end = self
            .rest
            .find(|c: char| c == '@' || c == ':' || c == '[' || is_addr_end(c))
            .unwrap_or(self.rest.len());
        let Some(rest) = self.rest[end..].strip_prefix('@') else {
            return Ok(Version::V1_0);
        };
        let (major, minor) = self.rest[..end]
            .split_once('.')
            .ok_or(Error::InvalidObjectUrl("invalid version"))?;
        let number = |s: &str| -> Result<u8> {
            if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
                return Err(Error::InvalidObjectUrl("invalid version"));
            }
            s.parse()
                .map_err(|_| Error::InvalidObjectUrl("invalid version"))
        };
        let version = Version {
            major: number(major)?,
            minor: number(minor)?,
        };
        self.rest = rest;
        Ok(version)
    }
}

/// Characters a host may contain: the unreserved and sub-delimiter
/// characters of RFC 3986 except `,`, and `%` for IPv6 zone ids. IPv6
/// addresses in brackets may also contain `:`.
fn is_host_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b"-._~%!$&'()*+;=".contains(&b)
}

/// Characters a key or name may contain without escaping.
fn is_unreserved(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b";/:?@&=+$,-_.!~*'()".contains(&b)
}

/// Undoes `%xx` escapes. Characters other than the unreserved ones and
/// escapes are rejected.
fn unescape(s: &str) -> Result<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len());
    let mut bytes = s.bytes();
    while let Some(b) = bytes.next() {
        if b == b'%' {
            let mut digit = || {
                bytes
                    .next()
                    .and_then(|d| (d as char).to_digit(16))
                    .ok_or(Error::InvalidObjectUrl("invalid % escape"))
            };
            let value = digit()? << 4 | digit()?;
            out.push(value as u8);
        } else if is_unreserved(b) {
            out.push(b);
        } else {
            return Err(Error::InvalidObjectUrl("character must be escaped"));
        }
    }
    Ok(out)
}

fn write_escaped(f: &mut fmt::Formatter<'_>, bytes: &[u8]) -> fmt::Result {
    for &b in bytes {
        if is_unreserved(b) {
            write!(f, "{}", b as char)?;
        } else {
            write!(f, "%{b:02X}")?;
        }
    }
    Ok(())
}

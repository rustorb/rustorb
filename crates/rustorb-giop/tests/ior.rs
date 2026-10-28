//! IOR, IIOP profile, corbaloc and corbaname tests.
//!
//! The fixtures were produced by omniORB 4.3.4: `genior`, the root context
//! IOR printed by `omniNames`, `object_to_string` of `string_to_object`
//! on corbaloc URLs, and a reply captured by the interop environment
//! (`interop/golden/`).

use rustorb_cdr::{ByteOrder, CdrDecode};
use rustorb_giop::{
    AlternateIiopAddress, CodeSetComponent, CodeSetComponentInfo, CorbaName, Corbaloc,
    DEFAULT_CORBALOC_PORT, Error, IiopAddr, IiopProfile, Ior, Message, ObjAddr, ObjectUrl,
    TAG_INTERNET_IOP, TAG_MULTIPLE_COMPONENTS, TaggedComponent, TaggedProfile, Version, code_set,
    component_tag, decode_message, orb_type, orb_type_component,
};

/// `genior IDL:rustorb.org/Test/Echo:1.0 example.com 2809 MyKey`.
const GENIOR: &str = "IOR:010000001e00000049444c3a727573746f72622e6f72672f546573742f4563686f3a312e3000000001000000000000005c000000010102000c0000006578616d706c652e636f6d00f90a0000050000004d794b65790000000200000000000000080000000100000000545441010000001c00000001000000010001000100000001000105090101000100000009010100";

/// Root context of `omniNames -start 12809` listening on
/// `giop:tcp:127.0.0.1:12809` and `giop:tcp:localhost:12810`.
const OMNINAMES: &str = "IOR:010000002b00000049444c3a6f6d672e6f72672f436f734e616d696e672f4e616d696e67436f6e746578744578743a312e300000010000000000000088000000010102000a0000003132372e302e302e310009320b0000004e616d6553657276696365000400000000000000080000000100000000545441010000001c000000010000000100010001000000010001050901010001000000090101000300000014000000010000000a0000006c6f63616c686f7374000a320354544108000000c565e16a01001dc2";

/// GIOP 1.0 reply to `resolve("interop/Echo")` from omniNames, from
/// `interop/golden/omniorb-cpp-to-omniorb-cpp.json`.
const RESOLVE_REPLY: &str = "47494f5001000101a80000000000000004000000000000002500000049444c3a727573746f72622e6f72672f496e7465726f70546573742f4563686f3a312e300000000001000000000000006400000001010200070000007365727665720000fa0a000011000000ff496e7465726f7054657374004563686f0000000200000000000000080000000100000000545441010000001c00000001000000010001000100000001000105090101000100000009010100";

/// `object_to_string(string_to_object(url))` in omniORB, for each URL.
const OMNIORB_CORBALOC: &[(&str, &str)] = &[
    (
        "corbaloc::example.com:2809/NameService",
        "IOR:010000000100000000000000010000000000000027000000010100000c0000006578616d706c652e636f6d00f90a00000b0000004e616d6553657276696365",
    ),
    (
        "corbaloc:iiop:1.2@example.com/Key%20With%2fSlash",
        "IOR:010000000100000000000000010000000000000030000000010102000c0000006578616d706c652e636f6d00f90a00000e0000004b657920576974682f536c617368000000000000",
    ),
    (
        "corbaloc::[::1]:2810/k",
        "IOR:01000000010000000000000001000000000000001500000001010000040000003a3a3100fa0a0000010000006b",
    ),
    (
        "corbaloc::a.example:1,:b.example:2/k",
        "IOR:010000000100000000000000020000000000000019000000010100000a000000612e6578616d706c65000100010000006b000000010000002400000001000000010000000300000014000000010000000a000000622e6578616d706c65000200",
    ),
    (
        "corbaloc::1.1@h:3,iiop:1.2@h2:4/k",
        "IOR:01000000010000000000000001000000000000002e000000010101000200000068000300010000006b00000001000000030000000e0000000100000003000000683200000400",
    ),
    (
        "corbaloc::/k",
        "IOR:010000000100000000000000010000000000000019000000010100000a0000006c6f63616c686f737400f90a010000006b",
    ),
];

fn hex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

fn omniorb_code_sets() -> CodeSetComponentInfo {
    CodeSetComponentInfo {
        for_char_data: CodeSetComponent {
            native_code_set: code_set::ISO_8859_1,
            conversion_code_sets: vec![code_set::UTF_8],
        },
        for_wchar_data: CodeSetComponent {
            native_code_set: code_set::UTF_16,
            conversion_code_sets: vec![code_set::UTF_16],
        },
    }
}

fn only_iiop_profile(ior: &Ior) -> IiopProfile {
    let profiles = ior.iiop_profiles().collect::<Result<Vec<_>, _>>().unwrap();
    assert_eq!(profiles.len(), 1);
    profiles.into_iter().next().unwrap()
}

#[test]
fn parse_genior() {
    let ior: Ior = GENIOR.parse().unwrap();
    assert_eq!(ior.type_id, "IDL:rustorb.org/Test/Echo:1.0");
    assert_eq!(ior.profiles.len(), 1);
    let profile = only_iiop_profile(&ior);
    assert_eq!(profile.version, Version::V1_2);
    assert_eq!(profile.host, "example.com");
    assert_eq!(profile.port, 2809);
    assert_eq!(profile.object_key, b"MyKey");
    assert_eq!(profile.orb_type(), Some(Ok(orb_type::OMNIORB)));
    assert_eq!(profile.code_sets(), Some(Ok(omniorb_code_sets())));
    assert_eq!(profile.alternate_addresses(), Ok(vec![]));
}

#[test]
fn parse_omninames_root_context() {
    let ior = Ior::from_ior_string(OMNINAMES).unwrap();
    assert_eq!(ior.type_id, "IDL:omg.org/CosNaming/NamingContextExt:1.0");
    let profile = only_iiop_profile(&ior);
    assert_eq!(profile.version, Version::V1_2);
    assert_eq!(profile.host, "127.0.0.1");
    assert_eq!(profile.port, 12809);
    assert_eq!(profile.object_key, b"NameService");
    let tags: Vec<u32> = profile.components.iter().map(|c| c.tag).collect();
    assert_eq!(
        tags,
        [
            component_tag::ORB_TYPE,
            component_tag::CODE_SETS,
            component_tag::ALTERNATE_IIOP_ADDRESS,
            component_tag::OMNIORB_PERSISTENT_ID,
        ]
    );
    assert_eq!(
        profile.alternate_addresses().unwrap(),
        [AlternateIiopAddress {
            host: "localhost".into(),
            port: 12810,
        }]
    );
    // The persistent id is raw octets, not an encapsulation.
    let id = profile
        .components_with_tag(component_tag::OMNIORB_PERSISTENT_ID)
        .next()
        .unwrap();
    assert_eq!(id.component_data, hex("c565e16a01001dc2"));
}

#[test]
fn ior_in_captured_reply() {
    let (_, message) = decode_message(&hex(RESOLVE_REPLY)).unwrap();
    let Message::Reply { body, .. } = message else {
        panic!("not a reply");
    };
    let ior = Ior::decode(&mut body.decoder()).unwrap();
    assert_eq!(ior.type_id, "IDL:rustorb.org/InteropTest/Echo:1.0");
    let profile = only_iiop_profile(&ior);
    assert_eq!(profile.version, Version::V1_2);
    assert_eq!((profile.host.as_str(), profile.port), ("server", 2810));
    assert_eq!(profile.object_key, b"\xffInteropTest\0Echo");
    assert_eq!(profile.code_sets(), Some(Ok(omniorb_code_sets())));
}

#[test]
fn ior_string_roundtrip_is_exact() {
    for s in [GENIOR, OMNINAMES] {
        let ior = Ior::from_ior_string(s).unwrap();
        assert_eq!(ior.to_ior_string(ByteOrder::Little).unwrap(), s);
        // Re-encoding every IIOP profile reproduces omniORB's bytes too.
        for (profile, iiop) in ior.profiles.iter().zip(ior.iiop_profiles()) {
            let again = iiop.unwrap().to_tagged_profile(ByteOrder::Little).unwrap();
            assert_eq!(&again, profile);
        }
    }
}

#[test]
fn ior_string_big_endian_roundtrip() {
    let ior = Ior::from_ior_string(OMNINAMES).unwrap();
    let profile = only_iiop_profile(&ior);
    let be = Ior {
        type_id: ior.type_id.clone(),
        profiles: vec![profile.to_tagged_profile(ByteOrder::Big).unwrap()],
    };
    let s = be.to_ior_string(ByteOrder::Big).unwrap();
    assert!(s.starts_with("IOR:00000000"));
    let parsed = Ior::from_ior_string(&s).unwrap();
    assert_eq!(parsed, be);
    assert_eq!(only_iiop_profile(&parsed), profile);
    assert_eq!(parsed.to_string().parse::<Ior>().unwrap(), parsed);
}

#[test]
fn ior_string_is_lenient_about_case_and_whitespace() {
    let upper = format!("ior:{}\n", GENIOR[4..].to_ascii_uppercase());
    assert_eq!(
        Ior::from_ior_string(&upper).unwrap(),
        Ior::from_ior_string(GENIOR).unwrap()
    );
}

#[test]
fn ior_string_errors() {
    for (s, err) in [
        ("", Error::InvalidIorString("missing IOR: prefix")),
        (
            "corbaloc::h/k",
            Error::InvalidIorString("missing IOR: prefix"),
        ),
        ("IOR:0", Error::InvalidIorString("odd number of hex digits")),
        ("IOR:0g", Error::InvalidIorString("invalid hex digit")),
        ("IOR:é", Error::InvalidIorString("invalid hex digit")),
        ("IOR:", Error::Cdr(rustorb_cdr::Error::EmptyEncapsulation)),
    ] {
        assert_eq!(Ior::from_ior_string(s), Err(err), "{s:?}");
    }
    assert!(matches!(
        Ior::from_ior_string("IOR:0100000005000000"),
        Err(Error::Cdr(_))
    ));
}

#[test]
fn nil_ior() {
    let nil = Ior::nil();
    assert!(nil.is_nil());
    let s = nil.to_ior_string(ByteOrder::Little).unwrap();
    assert_eq!(s, "IOR:01000000010000000000000000000000");
    assert!(Ior::from_ior_string(&s).unwrap().is_nil());
}

#[test]
fn iiop_profile_versions_roundtrip() {
    let components = vec![
        orb_type_component(orb_type::OMNIORB, ByteOrder::Big).unwrap(),
        omniorb_code_sets().to_component(ByteOrder::Little).unwrap(),
        TaggedComponent {
            tag: component_tag::NULL_TAG,
            component_data: vec![],
        },
    ];
    for minor in 0..=3 {
        for order in [ByteOrder::Big, ByteOrder::Little] {
            let profile = IiopProfile {
                version: Version { major: 1, minor },
                host: "host.example".into(),
                port: 65535,
                object_key: vec![0, 1, 2, 0xff],
                components: if minor == 0 {
                    vec![]
                } else {
                    components.clone()
                },
            };
            let tagged = profile.to_tagged_profile(order).unwrap();
            assert_eq!(tagged.tag, TAG_INTERNET_IOP);
            assert_eq!(tagged.profile_data[0], order.flag());
            assert_eq!(tagged.iiop().unwrap(), profile);
        }
    }
}

#[test]
fn iiop_profile_errors() {
    let mut profile = IiopProfile {
        version: Version::V1_0,
        host: "h".into(),
        port: 1,
        object_key: vec![],
        components: vec![TaggedComponent {
            tag: 0,
            component_data: vec![],
        }],
    };
    assert_eq!(
        profile.to_tagged_profile(ByteOrder::Big),
        Err(Error::ComponentsInIiop1_0)
    );
    profile.version = Version { major: 2, minor: 0 };
    assert_eq!(
        profile.to_tagged_profile(ByteOrder::Big),
        Err(Error::UnsupportedIiopVersion(Version {
            major: 2,
            minor: 0
        }))
    );
    let tagged = TaggedProfile {
        tag: TAG_INTERNET_IOP,
        profile_data: vec![0, 2, 0],
    };
    assert_eq!(
        tagged.iiop(),
        Err(Error::UnsupportedIiopVersion(Version {
            major: 2,
            minor: 0
        }))
    );
    let other = TaggedProfile {
        tag: 7,
        profile_data: vec![0],
    };
    assert_eq!(other.iiop(), Err(Error::UnexpectedProfileTag(7)));
    assert_eq!(
        other.multiple_components(),
        Err(Error::UnexpectedProfileTag(7))
    );
}

#[test]
fn iiop_profile_ignores_trailing_bytes() {
    let profile = IiopProfile {
        version: Version::V1_2,
        host: "h".into(),
        port: 1,
        object_key: b"k".to_vec(),
        components: vec![],
    };
    let mut data = profile.to_encapsulation(ByteOrder::Big).unwrap();
    data.extend_from_slice(b"future");
    assert_eq!(IiopProfile::from_encapsulation(&data).unwrap(), profile);
}

#[test]
fn corbaloc_to_ior_matches_omniorb() {
    for &(url, expected) in OMNIORB_CORBALOC {
        let loc = Corbaloc::parse(url).unwrap();
        let ior = loc.to_ior(ByteOrder::Little).unwrap();
        assert_eq!(ior, Ior::from_ior_string(expected).unwrap(), "{url}");
        assert_eq!(ior.to_ior_string(ByteOrder::Little).unwrap(), expected);
    }
}

#[test]
fn corbaloc_alternate_addresses() {
    let ior = Corbaloc::parse("corbaloc::a.example:1,:b.example:2/k")
        .unwrap()
        .to_ior(ByteOrder::Big)
        .unwrap();
    assert_eq!(ior.profiles.len(), 2);
    assert_eq!(ior.profiles[1].tag, TAG_MULTIPLE_COMPONENTS);
    let components = ior.profiles[1].multiple_components().unwrap();
    assert_eq!(
        AlternateIiopAddress::from_component(&components[0]).unwrap(),
        AlternateIiopAddress {
            host: "b.example".into(),
            port: 2
        }
    );

    let ior = Corbaloc::parse("corbaloc:iiop:1.2@a:1,iiop:1.2@b:2/k")
        .unwrap()
        .to_ior(ByteOrder::Big)
        .unwrap();
    let profile = only_iiop_profile(&ior);
    assert_eq!(profile.version, Version::V1_2);
    assert_eq!(
        profile.alternate_addresses().unwrap(),
        [AlternateIiopAddress {
            host: "b".into(),
            port: 2
        }]
    );
}

#[test]
fn parse_corbaloc() {
    let loc = Corbaloc::parse("corbaloc::example.com:2809/NameService").unwrap();
    assert_eq!(loc, Corbaloc::iiop("example.com", 2809, "NameService"));

    let loc = Corbaloc::parse("corbaloc:iiop:1.2@example.com/Key%20With%2fSlash").unwrap();
    assert_eq!(
        loc.addresses,
        [ObjAddr::Iiop(IiopAddr {
            version: Version::V1_2,
            host: "example.com".into(),
            port: DEFAULT_CORBALOC_PORT,
        })]
    );
    assert_eq!(loc.key, b"Key With/Slash");

    let loc = Corbaloc::parse("corbaloc::[::1]:2810,iiop:[fe80::1%25eth0],:h:/a/b").unwrap();
    let addrs: Vec<_> = loc.iiop_addresses().cloned().collect();
    assert_eq!(
        addrs,
        [
            IiopAddr::new("::1", 2810),
            IiopAddr::new("fe80::1%25eth0", 2809),
            IiopAddr::new("h", 2809),
        ]
    );
    assert_eq!(loc.key, b"a/b");

    let loc = Corbaloc::parse("corbaloc::,:/k").unwrap();
    assert_eq!(
        loc.addresses,
        [
            ObjAddr::Iiop(IiopAddr::new("localhost", 2809)),
            ObjAddr::Iiop(IiopAddr::new("localhost", 2809)),
        ]
    );

    let loc = Corbaloc::parse("CORBALOC:IIOP:h").unwrap();
    assert_eq!(loc, Corbaloc::iiop("h", 2809, ""));

    let loc = Corbaloc::parse("corbaloc:rir:/NameService").unwrap();
    assert!(loc.is_rir());
    assert_eq!(loc.key, b"NameService");
    assert!(loc.to_ior(ByteOrder::Big).is_err());

    let loc = Corbaloc::parse("corbaloc:ssliop:1.2@h:1234,omniunix:/tmp/sock:k/x").unwrap();
    assert_eq!(
        loc.addresses,
        [
            ObjAddr::Other {
                protocol: "ssliop".into(),
                address: "1.2@h:1234".into(),
            },
            ObjAddr::Other {
                protocol: "omniunix".into(),
                address: "".into(),
            },
        ]
    );
    assert_eq!(loc.key, b"tmp/sock:k/x");
    assert_eq!(
        loc.to_ior(ByteOrder::Big),
        Err(Error::InvalidObjectUrl("no IIOP address"))
    );
}

#[test]
fn corbaloc_errors() {
    for url in [
        "corbaloc:",
        "corbaloc:/k",
        "corbaloc:h/k",
        "corbaloc::h:99999/k",
        "corbaloc::h:12x/k",
        "corbaloc::h/a#b",
        "corbaloc::h/a b",
        "corbaloc::h/a%zz",
        "corbaloc::h/a%4",
        "corbaloc::h/é",
        "corbaloc::[::1/k",
        "corbaloc::[]:1/k",
        "corbaloc::[::1]x/k",
        "corbaloc::1@h/k",
        "corbaloc::1.@h/k",
        "corbaloc::x.1@h/k",
        "corbaloc::1.256@h/k",
        "corbaloc::1.2@/k",
        "corbaloc:rir:,:h/k",
        "corbaloc::h,rir:/k",
        "corbaloc:rir:x/k",
        "corbaloc:f o:h/k",
        "corbaname::h/k",
    ] {
        assert!(
            matches!(Corbaloc::parse(url), Err(Error::InvalidObjectUrl(_))),
            "{url}: {:?}",
            Corbaloc::parse(url)
        );
    }
}

#[test]
fn corbaloc_display_roundtrip() {
    for (url, written) in [
        (
            "corbaloc::example.com/NameService",
            "corbaloc:iiop:example.com:2809/NameService",
        ),
        (
            "corbaloc:iiop:1.2@[::1]:5,rir:/a%2Fb%23%00%ff",
            "corbaloc:iiop:1.2@[::1]:5,rir:/a/b%23%00%FF",
        ),
        ("corbaloc:x-y:abc/", "corbaloc:x-y:abc/"),
    ] {
        // rir: with another address is invalid, so build the value directly.
        let loc = match Corbaloc::parse(url) {
            Ok(loc) => loc,
            Err(_) => Corbaloc {
                addresses: vec![
                    ObjAddr::Iiop(IiopAddr {
                        version: Version::V1_2,
                        host: "::1".into(),
                        port: 5,
                    }),
                    ObjAddr::Rir,
                ],
                key: b"a/b#\0\xff".to_vec(),
            },
        };
        assert_eq!(loc.to_string(), written);
        if !written.contains("rir:") {
            assert_eq!(Corbaloc::parse(written).unwrap(), loc);
        }
    }
}

#[test]
fn parse_corbaname() {
    let name = CorbaName::parse("corbaname::ns.example#interop/Echo.obj").unwrap();
    assert_eq!(
        name.context,
        Corbaloc::iiop("ns.example", 2809, "NameService")
    );
    assert_eq!(name.name, "interop/Echo.obj");
    assert_eq!(
        name.to_string(),
        "corbaname:iiop:ns.example:2809/NameService#interop/Echo.obj"
    );

    let name = CorbaName::parse("corbaname:rir:#a%23b%20c").unwrap();
    assert!(name.context.is_rir());
    assert_eq!(name.context.key, b"NameService");
    assert_eq!(name.name, "a#b c");
    assert_eq!(name.to_string(), "corbaname:rir:/NameService#a%23b%20c");

    let name = CorbaName::parse("corbaname::h:1/Other").unwrap();
    assert_eq!(name.context.key, b"Other");
    assert_eq!(name.name, "");
    assert_eq!(name.to_string(), "corbaname:iiop:h:1/Other");
    assert_eq!(CorbaName::parse(&name.to_string()).unwrap(), name);

    for url in [
        "corbaname::h#a#b",
        "corbaname::h#%ff",
        "corbaname::h x",
        "corbaloc::h/k",
    ] {
        assert!(CorbaName::parse(url).is_err(), "{url}");
    }
}

#[test]
fn object_url_dispatch() {
    assert!(matches!(
        ObjectUrl::parse(GENIOR),
        Ok(ObjectUrl::Ior(ior)) if ior.type_id == "IDL:rustorb.org/Test/Echo:1.0"
    ));
    assert!(matches!(
        " corbaloc::h/k\n".parse::<ObjectUrl>(),
        Ok(ObjectUrl::Corbaloc(_))
    ));
    assert!(matches!(
        ObjectUrl::parse("CorbaName::h#x"),
        Ok(ObjectUrl::CorbaName(_))
    ));
    assert_eq!(
        ObjectUrl::parse("file:///tmp/ior"),
        Err(Error::InvalidObjectUrl("unknown scheme"))
    );
    for s in [GENIOR, "corbaloc:iiop:h:1/k", "corbaname:iiop:h:1/k#a/b"] {
        assert_eq!(ObjectUrl::parse(s).unwrap().to_string(), s);
    }
}

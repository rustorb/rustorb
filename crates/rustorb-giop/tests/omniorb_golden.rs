//! Reads the GIOP messages that omniORB and omniORBpy exchanged in the
//! interop tests (`interop/golden`, see `interop/README.md`), and checks
//! the wide string encoding and the fragment alignment against them.

use std::collections::HashMap;
use std::path::Path;

use rustorb_cdr::{ByteOrder, CdrDecode, CdrVersion, Decoder, Encoder};
use rustorb_giop::{AssembledMessage, Body, FragmentAssembler, Message, decode_message};
use serde_json::Value;

const PAIRS: [&str; 4] = [
    "omniorbpy-to-omniorbpy",
    "omniorbpy-to-omniorb-cpp",
    "omniorb-cpp-to-omniorbpy",
    "omniorb-cpp-to-omniorb-cpp",
];

/// Values of the echo_wstring cases, in the order the clients send them.
/// The omniORB C++ client sends the crab as a surrogate pair itself, which
/// gives the same bytes.
const WSTRINGS: [&str; 4] = ["", "hello", "こんにちは", "🦀"];
/// Values of the echo_wchar cases.
const WCHARS: [u16; 3] = [0x61, 0x3042, 0xE9];
/// Number of records in the echo_records[large] case.
const LARGE_RECORDS: usize = 400;

/// One message after fragments are joined, with the golden file fields of
/// its first message.
struct Golden {
    server: String,
    direction: String,
    operation: Option<String>,
    message: AssembledMessage,
}

impl Golden {
    fn body(&self) -> Body {
        match self.message.decode().unwrap().1 {
            Message::Request { body, .. } | Message::Reply { body, .. } => body,
            other => panic!("{:?} is not a request or reply", other.msg_type()),
        }
    }

    fn is(&self, direction: &str, operation: &str) -> bool {
        self.server == "server"
            && self.direction == direction
            && self.operation.as_deref() == Some(operation)
    }
}

/// Loads `interop/golden/<name>.json` and joins fragmented messages. The
/// golden files keep the Fragments of a message right after it.
fn load(name: &str) -> Vec<Golden> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../interop/golden")
        .join(format!("{name}.json"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path:?}: {e}"));
    let entries: Vec<Value> = serde_json::from_str(&text).unwrap();

    let mut assemblers: HashMap<(String, String), FragmentAssembler> = HashMap::new();
    let mut first: HashMap<(String, String), (Option<String>, usize)> = HashMap::new();
    let mut out = Vec::new();
    for (i, e) in entries.iter().enumerate() {
        let field = |k: &str| e[k].as_str().map(str::to_owned);
        let key = (field("server").unwrap(), field("direction").unwrap());
        let bytes = hex(&field("hex").unwrap());
        let asm = assemblers.entry(key.clone()).or_default();
        let had_pending = asm.has_pending();
        let joined = asm
            .push(&bytes)
            .unwrap_or_else(|err| panic!("{name} #{i}: {err}"));
        if !had_pending {
            first.insert(key.clone(), (field("operation"), i));
        }
        if let Some(message) = joined {
            let (operation, _) = first.remove(&key).unwrap();
            out.push(Golden {
                server: key.0,
                direction: key.1,
                operation,
                message,
            });
        }
    }
    for (key, asm) in &assemblers {
        assert!(!asm.has_pending(), "{name}: {key:?} ends inside a message");
    }
    out
}

fn hex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

fn files() -> impl Iterator<Item = (String, Vec<Golden>)> {
    PAIRS
        .into_iter()
        .flat_map(|p| [p.to_owned(), format!("{p}-giop1.1")])
        .map(|name| {
            let messages = load(&name);
            (name, messages)
        })
}

#[test]
fn every_message_decodes() {
    for (name, messages) in files() {
        assert!(messages.len() > 100, "{name}");
        for (i, m) in messages.iter().enumerate() {
            m.message
                .decode()
                .unwrap_or_else(|e| panic!("{name} #{i}: {e}"));
        }
    }
}

/// In the captures omniORB writes a GIOP 1.2 wstring as a BOM followed by
/// the stream byte order, with no BOM for an empty string, and a wchar as
/// big endian without a BOM. rustorb writes the same bytes and reads them
/// back.
#[test]
fn wide_strings_match_omniorb() {
    for name in PAIRS {
        let messages = load(name);
        for direction in ["c2s", "s2c"] {
            let wstrings: Vec<Body> = messages
                .iter()
                .filter(|m| m.is(direction, "echo_wstring"))
                .map(Golden::body)
                .collect();
            assert_eq!(wstrings.len(), WSTRINGS.len(), "{name} {direction}");
            for (body, value) in wstrings.iter().zip(WSTRINGS) {
                assert_eq!(body.byte_order(), ByteOrder::Little);
                assert_eq!(body.decoder().read_wstring().unwrap(), value);
                let mut enc =
                    Encoder::with_offset(body.byte_order(), CdrVersion::Giop1_2, body.offset());
                enc.write_wstring(value).unwrap();
                assert_eq!(enc.as_bytes(), body.as_bytes(), "{name} {value}");
            }

            // omniORBpy servers drop the connection on a wchar request, so
            // some pairs have no wchar replies.
            let wchars: Vec<Body> = messages
                .iter()
                .filter(|m| m.is(direction, "echo_wchar"))
                .map(Golden::body)
                .collect();
            assert!(wchars.len() == WCHARS.len() || direction == "s2c", "{name}");
            for (body, value) in wchars.iter().zip(WCHARS) {
                assert_eq!(body.decoder().read_wchar().unwrap(), value);
                let mut enc =
                    Encoder::with_offset(body.byte_order(), CdrVersion::Giop1_2, body.offset());
                enc.write_wchar(value).unwrap();
                assert_eq!(enc.as_bytes(), body.as_bytes(), "{name} {value:#x}");
            }
        }
    }
}

#[derive(Debug, PartialEq)]
struct Record {
    id: i32,
    name: String,
    hue: u32,
    pos: (f64, f64),
    values: Vec<i32>,
    active: bool,
}

impl Record {
    fn decode(dec: &mut Decoder<'_>) -> rustorb_cdr::Result<Record> {
        Ok(Record {
            id: dec.read_i32()?,
            name: dec.read_string()?,
            hue: dec.read_u32()?,
            pos: (dec.read_f64()?, dec.read_f64()?),
            values: Vec::<i32>::decode(dec)?,
            active: dec.read_bool()?,
        })
    }

    /// Record i of the echo_records[large] case.
    fn large(i: usize) -> Record {
        let id = i as i32;
        Record {
            id,
            name: format!("r{i}"),
            hue: (i % 3) as u32,
            pos: (f64::from(id) + 0.5, -0.25 * f64::from(id)),
            values: vec![id, id + 1],
            active: i % 2 == 0,
        }
    }
}

/// Reads a whole RecordSeq body; `None` if it does not decode exactly.
fn decode_records(body: &Body) -> Option<Vec<Record>> {
    let mut dec = body.decoder();
    let len = dec.read_len(1).ok()?;
    let records = (0..len)
        .map(|_| Record::decode(&mut dec))
        .collect::<rustorb_cdr::Result<_>>()
        .ok()?;
    dec.is_empty().then_some(records)
}

/// The echo_records[large] request and reply are sent in three fragments
/// each, with doubles after every fragment boundary. In GIOP 1.1 omniORB
/// aligns each Fragment relative to its own start, which differs from the
/// joined message by 4 bytes; in GIOP 1.2 the two agree.
#[test]
fn large_messages_across_fragments() {
    let expected: Vec<Record> = (0..LARGE_RECORDS).map(Record::large).collect();
    for (name, messages) in files() {
        let large: Vec<&Golden> = messages
            .iter()
            .filter(|m| {
                (m.is("c2s", "echo_records") || m.is("s2c", "echo_records"))
                    && m.message.was_fragmented()
            })
            .collect();
        assert_eq!(
            large.len(),
            2,
            "{name}: fragmented echo_records request and reply"
        );
        for m in large {
            assert!(m.message.segments().len() >= 2, "{name}");
            let records = decode_records(&m.body());
            assert_eq!(records.as_ref(), Some(&expected), "{name} {}", m.direction);

            // Joined without the fragment boundaries, GIOP 1.1 is misread.
            let (_, plain) = decode_message(m.message.as_bytes()).unwrap();
            let (Message::Request { body, .. } | Message::Reply { body, .. }) = plain else {
                unreachable!();
            };
            let plain = decode_records(&body);
            if name.ends_with("giop1.1") {
                assert_ne!(plain.as_ref(), Some(&expected), "{name}");
            } else {
                assert_eq!(plain.as_ref(), Some(&expected), "{name}");
            }
        }

        // The 10000 character string crosses a fragment boundary as well.
        let long = "x".repeat(10000);
        let strings = messages
            .iter()
            .filter(|m| m.is("c2s", "echo_string") || m.is("s2c", "echo_string"))
            .filter(|m| m.message.was_fragmented());
        let mut n = 0;
        for m in strings {
            assert_eq!(m.body().decoder().read_string().unwrap(), long, "{name}");
            n += 1;
        }
        assert_eq!(n, 2, "{name}");
    }
}

//! Extended CDR (DDS-XTypes 1.3 clause 7.4) and the RTPS SerializedPayload
//! header. The expected bytes come from the examples of the specification
//! where it has them (clause numbers in each test), and from its rules
//! otherwise.

use rustorb_cdr::xcdr::{
    EncapsulationId, Extensibility, ListEnd, PID_EXTENDED, PID_IGNORE, XcdrFormat,
};
use rustorb_cdr::{ByteOrder, Decoder, Encoder, Encoding, Error};

const BE: ByteOrder = ByteOrder::Big;
const LE: ByteOrder = ByteOrder::Little;

/// DDS-XTypes 7.6.3.1.2: the options record the padding after the data.
#[test]
fn payload_padding_in_options() {
    // struct TypeA { short member1; }
    let mut enc = Encoder::xcdr_payload(XcdrFormat::PlainCdr, BE);
    enc.write_i16(0x11);
    assert_eq!(
        enc.finish_xcdr_payload(),
        [0x00, 0x00, 0x00, 0x02, 0x00, 0x11, 0x00, 0x00]
    );
    // struct TypeB { short member1; char member2; }
    let mut enc = Encoder::xcdr_payload(XcdrFormat::PlainCdr, BE);
    enc.write_i16(0x23);
    enc.write_char(b'b');
    let b = enc.finish_xcdr_payload();
    assert_eq!(b, [0x00, 0x00, 0x00, 0x01, 0x00, 0x23, b'b', 0x00]);
    // struct TypeC { short member1; char member2; char member3; }
    let mut enc = Encoder::xcdr_payload(XcdrFormat::PlainCdr, BE);
    enc.write_i16(0x23);
    enc.write_char(b'b');
    enc.write_char(b'c');
    assert_eq!(
        enc.finish_xcdr_payload(),
        [0x00, 0x00, 0x00, 0x00, 0x00, 0x23, b'b', b'c']
    );

    // The reader leaves the padding out.
    let (mut dec, header) = Decoder::xcdr_payload(&b).unwrap();
    assert_eq!(header.id.format, XcdrFormat::PlainCdr);
    assert_eq!(header.id.order, BE);
    assert_eq!(header.options, [0x00, 0x01]);
    assert_eq!(dec.read_i16().unwrap(), 0x23);
    assert_eq!(dec.read_char().unwrap(), b'b');
    assert!(dec.is_empty());
}

/// DDSI-RTPS 2.5 10.2: alignment restarts after the representation
/// options, so a `long long` right after them needs no padding.
#[test]
fn payload_alignment_starts_after_header() {
    for format in [XcdrFormat::PlainCdr, XcdrFormat::PlainCdr2] {
        let mut enc = Encoder::xcdr_payload(format, LE);
        enc.write_u64(0x0102_0304_0506_0708);
        enc.write_octet(9);
        enc.write_u64(1);
        let bytes = enc.finish_xcdr_payload();
        assert_eq!(&bytes[4..12], [8, 7, 6, 5, 4, 3, 2, 1]);
        // XCDR1 aligns the second one to 8 from the origin, XCDR2 to 4.
        let pad = if format == XcdrFormat::PlainCdr { 7 } else { 3 };
        assert_eq!(bytes.len(), 4 + 8 + 1 + pad + 8);

        let (mut dec, _) = Decoder::xcdr_payload(&bytes).unwrap();
        assert_eq!(dec.encoding(), format.encoding());
        assert_eq!(dec.read_u64().unwrap(), 0x0102_0304_0506_0708);
        assert_eq!(dec.read_octet().unwrap(), 9);
        assert_eq!(dec.read_u64().unwrap(), 1);
        assert!(dec.is_empty());
    }
}

/// DDS-XTypes Table 39 and DDSI-RTPS 2.5 Table 10.3.
#[test]
fn encapsulation_identifiers() {
    let cases = [
        (XcdrFormat::PlainCdr, BE, [0x00, 0x00]),
        (XcdrFormat::PlainCdr, LE, [0x00, 0x01]),
        (XcdrFormat::PlCdr, BE, [0x00, 0x02]),
        (XcdrFormat::PlCdr, LE, [0x00, 0x03]),
        (XcdrFormat::PlainCdr2, BE, [0x00, 0x10]),
        (XcdrFormat::PlainCdr2, LE, [0x00, 0x11]),
        (XcdrFormat::PlCdr2, BE, [0x00, 0x12]),
        (XcdrFormat::PlCdr2, LE, [0x00, 0x13]),
        (XcdrFormat::DelimitedCdr2, BE, [0x00, 0x14]),
        (XcdrFormat::DelimitedCdr2, LE, [0x00, 0x15]),
    ];
    for (format, order, bytes) in cases {
        let id = EncapsulationId { format, order };
        assert_eq!(id.to_bytes(), bytes, "{id:?}");
        assert_eq!(EncapsulationId::from_bytes(bytes).unwrap(), id);
    }
    // DDS-XTypes Table 60 gives other values for the version 2 formats.
    let table60 = [
        ([0x00, 0x06], XcdrFormat::PlainCdr2, BE),
        ([0x00, 0x07], XcdrFormat::PlainCdr2, LE),
        ([0x00, 0x08], XcdrFormat::DelimitedCdr2, BE),
        ([0x00, 0x09], XcdrFormat::DelimitedCdr2, LE),
        ([0x00, 0x0a], XcdrFormat::PlCdr2, BE),
        ([0x00, 0x0b], XcdrFormat::PlCdr2, LE),
    ];
    for (bytes, format, order) in table60 {
        assert_eq!(
            EncapsulationId::from_bytes(bytes).unwrap(),
            EncapsulationId { format, order }
        );
    }
    // XML and unknown identifiers.
    for bytes in [[0x00, 0x04], [0x01, 0x00], [0x00, 0x20]] {
        assert!(matches!(
            EncapsulationId::from_bytes(bytes),
            Err(Error::InvalidXcdr(_))
        ));
    }
}

/// DDS-XTypes Table 35.
#[test]
fn formats_by_extensibility() {
    use Extensibility::*;
    let cases = [
        (Encoding::Xcdr1, Final, XcdrFormat::PlainCdr),
        (Encoding::Xcdr1, Appendable, XcdrFormat::PlainCdr),
        (Encoding::Xcdr1, Mutable, XcdrFormat::PlCdr),
        (Encoding::Xcdr2, Final, XcdrFormat::PlainCdr2),
        (Encoding::Xcdr2, Appendable, XcdrFormat::DelimitedCdr2),
        (Encoding::Xcdr2, Mutable, XcdrFormat::PlCdr2),
    ];
    for (encoding, kind, format) in cases {
        assert_eq!(XcdrFormat::new(encoding, kind), Some(format));
        assert_eq!(format.encoding(), encoding);
    }
    assert_eq!(XcdrFormat::new(Encoding::Giop1_2, Final), None);
}

/// DDS-XTypes 7.6.8, KeyHash example 2: PLAIN_CDR2 big endian of
/// `{ string<12> label = "BLUE"; long long id = 0x123456789abcdef0; }`.
/// The `long long` is aligned to 4.
#[test]
fn plain_cdr2_aligns_to_4() {
    let mut enc = Encoder::new(BE, Encoding::Xcdr2);
    enc.write_string("BLUE").unwrap();
    enc.write_u64(0x1234_5678_9abc_def0);
    assert_eq!(
        enc.as_bytes(),
        [
            0x00, 0x00, 0x00, 0x05, 0x42, 0x4c, 0x55, 0x45, 0x00, 0x00, 0x00, 0x00, 0x12, 0x34,
            0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0,
        ]
    );
    let mut dec = Decoder::new(enc.as_bytes(), BE, Encoding::Xcdr2);
    assert_eq!(dec.read_string().unwrap(), "BLUE");
    assert_eq!(dec.read_u64().unwrap(), 0x1234_5678_9abc_def0);
    assert!(dec.is_empty());

    // KeyHash example 3: a nested `long` then the string.
    let mut enc = Encoder::new(BE, Encoding::Xcdr2);
    enc.write_i32(0x1234_5678);
    enc.write_string("BLUE").unwrap();
    assert_eq!(
        enc.as_bytes(),
        [
            0x12, 0x34, 0x56, 0x78, 0x00, 0x00, 0x00, 0x05, 0x42, 0x4c, 0x55, 0x45, 0x00
        ]
    );
}

/// DDS-XTypes 7.4.1.1.2 and rule 4: `Char16` is an aligned code unit, a
/// `String<Char16>` an octet length and code units, without BOM or NUL.
#[test]
fn wide_characters() {
    for encoding in [Encoding::Xcdr1, Encoding::Xcdr2] {
        let mut enc = Encoder::new(BE, encoding);
        enc.write_octet(1);
        enc.write_wchar(0x00e9).unwrap();
        enc.write_wstring("a\u{1F600}").unwrap();
        enc.write_wstring("").unwrap();
        assert_eq!(
            enc.as_bytes(),
            [
                0x01, 0x00, 0x00, 0xe9, // octet, pad, wchar
                0x00, 0x00, 0x00, 0x06, // 3 code units
                0x00, 0x61, 0xd8, 0x3d, 0xde, 0x00, // 'a', surrogate pair
                0x00, 0x00, // pad
                0x00, 0x00, 0x00, 0x00, // empty
            ]
        );
        let mut dec = Decoder::new(enc.as_bytes(), BE, encoding);
        assert_eq!(dec.read_octet().unwrap(), 1);
        assert_eq!(dec.read_wchar().unwrap(), 0x00e9);
        assert_eq!(dec.read_wstring().unwrap(), "a\u{1F600}");
        assert_eq!(dec.read_wstring().unwrap(), "");
        assert!(dec.is_empty());
    }
    let odd = [0x00, 0x00, 0x00, 0x03, 0x00, 0x61, 0x00];
    let mut dec = Decoder::new(&odd, BE, Encoding::Xcdr2);
    assert_eq!(dec.read_wstring(), Err(Error::OddWstringLength(3)));
}

/// DELIMITED_CDR (rule 30): a DHEADER with the length of what follows.
/// A reader that knows fewer members skips the rest.
#[test]
fn dheader() {
    let mut enc = Encoder::xcdr_payload(XcdrFormat::DelimitedCdr2, LE);
    let mark = enc.begin_dheader();
    enc.write_octet(7);
    enc.write_u64(42);
    enc.write_string("new").unwrap();
    enc.end_dheader(mark).unwrap();
    enc.write_octet(0xEE);
    let bytes = enc.finish_xcdr_payload();
    assert_eq!(&bytes[..8], [0x00, 0x15, 0x00, 0x03, 20, 0, 0, 0]);

    // An older reader knows the first two members only.
    let (mut dec, _) = Decoder::xcdr_payload(&bytes).unwrap();
    let delimited = dec.read_dheader().unwrap();
    assert_eq!(dec.read_octet().unwrap(), 7);
    assert_eq!(dec.read_u64().unwrap(), 42);
    assert!(delimited.has_more(&dec));
    assert_eq!(delimited.remaining(&dec), 8);
    dec.end_dheader(delimited).unwrap();
    assert_eq!(dec.read_octet().unwrap(), 0xEE);
    assert!(dec.is_empty());

    // A DHEADER longer than the data, and an object longer than its
    // DHEADER.
    let mut dec = Decoder::new(&[9, 0, 0, 0, 1, 2], LE, Encoding::Xcdr2);
    assert!(matches!(dec.read_dheader(), Err(Error::InvalidXcdr(_))));
    let mut dec = Decoder::new(&[1, 0, 0, 0, 1, 2], LE, Encoding::Xcdr2);
    let delimited = dec.read_dheader().unwrap();
    dec.read_u16().unwrap();
    assert!(matches!(
        dec.end_dheader(delimited),
        Err(Error::InvalidXcdr(_))
    ));
}

/// PL_CDR2 (rules 21 and 22, 7.4.3.4.2): a DHEADER, then each member
/// after an EMHEADER. Members of 1, 2, 4 or 8 octets get length codes 0
/// to 3; others length code 4 and NEXTINT.
#[test]
fn emheader() {
    let mut enc = Encoder::xcdr_payload(XcdrFormat::PlCdr2, BE);
    let object = enc.begin_dheader();
    let m = enc.begin_member(1, false).unwrap();
    enc.write_octet(0xAA);
    enc.end_member(m).unwrap();
    let m = enc.begin_member(2, true).unwrap();
    enc.write_u16(0xBBBB);
    enc.end_member(m).unwrap();
    let m = enc.begin_member(3, false).unwrap();
    enc.write_u32(0xCCCC_CCCC);
    enc.end_member(m).unwrap();
    let m = enc.begin_member(0x0FFF_FFFF, false).unwrap();
    enc.write_u64(0xDDDD_DDDD_DDDD_DDDD);
    enc.end_member(m).unwrap();
    let m = enc.begin_member(5, false).unwrap();
    enc.write_string("hi").unwrap();
    enc.end_member(m).unwrap();
    enc.end_dheader(object).unwrap();
    let bytes = enc.finish_xcdr_payload();
    #[rustfmt::skip]
    let expected = [
        0x00, 0x12, 0x00, 0x01,
        0x00, 0x00, 0x00, 51, // DHEADER
        0x00, 0x00, 0x00, 0x01, 0xAA, 0x00, 0x00, 0x00, // LC 0, id 1
        0x90, 0x00, 0x00, 0x02, 0xBB, 0xBB, 0x00, 0x00, // M_FLAG, LC 1, id 2
        0x20, 0x00, 0x00, 0x03, 0xCC, 0xCC, 0xCC, 0xCC, // LC 2, id 3
        0x3F, 0xFF, 0xFF, 0xFF, 0xDD, 0xDD, 0xDD, 0xDD, 0xDD, 0xDD, 0xDD, 0xDD, // LC 3
        0x40, 0x00, 0x00, 0x05, 0x00, 0x00, 0x00, 0x07, // LC 4, id 5, NEXTINT 7
        0x00, 0x00, 0x00, 0x03, b'h', b'i', 0x00, // string
        0x00, // padding
    ];
    assert_eq!(bytes, expected);

    let (mut dec, _) = Decoder::xcdr_payload(&bytes).unwrap();
    let object = dec.read_dheader().unwrap();
    let mut seen = Vec::new();
    while object.has_more(&dec) {
        let m = dec.read_member_header().unwrap();
        seen.push((m.id, m.must_understand, m.len));
        match m.id {
            3 => assert_eq!(dec.read_u32().unwrap(), 0xCCCC_CCCC),
            5 => assert_eq!(dec.read_string().unwrap(), "hi"),
            _ => {} // skipped
        }
        dec.end_member(m).unwrap();
    }
    dec.end_dheader(object).unwrap();
    assert!(dec.is_empty());
    assert_eq!(
        seen,
        [
            (1, false, 1),
            (2, true, 2),
            (3, false, 4),
            (0x0FFF_FFFF, false, 8),
            (5, false, 7)
        ]
    );

    assert!(matches!(
        enc_member_id_too_large(),
        Err(Error::InvalidXcdr(_))
    ));
    let mut v1 = Encoder::new(BE, Encoding::Xcdr1);
    assert!(matches!(
        v1.begin_member(1, false),
        Err(Error::InvalidXcdr(_))
    ));
}

fn enc_member_id_too_large() -> rustorb_cdr::Result<()> {
    let mut enc = Encoder::new(BE, Encoding::Xcdr2);
    enc.begin_member(0x1000_0000, false).map(drop)
}

/// Length codes 5 to 7 share NEXTINT with the member: the reader is left
/// on it, and the length counts it.
#[test]
fn emheader_shared_nextint() {
    #[rustfmt::skip]
    let bytes = [
        0x50, 0x00, 0x00, 0x01, // LC 5, id 1
        0x00, 0x00, 0x00, 0x03, b'a', b'b', 0x00, 0x00, // string "ab", pad
        0x60, 0x00, 0x00, 0x02, // LC 6, id 2
        0x00, 0x00, 0x00, 0x02, 0, 0, 0, 1, 0, 0, 0, 2, // sequence<long> {1, 2}
        0x70, 0x00, 0x00, 0x03, // LC 7, id 3
        0x00, 0x00, 0x00, 0x01, 0, 0, 0, 0, 0, 0, 0, 9, // sequence<long long> {9}
    ];
    let mut dec = Decoder::new(&bytes, BE, Encoding::Xcdr2);
    let m = dec.read_member_header().unwrap();
    assert_eq!((m.id, m.len), (1, 7));
    assert_eq!(dec.read_string().unwrap(), "ab");
    dec.end_member(m).unwrap();
    let m = dec.read_member_header().unwrap();
    assert_eq!((m.id, m.len), (2, 12));
    let len = dec.read_len(4).unwrap();
    let values: Vec<i32> = (0..len).map(|_| dec.read_i32().unwrap()).collect();
    assert_eq!(values, [1, 2]);
    dec.end_member(m).unwrap();
    let m = dec.read_member_header().unwrap();
    assert_eq!((m.id, m.len), (3, 12));
    // Not read: skipped.
    dec.end_member(m).unwrap();
    assert!(dec.is_empty());

    // NEXTINT larger than the data.
    let short = [0x60, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x02, 0, 0, 0, 1];
    let mut dec = Decoder::new(&short, BE, Encoding::Xcdr2);
    assert!(matches!(
        dec.read_member_header(),
        Err(Error::InvalidXcdr(_))
    ));
}

/// PL_CDR (rules 23 and 24): each member after a parameter header,
/// aligned from the end of the header, and `PID_LIST_END` at the end.
#[test]
fn parameter_list() {
    let mut enc = Encoder::xcdr_payload(XcdrFormat::PlCdr, BE);
    let p = enc.begin_parameter(1, false).unwrap();
    enc.write_octet(0xAA);
    enc.end_parameter(p).unwrap();
    let p = enc.begin_parameter(2, true).unwrap();
    enc.write_u64(0x0102_0304_0506_0708);
    enc.end_parameter(p).unwrap();
    enc.end_parameter_list(ListEnd::Xtypes);
    let bytes = enc.finish_xcdr_payload();
    #[rustfmt::skip]
    let expected = [
        0x00, 0x02, 0x00, 0x00,
        0x00, 0x01, 0x00, 0x01, 0xAA, 0x00, 0x00, 0x00, // id 1, length 1, pad
        0x40, 0x02, 0x00, 0x08, // must understand, id 2, length 8
        0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, // no padding before it
        0x7F, 0x02, 0x00, 0x00, // PID_LIST_END
    ];
    assert_eq!(bytes, expected);

    let (mut dec, _) = Decoder::xcdr_payload(&bytes).unwrap();
    let p = dec.read_parameter(ListEnd::Xtypes).unwrap().unwrap();
    assert_eq!((p.id, p.must_understand, p.len), (1, false, 1));
    dec.end_parameter(p).unwrap();
    let p = dec.read_parameter(ListEnd::Xtypes).unwrap().unwrap();
    assert_eq!((p.id, p.must_understand, p.len), (2, true, 8));
    assert_eq!(dec.read_u64().unwrap(), 0x0102_0304_0506_0708);
    dec.end_parameter(p).unwrap();
    assert_eq!(dec.read_parameter(ListEnd::Xtypes).unwrap(), None);
    assert!(dec.is_empty());
}

/// Readers skip `PID_IGNORE`, accept `PID_LIST_END` without the
/// must-understand flag, and for the RTPS built-in types end at
/// `PID_SENTINEL`, whose length is not looked at.
#[test]
fn parameter_list_ends() {
    #[rustfmt::skip]
    let bytes = [
        0x3F, 0x03, 0x00, 0x04, 0xFF, 0xFF, 0xFF, 0xFF, // PID_IGNORE
        0x00, 0x05, 0x00, 0x02, 0x00, 0x07, 0x00, 0x00, // id 5
        0x3F, 0x02, 0x00, 0x00, // PID_LIST_END, no flag
    ];
    let mut dec = Decoder::new(&bytes, BE, Encoding::Xcdr1);
    let p = dec.read_parameter(ListEnd::Xtypes).unwrap().unwrap();
    assert_eq!(p.id, 5);
    assert_eq!(dec.read_u16().unwrap(), 7);
    dec.end_parameter(p).unwrap();
    assert_eq!(dec.read_parameter(ListEnd::Xtypes).unwrap(), None);
    assert!(dec.is_empty());
    assert_eq!(PID_IGNORE, 0x3F03);

    // PID_SENTINEL with a length that is not looked at.
    let rtps = [0x00, 0x01, 0xFF, 0xFF];
    let mut dec = Decoder::new(&rtps, BE, Encoding::Xcdr1);
    assert_eq!(dec.read_parameter(ListEnd::Rtps).unwrap(), None);
    // For a user type it is member 1, here longer than the data.
    let mut dec = Decoder::new(&rtps, BE, Encoding::Xcdr1);
    assert!(matches!(
        dec.read_parameter(ListEnd::Xtypes),
        Err(Error::InvalidXcdr(_))
    ));
    let mut enc = Encoder::new(LE, Encoding::Xcdr1);
    enc.end_parameter_list(ListEnd::Rtps);
    assert_eq!(enc.as_bytes(), [0x01, 0x00, 0x00, 0x00]);
}

/// DDS-XTypes 7.4.1.2, Figure 24: the extended parameter header, for a
/// member ID above 0x3F00 or a member longer than 65535 octets.
#[test]
fn extended_parameter_header() {
    for order in [BE, LE] {
        let mut enc = Encoder::new(order, Encoding::Xcdr1);
        let p = enc.begin_parameter(0x0012_3456, true).unwrap();
        enc.write_u32(0xCAFE_F00D);
        enc.end_parameter(p).unwrap();
        let header: &[u8] = match order {
            ByteOrder::Big => &[0x7F, 0x01, 0x00, 0x08, 0x40, 0x12, 0x34, 0x56, 0, 0, 0, 4],
            ByteOrder::Little => &[0x01, 0x7F, 0x08, 0x00, 0x56, 0x34, 0x12, 0x40, 4, 0, 0, 0],
        };
        assert_eq!(&enc.as_bytes()[..12], header);
        let mut dec = Decoder::new(enc.as_bytes(), order, Encoding::Xcdr1);
        let p = dec.read_parameter_header().unwrap();
        assert_eq!((p.id, p.must_understand, p.len), (0x0012_3456, true, 4));
        assert_eq!(dec.read_u32().unwrap(), 0xCAFE_F00D);
        dec.end_parameter(p).unwrap();
        assert!(dec.is_empty());
    }
    assert_eq!(PID_EXTENDED, 0x7F01);

    // A long member turns a short header into an extended one; the
    // member keeps its alignment from the end of the header.
    let mut enc = Encoder::new(BE, Encoding::Xcdr1);
    enc.write_octet(1);
    let p = enc.begin_parameter(9, false).unwrap();
    enc.write_u32(0x7000_0000);
    enc.write_octets(&[0x55; 70_000]);
    enc.write_u64(3);
    enc.end_parameter(p).unwrap();
    enc.write_octet(2);
    let len = 4 + 70_000 + 4 + 8; // the u64 is aligned to 8 from the header end
    let bytes = enc.into_bytes();
    assert_eq!(&bytes[4..8], [0x7F, 0x01, 0x00, 0x08]);
    assert_eq!(&bytes[8..12], [0x00, 0x00, 0x00, 0x09]);
    assert_eq!(&bytes[12..16], (len as u32).to_be_bytes());
    let mut dec = Decoder::new(&bytes, BE, Encoding::Xcdr1);
    assert_eq!(dec.read_octet().unwrap(), 1);
    let p = dec.read_parameter_header().unwrap();
    assert_eq!((p.id, p.len), (9, len));
    assert_eq!(dec.read_u32().unwrap(), 0x7000_0000);
    dec.skip(70_000).unwrap();
    assert_eq!(dec.read_u64().unwrap(), 3);
    dec.end_parameter(p).unwrap();
    assert_eq!(dec.read_octet().unwrap(), 2);
    assert!(dec.is_empty());

    let mut bad = Decoder::new(&[0x7F, 0x01, 0x00, 0x04, 0, 0, 0, 0], BE, Encoding::Xcdr1);
    assert!(matches!(
        bad.read_parameter_header(),
        Err(Error::InvalidXcdr(_))
    ));
}

/// Optional members of final and appendable types: in XCDR1 a parameter
/// header with length 0 when absent (7.4.1.1.5.2, rule 19); in XCDR2 a
/// boolean (rule 20).
#[test]
fn optional_members() {
    // struct { octet a; @optional long b; @optional long c; } with b
    // absent and c = 5.
    let mut enc = Encoder::xcdr_payload(XcdrFormat::PlainCdr, BE);
    enc.write_octet(1);
    let p = enc.begin_parameter(1, false).unwrap();
    enc.end_parameter(p).unwrap();
    let p = enc.begin_parameter(2, false).unwrap();
    enc.write_i32(5);
    enc.end_parameter(p).unwrap();
    let bytes = enc.finish_xcdr_payload();
    #[rustfmt::skip]
    let expected = [
        0x00, 0x00, 0x00, 0x00,
        0x01, 0x00, 0x00, 0x00,
        0x00, 0x01, 0x00, 0x00, // b absent
        0x00, 0x02, 0x00, 0x04, 0x00, 0x00, 0x00, 0x05, // c
    ];
    assert_eq!(bytes, expected);
    let (mut dec, _) = Decoder::xcdr_payload(&bytes).unwrap();
    assert_eq!(dec.read_octet().unwrap(), 1);
    let b = dec.read_parameter_header().unwrap();
    assert_eq!(b.len, 0);
    dec.end_parameter(b).unwrap();
    let c = dec.read_parameter_header().unwrap();
    assert_eq!(dec.read_i32().unwrap(), 5);
    dec.end_parameter(c).unwrap();
    assert!(dec.is_empty());

    let mut enc = Encoder::xcdr_payload(XcdrFormat::PlainCdr2, BE);
    enc.write_octet(1);
    enc.write_bool(false);
    enc.write_bool(true);
    enc.write_i32(5);
    assert_eq!(
        enc.finish_xcdr_payload(),
        [
            0x00, 0x10, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x05
        ]
    );
}

/// Nested parameter lists: each restores the alignment origin of the one
/// around it (7.4.1.2.4).
#[test]
fn nested_parameter_lists() {
    let mut enc = Encoder::xcdr_payload(XcdrFormat::PlCdr, LE);
    let outer = enc.begin_parameter(1, false).unwrap();
    enc.write_octet(0x11);
    let inner = enc.begin_parameter(1, false).unwrap();
    enc.write_u64(0x22);
    enc.end_parameter(inner).unwrap();
    enc.end_parameter_list(ListEnd::Xtypes);
    enc.end_parameter(outer).unwrap();
    let p = enc.begin_parameter(2, false).unwrap();
    enc.write_u64(0x33);
    enc.end_parameter(p).unwrap();
    enc.end_parameter_list(ListEnd::Xtypes);
    let bytes = enc.finish_xcdr_payload();

    let (mut dec, _) = Decoder::xcdr_payload(&bytes).unwrap();
    let outer = dec.read_parameter(ListEnd::Xtypes).unwrap().unwrap();
    assert_eq!(outer.len, 1 + 3 + 4 + 8 + 4);
    assert_eq!(dec.read_octet().unwrap(), 0x11);
    let inner = dec.read_parameter(ListEnd::Xtypes).unwrap().unwrap();
    assert_eq!(dec.read_u64().unwrap(), 0x22);
    dec.end_parameter(inner).unwrap();
    assert_eq!(dec.read_parameter(ListEnd::Xtypes).unwrap(), None);
    dec.end_parameter(outer).unwrap();
    let p = dec.read_parameter(ListEnd::Xtypes).unwrap().unwrap();
    assert_eq!((p.id, p.len), (2, 8));
    assert_eq!(dec.read_u64().unwrap(), 0x33);
    dec.end_parameter(p).unwrap();
    assert_eq!(dec.read_parameter(ListEnd::Xtypes).unwrap(), None);
    assert!(dec.is_empty());
}

#[test]
fn payload_errors() {
    assert!(matches!(
        Decoder::xcdr_payload(&[0x00, 0x01, 0x00]),
        Err(Error::UnexpectedEof { .. })
    ));
    // Padding longer than the data.
    assert!(matches!(
        Decoder::xcdr_payload(&[0x00, 0x01, 0x00, 0x03, 0x00]),
        Err(Error::InvalidXcdr(_))
    ));
    let (dec, _) = Decoder::xcdr_payload(&[0x00, 0x01, 0x00, 0x00]).unwrap();
    assert!(dec.is_empty());
}

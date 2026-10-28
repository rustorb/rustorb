use rustorb_cdr::{ByteOrder, CdrDecode, CdrEncode, CdrVersion, Decoder, Encoder, Error};

const V12: CdrVersion = CdrVersion::Giop1_2;

fn encode<T: CdrEncode + ?Sized>(value: &T, order: ByteOrder) -> Vec<u8> {
    let mut enc = Encoder::new(order, V12);
    value.encode(&mut enc).unwrap();
    enc.into_bytes()
}

fn decode<T: CdrDecode>(bytes: &[u8], order: ByteOrder) -> T {
    let mut dec = Decoder::new(bytes, order, V12);
    let value = T::decode(&mut dec).unwrap();
    assert!(dec.is_empty(), "{} bytes left", dec.remaining());
    value
}

fn roundtrip<T: CdrEncode + CdrDecode + PartialEq + std::fmt::Debug>(value: T) {
    for order in [ByteOrder::Big, ByteOrder::Little] {
        let bytes = encode(&value, order);
        assert_eq!(decode::<T>(&bytes, order), value, "{order:?}");
    }
}

#[test]
fn primitives_big_endian() {
    let mut enc = Encoder::new(ByteOrder::Big, V12);
    enc.write_octet(0xAB);
    enc.write_i16(-2);
    enc.write_u32(0x0102_0304);
    enc.write_bool(true);
    enc.write_u64(0x0102_0304_0506_0708);
    enc.write_f32(1.0);
    enc.write_f64(-2.0);
    assert_eq!(
        enc.as_bytes(),
        [
            0xAB, 0x00, // octet, pad to 2
            0xFF, 0xFE, // short
            0x01, 0x02, 0x03, 0x04, // ulong
            0x01, // boolean
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // pad to 16
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, // ulonglong
            0x3F, 0x80, 0x00, 0x00, // float
            0x00, 0x00, 0x00, 0x00, // pad to 32
            0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // double
        ]
    );

    let mut dec = Decoder::new(enc.as_bytes(), ByteOrder::Big, V12);
    assert_eq!(dec.read_octet().unwrap(), 0xAB);
    assert_eq!(dec.read_i16().unwrap(), -2);
    assert_eq!(dec.read_u32().unwrap(), 0x0102_0304);
    assert!(dec.read_bool().unwrap());
    assert_eq!(dec.read_u64().unwrap(), 0x0102_0304_0506_0708);
    assert_eq!(dec.read_f32().unwrap(), 1.0);
    assert_eq!(dec.read_f64().unwrap(), -2.0);
    assert!(dec.is_empty());
}

#[test]
fn primitives_little_endian() {
    let mut enc = Encoder::new(ByteOrder::Little, V12);
    enc.write_u16(0x0102);
    enc.write_i32(-1);
    enc.write_u64(0x0102_0304_0506_0708);
    assert_eq!(
        enc.as_bytes(),
        [
            0x02, 0x01, 0x00, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0x08, 0x07, 0x06, 0x05, 0x04, 0x03,
            0x02, 0x01,
        ]
    );
}

#[test]
fn primitive_roundtrips() {
    roundtrip(true);
    roundtrip(0x7Fu8);
    roundtrip(-5i8);
    roundtrip(i16::MIN);
    roundtrip(u16::MAX);
    roundtrip(i32::MIN);
    roundtrip(u32::MAX);
    roundtrip(i64::MIN);
    roundtrip(u64::MAX);
    roundtrip(f32::MIN_POSITIVE);
    roundtrip(std::f64::consts::PI);
}

#[test]
fn alignment_follows_stream_offset() {
    // A body that starts at offset 12 (after a GIOP header) is aligned
    // against the whole message, not against the start of the body.
    let mut enc = Encoder::with_offset(ByteOrder::Big, V12, 12);
    enc.write_octet(1);
    enc.write_u64(2);
    assert_eq!(enc.len(), 1 + 3 + 8);
    assert_eq!(enc.position(), 24);

    let mut dec = Decoder::with_offset(enc.as_bytes(), ByteOrder::Big, V12, 12);
    assert_eq!(dec.read_octet().unwrap(), 1);
    assert_eq!(dec.read_u64().unwrap(), 2);
}

#[test]
fn padding_is_zero_and_skipped_regardless_of_value() {
    let bytes = [0x07, 0xAA, 0xBB, 0xCC, 0x00, 0x00, 0x00, 0x2A];
    let mut dec = Decoder::new(&bytes, ByteOrder::Big, V12);
    assert_eq!(dec.read_octet().unwrap(), 7);
    assert_eq!(dec.read_u32().unwrap(), 42);
}

#[test]
fn invalid_boolean() {
    let mut dec = Decoder::new(&[2], ByteOrder::Big, V12);
    assert_eq!(dec.read_bool(), Err(Error::InvalidBoolean(2)));
}

#[test]
fn truncated_input() {
    let mut dec = Decoder::new(&[0, 0, 1], ByteOrder::Big, V12);
    assert_eq!(
        dec.read_u32(),
        Err(Error::UnexpectedEof {
            needed: 4,
            remaining: 3
        })
    );
    // Padding past the end is also an error.
    let mut dec = Decoder::new(&[0, 0], ByteOrder::Big, V12);
    dec.read_octet().unwrap();
    assert!(dec.read_u32().is_err());
}

#[test]
fn string_encoding() {
    assert_eq!(encode("Hi", ByteOrder::Big), [0, 0, 0, 3, b'H', b'i', 0]);
    assert_eq!(encode("", ByteOrder::Little), [1, 0, 0, 0, 0]);
    roundtrip(String::from("こんにちは"));
    roundtrip(String::new());
}

#[test]
fn string_decoding_errors() {
    let mut dec = Decoder::new(&[0, 0, 0, 2, b'a', b'b'], ByteOrder::Big, V12);
    assert_eq!(dec.read_string(), Err(Error::StringNotTerminated));

    let mut dec = Decoder::new(&[0, 0, 0, 2, 0xFF, 0], ByteOrder::Big, V12);
    assert_eq!(dec.read_string(), Err(Error::InvalidUtf8));
    // The raw bytes are still available for non-UTF-8 code sets.
    let mut dec = Decoder::new(&[0, 0, 0, 2, 0xE9, 0], ByteOrder::Big, V12);
    assert_eq!(dec.read_string_bytes().unwrap(), [0xE9]);

    let mut dec = Decoder::new(&[0, 0, 0, 100, b'a', 0], ByteOrder::Big, V12);
    assert_eq!(
        dec.read_string(),
        Err(Error::LengthTooLarge {
            len: 100,
            remaining: 2
        })
    );

    // Length 0 is tolerated as an empty string.
    let mut dec = Decoder::new(&[0, 0, 0, 0], ByteOrder::Big, V12);
    assert_eq!(dec.read_string().unwrap(), "");
}

#[test]
fn wstring_giop_1_2() {
    let mut enc = Encoder::new(ByteOrder::Little, V12);
    enc.write_wstring("Aé").unwrap();
    // Octet length, then big-endian UTF-16 with no BOM or terminator.
    assert_eq!(enc.as_bytes(), [4, 0, 0, 0, 0x00, 0x41, 0x00, 0xE9]);

    let mut dec = Decoder::new(enc.as_bytes(), ByteOrder::Little, V12);
    assert_eq!(dec.read_wstring().unwrap(), "Aé");

    // A BOM overrides the default order.
    let le_bom = [6, 0, 0, 0, 0xFF, 0xFE, 0x41, 0x00, 0xE9, 0x00];
    let mut dec = Decoder::new(&le_bom, ByteOrder::Little, V12);
    assert_eq!(dec.read_wstring().unwrap(), "Aé");

    // Peers that write the stream byte order without a BOM.
    let le = [4, 0, 0, 0, 0x41, 0x00, 0xE9, 0x00];
    let mut dec = Decoder::new(&le, ByteOrder::Little, V12);
    dec.set_wstring_default_order(ByteOrder::Little);
    assert_eq!(dec.read_wstring().unwrap(), "Aé");

    // Characters outside the BMP use surrogate pairs.
    for order in [ByteOrder::Big, ByteOrder::Little] {
        let mut enc = Encoder::new(order, V12);
        enc.write_wstring("🦀").unwrap();
        assert_eq!(enc.len(), 4 + 4);
        let mut dec = Decoder::new(enc.as_bytes(), order, V12);
        assert_eq!(dec.read_wstring().unwrap(), "🦀");
    }

    let odd = [3, 0, 0, 0, 0, 0x41, 0];
    let mut dec = Decoder::new(&odd, ByteOrder::Little, V12);
    assert_eq!(dec.read_wstring(), Err(Error::OddWstringLength(3)));

    let lone_surrogate = [0, 0, 0, 2, 0xD8, 0x00];
    let mut dec = Decoder::new(&lone_surrogate, ByteOrder::Big, V12);
    assert_eq!(dec.read_wstring(), Err(Error::InvalidUtf16));
}

#[test]
fn wstring_giop_1_1() {
    let v11 = CdrVersion::Giop1_1;
    let mut enc = Encoder::new(ByteOrder::Little, v11);
    enc.write_octet(9);
    enc.write_wstring("Hi").unwrap();
    // Length counts code units including the NUL; units are aligned and in
    // the stream byte order.
    assert_eq!(
        enc.as_bytes(),
        [9, 0, 0, 0, 3, 0, 0, 0, b'H', 0, b'i', 0, 0, 0]
    );
    let mut dec = Decoder::new(enc.as_bytes(), ByteOrder::Little, v11);
    dec.read_octet().unwrap();
    assert_eq!(dec.read_wstring().unwrap(), "Hi");
}

#[test]
fn wchar_by_version() {
    let mut enc = Encoder::new(ByteOrder::Big, CdrVersion::Giop1_0);
    assert_eq!(enc.write_wchar(0x41), Err(Error::WcharNotSupported));
    assert_eq!(enc.write_wstring("A"), Err(Error::WcharNotSupported));

    let mut enc = Encoder::new(ByteOrder::Little, CdrVersion::Giop1_1);
    enc.write_octet(0);
    enc.write_wchar(0x3042).unwrap();
    assert_eq!(enc.as_bytes(), [0, 0, 0x42, 0x30]);

    let mut enc = Encoder::new(ByteOrder::Little, V12);
    enc.write_wchar(0x3042).unwrap();
    assert_eq!(enc.as_bytes(), [2, 0x30, 0x42]);
    let mut dec = Decoder::new(enc.as_bytes(), ByteOrder::Little, V12);
    assert_eq!(dec.read_wchar().unwrap(), 0x3042);

    let with_bom = [4, 0xFF, 0xFE, 0x42, 0x30];
    let mut dec = Decoder::new(&with_bom, ByteOrder::Big, V12);
    assert_eq!(dec.read_wchar().unwrap(), 0x3042);

    let mut dec = Decoder::new(&[3, 0, 0, 0], ByteOrder::Big, V12);
    assert_eq!(dec.read_wchar(), Err(Error::InvalidWcharLength(3)));
}

#[test]
fn sequences() {
    assert_eq!(
        encode(&vec![1u16, 2, 3], ByteOrder::Big),
        [0, 0, 0, 3, 0, 1, 0, 2, 0, 3]
    );
    assert_eq!(
        encode(&vec![1u8, 2, 3], ByteOrder::Little),
        [3, 0, 0, 0, 1, 2, 3]
    );
    roundtrip(Vec::<u32>::new());
    roundtrip(vec![1.5f64, -0.25]);
    roundtrip(vec![String::from("a"), String::from("bc")]);
    roundtrip(vec![vec![1i16], vec![], vec![2, 3]]);
}

#[test]
fn sequence_length_guard() {
    // A huge length must fail before allocating.
    let bytes = [0xFF, 0xFF, 0xFF, 0xFF, 0, 0, 0, 0];
    let mut dec = Decoder::new(&bytes, ByteOrder::Big, V12);
    assert!(matches!(
        Vec::<u64>::decode(&mut dec),
        Err(Error::LengthTooLarge { .. })
    ));
    let mut dec = Decoder::new(&bytes, ByteOrder::Big, V12);
    assert!(matches!(dec.read_len(8), Err(Error::LengthTooLarge { .. })));
}

#[test]
fn arrays_have_no_length() {
    assert_eq!(encode(&[1u16, 2], ByteOrder::Big), [0, 1, 0, 2]);
    roundtrip([[1i32, 2], [3, 4]]);
    roundtrip([7u8; 5]);
}

/// A struct as generated code would implement it.
#[derive(Debug, PartialEq)]
struct Sample {
    flag: bool,
    value: f64,
    name: String,
    points: Vec<Point>,
}

#[derive(Debug, PartialEq)]
struct Point {
    x: i16,
    y: i32,
}

impl CdrEncode for Point {
    fn encode(&self, enc: &mut Encoder) -> rustorb_cdr::Result<()> {
        self.x.encode(enc)?;
        self.y.encode(enc)
    }
}

impl CdrDecode for Point {
    fn decode(dec: &mut Decoder<'_>) -> rustorb_cdr::Result<Self> {
        Ok(Point {
            x: i16::decode(dec)?,
            y: i32::decode(dec)?,
        })
    }
}

impl CdrEncode for Sample {
    fn encode(&self, enc: &mut Encoder) -> rustorb_cdr::Result<()> {
        self.flag.encode(enc)?;
        self.value.encode(enc)?;
        self.name.encode(enc)?;
        self.points.encode(enc)
    }
}

impl CdrDecode for Sample {
    fn decode(dec: &mut Decoder<'_>) -> rustorb_cdr::Result<Self> {
        Ok(Sample {
            flag: bool::decode(dec)?,
            value: f64::decode(dec)?,
            name: String::decode(dec)?,
            points: Vec::decode(dec)?,
        })
    }
}

#[test]
fn nested_struct() {
    let sample = Sample {
        flag: true,
        value: 0.5,
        name: "pt".into(),
        points: vec![Point { x: 1, y: -1 }, Point { x: 2, y: 3 }],
    };
    let bytes = encode(&sample, ByteOrder::Big);
    assert_eq!(
        bytes,
        [
            1, 0, 0, 0, 0, 0, 0, 0, // flag + pad
            0x3F, 0xE0, 0, 0, 0, 0, 0, 0, // value
            0, 0, 0, 3, b'p', b't', 0, 0, // name + pad to 4
            0, 0, 0, 2, // points length
            0, 1, 0, 0, 0xFF, 0xFF, 0xFF, 0xFF, // Point { 1, -1 }
            0, 2, 0, 0, 0, 0, 0, 3, // Point { 2, 3 }
        ]
    );
    roundtrip(sample);
}

#[test]
fn encapsulation() {
    // The outer stream is misaligned on purpose; the encapsulation restarts
    // alignment at its byte order octet.
    let mut enc = Encoder::new(ByteOrder::Big, V12);
    enc.write_octet(0xEE);
    enc.write_encapsulation(|inner| {
        inner.write_u32(0x0001_0001);
        inner.write_u64(5);
        Ok(())
    })
    .unwrap();
    assert_eq!(
        enc.as_bytes(),
        [
            0xEE, 0, 0, 0, // octet + pad
            0, 0, 0, 16, // encapsulation length
            0, 0, 0, 0, // byte order + pad
            0, 1, 0, 1, // ulong
            0, 0, 0, 0, 0, 0, 0, 5, // ulonglong
        ]
    );

    let mut dec = Decoder::new(enc.as_bytes(), ByteOrder::Big, V12);
    dec.read_octet().unwrap();
    let mut inner = dec.read_encapsulation().unwrap();
    assert!(dec.is_empty());
    assert_eq!(inner.byte_order(), ByteOrder::Big);
    assert_eq!(inner.read_u32().unwrap(), 0x0001_0001);
    assert_eq!(inner.read_u64().unwrap(), 5);
    assert!(inner.is_empty());
}

#[test]
fn encapsulation_with_different_byte_order() {
    let mut inner = Encoder::encapsulation(ByteOrder::Little, V12);
    inner.write_u32(1);
    let mut enc = Encoder::new(ByteOrder::Big, V12);
    enc.write_octet_seq(inner.as_bytes()).unwrap();

    let mut dec = Decoder::new(enc.as_bytes(), ByteOrder::Big, V12);
    let mut inner = dec.read_encapsulation().unwrap();
    assert_eq!(inner.byte_order(), ByteOrder::Little);
    assert_eq!(inner.read_u32().unwrap(), 1);
}

#[test]
fn encapsulation_errors() {
    assert_eq!(
        Decoder::encapsulation(&[], V12).unwrap_err(),
        Error::EmptyEncapsulation
    );
    assert_eq!(
        Decoder::encapsulation(&[7], V12).unwrap_err(),
        Error::InvalidByteOrder(7)
    );
}

#[test]
fn byte_order_flags() {
    assert_eq!(ByteOrder::Big.flag(), 0);
    assert_eq!(ByteOrder::Little.flag(), 1);
    assert_eq!(ByteOrder::from_flag(1), Ok(ByteOrder::Little));
    assert!(ByteOrder::from_flag(2).is_err());
}

use rustorb_cdr::{ByteOrder, CdrDecode, CdrEncode, Octets};
use rustorb_giop::{
    AssembledMessage, CancelRequestHeader, CodeSetContext, CompletionStatus, Error,
    FragmentAssembler, Ior, LocateReplyHeader, LocateRequestHeader, LocateStatus, Message,
    MessageEncoder, MessageHeader, MsgType, ReplyHeader, ReplyStatus, RequestHeader,
    ServiceContext, SystemExceptionBody, TAG_INTERNET_IOP, TaggedProfile, TargetAddress, Version,
    code_set, decode_message, response_flags, service_id,
};

const VERSIONS: [Version; 3] = [Version::V1_0, Version::V1_1, Version::V1_2];
const ORDERS: [ByteOrder; 2] = [ByteOrder::Big, ByteOrder::Little];

/// GIOP 1.2 little-endian request for `add(7)` on object key `key`.
const REQUEST_1_2_LE: &[u8] = &[
    b'G', b'I', b'O', b'P', 1, 2, 1, 0, 40, 0, 0, 0, // header
    5, 0, 0, 0, // request_id
    3, 0, 0, 0, // response_flags + reserved
    0, 0, 0, 0, // KeyAddr + pad
    3, 0, 0, 0, b'k', b'e', b'y', 0, // object key + pad
    4, 0, 0, 0, b'a', b'd', b'd', 0, // operation
    0, 0, 0, 0, // service_context
    0, 0, 0, 0, // pad body to 8
    7, 0, 0, 0, // long 7
];

/// GIOP 1.0 big-endian request for `echo("hi")` on object key `ok`, with a
/// code set service context.
const REQUEST_1_0_BE: &[u8] = &[
    b'G', b'I', b'O', b'P', 1, 0, 0, 0, 0, 0, 0, 63, // header
    0, 0, 0, 1, // one service context
    0, 0, 0, 1, // CodeSets
    0, 0, 0, 12, 0, 0, 0, 0, 0, 1, 0, 1, 0, 1, 1, 9, // encapsulation
    0, 0, 0, 2, // request_id
    1, 0, 0, 0, // response_expected + pad
    0, 0, 0, 2, b'o', b'k', 0, 0, // object key + pad
    0, 0, 0, 5, b'e', b'c', b'h', b'o', 0, 0, 0, 0, // operation + pad
    0, 0, 0, 0, // requesting_principal
    0, 0, 0, 3, b'h', b'i', 0, // string "hi"
];

fn code_sets(order: ByteOrder) -> ServiceContext {
    CodeSetContext {
        char_data: code_set::ISO_8859_1,
        wchar_data: code_set::UTF_16,
    }
    .to_service_context(order)
}

#[test]
fn decode_giop_1_2_request() {
    let (header, message) = decode_message(REQUEST_1_2_LE).unwrap();
    assert_eq!(header.version, Version::V1_2);
    assert_eq!(header.byte_order, ByteOrder::Little);
    assert_eq!(header.msg_type, MsgType::Request);
    let Message::Request { header, body } = message else {
        panic!("not a request");
    };
    assert_eq!(header.request_id, 5);
    assert!(header.response_expected());
    assert_eq!(header.target, TargetAddress::Key(b"key".to_vec()));
    assert_eq!(header.operation, "add");
    assert!(header.service_context.is_empty());
    assert_eq!(body.offset(), 48);
    assert_eq!(body.decoder().read_i32().unwrap(), 7);
}

#[test]
fn encode_giop_1_2_request_matches_fixture() {
    let header = RequestHeader {
        request_id: 5,
        response_flags: response_flags::SYNC_WITH_TARGET,
        target: TargetAddress::Key(b"key".to_vec()),
        operation: "add".into(),
        service_context: vec![],
        requesting_principal: vec![],
    };
    let mut m = MessageEncoder::request(Version::V1_2, ByteOrder::Little, &header).unwrap();
    m.body().write_i32(7);
    assert_eq!(m.finish().unwrap(), REQUEST_1_2_LE);
}

#[test]
fn decode_giop_1_0_request() {
    let (header, message) = decode_message(REQUEST_1_0_BE).unwrap();
    assert_eq!(header.version, Version::V1_0);
    let Message::Request { header, body } = message else {
        panic!("not a request");
    };
    assert_eq!(header.request_id, 2);
    assert_eq!(header.response_flags, response_flags::SYNC_WITH_TARGET);
    assert_eq!(header.target.object_key(), Some(&b"ok"[..]));
    assert_eq!(header.operation, "echo");
    let cs = CodeSetContext::find(&header.service_context)
        .unwrap()
        .unwrap();
    assert_eq!(cs.char_data, code_set::ISO_8859_1);
    assert_eq!(cs.wchar_data, code_set::UTF_16);
    assert_eq!(body.decoder().read_string().unwrap(), "hi");
}

#[test]
fn encode_giop_1_0_request_matches_fixture() {
    let header = RequestHeader {
        request_id: 2,
        response_flags: response_flags::SYNC_WITH_TARGET,
        target: TargetAddress::Key(b"ok".to_vec()),
        operation: "echo".into(),
        service_context: vec![code_sets(ByteOrder::Big)],
        requesting_principal: vec![],
    };
    let mut m = MessageEncoder::request(Version::V1_0, ByteOrder::Big, &header).unwrap();
    m.body().write_string("hi").unwrap();
    assert_eq!(m.finish().unwrap(), REQUEST_1_0_BE);
}

fn sample_targets() -> Vec<TargetAddress> {
    let profile = TaggedProfile {
        tag: TAG_INTERNET_IOP,
        profile_data: vec![0, 1, 2, 0, 9],
    };
    vec![
        TargetAddress::Key(b"NameService".to_vec()),
        TargetAddress::Profile(profile.clone()),
        TargetAddress::Reference {
            selected_profile_index: 0,
            ior: Ior {
                type_id: "IDL:omg.org/CosNaming/NamingContextExt:1.0".into(),
                profiles: vec![profile],
            },
        },
    ]
}

#[test]
fn request_roundtrip_all_versions() {
    for version in VERSIONS {
        for order in ORDERS {
            for target in sample_targets() {
                for flags in [response_flags::NONE, response_flags::SYNC_WITH_TARGET] {
                    let header = RequestHeader {
                        request_id: 0xDEAD_BEEF,
                        response_flags: flags,
                        target: target.clone(),
                        operation: "resolve_str".into(),
                        service_context: vec![
                            code_sets(order),
                            ServiceContext {
                                context_id: 0x4F4D_0000,
                                context_data: vec![1, 2, 3],
                            },
                        ],
                        requesting_principal: vec![],
                    };
                    let result = MessageEncoder::request(version, order, &header);
                    if version < Version::V1_2 && target.object_key().is_none() {
                        assert_eq!(result.unwrap_err(), Error::TargetNotKey);
                        continue;
                    }
                    let mut m = result.unwrap();
                    m.body().write_u64(42);
                    let bytes = m.finish().unwrap();
                    let (h, message) = decode_message(&bytes).unwrap();
                    assert_eq!(h.version, version);
                    assert_eq!(h.byte_order, order);
                    assert_eq!(h.total_len(), bytes.len());
                    let Message::Request { header: got, body } = message else {
                        panic!("not a request");
                    };
                    assert_eq!(got, header, "{version} {order:?}");
                    assert_eq!(body.decoder().read_u64().unwrap(), 42);
                }
            }
        }
    }
}

#[test]
fn reply_roundtrip_all_versions() {
    for version in VERSIONS {
        for order in ORDERS {
            let header = ReplyHeader {
                request_id: 9,
                reply_status: ReplyStatus::SystemException,
                service_context: vec![code_sets(order)],
            };
            let exception = SystemExceptionBody {
                exception_id: "IDL:omg.org/CORBA/OBJECT_NOT_EXIST:1.0".into(),
                minor_code_value: 0x4F4D_0001,
                completion_status: CompletionStatus::No,
            };
            let mut m = MessageEncoder::reply(version, order, &header).unwrap();
            exception.encode(m.body()).unwrap();
            let bytes = m.finish().unwrap();
            let (_, message) = decode_message(&bytes).unwrap();
            let Message::Reply { header: got, body } = message else {
                panic!("not a reply");
            };
            assert_eq!(got, header);
            if version == Version::V1_2 {
                assert_eq!(body.offset() % 8, 0);
            }
            let mut dec = body.decoder();
            assert_eq!(SystemExceptionBody::decode(&mut dec).unwrap(), exception);
            assert!(dec.is_empty());
        }
    }
}

#[test]
fn giop_1_2_empty_body_has_no_padding() {
    // The reply header ends at offset 33, off an 8-byte boundary.
    let header = ReplyHeader {
        request_id: 1,
        reply_status: ReplyStatus::NoException,
        service_context: vec![ServiceContext {
            context_id: 1,
            context_data: vec![0],
        }],
    };
    let mut m = MessageEncoder::reply(Version::V1_2, ByteOrder::Big, &header).unwrap();
    let _ = m.body();
    let bytes = m.finish().unwrap();
    // 12 header + 4 + 4 + 4 + 4 + 4 + 1 = 33 bytes, no padding.
    assert_eq!(bytes.len(), 33);
    let (_, message) = decode_message(&bytes).unwrap();
    let Message::Reply { body, .. } = message else {
        panic!("not a reply");
    };
    assert!(body.is_empty());

    // Padding without a body is tolerated too.
    let mut padded = bytes.clone();
    padded.extend_from_slice(&[0; 7]);
    padded[11] = (padded.len() - 12) as u8;
    let (_, message) = decode_message(&padded).unwrap();
    let Message::Reply { body, .. } = message else {
        panic!("not a reply");
    };
    assert!(body.is_empty());
}

#[test]
fn location_forward_reply() {
    let ior = Ior {
        type_id: "IDL:Test:1.0".into(),
        profiles: vec![TaggedProfile {
            tag: TAG_INTERNET_IOP,
            profile_data: vec![1, 1, 2],
        }],
    };
    let header = ReplyHeader {
        request_id: 3,
        reply_status: ReplyStatus::LocationForward,
        service_context: vec![],
    };
    let mut m = MessageEncoder::reply(Version::V1_2, ByteOrder::Little, &header).unwrap();
    ior.encode(m.body()).unwrap();
    let (_, message) = decode_message(&m.finish().unwrap()).unwrap();
    let Message::Reply { body, .. } = message else {
        panic!("not a reply");
    };
    assert_eq!(Ior::decode(&mut body.decoder()).unwrap(), ior);
}

#[test]
fn locate_messages_roundtrip() {
    for version in VERSIONS {
        for order in ORDERS {
            let request = LocateRequestHeader {
                request_id: 77,
                target: TargetAddress::Key(b"obj".to_vec()),
            };
            let bytes = MessageEncoder::locate_request(version, order, &request).unwrap();
            assert_eq!(
                decode_message(&bytes).unwrap().1,
                Message::LocateRequest(request)
            );

            let reply = LocateReplyHeader {
                request_id: 77,
                locate_status: LocateStatus::ObjectHere,
            };
            let bytes = MessageEncoder::locate_reply(version, order, &reply)
                .unwrap()
                .finish()
                .unwrap();
            let Message::LocateReply { header, body } = decode_message(&bytes).unwrap().1 else {
                panic!("not a locate reply");
            };
            assert_eq!(header, reply);
            assert!(body.is_empty());
        }
    }

    for target in sample_targets() {
        let request = LocateRequestHeader {
            request_id: 1,
            target,
        };
        let bytes =
            MessageEncoder::locate_request(Version::V1_2, ByteOrder::Big, &request).unwrap();
        assert_eq!(
            decode_message(&bytes).unwrap().1,
            Message::LocateRequest(request)
        );
    }
}

#[test]
fn header_only_messages() {
    for version in VERSIONS {
        for order in ORDERS {
            let bytes = MessageEncoder::close_connection(version, order).unwrap();
            assert_eq!(bytes.len(), 12);
            assert_eq!(decode_message(&bytes).unwrap().1, Message::CloseConnection);

            let bytes = MessageEncoder::message_error(version, order).unwrap();
            assert_eq!(decode_message(&bytes).unwrap().1, Message::MessageError);

            let cancel = CancelRequestHeader { request_id: 4 };
            let bytes = MessageEncoder::cancel_request(version, order, &cancel).unwrap();
            assert_eq!(
                decode_message(&bytes).unwrap().1,
                Message::CancelRequest(cancel)
            );
        }
    }
    assert_eq!(
        MessageEncoder::close_connection(Version::V1_2, ByteOrder::Big).unwrap(),
        [b'G', b'I', b'O', b'P', 1, 2, 0, 5, 0, 0, 0, 0]
    );
}

#[test]
fn header_flags() {
    let mut header = MessageHeader {
        version: Version::V1_1,
        byte_order: ByteOrder::Little,
        more_fragments: true,
        msg_type: MsgType::Fragment,
        message_size: 0x0102,
    };
    let bytes = header.to_bytes();
    assert_eq!(bytes, [b'G', b'I', b'O', b'P', 1, 1, 3, 7, 2, 1, 0, 0]);
    assert_eq!(MessageHeader::parse(&bytes).unwrap(), header);

    header.version = Version::V1_0;
    header.more_fragments = false;
    header.msg_type = MsgType::Reply;
    let bytes = header.to_bytes();
    assert_eq!(bytes[6], 1);
    assert_eq!(MessageHeader::parse(&bytes).unwrap(), header);
}

#[test]
fn header_errors() {
    let good = MessageEncoder::close_connection(Version::V1_2, ByteOrder::Big).unwrap();

    assert!(matches!(
        MessageHeader::parse(&good[..11]),
        Err(Error::Truncated { .. })
    ));

    let mut bad = good.clone();
    bad[0] = b'X';
    assert!(matches!(
        MessageHeader::parse(&bad),
        Err(Error::BadMagic(_))
    ));

    let mut bad = good.clone();
    bad[5] = 3;
    assert_eq!(
        MessageHeader::parse(&bad),
        Err(Error::UnsupportedVersion(Version { major: 1, minor: 3 }))
    );

    let mut bad = good.clone();
    bad[7] = 8;
    assert_eq!(
        MessageHeader::parse(&bad),
        Err(Error::UnknownMessageType(8))
    );

    // Fragment does not exist in GIOP 1.0.
    let mut bad = good.clone();
    bad[5] = 0;
    bad[7] = 7;
    assert_eq!(
        MessageHeader::parse(&bad),
        Err(Error::UnknownMessageType(7))
    );

    let mut bad = good.clone();
    bad[5] = 0;
    bad[6] = 2;
    assert_eq!(MessageHeader::parse(&bad), Err(Error::InvalidFlags(2)));
}

#[test]
fn message_length_errors() {
    let mut longer = REQUEST_1_2_LE.to_vec();
    longer.push(0);
    assert_eq!(decode_message(&longer), Err(Error::TrailingBytes(1)));
    assert!(matches!(
        decode_message(&REQUEST_1_2_LE[..REQUEST_1_2_LE.len() - 1]),
        Err(Error::Truncated { .. })
    ));

    // A header that claims a short body cuts the request header off.
    let mut short = REQUEST_1_2_LE[..20].to_vec();
    short[8] = 8;
    assert!(matches!(decode_message(&short), Err(Error::Cdr(_))));
}

#[test]
fn invalid_statuses() {
    let header = ReplyHeader {
        request_id: 1,
        reply_status: ReplyStatus::NoException,
        service_context: vec![],
    };
    let mut bytes = MessageEncoder::reply(Version::V1_2, ByteOrder::Big, &header)
        .unwrap()
        .finish()
        .unwrap();
    bytes[19] = 9;
    assert_eq!(decode_message(&bytes), Err(Error::InvalidReplyStatus(9)));

    let mut bytes = REQUEST_1_2_LE.to_vec();
    bytes[20] = 5;
    assert_eq!(
        decode_message(&bytes),
        Err(Error::InvalidAddressingDisposition(5))
    );
}

/// Splits a whole GIOP 1.2 message into an initial message and fragments
/// with bodies of `chunk` bytes (a multiple of 8).
fn fragment_1_2(whole: &[u8], request_id: u32, chunk: usize) -> Vec<Vec<u8>> {
    let header = MessageHeader::parse(whole).unwrap();
    let body = &whole[12..];
    // The first message carries the 12-byte header plus body bytes up to a
    // multiple of 8.
    let first_len = chunk - 4;
    let mut out = Vec::new();
    let mut first = MessageHeader {
        more_fragments: true,
        message_size: first_len as u32,
        ..header
    }
    .to_bytes()
    .to_vec();
    first.extend_from_slice(&body[..first_len]);
    out.push(first);
    let rest: Vec<&[u8]> = body[first_len..].chunks(chunk).collect();
    for (i, data) in rest.iter().enumerate() {
        let more = i + 1 < rest.len();
        let mut m =
            MessageEncoder::fragment(Version::V1_2, header.byte_order, request_id, more).unwrap();
        m.body().write_octets(data);
        out.push(m.finish().unwrap());
    }
    out
}

#[test]
fn reassemble_giop_1_2_fragments() {
    let header = RequestHeader {
        request_id: 11,
        response_flags: response_flags::SYNC_WITH_TARGET,
        target: TargetAddress::Key(b"key".to_vec()),
        operation: "send".into(),
        service_context: vec![],
        requesting_principal: vec![],
    };
    let payload: Vec<u32> = (0..50).collect();
    let mut m = MessageEncoder::request(Version::V1_2, ByteOrder::Little, &header).unwrap();
    payload.encode(m.body()).unwrap();
    let whole = m.finish().unwrap();

    let fragments = fragment_1_2(&whole, 11, 32);
    assert!(fragments.len() > 3);

    let mut asm = FragmentAssembler::default();
    // Interleave an unrelated unfragmented message.
    let close = MessageEncoder::close_connection(Version::V1_2, ByteOrder::Little).unwrap();
    let (last, init) = fragments.split_last().unwrap();
    for (i, f) in init.iter().enumerate() {
        assert_eq!(asm.push(f).unwrap(), None, "fragment {i}");
        if i == 1 {
            let got = asm.push(&close).unwrap().unwrap();
            assert_eq!(got.as_bytes(), &close[..]);
            assert!(!got.was_fragmented());
        }
    }
    assert!(asm.has_pending());
    let joined = asm.push(last).unwrap().unwrap();
    assert!(!asm.has_pending());
    assert_eq!(joined.as_bytes(), whole);

    let Message::Request { header: got, body } = joined.decode().unwrap().1 else {
        panic!("not a request");
    };
    assert_eq!(got, header);
    assert_eq!(Vec::<u32>::decode(&mut body.decoder()).unwrap(), payload);
}

/// Whether `inner` lies within the memory `outer` spans.
fn within(inner: &[u8], outer: &std::ops::Range<*const u8>) -> bool {
    let i = inner.as_ptr_range();
    outer.start <= i.start && i.end <= outer.end
}

#[test]
fn owned_messages_are_decoded_without_copying() {
    let header = RequestHeader {
        request_id: 12,
        response_flags: response_flags::SYNC_WITH_TARGET,
        target: TargetAddress::Key(b"key".to_vec()),
        operation: "send".into(),
        service_context: vec![],
        requesting_principal: vec![],
    };
    let payload = vec![9u8; 4096];
    let mut m = MessageEncoder::request(Version::V1_2, ByteOrder::Little, &header).unwrap();
    m.body().write_octet_seq(&payload).unwrap();
    let whole = m.finish().unwrap();

    // Unfragmented: the body is the buffer that was pushed.
    let mut asm = FragmentAssembler::default();
    let buf = whole.clone();
    let range = buf.as_ptr_range();
    let joined = asm.push_owned(buf).unwrap().unwrap();
    assert_eq!(joined.as_bytes().as_ptr_range(), range);
    let (_, message) = joined.into_decoded().unwrap();
    let Message::Request { header: got, body } = message else {
        panic!("not a request");
    };
    assert_eq!(got, header);
    assert_eq!(body.decoder().read_octet_seq().unwrap(), &payload[..]);
    assert!(within(body.as_bytes(), &range));
    // A clone shares the bytes too, and compares by content.
    let copy = body.clone();
    assert_eq!(copy.as_bytes().as_ptr(), body.as_bytes().as_ptr());
    assert_eq!(copy, body);

    // Fragmented: joined into the first fragment's buffer, decoded the same
    // as through `push` and `decode`.
    let fragments = fragment_1_2(&whole, 12, 512);
    assert!(fragments.len() > 3);
    let mut joined = None;
    for f in fragments.clone() {
        joined = asm.push_owned(f).unwrap();
    }
    let joined = joined.unwrap();
    assert_eq!(joined.as_bytes(), whole);
    let borrowed = joined.decode().unwrap();
    assert_eq!(joined.into_decoded().unwrap(), borrowed);
}

/// Large octet sequences are kept apart when written (a vectored write
/// sends them from where they are) and shared with the message buffer when
/// read; the bytes on the wire are those of a plain write.
#[test]
fn shared_octet_sequences_are_not_copied() {
    let header = RequestHeader {
        request_id: 5,
        response_flags: response_flags::SYNC_WITH_TARGET,
        target: TargetAddress::Key(b"key".to_vec()),
        operation: "send".into(),
        service_context: vec![],
        requesting_principal: vec![],
    };
    let big = Octets::from(vec![7u8; 100_000]);
    let small = Octets::from(vec![3u8; 10]);
    let write = |shared: bool| {
        let mut m = MessageEncoder::request(Version::V1_2, ByteOrder::Big, &header).unwrap();
        let body = m.body();
        body.write_octet(1);
        if shared {
            body.write_octet_seq_shared(&big).unwrap();
            body.write_octet_seq_shared(&small).unwrap();
        } else {
            body.write_octet_seq(&big).unwrap();
            body.write_octet_seq(&small).unwrap();
        }
        // Aligned after the shared octets as after copied ones.
        body.write_f64(1.5);
        m
    };
    let plain = write(false).finish().unwrap();
    let out = write(true).finish_shared().unwrap();
    assert_eq!(out.len(), plain.len());
    let pieces: Vec<&[u8]> = out.pieces().collect();
    assert_eq!(pieces.len(), 3);
    assert_eq!(pieces[1].as_ptr(), big.as_ptr());
    assert_eq!(pieces.concat(), plain);
    // Slices for a write that already sent part of the message.
    for from in [0, 1, 50, pieces[0].len(), pieces[0].len() + 7, plain.len() - 3] {
        let rest: Vec<u8> = out
            .slices_from(from, usize::MAX)
            .iter()
            .flat_map(|s| s.iter().copied())
            .collect();
        assert_eq!(rest, plain[from..]);
    }
    assert_eq!(out.slices_from(0, 2).len(), 2);
    assert_eq!(write(true).finish().unwrap(), plain);
    assert_eq!(out.clone().into_vec(), plain);

    // Read back: the large one points into the message, the short one is
    // copied.
    let buf = plain.clone();
    let range = buf.as_ptr_range();
    let joined = FragmentAssembler::default().push_owned(buf).unwrap().unwrap();
    let (_, Message::Request { body, .. }) = joined.into_decoded().unwrap() else {
        panic!("not a request");
    };
    let mut dec = body.decoder();
    assert_eq!(dec.read_octet().unwrap(), 1);
    let got = dec.read_octet_seq_shared().unwrap();
    assert_eq!(got, big);
    assert!(range.contains(&got.as_ptr()));
    let got_small = dec.read_octet_seq_shared().unwrap();
    assert_eq!(got_small, small);
    assert!(!range.contains(&got_small.as_ptr()));
    assert_eq!(dec.read_f64().unwrap(), 1.5);
}

#[test]
fn interleaved_giop_1_2_fragments() {
    let mut wholes = Vec::new();
    let mut streams = Vec::new();
    for id in [1u32, 2] {
        let header = ReplyHeader {
            request_id: id,
            reply_status: ReplyStatus::NoException,
            service_context: vec![],
        };
        let mut m = MessageEncoder::reply(Version::V1_2, ByteOrder::Big, &header).unwrap();
        vec![id; 30].encode(m.body()).unwrap();
        let whole = m.finish().unwrap();
        streams.push(fragment_1_2(&whole, id, 40));
        wholes.push(whole);
    }
    let mut asm = FragmentAssembler::default();
    let mut done = Vec::new();
    for i in 0..streams[0].len().max(streams[1].len()) {
        for s in &streams {
            if let Some(f) = s.get(i) {
                done.extend(asm.push(f).unwrap().map(AssembledMessage::into_bytes));
            }
        }
    }
    assert_eq!(done.len(), 2);
    assert!(done.contains(&wholes[0]) && done.contains(&wholes[1]));
}

#[test]
fn reassemble_giop_1_1_fragments() {
    let header = ReplyHeader {
        request_id: 1,
        reply_status: ReplyStatus::NoException,
        service_context: vec![],
    };
    let mut m = MessageEncoder::reply(Version::V1_1, ByteOrder::Big, &header).unwrap();
    m.body().write_octets(&[0xAB; 40]);
    let whole = m.finish().unwrap();

    let parsed = MessageHeader::parse(&whole).unwrap();
    let body = &whole[12..];
    let mut first = MessageHeader {
        more_fragments: true,
        message_size: 20,
        ..parsed
    }
    .to_bytes()
    .to_vec();
    first.extend_from_slice(&body[..20]);
    let mut frag = MessageEncoder::fragment(Version::V1_1, ByteOrder::Big, 0, false).unwrap();
    frag.body().write_octets(&body[20..]);
    let frag = frag.finish().unwrap();

    let mut asm = FragmentAssembler::default();
    assert_eq!(asm.push(&first).unwrap(), None);
    // Only one fragmented message at a time in GIOP 1.1.
    assert_eq!(asm.push(&first), Err(Error::FragmentInProgress));
    assert_eq!(asm.push(&frag).unwrap().unwrap().as_bytes(), whole);
}

#[test]
fn giop_1_1_fragments_align_per_fragment() {
    // As omniORB writes them: the first message ends on an 8-byte
    // boundary and the data of each Fragment is aligned relative to that
    // Fragment, starting at offset 12.
    let header = ReplyHeader {
        request_id: 5,
        reply_status: ReplyStatus::NoException,
        service_context: vec![],
    };
    for order in ORDERS {
        let mut m = MessageEncoder::reply(Version::V1_1, order, &header).unwrap();
        m.body().write_u32(7);
        m.body().write_u32(8);
        let mut first = m.finish().unwrap();
        first[6] |= 0x02; // more fragments
        assert_eq!(first.len() % 8, 0);

        let mut frag = MessageEncoder::fragment(Version::V1_1, order, 0, false).unwrap();
        frag.body().write_f64(1.5); // padded to offset 16 of the fragment
        frag.body().write_u32(9);
        let frag = frag.finish().unwrap();
        assert_eq!(frag.len(), 16 + 8 + 4);

        let mut asm = FragmentAssembler::default();
        assert_eq!(asm.push(&first).unwrap(), None);
        let joined = asm.push(&frag).unwrap().unwrap();
        assert!(joined.was_fragmented());
        assert_eq!(joined.as_bytes().len(), first.len() + frag.len() - 12);
        let Message::Reply { body, .. } = joined.decode().unwrap().1 else {
            panic!("not a reply");
        };
        let mut dec = body.decoder();
        assert_eq!(dec.read_u32().unwrap(), 7);
        assert_eq!(dec.read_u32().unwrap(), 8);
        assert_eq!(dec.read_f64().unwrap(), 1.5);
        assert_eq!(dec.read_u32().unwrap(), 9);
        assert!(dec.is_empty());

        // Read as one stream, the double would be taken 4 bytes too early.
        let (_, Message::Reply { body, .. }) = decode_message(joined.as_bytes()).unwrap() else {
            panic!("not a reply");
        };
        let mut dec = body.decoder();
        dec.read_u32().unwrap();
        dec.read_u32().unwrap();
        assert_ne!(dec.read_f64().unwrap(), 1.5);
    }
}

#[test]
fn fragment_errors() {
    let mut asm = FragmentAssembler::default();
    let frag = MessageEncoder::fragment(Version::V1_2, ByteOrder::Big, 5, false)
        .unwrap()
        .finish()
        .unwrap();
    assert_eq!(asm.push(&frag), Err(Error::UnexpectedFragment));

    assert_eq!(
        MessageEncoder::fragment(Version::V1_0, ByteOrder::Big, 0, false).unwrap_err(),
        Error::FragmentNotSupported
    );

    // Byte order must not change mid-message.
    let start = {
        let mut m =
            MessageEncoder::new(Version::V1_2, ByteOrder::Big, MsgType::Reply, true).unwrap();
        m.body().write_u32(5);
        m.finish().unwrap()
    };
    let other_order = MessageEncoder::fragment(Version::V1_2, ByteOrder::Little, 5, false)
        .unwrap()
        .finish()
        .unwrap();
    assert_eq!(asm.push(&start).unwrap(), None);
    assert_eq!(asm.push(&start), Err(Error::FragmentInProgress));
    assert_eq!(asm.push(&other_order), Err(Error::FragmentMismatch));

    // The size limit covers the reassembled message.
    let mut small = FragmentAssembler::new(20);
    assert_eq!(small.push(&start).unwrap(), None);
    let mut big = MessageEncoder::fragment(Version::V1_2, ByteOrder::Big, 5, false).unwrap();
    big.body().write_octets(&[0; 16]);
    assert!(matches!(
        small.push(&big.finish().unwrap()),
        Err(Error::FragmentTooLarge(_))
    ));

    // CancelRequest drops an incomplete message.
    let mut asm = FragmentAssembler::default();
    asm.push(&start).unwrap();
    assert!(asm.cancel(5));
    assert!(!asm.has_pending());
}

#[test]
fn code_set_context_roundtrip() {
    for order in ORDERS {
        let ctx = CodeSetContext {
            char_data: code_set::UTF_8,
            wchar_data: code_set::UTF_16,
        };
        let sc = ctx.to_service_context(order);
        assert_eq!(sc.context_id, service_id::CODE_SETS);
        assert_eq!(sc.context_data[0], order.flag());
        assert_eq!(CodeSetContext::from_service_context(&sc).unwrap(), ctx);
    }
    assert!(CodeSetContext::find(&[]).is_none());
}

//! Splits the input into GIOP messages by their header sizes and feeds
//! them to a fragment assembler, decoding whatever comes out whole.
#![no_main]

use libfuzzer_sys::fuzz_target;
use rustorb_giop::{FragmentAssembler, MessageHeader, decode_message};

fuzz_target!(|data: &[u8]| {
    let mut asm = FragmentAssembler::new(1 << 16);
    let mut rest = data;
    while let Ok(header) = MessageHeader::parse(rest) {
        let len = header.total_len().min(rest.len());
        let (msg, tail) = rest.split_at(len);
        rest = tail;
        if let Ok(Some(whole)) = asm.push(msg) {
            let _ = decode_message(&whole);
        }
    }
});

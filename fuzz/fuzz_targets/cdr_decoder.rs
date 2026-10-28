//! Drives the CDR decoder with a sequence of reads chosen by the input.
//!
//! Input layout: one config octet (byte order, GIOP version, stream
//! offset), one octet giving the number of operations, the operation
//! octets, then the CDR payload.
#![no_main]

use libfuzzer_sys::fuzz_target;
use rustorb_cdr::{ByteOrder, CdrDecode, CdrVersion, Decoder};

fuzz_target!(|data: &[u8]| {
    let [config, count, rest @ ..] = data else {
        return;
    };
    let count = (*count as usize).min(rest.len());
    let (ops, payload) = rest.split_at(count);

    let order = if config & 1 == 0 {
        ByteOrder::Big
    } else {
        ByteOrder::Little
    };
    let version = match (config >> 1) & 3 {
        0 => CdrVersion::Giop1_0,
        1 => CdrVersion::Giop1_1,
        _ => CdrVersion::Giop1_2,
    };
    let offset = (config >> 3) as usize;
    let mut dec = Decoder::with_offset(payload, order, version, offset);
    if config & 0x80 != 0 {
        dec.set_wstring_default_order(ByteOrder::Little);
    }
    for op in ops {
        if read(&mut dec, *op).is_err() {
            break;
        }
    }
});

fn read(dec: &mut Decoder<'_>, op: u8) -> rustorb_cdr::Result<()> {
    match op % 20 {
        0 => drop(dec.read_octet()?),
        1 => drop(dec.read_bool()?),
        2 => drop(dec.read_i16()?),
        3 => drop(dec.read_u32()?),
        4 => drop(dec.read_u64()?),
        5 => drop(dec.read_f32()?),
        6 => drop(dec.read_f64()?),
        7 => drop(dec.read_string()?),
        8 => drop(dec.read_string_bytes()?),
        9 => drop(dec.read_wchar()?),
        10 => drop(dec.read_wstring()?),
        11 => drop(dec.read_octet_seq()?),
        12 => drop(Vec::<u16>::decode(dec)?),
        13 => drop(Vec::<String>::decode(dec)?),
        14 => drop(Vec::<Vec<u64>>::decode(dec)?),
        15 => drop(<[i32; 3]>::decode(dec)?),
        16 => {
            let mut inner = dec.read_encapsulation()?;
            while inner.read_u32().is_ok() {}
        }
        17 => dec.align(8)?,
        18 => dec.skip((op / 20) as usize)?,
        _ => drop(dec.read_len(4)?),
    }
    Ok(())
}

//! Drives the XCDR headers of the CDR decoder with a sequence of reads
//! chosen by the input.
//!
//! Input layout: one octet giving the number of operations, the operation
//! octets, then an RTPS SerializedPayload (encapsulation header and XCDR
//! data).
#![no_main]

use libfuzzer_sys::fuzz_target;
use rustorb_cdr::xcdr::{Delimited, ListEnd, MemberHeader, Parameter};
use rustorb_cdr::Decoder;

/// The headers opened and not yet ended, innermost last.
enum Open {
    Delimited(Delimited),
    Member(MemberHeader),
    Parameter(Parameter),
}

fuzz_target!(|data: &[u8]| {
    let [count, rest @ ..] = data else {
        return;
    };
    let count = (*count as usize).min(rest.len());
    let (ops, payload) = rest.split_at(count);
    let Ok((mut dec, _)) = Decoder::xcdr_payload(payload) else {
        return;
    };
    let mut open = Vec::new();
    for op in ops {
        if read(&mut dec, &mut open, *op).is_err() {
            break;
        }
    }
});

fn read(dec: &mut Decoder<'_>, open: &mut Vec<Open>, op: u8) -> rustorb_cdr::Result<()> {
    match op % 16 {
        0 => drop(dec.read_octet()?),
        1 => drop(dec.read_u16()?),
        2 => drop(dec.read_u32()?),
        3 => drop(dec.read_u64()?),
        4 => drop(dec.read_long_double()?),
        5 => drop(dec.read_string()?),
        6 => drop(dec.read_wchar()?),
        7 => drop(dec.read_wstring()?),
        8 => open.push(Open::Delimited(dec.read_dheader()?)),
        9 => open.push(Open::Member(dec.read_member_header()?)),
        10 => open.push(Open::Parameter(dec.read_parameter_header()?)),
        11 | 12 => {
            let end = if op % 16 == 11 { ListEnd::Xtypes } else { ListEnd::Rtps };
            if let Some(param) = dec.read_parameter(end)? {
                open.push(Open::Parameter(param));
            }
        }
        13 => {
            if let Some(Open::Delimited(d)) = open.last() {
                let _ = (d.has_more(dec), d.remaining(dec));
            }
        }
        14 => dec.skip((op / 16) as usize)?,
        _ => match open.pop() {
            Some(Open::Delimited(d)) => dec.end_dheader(d)?,
            Some(Open::Member(m)) => dec.end_member(m)?,
            Some(Open::Parameter(p)) => dec.end_parameter(p)?,
            None => {}
        },
    }
    Ok(())
}

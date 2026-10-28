//! Parses the input as a stringified IOR, corbaloc or corbaname URL, and
//! checks that writing a parsed value and parsing it again gives the same
//! value.
#![no_main]

use libfuzzer_sys::fuzz_target;
use rustorb_cdr::ByteOrder;
use rustorb_giop::ObjectUrl;

fuzz_target!(|data: &[u8]| {
    let Ok(s) = std::str::from_utf8(data) else {
        return;
    };
    let Ok(url) = ObjectUrl::parse(s) else {
        return;
    };
    let written = url.to_string();
    let again = ObjectUrl::parse(&written)
        .unwrap_or_else(|e| panic!("{written:?} does not parse again: {e}"));
    assert_eq!(again, url, "{written:?}");
    match &url {
        ObjectUrl::Ior(ior) => ior.iiop_profiles().for_each(drop),
        ObjectUrl::Corbaloc(loc) => drop(loc.to_ior(ByteOrder::Big)),
        ObjectUrl::CorbaName(name) => drop(name.context.to_ior(ByteOrder::Little)),
    }
});

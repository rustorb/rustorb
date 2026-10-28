//! Decodes the input as an IOR encapsulation, reads every profile and
//! component, and checks that IIOP profiles and the IOR encode back to
//! values that decode the same.
#![no_main]

use libfuzzer_sys::fuzz_target;
use rustorb_cdr::ByteOrder;
use rustorb_giop::{
    AlternateIiopAddress, CodeSetComponentInfo, Ior, TaggedComponent, component_tag,
};

fn read_components(components: &[TaggedComponent]) {
    for c in components {
        match c.tag {
            component_tag::CODE_SETS => drop(CodeSetComponentInfo::from_component(c)),
            component_tag::ALTERNATE_IIOP_ADDRESS => {
                drop(AlternateIiopAddress::from_component(c))
            }
            _ => {}
        }
    }
}

fuzz_target!(|data: &[u8]| {
    let Ok(ior) = Ior::from_encapsulation(data) else {
        return;
    };
    let order = if data[0] == 0 {
        ByteOrder::Big
    } else {
        ByteOrder::Little
    };
    for profile in &ior.profiles {
        if let Ok(iiop) = profile.iiop() {
            drop(iiop.orb_type());
            drop(iiop.code_sets());
            drop(iiop.alternate_addresses());
            read_components(&iiop.components);
            let encoded = iiop.to_tagged_profile(order).expect("decoded profile encodes");
            assert_eq!(encoded.iiop().as_ref(), Ok(&iiop));
        }
        if let Ok(components) = profile.multiple_components() {
            read_components(&components);
        }
    }
    let encoded = ior.to_encapsulation(order).expect("decoded IOR encodes");
    assert_eq!(Ior::from_encapsulation(&encoded).as_ref(), Ok(&ior));
    let s = ior.to_ior_string(order).expect("decoded IOR encodes");
    assert_eq!(s.parse::<Ior>().as_ref(), Ok(&ior));
});

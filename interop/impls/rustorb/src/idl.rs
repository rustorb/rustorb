//! The code `rustorb-idl -brust` generates from the test IDLs
//! (`crates/rustorb-codegen/generate-rust.sh` writes it to `idl/`), shared
//! by the client and the server.

#![allow(dead_code)]

include!("idl/interop_test.rs");
include!("idl/any_test.rs");
include!("idl/any_test_hidden.rs");
include!("idl/value_test.rs");
include!("idl/poa_probe.rs");

/// CustomTest.idl, in a module of its own for its `register_value_types`,
/// with the marshaling of its custom valuetype.
pub mod custom {
    include!("idl/custom_test.rs");

    use rustorb_core::idl::{CustomMarshal, DataInputStream, DataOutputStream, Long, Sys, ULong};

    use crate::idl::value_test::Point;
    use custom_test::BlobState;

/// As CustomTest.idl says: n, label, pt, the length of values and values.
impl CustomMarshal for BlobState {
    fn marshal(&self, os: &mut DataOutputStream<'_, '_>) -> Sys<()> {
        os.write::<Long>(&self.n)?;
        os.write::<rustorb_core::idl::String>(&self.label)?;
        os.write::<Point>(&self.pt)?;
        os.write::<ULong>(&(self.values.len() as u32))?;
        os.write_array::<Long>(&self.values)
    }

    fn unmarshal(&mut self, is: &mut DataInputStream<'_, '_>) -> Sys<()> {
        self.n = is.read::<Long>()?;
        self.label = is.read::<rustorb_core::idl::String>()?;
        self.pt = is.read::<Point>()?;
        let len = is.read::<ULong>()? as usize;
        self.values = is.read_array::<Long>(len)?;
        Ok(())
    }
}
}

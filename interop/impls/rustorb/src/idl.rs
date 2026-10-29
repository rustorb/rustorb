//! The code `rustorb-idl -brust` generates from the test IDLs
//! (`crates/rustorb-codegen/generate-rust.sh` writes it to `idl/`), shared
//! by the client and the server.

#![allow(dead_code)]

include!("idl/interop_test.rs");
include!("idl/any_test.rs");
include!("idl/any_test_hidden.rs");
include!("idl/value_test.rs");
include!("idl/poa_probe.rs");

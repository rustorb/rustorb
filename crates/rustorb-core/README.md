# rustorb-core

The ORB core of rustorb: IIOP (and SSLIOP over rustls) connections, request
dispatch, the POA with servant managers and the thread policies, Any and
TypeCode, DynAny, valuetypes, DII / DSI, policies and call timeouts.

Network I/O runs on an internal tokio runtime and servants on a separate
pool of ORB threads; the public API is synchronous, with an optional async
API. Stubs and skeletons for Rust are generated from IDL by
[rustorb-codegen](https://crates.io/crates/rustorb-codegen).

rustorb is a CORBA ORB written in Rust, with Python, C++ and Java bindings.
See the [repository](https://github.com/rustorb/rustorb) for the other crates,
the bindings and the documentation.

## License

Licensed under either of [Apache License, Version 2.0](https://github.com/rustorb/rustorb/blob/main/LICENSE-APACHE)
or [MIT license](https://github.com/rustorb/rustorb/blob/main/LICENSE-MIT) at your option.

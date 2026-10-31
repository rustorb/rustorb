# rustorb

A CORBA ORB with its core written in Rust, and bindings for Rust, Python,
C++ and Java. ([日本語](README.ja.md))

rustorb speaks GIOP 1.0 to 1.2 over IIOP and SSLIOP, and is tested in CI
against omniORB, omniORBpy and JacORB in every combination of client, server
and naming service. It aims to replace omniORB / omniORBpy in existing
systems: the Python binding generates the same API as omniidl -bpython, and
a compatibility package runs omniORBpy applications such as
OpenRTM-aist-Python unchanged.

- **Protocol**: CDR, GIOP 1.0–1.2 with fragments, IORs, `corbaloc:` and
  `corbaname:`, code set negotiation, SSLIOP (TLS with rustls)
- **Server side**: the POA with the standard policies, servant managers,
  default servants, adapter activators and the three thread models
- **Types**: Any, TypeCode, DynAny, `fixed`, `long double`, valuetypes
  (custom, truncatable, boxed, `supports`), abstract interfaces
- **Naming Service**: CosNaming client and server, and the `rustorb-names`
  daemon (a replacement for omniNames, with persistence)
- **More**: DII / DSI, policies and call timeouts, an optional async API

## Install

| Language | Package | IDL compiler |
| --- | --- | --- |
| Rust | `rustorb-core`, `rustorb-naming` (crates.io) | `rustorb-codegen` in `build.rs` |
| Python | `pip install rustorb` (abi3 wheels for CPython 3.9+) | `rustorb-idl -bpython` |
| C++ (C++17) | `rustorb-native` archive of the GitHub release, or `cmake --install` of `cpp/`; then `find_package(rustorb)` | `rustorb-idl -bc++11` |
| Java (22+) | `rustorb-<version>.jar` and the shared library of `rustorb-native` from the GitHub release | `rustorb-idl -bjava` |

The IDL compiler is `pip install rustorb-idl` for every language. The naming
service is `cargo install rustorb-naming` (the `rustorb-names` binary).

## Quick start

```idl
// Echo.idl
module Example {
  interface Echo {
    string echo(in string message);
  };
};
```

A Python server and client:

```sh
pip install rustorb rustorb-idl
rustorb-idl -bpython Echo.idl          # Example/, Example__POA/
```

```python
import sys
from rustorb import CORBA, PortableServer
import Example, Example__POA

class EchoImpl(Example__POA.Echo):
    def echo(self, message):
        return message

orb = CORBA.ORB_init(sys.argv)
poa = orb.resolve_initial_references("RootPOA")
poa._get_the_POAManager().activate()
ior = orb.object_to_string(EchoImpl()._this())

echo = orb.string_to_object(ior)._narrow(Example.Echo)
print(echo.echo("hello"))
orb.destroy()
```

The [getting started guide](docs/getting-started.md) goes through the same
server and client in Rust, Python, C++ and Java, registered in the naming
service; the code is in [examples/](examples/), and
`examples/run-all.sh` runs every server with every client.

## Documentation

- [Getting started](docs/getting-started.md) and [examples](examples/)
- [Changelog](CHANGELOG.md) and [how releases are made](docs/release.md)
- Design documents (in Japanese): [development plan](docs/development-plan.md),
  [Rust mapping](docs/rust-mapping.md), [Python binding](docs/python-binding.md),
  [Python generator](docs/python-codegen.md), [omniORBpy compatibility](docs/omniorb-compat.md),
  [C++ binding](docs/cpp-binding.md), [Java binding](docs/java-binding.md),
  [POA](docs/poa-design.md), [Any and TypeCode](docs/any-typecode.md),
  [valuetypes](docs/valuetype.md), [SSLIOP](docs/ssliop.md),
  [timeouts](docs/timeouts.md), [async API](docs/async-api.md)
- Interoperability tests: [interop/](interop/README.md)

## Building from source

```sh
cargo build --workspace
cargo test --workspace
```

The IDL compiler, the Python, C++ and Java tests need
[idl_parser](https://github.com/sugarsweetrobotics/idl_parser) importable
(`pip install idl_parser`, or a checkout on `PYTHONPATH`):

```sh
python/build-native.sh                    # the native module, into python/rustorb
python3 -m unittest discover -s python/tests
python3 -m unittest discover -s crates/rustorb-codegen/python/tests
cmake -S cpp -B build/cpp && cmake --build build/cpp && ctest --test-dir build/cpp
mvn -f java/pom.xml verify                # Java 22 or later
```

The Rust code generated from IDL that is checked in (`rustorb-naming`,
`interop/impls/rustorb`, the generator tests) is regenerated with
`crates/rustorb-codegen/generate-rust.sh` when the IDL or the generator
changes; CI checks it. Fuzzing needs nightly and cargo-fuzz:
`cargo +nightly fuzz run giop_message`.

## License

rustorb is licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

The IDL compiler (`rustorb-idl` and the `rustorb-codegen` crate) is
GPL-3.0-or-later, because it builds on idl_parser. Code it generates is not
covered by that license: the owner of the input IDL chooses its license, and
the boilerplate rustorb puts into it is MIT OR Apache-2.0.

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in rustorb by you, as defined in the Apache-2.0
license, shall be dual licensed as above (GPL-3.0-or-later for the IDL
compiler), without any additional terms or conditions.

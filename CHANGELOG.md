# Changelog

All notable changes to rustorb are written here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/) (before 1.0, a minor version may
break the API).

## [Unreleased]

## [0.1.1] - 2026-11-02

### Added

- `OrbOptions::follow_linger` and `follow_limit` (`-ORBfollowLinger US`,
  `-ORBfollowLimit N`): how long and how many times a server thread goes on
  reading the next request on its connection. The defaults (1 ms, 64) are the
  old fixed values (#86)
- A comparison of the IDL to Rust mapping with other implementations
  (`docs/rust-mapping-comparison.md`)

### Fixed

- `rustorb-idl -C DIR` creates `DIR` when it does not exist, as `omniidl -C`
  does, instead of stopping with `FileNotFoundError` (#95)

## [0.1.0] - 2026-10-31

The first release: a CORBA ORB with its core in Rust, and bindings for Rust,
Python, C++ and Java.

### Protocol and ORB core (`rustorb-cdr`, `rustorb-giop`, `rustorb-core`)

- CDR with both byte orders, GIOP 1.0 to 1.2 (fragments, `LocateRequest`,
  `LOCATION_FORWARD`, `NEEDS_ADDRESSING_MODE`, `CloseConnection`), IIOP 1.0
  to 1.2 profiles and tagged components, code set negotiation (UTF-8 and
  UTF-16 natively)
- Stringified IORs, `corbaloc:` and `corbaname:` URLs, `-ORBInitRef` and
  `-ORBDefaultInitRef`
- SSLIOP: IIOP over TLS with rustls, with client and server certificates
- The POA: the standard policies, servant activators and locators, default
  servants, adapter activators, ORB_CTRL_MODEL / SINGLE_THREAD_MODEL /
  MAIN_THREAD_MODEL, colocated calls, `PortableServer::Current`
- Any and TypeCode, DynAny, `fixed`, `long double` (IEEE 754 binary128 on the
  wire), valuetypes (sharing, cycles, truncatable, value boxes, custom
  marshaling, `supports`, factories and initializers), abstract interfaces
- DII and DSI (core and Python)
- `Value::release`, which frees valuetypes in cycles that nothing else refers
  to; the core uses it for the values read by a stream, in an Any of the C
  ABI (C++ and Java), in the copies `DynAny::equal` makes, and those the
  Python binding converted (#85)
- `create_policy`, `register_initial_reference`, policy overrides, and call
  timeouts (`RelativeRoundtripTimeoutPolicy` and ORB options)
- A synchronous API on an internal tokio runtime, with servants on a
  separate pool of ORB threads, and an optional async API
  (`Orb::with_runtime`)

### Naming Service (`rustorb-naming`)

- CosNaming client stubs (`NamingContext`, `NamingContextExt`,
  `BindingIterator`) and server
- The `rustorb-names` daemon, a replacement for omniNames, with an
  append-only log and checkpoints

### IDL compiler (`rustorb-idl`, `rustorb-codegen`)

- Backends: `-brust`, `-bpython` (the layout of omniidl -bpython),
  `-bc++11` (IDL to C++11) and `-bjava` (IDL to Java, with portable stubs
  and streams); IDL 4 int8 / uint8, bitmask, bitset and some annotations in
  C++
- `rustorb_codegen::Config` runs `-brust` from a build script
- Custom valuetypes of the Rust mapping go in an Any:
  `register_value_types()` registers their readers with the core (#85)
- Built on idl_parser 0.6.2 or later
- Ships `CosNaming.idl` (`rustorb-idl --idl-dir`, also searched for `#include`)

### Bindings

- Python (`rustorb`): the standard IDL to Python mapping with PyO3, one abi3
  wheel for CPython 3.9 or later. `rustorb-omniorb-compat` provides
  `from omniORB import CORBA` for omniORBpy applications; OpenRTM-aist-Python
  and rtshell run on it
- C++ (`rustorb-ffi` and `cpp/include/rustorb`): the IDL to C++11 mapping
  (C++17) on a stable C ABI; installs a CMake package
  (`find_package(rustorb)`, `rustorb::rustorb`, `rustorb_idl_generate()`)
- Java (`java/`): the IDL to Java mapping on the C ABI through the FFM API;
  needs Java 22 or later and `--enable-native-access=ALL-UNNAMED`

### Documentation

- A getting started guide (`docs/getting-started.md`) and examples in the four
  languages (`examples/`), run against each other in CI

### Interoperability

Tested in CI with omniORB C++, omniORBpy and JacORB, in every combination of
client, server and naming service (omniNames or `rustorb-names`), over GIOP
1.2 and 1.1, and with OpenRTM-aist (C++ and Python). The known differences
are listed in `interop/known-failures.txt`; they are on the other ORBs'
side (for example omniORB's `wchar` handling, `long double` in omniORB and
omniORBpy, and valuetypes in JacORB on Java 17).

### Known limitations

- Out of scope: CORBA Component Model, Event / Notification Service, the
  Interface Repository, CSIv2, the C++98 mapping
- IDL 4 `map` and `@optional` have no GIOP encoding and are rejected
- Valuetype codebase URLs are skipped when read; C++ and Java have no API to
  break valuetype cycles (a C++ application breaks its own `shared_ptr`
  cycles; Java relies on the GC) (#85)
- On Windows the read path of #66 / #67 falls back to the I/O task (#86)

[Unreleased]: https://github.com/rustorb/rustorb/compare/v0.1.1...HEAD
[0.1.1]: https://github.com/rustorb/rustorb/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/rustorb/rustorb/releases/tag/v0.1.0

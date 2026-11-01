# Changelog

All notable changes to rustorb are written here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/) (before 1.0, a minor version may
break the API).

## [Unreleased]

### Added

- Trace output (`-ORBtraceLevel N`, `-ORBtraceTime 0|1`; `OrbOptions::trace_level`,
  `trace_time`; docs/tracing.md): connections, requests and replies, system
  exceptions, `LOCATION_FORWARD`, POA events and each GIOP message, as events
  of the `tracing` crate. Without a global subscriber, the events the level
  selects go to standard error (C++ and Java use this); Python hands them to
  `logging` (loggers `rustorb.*`) and has `rustorb.set_trace_level` (#105)
- `-ORBgiopMaxMsgSize N` sets `OrbOptions::max_message_size` from every
  binding (docs/message-size.md). The limit now applies to what the ORB sends
  too: a request over it is not sent (`MARSHAL`, completed no), and a reply
  over it is replaced by `MARSHAL`, completed yes (#102)
- `-ORBgiopFragmentSize N` (`OrbOptions::fragment_size`): GIOP 1.2 requests
  and replies longer than N bytes are sent as a first message and `Fragment`
  messages of at most N bytes. Off (0) by default; GIOP 1.1 messages are
  never split (#102)
- `rustorb-idl -brust` maps abstract interfaces (an enum of an object
  reference or a value, and the trait `AServant`) and local interfaces (a
  trait, used as `Arc<dyn L>`); the core's `idl` module has the helpers
  (`AbstractValue`, `register_supported`, `ValueHandle::read_base`). The
  rustorb interop client and server run the abstract interface suite (#104)

### Changed

- The warnings the core wrote to standard error (a servant that panicked, say)
  are trace events of level 1, the default: `-ORBtraceLevel 0` silences them,
  and a `tracing` subscriber of the application receives them instead (#105)
- `rustorb-idl -brust` rejects operations with a `context` clause, which it
  used to drop silently (#104)

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

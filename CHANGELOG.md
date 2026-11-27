# Changelog

All notable changes to rustorb are written here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/) (before 1.0, a minor version may
break the API).

## [Unreleased]

### Added

- `rustorb_core::stats`: process-wide counters of what the ORB does inside
  (I/O task wake-ups, socket reads that found nothing, threads reading a
  connection themselves, follow, the upcall pool, direct and queued
  writes), off by default; `-ORBstatistics 1` / `OrbOptions::statistics`
  or `stats::set_enabled` turns them on. For measuring the speed work of
  docs/perf-plan.md (#204).
- `rustorb_core::stats`: `reply_keeps` and `keep_resumes` (#207),
  `connects` and `connection_shares` (#208), `local_direct` and
  `local_general` (#209).
- `rustorb_giop::Body::from_vec`: a body that is not part of a message.
- `rustorb_core::idl::SharedOctets`: the descriptor of `sequence<octet>`
  as the shared `rustorb_core::Octets`, which a large one is received in
  without being copied out of the message and sent from without being
  copied into it (#211, docs/large-data.md). `rustorb-idl -brust
  -Wbshared_octets` maps `sequence<octet>` to it; the default stays
  `Vec<u8>`.
- `rustorb_core::stats`: `low_water_sets` (#211).
- `-ORBmaxConnectionsPerServer N` / `OrbOptions::max_connections_per_server`
  (also accepted as omniORB's `-ORBmaxGIOPConnectionPerServer`): the most
  connections a client opens to one server endpoint (#208).
- interop/bench: `run-bench.py --quick` (the short before/after set),
  `--stats` (rustorb's counters per call), context switches per call
  (client and server, the server's by thread), and `compare-bench.py`, the
  table of the differences between two runs. `cargo bench -p rustorb-core`
  has `orb/oneway`, `orb/echo_long_threads_one_ref/4` and
  `orb/echo_long_in_process`.

### Changed

- While a large message comes in over TCP without TLS, on Linux, the
  socket counts as readable only once 256 KiB more of it (or all the rest)
  has arrived (`SO_RCVLOWAT`), so the reader wakes once per such piece
  instead of once per segment (#211). With `-Wbshared_octets`, receiving
  1 MiB went from 1.25 to about 2.3 GB/s in the cloud benchmark.
- Calls to an object of the same ORB skip GIOP (#209,
  docs/colocated-calls.md). From GIOP 1.2 on, only the arguments are
  marshalled, the servant runs on the calling thread and the reply comes
  back as its body; system exceptions and `LOCATION_FORWARD` are not
  encoded. A POA manager that is not active, a SINGLE_THREAD_MODEL or
  MAIN_THREAD_MODEL POA, and server interceptors still take the general
  path, which now gets a request header made directly instead of a whole
  message encoded and decoded. A colocated ping went from 2.4 µs to 0.9 µs
  in the cloud benchmark.
- The end of a request no longer makes a system call: the counts of
  requests in progress (per POA, per POA manager, per server) wake a
  waiter only when there is one. Server oneway throughput rose by about
  20% in the cloud benchmark.
- A client opens up to 4 connections to one server endpoint, as calls to
  it run at the same time, instead of putting every call on one (#208,
  docs/connections.md). A call takes a connection no other call is on,
  preferring the one its thread used last, opens another while under
  `-ORBmaxConnectionsPerServer`, and at the limit shares the least busy
  one. One thread calling in turn still uses one connection;
  `-ORBmaxConnectionsPerServer 1` puts every call on one, as before. An ORB
  that offers bidirectional GIOP opens one. A `CloseConnection` under a
  request also closes the other idle connections to that endpoint, and a
  request is sent again up to 3 + the limit times (was 4). Four threads
  calling one reference went from 31,000 to about 120,000 calls/s in the
  cloud benchmark.
- Smaller calls cost less (#207, docs/small-call-cost.md). A blocking
  caller that read its own reply keeps reading the connection until its
  next call, instead of putting it back on the runtime's poller each call
  (`-ORBfollowLinger` bounds how long; another caller waiting for the I/O
  task gets it back at once), and waits for the socket before it reads. An
  object reference works out the addresses of its IOR once instead of for
  every call, a connection makes its code set context once, and requests
  are written from borrowed parts (`rustorb_giop::RequestParts`,
  `MessageEncoder::request_parts`, `TargetRef`). A ping allocates 5 times
  on the client instead of 24, and makes 3 system calls instead of 7;
  its client CPU time fell by about a quarter in the cloud benchmark.
- `rustorb-typesupport`: an array of a primitive type (`Array<D, N>`) is
  written and read as a sequence's elements are, so an `octet` array is
  copied in one go instead of an octet at a time; the octets on the wire
  are the same. Decoding a 1 MiB `octet` array took about 3 ms
  (rustorb-dds#112).

## [0.7.1] - 2026-11-24

A patch release: the C mapping for Windows, the .NET types in a package of
their own, trusted publishing and a Ruby 3.0 fix. The C ABI version stays 3,
and the Rust, Python, C++ and Java APIs are unchanged.

### Added

- .NET: `Omg.Types` (`ISequence<T>`, `Sequence<T>` and the mapping's
  attributes) is in a package of its own, `Rustorb.Types`
  (`dotnet/src/Rustorb.Types`), which `Rustorb` depends on, so that the
  DDS type support of `rustorb-idl -bcsharp -Wbdds_only` (rustorb-dds's
  `Rustorb.Dds`) can use it without the ORB. `Rustorb.Corba` forwards the
  types (`TypeForwardedTo`), so code built against 0.7.0 still runs.
- C: the release also has `rustorb-c-<version>-windows-x86_64.zip`, the
  headers with `rustorb_c.lib` and `rustorb_ffi.lib` built with MSVC. The
  C tests run on Windows (MSVC) in CI.

### Changed

- The release workflow publishes to NuGet and RubyGems with trusted
  publishing (OpenID Connect); the NUGET_API_KEY and RUBYGEMS_API_KEY
  secrets are still used when set.

### Fixed

- Ruby: on Ruby 3.0, a program that required `minitest/autorun` before
  `corba` hung at its first upcall: the extension stopped its dispatcher
  threads from an end proc, which ran before minitest ran the tests. The
  dispatchers now end when Ruby kills them at exit (their job answered, the
  queued ones failed after the last one), and what is left stops at the
  VM's end (`ruby_vm_at_exit`). The test helper requires `minitest/autorun`
  first, so that the suite checks it.

## [0.7.0] - 2026-11-22

Three new language bindings: .NET (C#), C and Ruby (plan #176). The Rust,
Python, C++ and Java APIs are unchanged. The C ABI version stays 3, and
`rustorb.h` is the same as in 0.6.0. The pinned IDL compiler becomes
rustorb-idl 0.5.0, which adds `-bcsharp`, `-bc` and `-bruby`.

### Added

- .NET: the IDL4 to C# mapping (formal/21-07-01) for .NET 8 or later, as
  the NuGet package `Rustorb` with `librustorb_ffi` for Linux x64 / arm64,
  macOS arm64 and Windows x64 in `runtimes/<rid>/native`. It covers the ORB,
  the POA with servant managers, CosNaming, valuetypes, DynAny, the DII and
  DSI, callback AMI, Portable Interceptors and local interfaces
  (docs/csharp-binding.md).
- C: the OMG C Language Mapping 1.0 in C99 (`librustorb_c`), released as
  `rustorb-c-<version>-<platform>.tar.gz` for Linux x86_64 / aarch64 and
  macOS arm64: the headers under `include/rustorb/` and `librustorb_c.a`
  with `librustorb_ffi.a` in `lib/`. Besides the mapping it has DynAny, the
  DII and DSI, servant managers, callback AMI (`rustorb/messaging.h`) and
  Portable Interceptors (`rustorb/pi.h`) (docs/c-binding.md).
- Ruby: the OMG Ruby Language Mapping 1.2 for Ruby 3.0 or later, as the gem
  `rustorb`. It carries `librustorb_ffi.a` for Linux x86_64 / aarch64 and
  macOS arm64, and its C extension is built when the gem is installed (Ruby's
  headers and a C compiler are needed). It has the same features as the .NET
  binding (docs/ruby-binding.md).
- The interop suites, the wire and PI probes and the naming check run the
  three new bindings (`rustorbcs`, `rustorbc`, `rustorbrb`) against
  omniORB, omniORBpy, JacORB and the other rustorb bindings.
- `examples/` has the Echo server and client in C#, C and Ruby, and
  `examples/run-all.sh` runs every server with every client of the seven
  languages.

### Fixed

- A callback request (AMI) to an object of the same ORB ignored the call
  timeout: it now ends with `TIMEOUT` (`COMPLETED_MAYBE`) like one to a
  remote object.

## [0.6.0] - 2026-11-16

C++ and Java anys now hold valuetypes by reference, which changes what
inserting an instance again does (see below); hence a new minor version. The
pinned IDL compiler stays rustorb-idl 0.4.0. The C ABI version stays 3: only
functions are added
(`rorb_out_keeps_hosts`, `rorb_out_begin_host_value`, `rorb_in_set_host_value`,
`rorb_in_keeps_hosts`).

### Added

- The GitHub Release has the naming service `rustorb-names` built for
  Linux x86_64 / aarch64, macOS arm64 and Windows x86_64: in the `bin/` of
  each `rustorb-native` archive, and alone in `rustorb-names-<version>-<platform>`
  archives, so it no longer needs `cargo install rustorb-naming`.

### Changed

- C++ and Java anys hold their valuetypes by reference (docs/valuetype.md,
  #164): the core's instance of a value stands for the C++ or Java instance
  inserted or extracted. One instance put in several anys stays one, so the
  DII and the DSI keep valuetypes shared among the arguments (and the result)
  of a request, as static stubs and skeletons do, and so do the any arguments
  of static stubs; extracting from an any gives the same instance each time.
  Inserting an instance again replaces the state its earlier anys hold. The
  known failures `same[shared]` of `rustorbcpp-dii` and `rustorbjava-dii` are
  gone.
- `rustorb_core`: `ValueRef::downgrade` and `WeakValueRef`,
  `OutStream::log_values` / `written_values`, `InStream::preset_value` /
  `value_at`, and `Encoder::value_count` in rustorb-cdr, for the bindings.

### Fixed

- Python: the docstring of `rustorb.timeouts` gave the thread's timeout
  before the reference's; the reference's comes first (docs/timeouts.md).
- The GitHub Release no longer carries `rustorb-mapping-tests-0.0.0.crate`,
  a test package that is not published.

## [0.5.0] - 2026-11-15

The C ABI version stays 3: the DII and DSI, SyncScope and POAManagerFactory
only add functions and constants (`rorb_request_*`, `rorb_server_request_*`,
`rorb_any_typed`, `rorb_poa_*_poa_manager*`, `RORB_SYNC_SCOPE_POLICY_TYPE`,
`RORB_MANAGER_ALREADY_EXISTS`). The pinned IDL compiler is rustorb-idl 0.4.0,
which makes the IDL `context` clause an error in every backend (#151).

### Added

- `omniORB.URI` in rustorb-omniorb-compat (docs/omniorb-compat.md, #152):
  `stringToName`, `nameToString` and `addrAndNameToURI`, with omniORBpy
  4.3.4's rules, measured by running it (they differ in details from the
  naming service's `to_name` / `to_url`).
- `Messaging::SyncScopePolicy` (docs/sync-scope.md, #153) in the core, Python,
  the C ABI / C++ and Java: `create_policy` with a `Messaging::SyncScope`,
  overridden on a reference, the thread or the ORB like the timeout. A oneway
  call returns before the request is written (`SYNC_NONE`), once it is
  written (`SYNC_WITH_TRANSPORT`, the default and the old behaviour), or once
  the server answers before (`SYNC_WITH_SERVER`) or after
  (`SYNC_WITH_TARGET`) the servant runs; the last two raise the system
  exceptions of a two-way call. The interceptors' `sync_scope` and
  `get_request_policy` give it.
- The DII and the DSI in the C ABI and C++ (docs/dii-dsi.md, #155):
  `rorb_request_*` and `rorb_server_request_*`; in C++, `CORBA::Request`,
  `NVList`, `NamedValue`, `ExceptionList`, `Object::_request` /
  `_create_request`, `ORB::create_list`, `send_multiple_requests_*`,
  `poll_next_response` / `get_next_response`,
  `PortableServer::DynamicImplementation` and `CORBA::ServerRequest`, with
  user exceptions as `CORBA::UnknownUserException`. Interceptors get the
  arguments, result and exceptions of these requests. The interop matrix
  gains `rustorbcpp-dii` (the C++ client and server through the DII and the
  DSI). Contexts stay `NO_IMPLEMENT`, and one valuetype in two C++ anys is
  not shared.
- The DII and the DSI in Java (docs/dii-dsi.md, #156), on the C ABI's:
  `org.omg.CORBA.Request`, `NVList`, `NamedValue`, `ExceptionList`,
  `ContextList`, `Environment`, `UnknownUserException`, `ARG_IN` / `ARG_OUT` /
  `ARG_INOUT`, `Object._request` / `_create_request`, `ORB.create_list`,
  `send_multiple_requests_*`, `poll_next_response` / `get_next_response`,
  `org.omg.PortableServer.DynamicImplementation` and
  `org.omg.CORBA.ServerRequest`. The exception of a call is not thrown but
  put in the request's `env()` (a user exception as an
  `UnknownUserException`), as JacORB does. Interceptors get the arguments, result and exceptions of these
  requests. The interop matrix gains `rustorbjava-dii` (the Java client and
  server through the DII and the DSI). Contexts stay `NO_IMPLEMENT`, and one
  valuetype in two Java anys is not shared.
- `PortableServer::POAManagerFactory` (CORBA 3; docs/poa-design.md, #154) in
  the core, Python, the C ABI / C++ and Java: `POA::the_POAManagerFactory`,
  `create_POAManager`, `list`, `find` and `POAManager::get_id`. Every POA
  manager has an identity unique in the ORB (`RootPOAManager` for the
  RootPOA's, `POAManager<n>` for those `create_POA` makes); a manager ends
  with the last of its POAs, and `create_POA` with such a manager raises
  `OBJECT_NOT_EXIST`. Any policy given to `create_POAManager` raises
  `PolicyError`. The C ABI adds `rorb_poa_create_poa_manager`,
  `rorb_poa_list_poa_managers`, `rorb_poa_find_poa_manager`,
  `rorb_poa_manager_get_id`, `rorb_poa_manager_equal` and the local exception
  `RORB_MANAGER_ALREADY_EXISTS` (27). Rust's `PoaError` gets
  `ManagerAlreadyExists` and `Policy`.
- `giop:unix` endpoints, omniORB's Unix domain sockets (docs/unix-transport.md,
  #157), on Linux and macOS: `-ORBendPoint giop:unix:PATH` (an empty PATH
  makes a socket in `/tmp/omni-USER/`, a PATH ending in `/` one in that
  directory), `OrbOptions::unix_endpoints` and `Orb::unix_endpoints`. The
  sockets go into IORs as omniORB's `TAG_OMNIORB_UNIX_TRANS` components
  (`rustorb_giop::UnixTrans`), and clients use one before TCP or TLS when it
  names their own host, falling back to the other addresses when it cannot be
  reached. Bidirectional GIOP works over them. Python, C++ and Java take the
  same ORB argument. A socket file is removed when the ORB shuts down; a
  stale socket at the path is replaced, but, unlike omniORB, a socket in use
  or a file that is not a socket makes the endpoint fail. The interop matrix
  gains `--unix`.

### Fixed

- Java: `POA.the_children()` no longer leaks a native POA handle per child.
- rustorb-cdr writes the XCDR2 encapsulation identifiers of DDS-XTypes
  Table 60, the RTPS ones (`0x0006` to `0x000b`), instead of those of Table
  39 and DDSI-RTPS 2.5 Table 10.3 (`0x0010` to `0x0015`), which Cyclone DDS
  0.10 drops (rustorb/rustorb-dds#65). Both are still read.

### Changed

- A rustorb server answers a oneway request with `response_flags` 0x01
  (`SYNC_WITH_SERVER`) before calling the servant, as GIOP 1.2 says, instead
  of after it.

## [0.4.0] - 2026-11-09

The C ABI version is 3 (callback AMI added `rorb_object_sendc`,
`rorb_cdr_encode` and `rorb_in_from_bytes`). Callback AMI needs code generated
by rustorb-idl 0.3.0 or later with `-Wbami`.

### Added

- The core of AMI (docs/ami.md, #136): `ObjectRef::request_callback` returns a
  `CallbackRequest` that `send`s without waiting and hands the reply or the
  exception to a callback, exactly once, on a thread of the ORB's new reply
  pool. The C ABI has it as `rorb_object_sendc` with a `rorb_sendc_done_fn`.
  A callback that cannot be delivered on the pool (the ORB was destroyed)
  never runs on a thread of the runtime, where the synchronous API fails.
- Callback AMI in Python (docs/ami.md, #138): with code generated by
  `rustorb-idl -bpython -Wbami` (rustorb-idl 0.3.0 or later), the stubs'
  `sendc_<op>(ami_handler, ...)` send without waiting and the reply goes to
  the `AMI_<I>Handler`'s `<op>`, an exception to its `<op>_excep` with a
  `Messaging.ExceptionHolder` (`raise_exception()`,
  `raise_exception_with_list()`). New in `rustorb.Messaging`: `ReplyHandler`
  and `ExceptionHolder`; new module `rustorb.Messaging__POA`.
- Callback AMI in C++ (docs/ami.md, #141): with code generated by
  `rustorb-idl -bc++11 -Wbami` (rustorb-idl 0.3.0 or later), the stubs'
  `sendc_<op>(ami_handler, ...)` send without waiting and the reply goes to
  the `AMI_<I>Handler`'s `<op>`, an exception to its `<op>_excep` with a
  `Messaging::ExceptionHolder`. New header `<rustorb/messaging.h>`:
  `Messaging::ReplyHandler`, `POA_Messaging::ReplyHandler`,
  `Messaging::ExceptionHolder` (`raise_exception()`,
  `raise_exception_with_list()`). CMake's `rustorb_idl_generate()` takes
  `OPTIONS` (such as `-Wbami`).
- Callback AMI in Java (docs/ami.md, #139): with code generated by
  `rustorb-idl -bjava -Wbami` (rustorb-idl 0.3.0 or later), the stub classes'
  `sendc_<op>(ami_handler, ...)` (cast a reference to `_<I>Stub`) send
  without waiting and the reply goes to the `AMI_<I>Handler`'s `<op>`, an
  exception to its `<op>_excep` with an `org.omg.Messaging.ExceptionHolder`
  (`raise_exception()`, `raise_exception_with_list()`). New in
  `org.omg.Messaging`: `ReplyHandler` and its classes, `ExceptionHolder` and
  its classes.
- C ABI: `rorb_cdr_encode` and `rorb_in_from_bytes`, plain CDR from offset 0
  in either byte order, the form of `ExceptionHolder::marshaled_exception`.
- Interop: `interop/run-ami-probe.sh` checks callback AMI from the Python, C++
  and Java clients against the omniORB C++, omniORBpy, JacORB and rustorb
  servers (results, inout and out arguments, user and system exceptions,
  attributes, a nil handler, a call timeout), in CI (#140).

### Changed

- `tools/rustorb-idl.txt` pins rustorb-idl 0.3.0, the first release with
  `-Wbami`; CI no longer checks out a rustorb-idl commit (#140).

### Fixed

- Java: an `ExceptionHolder` that came through the ORB raised `UNKNOWN` for a
  user exception whose repository id has a dotted prefix (`#pragma prefix
  "rustorb.org"`): the Helper lookup now also tries the class name without
  the prefix, as the IDL to Java mapping leaves it out of package names.

## [0.3.0] - 2026-11-03

The C ABI version stays 2. Request ids and the moved IDL compiler are the
changes to look at when upgrading.

### Added

- Bidirectional GIOP (docs/bidir-giop.md): `-ORBofferBiDirectionalGIOP 1`
  (`OrbOptions::offer_bidir`) sends the ORB's endpoints in a `BI_DIR_IIOP`
  service context with the first GIOP 1.2 request on each new connection, and
  requests the server sends back over it reach the POAs with the
  `BidirectionalPolicy` `BOTH` (`Policy::Bidirectional`, policy type 37 for
  `create_policy`; others answer `NO_PERMISSION`);
  `-ORBacceptBiDirectionalGIOP 1` (`OrbOptions::accept_bidir`) makes the
  server call the objects at those addresses over that connection, unless it
  has a usable connection to the address already. A `BidirectionalPolicy`
  override of the ORB does the same as both options. Idle connections are
  closed only when nothing runs or waits in either direction. Python has the
  `BiDirPolicy` module (also as `omniORB.BiDirPolicy`), the C ABI
  `RORB_BIDIRECTIONAL_POLICY_TYPE`, C++ `BiDirPolicy::BidirectionalPolicy` and
  Java `org.omg.BiDirPolicy`; `interop/run-bidir-probe.sh` checks both
  directions with omniORB C++ and omniORBpy (#103)
- `interop/run-pi-probe.sh` checks Portable Interceptors against omniORBpy
  (client and server) and omniORB C++ (client), which carry the probe's
  service contexts with their own request interceptors; the CI interop
  workflow runs it (#125)

### Changed

- The IDL compiler (`rustorb-idl` and the `rustorb-codegen` crate) moved to its
  own repository, https://github.com/rustorb/rustorb-idl, with its history,
  and is released from there on its own (#118). This repository is MIT OR
  Apache-2.0 only now. Its tests, examples and checked-in generated code use
  the version pinned in `tools/rustorb-idl.txt` (`pip install -r
  tools/rustorb-idl.txt`; `RUSTORB_IDL_DIR` for a checkout). The tests that
  run the generated Rust code moved to `tests/mapping`
- Client request ids are even (0, 2, 4, …): the side that opened a connection
  uses even ids and the other side odd ones, as bidirectional GIOP needs (#103)
- GIOP 1.0 / 1.1 replies now carry the service contexts that server
  interceptors add in `send_reply` / `send_exception` / `send_other` even when
  they move the body by 4 bytes: the padding before each 8-byte aligned item
  of the body is written again (`Encoder::track_align8` / `take_align8`,
  `OutMessage::with_reply_header`). Only a body with a valuetype, whose chunk
  lengths and indirections count that padding, still drops them with a
  warning (#126)

## [0.2.0] - 2026-11-02

### Added

- DDS type support in the Rust generator (`rustorb-idl -brust -Wbdds`, or
  `-Wbdds_only` without the CORBA code; `Config::dds` / `Config::dds_only`;
  docs/dds-type-support.md): structs, unions, enums and bitmasks implement
  the traits of the new crate `rustorb-typesupport` (MIT OR Apache-2.0,
  depends only on `rustorb-cdr`): XCDR1 and XCDR2 with `@final`,
  `@appendable` and `@mutable`, `@optional`, `@external`, member IDs from
  `@id` / `@hashid` / `@autoid`, `@must_understand`, `@key` with the key
  holder and key hash of DDS-XTypes 7.6.8, the RTPS SerializedPayload, and
  topic types from `@nested` / `@default_nested` / `@topic` (#113)
- Portable Interceptors in C, C++ and Java: the C ABI (`rustorb.h`: interceptor
  and initializer vtables, `rorb_register_orb_initializer`, `rorb_orb_init_pi`,
  the `rorb_cri_*` / `rorb_sri_*` / `rorb_ior_info_*` / `rorb_init_info_*`
  accessors, PICurrent and `Codec`), `<rustorb/pi.h>` for C++ (the
  `PortableInterceptor` and `IOP` modules; `ORB_init` passes its ORB id), and
  `org.omg.PortableInterceptor`, `org.omg.IOP` and `org.omg.Dynamic` for Java,
  generated from `java/idl/PortableInterceptor.idl`, with initializers named by
  `org.omg.PortableInterceptor.ORBInitializerClass.*` properties and the ORB id
  by `org.omg.CORBA.ORBId`. `interop/run-pi-probe.sh` gains rustorbcpp and
  rustorbjava (#100)
- Portable Interceptors in Python (`rustorb.PortableInterceptor`,
  `rustorb.IOP`): `ORBInitializer` with `register_orb_initializer`, client
  and server request interceptors, IOR interceptors, `PICurrent` and
  `Codec`, on the standard IDL to Python mapping; `PICurrent` and
  `CodecFactory` initial references (#100)
- Portable Interceptors in the core (`rustorb_core::pi`, `rustorb_core::iop`;
  docs/portable-interceptors.md): `ORBInitializer` (registered globally or in
  `OrbOptions::initializers`), client and server request interceptors with
  service contexts and `ForwardRequest`, IOR interceptors adding tagged
  components, `PICurrent` slots, `IOP::Codec` (CDR encapsulations 1.0 to 1.2)
  and `Orb::codec_factory`. `-ORBid` and `-ORBServerId` set
  `OrbOptions::orb_id` and `server_id`. Without interceptors nothing changes
  on the call path; interop with JacORB in `interop/run-pi-probe.sh` (#100)
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
- Idle connections are closed (docs/connections.md): a client connection with
  no reply awaited and nothing sent or received for `-ORBoutConScanPeriod S`
  seconds (`OrbOptions::client_idle_timeout`, default 120 s), and a server
  connection with no request running for `-ORBinConScanPeriod S` seconds
  (`server_idle_timeout`, default 180 s, closed with `CloseConnection`; 0
  turns either off). Calls made afterwards open a new connection.
  `-ORBmaxServerConnections N` (`max_server_connections`) caps the server's
  connections, closing the one used least recently to make room (#101)
- `rustorb-idl -brust` maps abstract interfaces (an enum of an object
  reference or a value, and the trait `AServant`) and local interfaces (a
  trait, used as `Arc<dyn L>`); the core's `idl` module has the helpers
  (`AbstractValue`, `register_supported`, `ValueHandle::read_base`). The
  rustorb interop client and server run the abstract interface suite (#104)
- rustorb-cdr: the `xcdr` module writes and reads the headers of DDS-XTypes
  Extended CDR: the RTPS SerializedPayload header (`Encoder::xcdr_payload`,
  `Decoder::xcdr_payload`; alignment restarts after it and the options record
  the final padding), DHEADER (`begin_dheader` / `read_dheader`), the XCDR2
  member header EMHEADER (`begin_member` / `read_member_header`, length codes
  0 to 7) and the XCDR1 parameter headers, short and `PID_EXTENDED`
  (`begin_parameter` / `read_parameter`), for mutable types and optional
  members. `wchar` and `wstring` are written and read with XCDR (#112)
- `rustorb-idl -brust` maps abstract valuetypes (a handle and the trait
  `AMethods`, implemented for the derived valuetypes) and valuetype
  operations that raise user exceptions (per-operation error enums) (#104)

### Changed

- The C ABI version is 2 (`RORB_ABI_VERSION`): Portable Interceptors, and the
  local exception codes 21 to 26 (#100)
- A request left without a reply by `CloseConnection` is sent again up to 4
  times on new connections (was 2) before `TRANSIENT`; a server ignores the
  requests that expect a reply once it has decided to send `CloseConnection`,
  so that the client's resend does not run them twice (#101)
- rustorb-cdr: `CdrVersion` is now `Encoding`, which adds `Xcdr1` and `Xcdr2`
  (DDS-XTypes Extended CDR) to the GIOP versions, and `Encoder::version` and
  `Decoder::version` are now `encoding`. XCDR2 aligns no primitive to more
  than 4 octets (`Encoding::max_align`). GIOP encoding is unchanged (#112)
- The warnings the core wrote to standard error (a servant that panicked, say)
  are trace events of level 1, the default: `-ORBtraceLevel 0` silences them,
  and a `tracing` subscriber of the application receives them instead (#105)
- `rustorb-idl -brust` rejects operations with a `context` clause, which it
  used to drop silently (#104)

### Fixed

- A client connection closed by the server stayed open, socket and reader task,
  until the next call to the same endpoint, and when two calls connected to an
  endpoint at once the connection not kept stayed open until the server closed
  it (#101)
- Python: a process with trace output could abort at exit with
  `Fatal Python error`, the log thread writing to `sys.stderr` (or calling
  `logging`) while the interpreter shut down; the exit hook now stops that
  thread first (#123)

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

[Unreleased]: https://github.com/rustorb/rustorb/compare/v0.7.1...HEAD
[0.7.1]: https://github.com/rustorb/rustorb/compare/v0.7.0...v0.7.1
[0.7.0]: https://github.com/rustorb/rustorb/compare/v0.6.0...v0.7.0
[0.6.0]: https://github.com/rustorb/rustorb/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/rustorb/rustorb/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/rustorb/rustorb/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/rustorb/rustorb/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/rustorb/rustorb/compare/v0.1.1...v0.2.0
[0.1.1]: https://github.com/rustorb/rustorb/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/rustorb/rustorb/releases/tag/v0.1.0

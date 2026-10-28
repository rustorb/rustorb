# Interop tests

Docker based interop tests between CORBA implementations, run by
[`.github/workflows/interop.yml`](../.github/workflows/interop.yml) on every
push and pull request. rustorb has no ORB yet, so for now the matrix pairs
omniORB with itself. rustorb is added later as one more implementation under
`impls/` (see [Adding an implementation](#adding-an-implementation)).

| Path | Contents |
| --- | --- |
| `idl/InteropTest.idl` | Stage 1 test IDL (no Any) |
| `impls/omniorbpy/` | Echo server and test client on omniORBpy, and the naming service check |
| `impls/omniorb-cpp/` | Echo server and test client on omniORB C++ |
| `docker/omniorb/Dockerfile` | omniORB 4.3.4 and omniORBpy 4.3.4 built from source, with the implementations above |
| `compose.yaml` | naming service, server and client containers |
| `run-matrix.sh` | runs every client / server combination |
| `run-naming-check.sh` | runs a fixed CosNaming operation sequence and compares it with `expected/naming-check.txt` |
| `known-failures.txt` | failures caused by the existing ORBs, treated as expected |
| `scripts/giop-extract` | splits a pcap file into GIOP messages |
| `scripts/golden-compare` | compares captured GIOP messages with `golden/*.json` |
| `golden/` | GIOP messages captured from each client / server pair |

## Running locally

Docker with the Compose plugin is needed. The first build compiles omniORB
and omniORBpy, which takes a few minutes.

```sh
interop/run-matrix.sh --build --capture        # all pairs, with GIOP captures
interop/run-matrix.sh --capture --giop 1.1     # all pairs, clients limited to GIOP 1.1
interop/run-matrix.sh omniorbpy:omniorb-cpp    # one pair: CLIENT:SERVER
interop/run-naming-check.sh
interop/scripts/golden-compare --all interop/out interop/golden
```

Logs and captures go to `interop/out/<client>-to-<server>/` (with
`-giop1.1` appended for `--giop 1.1`):
`client.log`, `server.log`, `naming.log`, and with `--capture`,
`capture/capture.pcap` and `capture/messages/` (one `.bin` file per GIOP
message, header included, and a `messages.json` index).

Each line of the summary is `PASS`, `XFAIL` (only failures listed in
`known-failures.txt`) or `FAIL`. A listed failure that starts passing is also
a `FAIL`, so that the list stays current.

## Topology

```
          corbaloc::naming:2809/NameService
 client ─────────────────────────────────────▶ naming (omniNames :2809)
  :2811 ◀──── callbacks ────┐                     ▲
   │                        │                     │ bind interop/Echo
   └──── Echo calls ──────▶ server :2810 ─────────┘
```

All three run in their own container on one Compose network. Every IOR
carries the host name (`naming`, `server`, `client`) and a fixed port rather
than an IP address, and the Echo and Callback objects live in PERSISTENT /
USER_ID POAs with fixed object ids (`InteropTest/Echo`,
`InteropTestClient/Callback`). Object keys and IORs are therefore the same on
every run, which is what makes the captures usable as golden files.

## The test IDL

`idl/InteropTest.idl` covers what the stage 1 plan lists: basic types,
string / wstring, struct, enum, unions (long and enum discriminators,
multiple labels, default, implicit default), sequences, arrays, typedefs,
user and system exceptions, in / out / inout, attributes, oneway, object
reference passing (including nil) and callbacks from the server to the
client. Any is left for stage 2 (M5).

The comment on each operation is its specification. Every server must
implement it exactly, and every client runs the same cases (78 at the
moment) with the same values, so that the captures from different clients
can be compared too. For example, the char string test sends `caf\xe9` in
ISO-8859-1, omniORB's native char code set, from both clients.

## Implementation contract

`impls/<name>/server` and `impls/<name>/client` are executables that take:

| Option | Meaning |
| --- | --- |
| `--naming CORBALOC` | root naming context, e.g. `corbaloc::naming:2809/NameService` |
| `--name PATH` | slash separated name of the Echo object (the server creates missing contexts) |
| `--host HOST` / `--port N` | host name and fixed port to listen on and publish in IORs |
| `--timeout S` | how long to retry while the naming service or server is not up yet |
| `--shutdown-server` | (client) call `Echo::shutdown` at the end |

Other arguments are ORB options. The server prints `server ready: PATH`
once bound. The client prints `ok   <case>` or `FAIL <case>: <reason>` per
case, then `N passed, M failed`, and exits with 0 only when all cases pass.

## Adding an implementation

1. Add `impls/<name>/` with `server` and `client` following the contract.
2. Put it in an image: either extend `docker/omniorb/Dockerfile`, or build
   another image and point the role at it with `SERVER_IMAGE` /
   `CLIENT_IMAGE`. The client image needs `tcpdump` and the `scripts/`
   directory for `--capture`.
3. Add the name to `IMPLS` in `run-matrix.sh` (or the environment) so that
   it is paired with every other implementation.
4. To test another naming service, set `NAMING_IMAGE` and `NAMING_CMD`; it
   must listen on `naming:2809` with the object key `NameService`.

## Golden files

`golden/<client>-to-<server>.json` holds every GIOP message the client sent
and received (naming service, Echo server and callbacks), in request / reply
order, with the decoded header fields and the whole message as hex. The
Fragments of a message follow it. `golden/<client>-to-<server>-giop1.1.json`
is the same run with the client limited to GIOP 1.1 (omniORB's
`ORBmaxGIOPVersion`, set through `CLIENT_MAX_GIOP` in `compose.yaml`). The CI
job captures a fresh run and compares it with these files. It ignores
request ids, the alignment padding in GIOP headers, the `posted` polling
(its count depends on timing) and CloseConnection messages. The oneway test
runs last for the same reason. Repeated local runs match byte for byte.

**omniORB does not clear alignment padding.** The padding bytes in headers
and bodies hold whatever was left in its buffer (for example `78 78` from
the earlier 10000-character `x` string, or `44 4c 3a` from `IDL:`). Header
padding is located by `giop-extract` (the `padding` field) and masked;
body padding is stable between runs of the same pair, so it is compared as
is. The consequence for rustorb: its encoder should write zeros, and golden
tests must mask padding (or compare decoded values) rather than expect
omniORB's bytes there. Apart from padding, the omniORB C++ and omniORBpy
clients send identical request bodies for every Echo operation.

They are the reference for the rustorb CDR / GIOP golden tests
([`crates/rustorb-giop/tests/omniorb_golden.rs`](../crates/rustorb-giop/tests/omniorb_golden.rs)):
a rustorb encoder must produce the same bytes as omniORB for the same
request, and the decoder must read omniORB's replies. When the IDL, the test cases or the
omniORB version change, regenerate them:

```sh
interop/run-matrix.sh --capture
interop/scripts/golden-compare --update --all interop/out interop/golden
```

## omniORB wire format findings

Checked against the captures and the omniORB 4.3.4 sources; the golden
tests in `rustorb-giop` keep rustorb in line with them.

- **wstring (GIOP 1.2)**: omniORB always sends a BOM followed by UTF-16 in
  the stream byte order (`TCS_W_UTF_16::marshalWString` in
  `cs-UTF-16.cc`), so little-endian messages carry `ff fe` and
  little-endian code units. An empty wstring is just a zero length, with no
  BOM. A received wstring without a BOM is read as big endian, as the
  specification says. rustorb writes the same bytes by default
  (`Encoder::set_wstring_bom(false)` gives the big-endian form without a
  BOM) and reads both.
- **wchar (GIOP 1.2)**: an octet length of 2 and the code unit in big
  endian, no BOM, whatever the stream byte order. On receipt a length of 4
  means a BOM followed by the code unit.
- **Fragments (GIOP 1.1)**: the data in each Fragment is aligned relative
  to the start of that Fragment message, not to the joined message.
  omniORB ends every fragment but the last on an 8-byte boundary (fragments
  are 8192 bytes here) and continues at offset 12 of the next one, so an
  8-byte value after a boundary is 4 bytes off if the fragments are simply
  concatenated. The `echo_records[large]` case (400 records with doubles)
  shows this in both directions. rustorb's `FragmentAssembler` keeps the
  fragment boundaries for the decoder.
- **Fragments (GIOP 1.2)**: the data starts at offset 16 of each Fragment
  and fragments end on 8-byte boundaries, so the per-fragment and the
  joined alignment agree.
- **wchar / wstring over GIOP 1.1** do not work in omniORB: its UTF-16 code
  set is registered for GIOP 1.2 only (see `known-failures.txt`).

## Known issues in omniORB found by these tests

- **omniORBpy cannot unmarshal wchar** (4.3.2 and 4.3.4): on Python 3.3 and
  later, `unmarshalPyObjectWChar` in `modules/pyMarshal.cc` reads a wchar
  with `unmarshalChar()`, so a GIOP 1.2 wchar (length octet plus UTF-16)
  leaves bytes behind. The omniORBpy server closes the connection with
  "Garbage left at end of input message", and the omniORBpy client raises
  `COMM_FAILURE`. Marshalling is correct, so rustorb receiving wchar from
  omniORBpy is not affected.
- **omniORB C++ does not convert wchar_t above U+FFFF to surrogate pairs**:
  sending `L"\U0001F980"` raises `BAD_PARAM`, and received surrogate pairs
  stay as two `wchar_t` values. The C++ client therefore sends the
  surrogate pair itself.

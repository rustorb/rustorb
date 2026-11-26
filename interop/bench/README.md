# rustorb / omniORB comparison benchmark

The programs and the script behind [docs/omniorb-comparison.md](../../docs/omniorb-comparison.md).
Every implementation serves the same interface ([Bench.idl](Bench.idl)) and runs
the same cases with the same options and output, so any client can be timed
against any server, and each one alone in one process.

| implementation | language | program |
| --- | --- | --- |
| `rustorb` | Rust | [../impls/rustorb/src/bench.rs](../impls/rustorb/src/bench.rs) (bin `rustorb-bench`) |
| `rustorbcpp` | C++ (IDL to C++11) | [rustorb-cpp/bench.cc](rustorb-cpp/bench.cc) |
| `rustorbpy` | Python | [python/bench.py](python/bench.py) (`--orb rustorb`) |
| `rustorbjava` | Java | [java/BenchMain.java](java/BenchMain.java) |
| `rustorbcs` | C# | [csharp/Program.cs](csharp/Program.cs) |
| `rustorbc` | C | [c/bench.c](c/bench.c) |
| `rustorbrb` | Ruby | [ruby/bench.rb](ruby/bench.rb) |
| `omniorbcpp` | C++ (classic mapping) on omniORB | [omniorb-cpp/bench.cc](omniorb-cpp/bench.cc) |
| `omniorbpy` | Python on omniORBpy | [python/bench.py](python/bench.py) (`--orb omniorb`) |

The two C++ programs are the same program in the two C++ mappings; the Python
program is one script for both ORBs (the standard Python mapping).

## Running

```sh
pip install -r tools/rustorb-idl.txt        # rustorb-idl, for the stubs
interop/bench/run-bench.py                  # everything that can be built, 3 runs each
interop/bench/run-bench.py --impls rustorb,omniorbcpp --pairs same --repeat 5
interop/bench/run-bench.py --modes pairs --transports tcp,unix --cases ping,octets:1048576
```

### Before and after a change

`--quick` runs the short set the speed work compares
([docs/perf-plan.md](../../docs/perf-plan.md#測り方)): rustorb and omniORB C++,
each against itself, `ping`, `any`, `oneway`, `threads:4`, `get:1048576`, and
`ping` in one process. `compare-bench.py` prints the differences of two runs
as a Markdown table.

```sh
interop/bench/run-bench.py --quick --stats --out /tmp/before
# ... the change ...
interop/bench/run-bench.py --quick --stats --out /tmp/after
interop/bench/compare-bench.py /tmp/before /tmp/after --counters
```

Every run records the context switches per call: the client's (wait4, Linux
and macOS) and, on Linux, the server's, also by thread name
(`server.csw[rustorb-io]`, ...). `--stats` adds rustorb's internal counters
per call (`rustorb_core::stats`; `rustorb-bench client --stats` and
`rustorb-bench serve --stats-file PATH`), for the rustorb client and server.

`run-bench.py --help` lists the options. It builds each implementation into
`interop/out/bench-build/` (skipping, with the reason, those whose tools are
missing) and writes `result.json` and `result.md` to `interop/out/bench/DATE/`.

omniORB is used only as a binary (the clean-room policy): `omniidl`, the
headers and the libraries of an installed omniORB (a distribution package or
a build of the release tarball). Set `OMNIIDL` when the `omniidl` script needs
another Python than the default one (Ubuntu 24.04's package:
`OMNIIDL="python3.12 /usr/bin/omniidl"`), and `OMNIORB_PREFIX` when the
headers and libraries are not under the directory above omniidl's `bin`.
omniORBpy must be importable (on `PYTHONPATH`, or `PREFIX/python`).

## A program by hand

```sh
rustorb-bench serve -ORBendPoint giop:tcp:127.0.0.1: > ior.txt &
rustorb-bench client --ior "$(cat ior.txt)" --cases ping,octets:1048576 --seconds 2
rustorb-bench inproc --cases ping
```

`serve` prints the IOR of its `Bench::Target` and runs until killed (or
`shutdown`). The rustorb program also takes `--stats` (client, inproc: a
line of internal counts after each case) and `--stats-file PATH` (serve: the
counts so far, written every 50 ms). `client` and `inproc` take `--cases LIST`, `--seconds S` (each
case, default 2) and `--warmup S` (default 0.3); the other arguments go to
ORB_init. run-bench.py passes `-ORBgiopMaxMsgSize 67108864` to every ORB
(omniORB's default limit is 2 MB).

## Cases

| case | what one call does | numbers |
| --- | --- | --- |
| `first` | the first `ping` on a fresh reference (the connection is made); run-bench.py runs 10 client processes for it | latency, client process time |
| `ping` | `void ping()` | latency |
| `long` | `long echo_long(long)` | latency |
| `string[:N]` | `echo_string` of N characters (100) | latency |
| `octets:N` | `echo_octets` of N octets (N each way) | latency, MB/s of both directions |
| `put:N` | `put_octets` of N octets, a `unsigned long` back | latency, MB/s |
| `get:N` | `get_octets(N)`, N octets back | latency, MB/s |
| `samples:N` | `echo_samples` of N structs (`long`, 3 `double`, a short string) | latency |
| `any` | `echo_any` of an any holding one such struct | latency |
| `oneway` | `oneway post(long)` as fast as possible, then `take_posted()` until every one has arrived | calls/s (to the last arrival), the sending rate |
| `threads:N` | `echo_long` from N threads sharing one object reference | calls/s |
| `procs:N` | (run-bench.py) N client processes running `long` at once | calls/s summed |

A timed case first calls for `--warmup` seconds, then times every call for
`--seconds` and prints one JSON line: `calls`, `rate` (calls/s), `mean_us`,
`p50_us`, `p90_us`, `p99_us`, `max_us`, `mb_per_s`, and `total_calls` (with
the warm-up). Every reply is checked; a wrong one or an exception makes the
exit status 1.

In pairs mode run-bench.py runs one client process per case and records the
CPU time of the client (wait4, less the CPU time of a client process that runs
no case) and of the server (/proc, over the client's run), divided by
`total_calls`, and the server's resident memory after it printed its IOR and
at its peak (VmHWM). macOS has no /proc: there the server's CPU time comes from
`ps` (10 ms steps) and only the memory after start is recorded.

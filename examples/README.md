# Examples

One IDL interface ([idl/Echo.idl](idl/Echo.idl)) with a server and a client
in each binding. Each server registers itself in the naming service as
`Echo`; each client looks it up, calls `echo` and, with `--shutdown`, stops
the server. Every server works with every client.

| | Server | Client | Build |
| --- | --- | --- | --- |
| Rust | [rust/src/bin/server.rs](rust/src/bin/server.rs) | [rust/src/bin/client.rs](rust/src/bin/client.rs) | `cargo build` in `rust/` (`build.rs` runs rustorb-idl) |
| Python | [python/server.py](python/server.py) | [python/client.py](python/client.py) | `rustorb-idl -bpython` |
| C++ | [cpp/server.cc](cpp/server.cc) | [cpp/client.cc](cpp/client.cc) | CMake, `find_package(rustorb)` |
| Java | [java/Server.java](java/Server.java) | [java/Client.java](java/Client.java) | `rustorb-idl -bjava`, `javac` |

The [getting started guide](../docs/getting-started.md) explains them step by
step. To build all four and run every server with every client against a
fresh `rustorb-names`:

```sh
examples/run-all.sh                        # cargo, cmake, a C++ compiler, Python 3.9+, Java 22+ and Maven
EXAMPLES="rust python cpp" examples/run-all.sh
```

By hand, start a naming service and pass its address to each program:

```sh
rustorb-names -memory -start 2809 &
examples/rust/target/debug/server -ORBInitRef NameService=corbaloc::localhost:2809/NameService &
PYTHONPATH=gen python examples/python/client.py -ORBInitRef NameService=corbaloc::localhost:2809/NameService --shutdown
```

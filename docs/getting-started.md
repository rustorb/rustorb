# Getting started

This guide builds one small system four times: a server that implements an
IDL interface and registers it in the naming service, and a client that
looks it up and calls it, in Rust, Python, C++ and Java. Every server works
with every client, and with omniORB, omniORBpy and JacORB ones. The full
code is in [examples/](../examples/); `examples/run-all.sh` builds all four
and runs each server with each client.

## 1. Install the tools

```sh
pip install rustorb-idl              # the IDL compiler, for every language
cargo install rustorb-naming         # rustorb-names, the naming service
```

`rustorb-idl` needs Python 3.9 or later and installs
[idl_parser](https://github.com/sugarsweetrobotics/idl_parser). Any CORBA
naming service works in place of `rustorb-names`, omniNames included.

## 2. Start the naming service

```sh
rustorb-names -start 2809 -logdir /tmp/rustorb-names   # first run
rustorb-names -logdir /tmp/rustorb-names               # later runs, same port and bindings
rustorb-names -memory -start 2809                      # no persistence
```

The root context is `corbaloc::localhost:2809/NameService`. Programs find it
with the standard ORB option, which every binding accepts:

```sh
-ORBInitRef NameService=corbaloc::localhost:2809/NameService
```

## 3. Write the IDL

[examples/idl/Echo.idl](../examples/idl/Echo.idl):

```idl
module Example {
  interface Echo {
    string echo(in string message);
    oneway void shutdown();
  };
};
```

The naming service's own IDL, `CosNaming.idl`, comes with rustorb-idl:
`rustorb-idl --idl-dir` prints its directory, which is also searched for
`#include "CosNaming.idl"`. Rust and Java have the CosNaming stubs built
in; Python and C++ generate them from that file.

## 4. Rust

Add the crates, and generate the code from `build.rs`
([examples/rust](../examples/rust/)):

```toml
[dependencies]
rustorb-core = "0.1"
rustorb-naming = "0.1"

[build-dependencies]
rustorb-codegen = "0.1"
```

```rust
// build.rs: runs rustorb-idl -brust into $OUT_DIR/echo.rs
fn main() {
    rustorb_codegen::Config::new().compile(&["idl/Echo.idl"]).unwrap();
}

// src/lib.rs
include!(concat!(env!("OUT_DIR"), "/echo.rs"));
```

The IDL module `Example` becomes the Rust module `example`, with the
reference type `Echo`, the servant trait `EchoServant` and its wrapper
`EchoPoa`. Operations return `Result<_, SystemException>` (or an error enum
for operations with `raises`).

```rust
struct EchoImpl { orb: Orb }

impl EchoServant for EchoImpl {
    fn echo(&self, message: &str) -> Result<String, SystemException> {
        Ok(message.to_owned())
    }
    fn shutdown(&self) -> Result<(), SystemException> {
        self.orb.shutdown(false)
    }
}

// server
let mut args: Vec<String> = std::env::args().skip(1).collect();
let orb = Orb::init(&mut args)?;                 // takes the -ORB options out of args
let poa = orb.root_poa()?;
poa.the_poa_manager().activate()?;
let (_, echo) = EchoPoa(EchoImpl { orb: orb.clone() }).activate(&poa)?;
let root = NamingContext::narrow(orb.resolve_initial_references("NameService")?)?
    .ok_or("not a NamingContext")?;
root.rebind(&to_name("Echo")?, echo.object())?;
orb.run();

// client
let root = NamingContextExt::narrow(orb.resolve_initial_references("NameService")?)?
    .ok_or("not a NamingContextExt")?;
let obj = root.resolve_str("Echo")?.ok_or("nil")?;
let echo = Echo::narrow(&obj)?.ok_or("not an Example::Echo")?;
println!("{}", echo.echo("hello from Rust")?);
```

The public API is synchronous; the ORB runs its own tokio runtime inside and
calls servants on its own threads (so a servant keeps its state in a
`Mutex` or the like). An async API is available too
([async-api.md](async-api.md)). The mapping is described in
[rust-mapping.md](rust-mapping.md).

## 5. Python

```sh
pip install rustorb
rustorb-idl -bpython -C gen Echo.idl "$(rustorb-idl --idl-dir)/CosNaming.idl"
export PYTHONPATH=gen
```

The generated code is the standard IDL to Python mapping, in the same
layout as omniidl -bpython: `Example` for the stubs and `Example__POA` for
the skeletons ([examples/python](../examples/python/)).

```python
from rustorb import CORBA, PortableServer
import CosNaming
import Example, Example__POA

class EchoImpl(Example__POA.Echo):
    def __init__(self, orb):
        self.orb = orb
    def echo(self, message):
        return message
    def shutdown(self):
        self.orb.shutdown(False)

# server
orb = CORBA.ORB_init(sys.argv)
poa = orb.resolve_initial_references("RootPOA")
poa._get_the_POAManager().activate()
echo = EchoImpl(orb)._this()
root = orb.resolve_initial_references("NameService")._narrow(CosNaming.NamingContext)
root.rebind([CosNaming.NameComponent("Echo", "")], echo)
orb.run()

# client
root = orb.resolve_initial_references("NameService")._narrow(CosNaming.NamingContextExt)
echo = root.resolve_str("Echo")._narrow(Example.Echo)
print(echo.echo("hello from Python"))
```

**Coming from omniORBpy**: code written for omniORBpy runs on rustorb with
`from rustorb import CORBA, PortableServer` in place of
`from omniORB import ...`. To run it unchanged (OpenRTM-aist-Python, rtshell),
install `rustorb-omniorb-compat` in place of omniORBpy: it provides the
`omniORB` package and the top-level `CORBA` and `CosNaming` modules
([omniorb-compat.md](omniorb-compat.md)). Stubs generated by omniidl
-bpython can be kept, but regenerating them with `rustorb-idl -bpython` is
recommended.

## 6. C++

The C++ binding is the IDL to C++11 mapping, compiled as C++17, on a static
library (`librustorb_ffi`). Take `rustorb-native-<version>-<platform>` from
the GitHub release, or install it from the source tree (cargo builds the
library):

```sh
cmake -S cpp -B build/cpp -DRUSTORB_BUILD_TESTS=OFF
cmake --build build/cpp && cmake --install build/cpp --prefix $HOME/rustorb
```

Then, in your project ([examples/cpp](../examples/cpp/)), with
`-DCMAKE_PREFIX_PATH=$HOME/rustorb` and `rustorb-idl` on `PATH`:

```cmake
find_package(rustorb 0.1 REQUIRED)
rustorb_idl_generate(SOURCES Echo.idl "${RUSTORB_IDL_DIR}/CosNaming.idl")
add_executable(server server.cc ${SOURCES})
target_include_directories(server PRIVATE "${CMAKE_CURRENT_BINARY_DIR}/idl")
target_link_libraries(server PRIVATE rustorb::rustorb)
```

```cpp
#include "CosNaming.hh"
#include "Echo.hh"

class EchoImpl : public CORBA::servant_traits<Example::Echo>::base_type {
 public:
  explicit EchoImpl(IDL::traits<CORBA::ORB>::ref_type orb) : orb_(std::move(orb)) {}
  std::string echo(const std::string& message) override { return message; }
  void shutdown() override { orb_->shutdown(false); }
 private:
  IDL::traits<CORBA::ORB>::ref_type orb_;
};

// server
auto orb = CORBA::ORB_init(argc, argv);
auto poa = IDL::traits<PortableServer::POA>::narrow(orb->resolve_initial_references("RootPOA"));
poa->the_POAManager()->activate();
auto echo = CORBA::make_reference<EchoImpl>(orb)->_this();
auto root = IDL::traits<CosNaming::NamingContext>::narrow(
    orb->resolve_initial_references("NameService"));
root->rebind(CosNaming::Name{CosNaming::NameComponent("Echo", "")}, echo);
orb->run();

// client
auto root = IDL::traits<CosNaming::NamingContextExt>::narrow(
    orb->resolve_initial_references("NameService"));
auto echo = IDL::traits<Example::Echo>::narrow(root->resolve_str("Echo"));
std::cout << echo->echo("hello from C++") << std::endl;
```

Without CMake: `rustorb-idl -bc++11 -C gen Echo.idl`, then compile with
`-std=c++17 -I<prefix>/include -Igen` and link `librustorb_ffi.a` with
`-lpthread -ldl -lm` (see [cpp-binding.md](cpp-binding.md)).

## 7. Java

The Java binding is the IDL to Java mapping (`org.omg.CORBA`,
`org.omg.PortableServer`, `org.omg.CosNaming`) on the same library, through
the FFM API, so it needs **Java 22 or later**. Take `rustorb-<version>.jar`
and the shared library (`lib/librustorb_ffi.so`, `lib/librustorb_ffi.dylib`
or `bin/rustorb_ffi.dll` in `rustorb-native`) from the GitHub release
([examples/java](../examples/java/)):

```sh
rustorb-idl -bjava -C gen Echo.idl
javac --release 22 -cp rustorb-0.1.1.jar -d classes Server.java Client.java $(find gen -name '*.java')
java --enable-native-access=ALL-UNNAMED -Drustorb.ffi.library=/path/to/librustorb_ffi.so \
     -cp rustorb-0.1.1.jar:classes Server -ORBInitRef NameService=corbaloc::localhost:2809/NameService
```

```java
static final class EchoImpl extends EchoPOA {
    private final ORB orb;
    EchoImpl(ORB orb) { this.orb = orb; }
    @Override public String echo(String message) { return message; }
    @Override public void shutdown() { orb.shutdown(false); }
}

// server
ORB orb = ORB.init(args, null);
POA poa = POAHelper.narrow(orb.resolve_initial_references("RootPOA"));
poa.the_POAManager().activate();
Echo echo = EchoHelper.narrow(poa.servant_to_reference(new EchoImpl(orb)));
NamingContext root = NamingContextHelper.narrow(orb.resolve_initial_references("NameService"));
root.rebind(new NameComponent[] {new NameComponent("Echo", "")}, echo);
orb.run();

// client
NamingContextExt root = NamingContextExtHelper.narrow(orb.resolve_initial_references("NameService"));
Echo echo = EchoHelper.narrow(root.resolve_str("Echo"));
System.out.println(echo.echo("hello from Java"));
```

Stubs generated by other IDL compilers (JacORB's, for example) work too, as
portable stubs ([java-binding.md](java-binding.md)).

## 8. Run them together

Start any server, then any client, with the same `-ORBInitRef`; the last
client can pass `--shutdown` to stop the server. Or let the script build
and run all sixteen pairs:

```sh
examples/run-all.sh                        # needs cargo, cmake, a C++ compiler, Java 22+ and Maven
EXAMPLES="rust python" examples/run-all.sh
```

## ORB options

Every binding takes the same `-ORB...` options (from the command line, or
`ORB.init`'s properties in Java), for example:

| Option | Meaning |
| --- | --- |
| `-ORBInitRef NAME=URL` | an initial reference (`corbaloc:`, `corbaname:` or an IOR) |
| `-ORBDefaultInitRef URL` | where other initial references are looked up |
| `-ORBendPoint giop:tcp:HOST:PORT` | where the server listens (default: any port) |
| `-ORBendPointPublish giop:tcp:HOST:PORT` | the address written into IORs |
| `-ORBmaxGIOPVersion 1.1` | the highest GIOP version to send |
| `-ORBclientCallTimeOutPeriod MS` | a timeout for every call ([timeouts.md](timeouts.md)) |
| `-ORBendPoint giop:ssl:HOST:PORT`, `-ORBsslCAFile`, `-ORBsslKeyFile` | SSLIOP ([ssliop.md](ssliop.md)) |

## What is not there

The CORBA Component Model, the Event and Notification services, the
Interface Repository, CSIv2 and the C++98 mapping are out of scope. IDL 4
`map` and `@optional` have no GIOP encoding and are rejected. The known
differences from other ORBs are in
[interop/known-failures.txt](../interop/known-failures.txt) and the
[changelog](../CHANGELOG.md).

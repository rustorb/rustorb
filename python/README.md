# rustorb

The Python binding of [rustorb](https://github.com/rustorb/rustorb), a CORBA
ORB with its core written in Rust. It follows the standard IDL to Python
mapping and the layout of omniidl -bpython, so code written for omniORBpy
needs only its imports changed (or none, with `rustorb-omniorb-compat`).

```sh
pip install rustorb rustorb-idl
rustorb-idl -bpython Echo.idl
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
echo = EchoImpl()._this()
print(echo.echo("hello"))
orb.destroy()
```

The wheels are abi3, for CPython 3.9 or later, with the ORB core compiled
in: GIOP 1.0–1.2, IIOP and SSLIOP, the POA, Any / TypeCode / DynAny,
valuetypes, DII / DSI and call timeouts. See the
[getting started guide](https://github.com/rustorb/rustorb/blob/main/docs/getting-started.md).

Licensed under MIT OR Apache-2.0.

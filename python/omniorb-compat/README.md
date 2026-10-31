# rustorb-omniorb-compat

Runs applications written for omniORBpy on
[rustorb](https://github.com/rustorb/rustorb): it provides the `omniORB`
package (`from omniORB import CORBA, PortableServer`), the top-level
`CORBA` / `PortableServer` modules and the `CosNaming` stubs, on the
`rustorb` package. OpenRTM-aist-Python and rtshell run on it.

```sh
pip install rustorb rustorb-omniorb-compat
```

Install it instead of omniORBpy, not next to it. The omniORBpy extensions
it covers, and the differences, are listed in
[docs/omniorb-compat.md](https://github.com/rustorb/rustorb/blob/main/docs/omniorb-compat.md).

Licensed under MIT OR Apache-2.0.

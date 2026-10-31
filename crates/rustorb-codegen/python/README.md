# rustorb-idl

The IDL compiler of [rustorb](https://github.com/rustorb/rustorb), a CORBA
ORB with its core written in Rust. It reads IDL with
[idl_parser](https://github.com/sugarsweetrobotics/idl_parser) and writes
stubs and skeletons for:

- `-bpython`: the standard IDL to Python mapping, in the layout of
  omniidl -bpython (for the `rustorb` package)
- `-brust`: Rust (for `rustorb-core`; or `rustorb-codegen` in `build.rs`)
- `-bc++11`: the IDL to C++11 mapping (for the rustorb C++ binding)
- `-bjava`: the IDL to Java mapping (for the rustorb Java binding)

```sh
pip install rustorb-idl
rustorb-idl -bpython -C gen Echo.idl
rustorb-idl -bc++11 -C gen Echo.idl "$(rustorb-idl --idl-dir)/CosNaming.idl"
```

The options follow omniidl (`-I`, `-D`, `-C`, `-Wb...`; see
`rustorb-idl --help`). `rustorb-idl --idl-dir` prints the directory of the
standard IDL files that come with it (CosNaming.idl), which is also searched
for `#include`.

Licensed under GPL-3.0-or-later. Code produced by the generator is not
covered by this license.

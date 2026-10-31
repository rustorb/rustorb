# rustorb-codegen

Runs the rustorb IDL compiler (`rustorb-idl -brust`) from a build script and
writes the Rust stubs and skeletons to `OUT_DIR`:

```rust
// build.rs
fn main() {
    rustorb_codegen::Config::new()
        .include("idl")
        .compile(&["idl/Example.idl"]) // $OUT_DIR/example.rs
        .unwrap();
}
```

and in the crate, `include!(concat!(env!("OUT_DIR"), "/example.rs"));`.
The generator itself is the `rustorb-idl` Python package (it needs
[idl_parser](https://github.com/sugarsweetrobotics/idl_parser)), which also
has the Python, C++11 and Java backends.

rustorb is a CORBA ORB written in Rust, with Python, C++ and Java bindings.
See the [repository](https://github.com/rustorb/rustorb) for the other crates,
the bindings and the documentation.

## License

GPL-3.0-or-later, because the generator builds on idl_parser. Code produced
by the generator is not covered by this license, and a build dependency on
this crate does not make the crate that uses it GPL.

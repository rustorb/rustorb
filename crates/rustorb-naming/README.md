# rustorb-naming

CosNaming (Naming Service 1.3) for rustorb: client stubs for
`NamingContext`, `NamingContextExt` and `BindingIterator`, a server with
pluggable persistence (an append-only log and checkpoints by default), and
the `rustorb-names` daemon, a replacement for omniNames.

```sh
cargo install rustorb-naming
rustorb-names -start 2809 -logdir /tmp/names
```

rustorb is a CORBA ORB written in Rust, with Python, C++ and Java bindings.
See the [repository](https://github.com/rustorb/rustorb) for the other crates,
the bindings and the documentation.

## License

Licensed under either of [Apache License, Version 2.0](https://github.com/rustorb/rustorb/blob/main/LICENSE-APACHE)
or [MIT license](https://github.com/rustorb/rustorb/blob/main/LICENSE-MIT) at your option.

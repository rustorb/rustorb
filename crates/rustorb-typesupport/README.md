# rustorb-typesupport

DDS type support for the Rust code that `rustorb-idl -brust -Wbdds` writes:
the traits the generated types implement, and what they share.

- [`TypeSupport`] reads and writes a value in Extended CDR (XCDR1 and XCDR2,
  DDS-XTypes 1.3 clause 7.4), with the headers of its extensibility kind
  (`@final`, `@appendable`, `@mutable`) and of `@optional` members.
- [`TopicType`] is a type a DataWriter publishes: its type name, its
  encapsulated payload (the RTPS SerializedPayload) and its key hash
  (DDS-XTypes 7.6.8).
- Descriptors for the IDL types that share a Rust type: `char` and `wchar`
  are both `char`, `string`, `wstring` and their bounded forms are all
  `String`, sequences are `Vec`.

The crate depends only on `rustorb-cdr`. A DDS implementation (such as
[rustorb-dds](https://github.com/rustorb/rustorb-dds)) uses it, so that code
generated once works with any of them, and so that the same Rust type can
have both the CORBA and the DDS impls.

rustorb is a CORBA ORB written in Rust, with Python, C++ and Java bindings.
See the [repository](https://github.com/rustorb/rustorb) for the other crates,
the bindings and the documentation.

## License

Licensed under either of [Apache License, Version 2.0](https://github.com/rustorb/rustorb/blob/main/LICENSE-APACHE)
or [MIT license](https://github.com/rustorb/rustorb/blob/main/LICENSE-MIT) at your option.

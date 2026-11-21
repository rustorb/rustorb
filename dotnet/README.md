# Rustorb for .NET

The IDL4 to C# mapping (OMG formal/21-07-01) on rustorb: the `Corba`,
`PortableServer`, `CosNaming` and `Omg.Types` APIs over the C ABI of
`librustorb_ffi` (`crates/rustorb-ffi`), for .NET 8 or later. Stubs and
skeletons come from `rustorb-idl -bcsharp`.

```sh
cargo build --release -p rustorb-ffi
dotnet build -c Release dotnet/src/Rustorb.Corba
dotnet test dotnet/tests/Rustorb.Tests     # runs cargo and rustorb-idl itself
```

| Path | Contents |
| --- | --- |
| `src/Rustorb.Corba` | the runtime (NuGet package `Rustorb`) |
| `tests/Rustorb.Tests` | xUnit tests on the code generated from `interop/idl` and `cpp/tests/idl/CxxMapping.idl` |
| `tools/gen-pinvoke.py` | writes `src/Rustorb.Corba/Native/Ffi.g.cs` from `rustorb.h` (`--check` in CI) |
| `generate-stubs.sh` | regenerates the CosNaming stubs of the runtime |

The library is found through `RUSTORB_FFI_LIBRARY`, then next to the
assembly (`runtimes/<rid>/native` in the package), then where the OS loader
looks. See [docs/csharp-binding.md](../docs/csharp-binding.md) (Japanese).

Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

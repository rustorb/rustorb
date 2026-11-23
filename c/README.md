# Rustorb for C

The OMG C Language Mapping 1.0 (formal/99-07-35) on rustorb: `librustorb_c`,
the `CORBA_*` and `PortableServer_*` APIs in C99 over the C ABI of
`librustorb_ffi` (`crates/rustorb-ffi`). Stubs and skeletons come from
`rustorb-idl -bc`.

```sh
pip install -r ../tools/rustorb-idl.txt
cmake -S c -B build/c          # from the repository root; runs cargo too
cmake --build build/c
ctest --test-dir build/c       # -DRUSTORB_SANITIZE=address for ASan/LSan
```

| Path | Contents |
| --- | --- |
| `include/rustorb/orb.h` | `CORBA_*`: types, `CORBA_Environment`, `CORBA_free`, sequences, `CORBA_any`, TypeCode, Object, ORB, Policy |
| `include/rustorb/poa.h` | `PortableServer_*`: ServantBase, POA, POAManager, Current, policies |
| `include/rustorb/runtime.h` | what generated code uses (type and operation descriptors); not for applications |
| `include/rustorb/CosNaming.h`, `src/CosNaming.c` | CosNaming, generated (`generate-stubs.sh`) |
| `src/` | the runtime |
| `tests/` | the interop cases against an in-process servant, and the mapping test on `cpp/tests/idl/CxxMapping.idl` |

Link `librustorb_c.a` and `librustorb_ffi.a` with the system libraries
(`-lpthread -ldl -lm` on Linux; `-framework CoreFoundation -framework
Security` on macOS). With MSVC on Windows, link `rustorb_c.lib` and
`rustorb_ffi.lib` with `ws2_32.lib userenv.lib ntdll.lib bcrypt.lib`, and
compile with `/MD` (the C runtime of `rustorb_ffi.lib`). See [docs/c-binding.md](../docs/c-binding.md)
(Japanese).

Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

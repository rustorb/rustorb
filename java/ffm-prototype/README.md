# FFM prototype

A minimal Java binding on the FFM API (`java.lang.foreign`, Java 22 or
later) that calls the C ABI of `rustorb-ffi` directly, and the same
benchmark on it and on the JNI binding (`java/`). Not built by CI or Maven.

```sh
IDL_PARSER_DIR=path/to/idl_parser java/ffm-prototype/run-bench.sh
```

The results and the comparison are in [docs/java-ffm.md](../../docs/java-ffm.md) (#51).

# Rustorb for Ruby

The OMG Ruby Language Mapping (RCLM 1.2, formal/2013-04-05) on rustorb: the
`rustorb` gem, with `CORBA`, `PortableServer` and `CosNaming` in Ruby over
the C ABI of `librustorb_ffi` (`crates/rustorb-ffi`), through the
`rustorb_ext` extension. Stubs and skeletons come from `rustorb-idl -bruby`
(`FooC.rb` and `FooS.rb`).

```sh
pip install -r ../tools/rustorb-idl.txt
rake            # cargo build of librustorb_ffi, the extension, the tests
rake compile    # only the extension, into lib/rustorb
```

```ruby
require "corba"
require "HelloS"

class HelloImpl < POA::Example::Hello
  def say(message) = message
end

orb = CORBA.ORB_init(ARGV)
poa = orb.resolve_initial_references("RootPOA")
poa.the_POAManager.activate
puts orb.object_to_string(HelloImpl.new._this)
orb.run
```

| Path | Contents |
| --- | --- |
| `ext/rustorb_ext/` | the extension: the ABI handles, the CDR streams, the GVL and the threads that serve requests |
| `lib/rustorb/` | the mapping: exceptions, TypeCode, marshalling, Any, Object and stubs, ORB, PortableServer, DynAny, the DII and DSI, valuetypes, AMI, Portable Interceptors |
| `lib/CosNamingC.rb`, `lib/CosNamingS.rb` | CosNaming, generated (`rake cos_naming`) |
| `test/` | the runtime, the mapping test on `cpp/tests/idl/CxxMapping.idl` and the interop cases against an in-process servant |

Ruby 3.0 or later on Linux and macOS. The extension links
`librustorb_ffi.a` statically (`RUSTORB_FFI_LIBRARY`, default the
checkout's `target/release/librustorb_ffi.a`). See
[docs/ruby-binding.md](../docs/ruby-binding.md) (Japanese).

Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

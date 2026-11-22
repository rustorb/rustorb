# Builds rustorb_ext against librustorb_ffi (crates/rustorb-ffi), linked
# statically. RUSTORB_FFI_LIBRARY names the library (default: the release
# build of the checkout this gem is in, target/release/librustorb_ffi.a);
# RUSTORB_FFI_INCLUDE the directory of rustorb/rustorb.h.
require "mkmf"

root = File.expand_path("../../..", __dir__)
lib = ENV["RUSTORB_FFI_LIBRARY"] || File.join(root, "target", "release", "librustorb_ffi.a")
inc = ENV["RUSTORB_FFI_INCLUDE"] || File.join(root, "crates", "rustorb-ffi", "include")
abort "librustorb_ffi not found: #{lib} (cargo build --release -p rustorb-ffi)" unless File.exist?(lib)

$INCFLAGS << " -I#{inc}"
$CFLAGS << " -std=c99 -Wall -Wextra -Wno-unused-parameter -D_POSIX_C_SOURCE=200809L"
$LOCAL_LIBS << " #{lib}"
if RUBY_PLATFORM =~ /darwin/
  $LDFLAGS << " -framework CoreFoundation -framework Security"
else
  $LIBS << " -lpthread -ldl -lm"
end
create_makefile("rustorb/rustorb_ext")

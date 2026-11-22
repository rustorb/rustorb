# Builds rustorb_ext against librustorb_ffi (crates/rustorb-ffi), linked
# statically. RUSTORB_FFI_LIBRARY names the library, RUSTORB_FFI_INCLUDE
# the directory of rustorb/rustorb.h. By default: the ones the released gem
# carries for this platform (vendor/<platform>/librustorb_ffi.a and
# vendor/include), else those of the checkout this gem is in
# (target/release/librustorb_ffi.a after cargo build --release -p
# rustorb-ffi).
require "mkmf"

gem_root = File.expand_path("../..", __dir__)
root = File.expand_path("..", gem_root)
cpu = RbConfig::CONFIG["host_cpu"].sub(/\Aarm64\z/, "aarch64")
os = RbConfig::CONFIG["host_os"] =~ /darwin/ ? "macos" : "linux"
vendor = File.join(gem_root, "vendor")
vendored = File.join(vendor, "#{os}-#{cpu}", "librustorb_ffi.a")
lib = ENV["RUSTORB_FFI_LIBRARY"] ||
      (File.exist?(vendored) ? vendored : File.join(root, "target", "release", "librustorb_ffi.a"))
inc = ENV["RUSTORB_FFI_INCLUDE"] ||
      (File.exist?(File.join(vendor, "include", "rustorb", "rustorb.h")) ? File.join(vendor, "include") :
       File.join(root, "crates", "rustorb-ffi", "include"))
unless File.exist?(lib)
  abort "librustorb_ffi not found for #{os}-#{cpu}: #{lib} (set RUSTORB_FFI_LIBRARY, or cargo build --release -p rustorb-ffi)"
end

$INCFLAGS << " -I#{inc}"
$CFLAGS << " -std=c99 -Wall -Wextra -Wno-unused-parameter -D_POSIX_C_SOURCE=200809L"
$LOCAL_LIBS << " #{lib}"
if RUBY_PLATFORM =~ /darwin/
  $LDFLAGS << " -framework CoreFoundation -framework Security"
else
  $LIBS << " -lpthread -ldl -lm"
end
create_makefile("rustorb/rustorb_ext")

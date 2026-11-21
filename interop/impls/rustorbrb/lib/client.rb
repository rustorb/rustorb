# frozen_string_literal: true

# The rustorb Ruby InteropTest client (rustorbrb): resolves Echo from the
# naming service, runs every case (lib/cases.rb) and exits non-zero if one
# fails.
#
# Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

require_relative "common"
require "cases"

opts, orb_args = Interop.parse_args(ARGV, ["--shutdown-server"])
abort "rustorbrb runs the basic suite only (rustorb#188)" unless opts[:suite] == "basic"
orb = Interop.orb_init(opts, orb_args)
poa = Interop.persistent_poa(orb, "InteropTestClient")
root = Interop.root_context(orb, opts[:timeout])
name = Interop.to_name(opts[:name])
obj = Interop.retry_for("the Echo server", opts[:timeout]) { root.resolve(name) }
echo = InteropTest::Echo._narrow(obj)
Interop.retry_for("the Echo server", opts[:timeout]) { echo._non_existent? }

r = Runner.new
Cases.run(r, echo, poa)
echo.shutdown if opts[:shutdown_server]
r.summary
orb.destroy
exit(r.failed.empty? ? 0 : 1)

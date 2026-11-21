# frozen_string_literal: true

# The rustorb Ruby InteropTest client (rustorbrb): resolves Echo from the
# naming service, runs every case (lib/cases.rb) and exits non-zero if one
# fails.
#
# Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

require_relative "common"
require "cases"

opts, orb_args = Interop.parse_args(ARGV, ["--shutdown-server"])
SUITES = {
  "value" => ["valuetest", "ValueCases", "ValueTest::ValueEcho", :register_factories],
  "abstract" => ["abstracttest", "AbstractCases", "AbstractTest::AbstractEcho", :register],
  "custom" => ["customtest", "CustomCases", "CustomTest::CustomEcho", :register]
}.freeze
abort "unknown suite #{opts[:suite]}" unless %w[basic any].include?(opts[:suite]) || SUITES.key?(opts[:suite])
orb = Interop.orb_init(opts, orb_args)
poa = Interop.persistent_poa(orb, "InteropTestClient")
root = Interop.root_context(orb, opts[:timeout])
name = Interop.to_name(opts[:name])
r = Runner.new
if SUITES.key?(opts[:suite])
  # The valuetype suites against the echo object at the name, which also
  # stops the server.
  file, cases, iface, register = SUITES[opts[:suite]]
  require file
  mod = Object.const_get(cases)
  mod.public_send(register, orb)
  obj = Interop.retry_for("the #{iface} server", opts[:timeout]) { root.resolve(name) }
  echo = Object.const_get(iface)._narrow(obj)
  Interop.retry_for("the #{iface} server", opts[:timeout]) { echo._non_existent? }
  mod.run(r, echo)
  echo.shutdown if opts[:shutdown_server]
elsif opts[:suite] == "any"
  # The stage 2 suite against the AnyEcho object at the name; the server's
  # Echo object is at the name + "-Echo".
  require "anytest"
  require "AnyTestHiddenC"
  obj = Interop.retry_for("the AnyEcho server", opts[:timeout]) { root.resolve(name) }
  any_echo = AnyTest::AnyEcho._narrow(obj)
  Interop.retry_for("the AnyEcho server", opts[:timeout]) { any_echo._non_existent? }
  AnyCases.run(r, any_echo)
  if opts[:shutdown_server]
    echo_name = Interop.to_name("#{opts[:name]}-Echo")
    InteropTest::Echo._narrow(root.resolve(echo_name)).shutdown
  end
else
  obj = Interop.retry_for("the Echo server", opts[:timeout]) { root.resolve(name) }
  echo = InteropTest::Echo._narrow(obj)
  Interop.retry_for("the Echo server", opts[:timeout]) { echo._non_existent? }
  Cases.run(r, echo, poa)
  echo.shutdown if opts[:shutdown_server]
end
r.summary
orb.destroy
exit(r.failed.empty? ? 0 : 1)

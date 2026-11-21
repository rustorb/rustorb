# frozen_string_literal: true

# The rustorb Ruby InteropTest::Echo server (rustorbrb): binds Echo in the
# naming service and serves until Echo::shutdown.
#
# Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

require_relative "common"
require "echo_servant"

opts, orb_args = Interop.parse_args(ARGV)
abort "rustorbrb runs the basic suite only (rustorb#188)" unless opts[:suite] == "basic"
orb = Interop.orb_init(opts, orb_args)
poa = Interop.persistent_poa(orb, "InteropTest")
poa.activate_object_with_id("Echo".b, EchoServant.new(orb))
obj = poa.id_to_reference("Echo".b)

root = Interop.root_context(orb, opts[:timeout])
name = Interop.to_name(opts[:name])
ctx = root
name[0...-1].each do |c|
  ctx = begin
    ctx.bind_new_context([c])
  rescue CosNaming::NamingContext::AlreadyBound
    CosNaming::NamingContext._narrow(ctx.resolve([c]))
  end
end
ctx.rebind(name[-1..], obj)
puts "server ready: #{opts[:name]}"
$stdout.flush

orb.run
orb.destroy
puts "server stopped"
$stdout.flush

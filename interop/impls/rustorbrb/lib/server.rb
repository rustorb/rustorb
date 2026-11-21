# frozen_string_literal: true

# The rustorb Ruby InteropTest::Echo server (rustorbrb): binds Echo in the
# naming service and serves until Echo::shutdown.
#
# Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

require_relative "common"
require "echo_servant"

opts, orb_args = Interop.parse_args(ARGV)
abort "rustorbrb runs the basic and any suites (rustorb#188)" unless %w[basic any].include?(opts[:suite])
orb = Interop.orb_init(opts, orb_args)
any_suite = opts[:suite] == "any"
poa = Interop.persistent_poa(orb, any_suite ? "AnyTest" : "InteropTest")
poa.activate_object_with_id("Echo".b, EchoServant.new(orb))
obj = poa.id_to_reference("Echo".b)

def bind(root, path, obj)
  name = Interop.to_name(path)
  ctx = root
  name[0...-1].each do |c|
    ctx = begin
      ctx.bind_new_context([c])
    rescue CosNaming::NamingContext::AlreadyBound
      CosNaming::NamingContext._narrow(ctx.resolve([c]))
    end
  end
  ctx.rebind(name[-1..], obj)
end

root = Interop.root_context(orb, opts[:timeout])
if any_suite
  # The stage 2 suite: AnyEcho at the name, with the Echo object next to it
  # (at the name + "-Echo") for the client to shut the server down.
  require "anytest"
  poa.activate_object_with_id("AnyEcho".b, AnyEchoServant.new)
  bind(root, opts[:name], poa.id_to_reference("AnyEcho".b))
  bind(root, "#{opts[:name]}-Echo", obj)
else
  bind(root, opts[:name], obj)
end
puts "server ready: #{opts[:name]}"
$stdout.flush

orb.run
orb.destroy
puts "server stopped"
$stdout.flush

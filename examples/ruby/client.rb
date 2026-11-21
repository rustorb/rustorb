# frozen_string_literal: true

# Looks "Echo" up in the naming service and calls it.
#
#   ruby -Igen client.rb -ORBInitRef NameService=corbaloc::localhost:2809/NameService [--shutdown]

require "corba"
require "CosNamingC"
require "EchoC"

orb = CORBA.ORB_init(ARGV)
shutdown = ARGV.include?("--shutdown")

root = CosNaming::NamingContextExt._narrow(orb.resolve_initial_references("NameService"))
echo = Example::Echo._narrow(root.resolve_str("Echo"))

puts "client: #{echo.echo('hello from Ruby')}"
echo.shutdown if shutdown
orb.destroy

# frozen_string_literal: true

# Serves Example::Echo and binds it in the naming service as "Echo".
#
#   rustorb-idl -bruby -C gen ../idl/Echo.idl
#   ruby -Igen server.rb -ORBInitRef NameService=corbaloc::localhost:2809/NameService

require "corba"
require "CosNamingC"
require "EchoS"

class EchoImpl < POA::Example::Echo
  def initialize(orb)
    super()
    @orb = orb
  end

  def echo(message)
    puts "server: #{message}"
    $stdout.flush
    message
  end

  def shutdown
    @orb.shutdown(false)
  end
end

orb = CORBA.ORB_init(ARGV)
poa = orb.resolve_initial_references("RootPOA")
poa.the_POAManager.activate
echo = EchoImpl.new(orb)._this

root = CosNaming::NamingContext._narrow(orb.resolve_initial_references("NameService"))
root.rebind([CosNaming::NameComponent.new("Echo", "")], echo)
puts "server: ready"
$stdout.flush

orb.run
orb.destroy

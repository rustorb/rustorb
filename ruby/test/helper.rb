# frozen_string_literal: true

# What the tests share: the load path (the built extension, the runtime,
# the code rustorb-idl generated into test/gen by `rake gen`) and one ORB
# with its RootPOA active.

$LOAD_PATH.unshift(File.expand_path("gen", __dir__))
$LOAD_PATH.unshift(File.expand_path("../../interop/impls/rustorbrb/lib", __dir__))
$LOAD_PATH.unshift(File.expand_path("../lib", __dir__))

# minitest/autorun first, so that the tests (which it runs at exit) run
# before anything corba registers at exit: the dispatcher threads must still
# serve them, whatever the order of the requires (Ruby 3.0 hung here when
# rustorb stopped its dispatchers from an end proc).
require "minitest/autorun"
require "corba"

module TestORB
  def self.orb
    @orb ||= CORBA.ORB_init([]).tap do |orb|
      orb.resolve_initial_references("RootPOA").the_POAManager.activate
    end
  end

  def self.root_poa
    @root_poa ||= orb.resolve_initial_references("RootPOA")
  end

  # A reference to servant (activated in the RootPOA) through its IOR, so
  # that calls go through the ORB as from another process.
  def self.ref(servant, mod)
    ior = orb.object_to_string(root_poa.servant_to_reference(servant))
    mod._narrow(orb.string_to_object(ior))
  end
end

# frozen_string_literal: true

# rustorb (Ruby) server for the POA and thread model probes
# (PoaProbe.idl), the same as impls/rustorbpy/poa_probe_server.py on the
# rustorb gem.
#
#   poa-probe-server --host HOST --port N --ior-file FILE
#
# Publishes Target objects in POAs with different policies and writes the
# IOR of the Control object to FILE. scripts/poa-probe drives it; see
# run-poa-probe.sh. Threads are those rustorb runs the upcalls on
# (Rustorb::Native.upcall_thread), not the Ruby dispatcher threads.
#
# Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

here = File.expand_path("..", __dir__)
$LOAD_PATH.unshift(File.join(here, "gen"), File.join(here, "rt"))
require "corba"
require "PoaProbeS"

START = Process.clock_gettime(Process::CLOCK_MONOTONIC)
TARGET_ID = "IDL:rustorb.org/PoaProbe/Target:1.0"

def now = Process.clock_gettime(Process::CLOCK_MONOTONIC) - START

# Short, stable labels for threads in the order they are first seen.
module Threads
  @lock = Mutex.new
  @labels = {}

  def self.label
    t = Rustorb::Native.upcall_thread
    return "main" if t == Rustorb::Native.main_thread

    @lock.synchronize { @labels[t] ||= "T#{@labels.size + 1}" }
  end
end

EVENTS = []
EVENTS_LOCK = Mutex.new

def event(text) = EVENTS_LOCK.synchronize { EVENTS << text }

def poa_path(poa)
  names = []
  until poa.the_parent.nil?
    names << poa.the_name
    poa = poa.the_parent
  end
  names.empty? ? "RootPOA" : names.reverse.join("/")
end

def latin1(oid) = oid.b.force_encoding(Encoding::ISO_8859_1).encode(Encoding::UTF_8)

class TargetServant < POA::PoaProbe::Target
  def initialize(current)
    super()
    @current = current
  end

  def work(ms)
    start = now
    thread = Threads.label
    oid = latin1(@current.get_object_id)
    poa = poa_path(@current.get_POA)
    sleep(ms / 1000.0) if ms.positive?
    PoaProbe::CallInfo.new(thread, oid, poa, start, now)
  end

  def post(seq, ms)
    sleep(ms / 1000.0) if ms.positive?
    event("post #{seq}")
  end

  def relay(other, ms)
    outer = Threads.label
    other._call_timeout = 3.0
    [other.work(ms), outer]
  end
end

class Activator < PortableServer::ServantActivator
  def initialize(servant, forward)
    super()
    @servant = servant
    @forward = forward
  end

  def incarnate(oid, _adapter)
    name = latin1(oid)
    event("incarnate #{name}")
    raise CORBA::OBJECT_NOT_EXIST.new("", 0, CORBA::COMPLETED_NO) if name == "bad"
    raise PortableServer::ForwardRequest.new(@forward) if name == "fwd"

    @servant
  end

  def etherealize(oid, _adapter, _servant, _cleanup_in_progress, _remaining_activations)
    event("etherealize #{latin1(oid)}")
  end
end

class Locator < PortableServer::ServantLocator
  def initialize(servant, forward)
    super()
    @servant = servant
    @forward = forward
  end

  def preinvoke(oid, _adapter, operation)
    name = latin1(oid)
    event("preinvoke #{name} #{operation}")
    raise CORBA::OBJECT_NOT_EXIST.new("", 0, CORBA::COMPLETED_NO) if name == "bad"
    raise PortableServer::ForwardRequest.new(@forward) if name == "fwd"

    [@servant, nil]
  end

  def postinvoke(oid, _adapter, operation, _cookie, _servant)
    event("postinvoke #{latin1(oid)} #{operation}")
  end
end

# Creates POAs named lazy* on demand, with a default servant.
class LazyActivator < PortableServer::AdapterActivator
  def initialize(servant)
    super()
    @servant = servant
  end

  def unknown_adapter(parent, name)
    event("unknown_adapter #{name}")
    return false unless name.start_with?("lazy")

    poa = parent.create_POA(name, parent.the_POAManager, [
      parent.create_lifespan_policy(PortableServer::PERSISTENT),
      parent.create_id_assignment_policy(PortableServer::USER_ID),
      parent.create_servant_retention_policy(PortableServer::NON_RETAIN),
      parent.create_request_processing_policy(PortableServer::USE_DEFAULT_SERVANT),
      parent.create_id_uniqueness_policy(PortableServer::MULTIPLE_ID)
    ])
    poa.set_servant(@servant)
    true
  end
end

class ControlServant < POA::PoaProbe::Control
  def initialize(orb, root, managed_manager, current, system_refs)
    super()
    @orb = orb
    @root = root
    @managed = managed_manager
    @current = current
    @system_refs = system_refs
  end

  def find(path)
    poa = @root
    path.split("/").reject { |p| p.empty? || p == "RootPOA" }.each { |n| poa = poa.find_POA(n, true) }
    poa
  rescue PortableServer::POA::AdapterNonExistent
    raise PoaProbe::Failed.new("no POA #{path}")
  end

  def oid_of(s) = s.encode(Encoding::ISO_8859_1).b

  def reference(poa, oid)
    return @orb.object_to_string(@system_refs[poa]) if @system_refs.key?(poa)

    @orb.object_to_string(find(poa).create_reference_with_id(oid_of(oid), TARGET_ID))
  end

  def set_state(state)
    case state
    when "active" then @managed.activate
    when "holding" then @managed.hold_requests(false)
    when "discarding" then @managed.discard_requests(false)
    when "inactive" then @managed.deactivate(false, false)
    else raise PoaProbe::Failed.new("unknown state #{state}")
    end
  rescue PortableServer::POAManager::AdapterInactive
    raise PoaProbe::Failed.new("AdapterInactive")
  end

  def deactivate_object(poa, oid)
    find(poa).deactivate_object(oid_of(oid))
  rescue PortableServer::POA::ObjectNotActive
    raise PoaProbe::Failed.new("ObjectNotActive")
  end

  def activate_object(poa, oid)
    find(poa).activate_object_with_id(oid_of(oid), TargetServant.new(@current))
  rescue PortableServer::POA::ObjectAlreadyActive, PortableServer::POA::ServantAlreadyActive => e
    raise PoaProbe::Failed.new(e.class.name.split("::").last)
  end

  def destroy_poa(poa) = find(poa).destroy(true, false)

  def events
    EVENTS_LOCK.synchronize do
      r = EVENTS.dup
      EVENTS.clear
      r
    end
  end

  def shutdown_inside(wait_for_completion)
    @orb.shutdown(wait_for_completion)
    ""
  rescue CORBA::SystemException => e
    format("%s minor 0x%08x", e._id, e.minor)
  end

  def shutdown_later(delay_ms)
    Thread.new do
      sleep(delay_ms / 1000.0)
      @orb.shutdown(false)
    end
    nil
  end
end

host = ""
port = 0
ior_file = nil
orb_args = []
args = ARGV.dup
until args.empty?
  a = args.shift
  case a
  when "--host" then host = args.shift
  when "--port" then port = Integer(args.shift)
  when "--ior-file" then ior_file = args.shift
  else orb_args << a
  end
end
abort "--ior-file is required" unless ior_file

orb_args += ["-ORBendPoint", "giop:tcp:#{host}:#{port}"] if port.positive?
orb = CORBA.ORB_init(orb_args)
root = orb.resolve_initial_references("RootPOA")
current = orb.resolve_initial_references("POACurrent")
root_manager = root.the_POAManager
root_manager.activate
servant = TargetServant.new(current)

policies = lambda do |*extra, lifespan: PortableServer::PERSISTENT, ids: PortableServer::USER_ID|
  [root.create_lifespan_policy(lifespan), root.create_id_assignment_policy(ids)] + extra
end
create = ->(parent, name, pol, manager = nil) { parent.create_POA(name, manager || root_manager, pol) }

system_refs = {}
# RootPOA: TRANSIENT, SYSTEM_ID, IMPLICIT_ACTIVATION.
system_refs["RootPOA"] = servant._this

persistent_user = create.call(root, "persistent_user", policies.call)
persistent_user.activate_object_with_id("obj".b, servant)
persistent_user.activate_object_with_id("victim".b, TargetServant.new(current))
nested = create.call(persistent_user, "nested", policies.call)
nested.activate_object_with_id("obj".b, servant)

persistent_system = create.call(root, "persistent_system", policies.call(ids: PortableServer::SYSTEM_ID))
oid = persistent_system.activate_object(TargetServant.new(current))
system_refs["persistent_system"] = persistent_system.id_to_reference(oid)

transient_user = create.call(root, "transient_user", policies.call(lifespan: PortableServer::TRANSIENT))
transient_user.activate_object_with_id("obj".b, servant)

transient_system = create.call(root, "transient_system",
                               policies.call(lifespan: PortableServer::TRANSIENT, ids: PortableServer::SYSTEM_ID))
oid = transient_system.activate_object(TargetServant.new(current))
system_refs["transient_system"] = transient_system.id_to_reference(oid)

single = root.create_thread_policy(PortableServer::SINGLE_THREAD_MODEL)
%w[single single2].each do |name|
  poa = create.call(root, name, policies.call(single))
  poa.activate_object_with_id("a".b, servant)
  poa.activate_object_with_id("b".b, TargetServant.new(current))
end

main_poa = create.call(root, "main", policies.call(root.create_thread_policy(PortableServer::MAIN_THREAD_MODEL)))
main_poa.activate_object_with_id("a".b, servant)
main_poa.activate_object_with_id("b".b, TargetServant.new(current))

managed = root.create_POA("managed", nil, policies.call)
managed_manager = managed.the_POAManager
managed.activate_object_with_id("obj".b, servant)

doomed = create.call(root, "doomed", policies.call)
doomed.activate_object_with_id("obj".b, servant)

forward = persistent_user.id_to_reference("obj".b)
activator = create.call(root, "activator", policies.call(
  root.create_request_processing_policy(PortableServer::USE_SERVANT_MANAGER)
))
activator.set_servant_manager(Activator.new(servant, forward))
locator = create.call(root, "locator", policies.call(
  root.create_request_processing_policy(PortableServer::USE_SERVANT_MANAGER),
  root.create_servant_retention_policy(PortableServer::NON_RETAIN)
))
locator.set_servant_manager(Locator.new(servant, forward))

default = create.call(root, "default", policies.call(
  root.create_request_processing_policy(PortableServer::USE_DEFAULT_SERVANT),
  root.create_servant_retention_policy(PortableServer::NON_RETAIN),
  root.create_id_uniqueness_policy(PortableServer::MULTIPLE_ID)
))
default.set_servant(servant)

root.the_activator = LazyActivator.new(servant)

# The Control object has its own POAManager, so that it keeps working
# whatever the probe does to the others.
control_poa = root.create_POA("control", nil, policies.call)
control = ControlServant.new(orb, root, managed_manager, current, system_refs)
control_poa.activate_object_with_id("Control".b, control)
control_poa.the_POAManager.activate
managed_manager.activate

File.write("#{ior_file}.tmp", "#{orb.object_to_string(control_poa.id_to_reference('Control'.b))}\n")
File.rename("#{ior_file}.tmp", ior_file)
puts "server ready"
$stdout.flush

orb.run
puts "server stopped"
$stdout.flush
orb.destroy

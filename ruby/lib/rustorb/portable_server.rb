# frozen_string_literal: true

# PortableServer: the POA, its POAManager, the policies, Current and the
# servant base class. Object ids are binary Strings.
#
# Requests reach Ruby servants on a pool of Ruby threads (the
# dispatchers), which grows by one whenever every dispatcher is busy.
#
# Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

module Rustorb
  module Native
    def self.spawn_dispatcher
      t = Thread.new { Native.dispatcher_loop }
      t.name = "rustorb-dispatcher" if t.respond_to?(:name=)
      t.report_on_exception = true
      t
    end
  end
end

module PortableServer
  @dispatch_lock = Mutex.new

  # Starts dispatchers until n are running.
  def self.ensure_dispatchers(n = 2)
    @dispatch_lock.synchronize do
      return if Rustorb::Native.dispatchers >= n

      (n - Rustorb::Native.dispatchers).times { Rustorb::Native.spawn_dispatcher }
      Thread.pass until Rustorb::Native.dispatchers >= n
    end
  end

  # --- policies ---

  THREAD_POLICY_ID = 16
  LIFESPAN_POLICY_ID = 17
  ID_UNIQUENESS_POLICY_ID = 18
  ID_ASSIGNMENT_POLICY_ID = 19
  IMPLICIT_ACTIVATION_POLICY_ID = 20
  SERVANT_RETENTION_POLICY_ID = 21
  REQUEST_PROCESSING_POLICY_ID = 22

  ORB_CTRL_MODEL = 0
  SINGLE_THREAD_MODEL = 1
  MAIN_THREAD_MODEL = 2
  TRANSIENT = 0
  PERSISTENT = 1
  UNIQUE_ID = 0
  MULTIPLE_ID = 1
  USER_ID = 0
  SYSTEM_ID = 1
  IMPLICIT_ACTIVATION = 0
  NO_IMPLICIT_ACTIVATION = 1
  RETAIN = 0
  NON_RETAIN = 1
  USE_ACTIVE_OBJECT_MAP_ONLY = 0
  USE_DEFAULT_SERVANT = 1
  USE_SERVANT_MANAGER = 2

  # A POA policy: its type and value (a local object).
  class Policy
    attr_reader :policy_type, :value

    def initialize(policy_type, value)
      @policy_type = policy_type
      @value = value
    end

    def copy
      self.class.new(@policy_type, @value)
    end

    def destroy; end

    def ==(other)
      other.is_a?(Policy) && other.policy_type == @policy_type && other.value == @value
    end
  end

  class ThreadPolicy < Policy; end
  class LifespanPolicy < Policy; end
  class IdUniquenessPolicy < Policy; end
  class IdAssignmentPolicy < Policy; end
  class ImplicitActivationPolicy < Policy; end
  class ServantRetentionPolicy < Policy; end
  class RequestProcessingPolicy < Policy; end

  POLICY_CLASSES = [ThreadPolicy, LifespanPolicy, IdUniquenessPolicy, IdAssignmentPolicy,
                    ImplicitActivationPolicy, ServantRetentionPolicy, RequestProcessingPolicy].freeze

  # --- exceptions of the POA, the POAManager and Current ---

  def self.local_exception(container, name, code, members = [])
    klass = Class.new(CORBA::UserException)
    container.const_set(name, klass)
    klass.const_set(:ID, "IDL:omg.org/#{container.name.gsub('::', '/')}/#{name}:1.0")
    klass.const_set(:MEMBERS, members.freeze)
    members.each { |m| klass.attr_accessor(m) }
    Rustorb::Native::LOCAL_EXCEPTIONS[code] = members.empty? ? ->(_) { klass.new } : ->(i) { klass.new(i) }
    klass
  end

  class POA
    PortableServer.local_exception(self, :AdapterAlreadyExists, 1)
    PortableServer.local_exception(self, :AdapterNonExistent, 2)
    PortableServer.local_exception(self, :InvalidPolicy, 3, %w[index])
    PortableServer.local_exception(self, :ServantAlreadyActive, 4)
    PortableServer.local_exception(self, :ObjectAlreadyActive, 5)
    PortableServer.local_exception(self, :ServantNotActive, 6)
    PortableServer.local_exception(self, :ObjectNotActive, 7)
    PortableServer.local_exception(self, :WrongPolicy, 8)
    PortableServer.local_exception(self, :WrongAdapter, 9)
    PortableServer.local_exception(self, :NoServant, 11)

    def self._wrap(native)
      native && new(native)
    end

    def self._narrow(obj)
      return obj if obj.nil? || obj.is_a?(POA)

      raise CORBA::BAD_PARAM.new("not a PortableServer::POA")
    end

    def initialize(native)
      @native = native
    end

    def _native
      @native
    end

    def the_name
      @native.name
    end

    def the_parent
      POA._wrap(@native.parent)
    end

    def the_children
      @native.children.map { |c| POA.new(c) }
    end

    def the_POAManager
      POAManager.new(@native.manager)
    end

    # The seven policies of the POA.
    def the_policies
      @native.policies.each_with_index.map do |v, i|
        POLICY_CLASSES[i].new(THREAD_POLICY_ID + i, v)
      end
    end

    def create_POA(name, manager, policies)
      list = policies.map do |p|
        raise CORBA::BAD_PARAM.new("#{p.inspect} is not a policy") unless p.respond_to?(:policy_type)

        [p.policy_type, p.value]
      end
      POA.new(@native.create_poa(name, manager&._native, list))
    end

    def find_POA(name, activate_it)
      POA.new(@native.find_poa(name, activate_it))
    end

    def destroy(etherealize_objects, wait_for_completion)
      @native.destroy(etherealize_objects, wait_for_completion)
    end

    { create_thread_policy: [ThreadPolicy, THREAD_POLICY_ID],
      create_lifespan_policy: [LifespanPolicy, LIFESPAN_POLICY_ID],
      create_id_uniqueness_policy: [IdUniquenessPolicy, ID_UNIQUENESS_POLICY_ID],
      create_id_assignment_policy: [IdAssignmentPolicy, ID_ASSIGNMENT_POLICY_ID],
      create_implicit_activation_policy: [ImplicitActivationPolicy, IMPLICIT_ACTIVATION_POLICY_ID],
      create_servant_retention_policy: [ServantRetentionPolicy, SERVANT_RETENTION_POLICY_ID],
      create_request_processing_policy: [RequestProcessingPolicy, REQUEST_PROCESSING_POLICY_ID] }
      .each do |m, (klass, type)|
      define_method(m) { |value| klass.new(type, value) }
    end

    def activate_object(servant)
      @native.activate_object(servant._servant_ref)
    end

    def activate_object_with_id(oid, servant)
      @native.activate_object_with_id(oid, servant._servant_ref)
    end

    def deactivate_object(oid)
      @native.deactivate_object(oid)
    end

    def servant_to_id(servant)
      @native.servant_to_id(servant._servant_ref)
    end

    def servant_to_reference(servant)
      servant._narrow_ref(@native.servant_to_reference(servant._servant_ref))
    end

    def id_to_servant(oid)
      @native.id_to_servant(oid)
    end

    def id_to_reference(oid)
      CORBA::Object._from_ref(@native.id_to_reference(oid))
    end

    def reference_to_id(obj)
      @native.reference_to_id(obj._native_ref)
    end

    def reference_to_servant(obj)
      @native.reference_to_servant(obj._native_ref)
    end

    def create_reference(intf)
      ref_for(intf, @native.create_reference(intf))
    end

    def create_reference_with_id(oid, intf)
      ref_for(intf, @native.create_reference_with_id(oid, intf))
    end

    def get_servant
      @native.get_servant
    end

    def set_servant(servant)
      @native.set_servant(servant._servant_ref)
    end

    def ==(other)
      other.is_a?(POA) && other.the_name == the_name && other.the_POAManager == the_POAManager
    end

    private

    def ref_for(intf, ref)
      mod = Rustorb.find_interface(intf)
      mod ? mod::Stub._from_ref(ref) : CORBA::Object._from_ref(ref)
    end
  end

  class POAManager
    HOLDING = 0
    ACTIVE = 1
    DISCARDING = 2
    INACTIVE = 3

    PortableServer.local_exception(self, :AdapterInactive, 10)

    def initialize(native)
      @native = native
    end

    def _native
      @native
    end

    def activate
      @native.activate
    end

    def hold_requests(wait_for_completion)
      @native.hold_requests(wait_for_completion)
    end

    def discard_requests(wait_for_completion)
      @native.discard_requests(wait_for_completion)
    end

    def deactivate(etherealize_objects, wait_for_completion)
      @native.deactivate(etherealize_objects, wait_for_completion)
    end

    def get_state
      @native.state
    end

    def get_id
      @native.id
    end

    def ==(other)
      other.is_a?(POAManager) && @native.equal(other._native)
    end
  end

  # PortableServer::Current: the POA and object of the request this thread
  # is serving.
  class Current
    PortableServer.local_exception(self, :NoContext, 12)

    def self.instance
      @instance ||= new
    end

    def get_POA
      POA.new(Rustorb::Native.current_poa)
    end

    def get_object_id
      Rustorb::Native.current_object_id
    end

    def get_reference
      CORBA::Object._from_ref(Rustorb::Native.current_reference)
    end

    def get_servant
      Rustorb::Native.current_servant
    end
  end

  # The base of every servant. A generated skeleton class (POA::Mod::Iface)
  # sets IDS (its repository ids, its own first) and OPERATIONS (wire name
  # => [method name, Rustorb::Operation], inherited ones included).
  class Servant
    IDS = [].freeze
    OPERATIONS = {}.freeze
  end

  # What a servant is made of, for Servant and the valuetypes that support
  # an interface (an instance is activated like a servant). The class has
  # IDS and _operations.
  module ServantMethods
    def _default_POA
      PortableServer::POA.new(Rustorb::Native.default_root_poa)
    end

    # The reference of the servant, activating it in its default POA if it
    # is not active there (implicit activation).
    def _this
      # Inside a request on this servant: the reference it was called on.
      current = @__rustorb_servant_ref&.current_reference
      return _narrow_ref(current) if current

      _default_POA.servant_to_reference(self)
    end

    def _is_a?(id)
      self.class::IDS.include?(id) || id == "IDL:omg.org/CORBA/Object:1.0"
    end

    def _repository_id
      self.class::IDS.first
    end

    def _non_existent
      false
    end

    # --- what the POA and the generated code use ---

    def _servant_ref
      PortableServer.ensure_dispatchers
      @__rustorb_servant_ref ||= Rustorb::Native.servant_ref(self, self.class::IDS)
    end

    def _narrow_ref(ref)
      mod = Rustorb.find_interface(self.class::IDS.first)
      mod ? mod::Stub._from_ref(ref) : CORBA::Object._from_ref(ref)
    end

    BAD_OPERATION_MINOR = 0x4f4d0002

    # Called (on a dispatcher) for each request: reads the arguments, calls
    # the method and writes the reply. Returns true when the reply is a
    # result or a user exception, false when it is a system exception.
    def _rustorb_dispatch(op_name, inp, reply)
      entry = self.class._operations[op_name]
      unless entry
        return _system_reply(reply, CORBA::BAD_OPERATION.new("", BAD_OPERATION_MINOR, CORBA::COMPLETED_NO))
      end

      meth, op = entry
      begin
        args = op.in_params.map { |p| Rustorb::Marshal.read(inp, p[1]) }
        raise CORBA::MARSHAL.new("bad arguments of #{op_name}", 0, CORBA::COMPLETED_NO) if inp.failed?
      rescue CORBA::SystemException => e
        return _system_reply(reply, e)
      end

      begin
        unless respond_to?(meth)
          raise CORBA::NO_IMPLEMENT.new("#{self.class}##{meth}", 0, CORBA::COMPLETED_NO)
        end

        r = public_send(meth, *args)
      rescue CORBA::UserException => e
        return _user_reply(reply, op, e)
      rescue CORBA::SystemException => e
        return _system_reply(reply, e)
      rescue StandardError, ScriptError => e
        warn "rustorb: #{self.class}##{meth} raised #{e.class}: #{e.message}" if $VERBOSE
        return _system_reply(reply, CORBA::UNKNOWN.new(e.message, 0, CORBA::COMPLETED_MAYBE))
      end
      return true if op.oneway

      begin
        out = reply.output
        n = (op.result ? 1 : 0) + op.out_params.size
        values = n > 1 ? Array(r) : [r]
        if n > 1 && values.size != n
          raise CORBA::BAD_PARAM.new("#{self.class}##{meth} must return #{n} values", 0,
                                     CORBA::COMPLETED_YES)
        end

        i = 0
        if op.result
          Rustorb::Marshal.write(out, op.result, values[0])
          i = 1
        end
        op.out_params.each_with_index { |p, k| Rustorb::Marshal.write(out, p[1], values[i + k]) }
        true
      rescue CORBA::SystemException => e
        _system_reply(reply, e.class.new("", e.minor, CORBA::COMPLETED_YES))
      end
    end

    private

    def _user_reply(reply, op, exc)
      klass = op.find_exception(exc._id)
      unless klass
        return _system_reply(reply, CORBA::UNKNOWN.new("undeclared #{exc._id}", 0, CORBA::COMPLETED_MAYBE))
      end

      begin
        out = reply.user_exception(exc._id)
        Rustorb::Marshal.write(out, klass._tc, exc)
        true
      rescue CORBA::SystemException => e
        _system_reply(reply, e.class.new("", e.minor, CORBA::COMPLETED_YES))
      end
    end

    def _system_reply(reply, exc)
      reply.system_exception(exc._id, exc.minor, exc.completed)
      false
    end
  end

  class Servant
    include ServantMethods
  end
end

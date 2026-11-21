# frozen_string_literal: true

# CORBA.ORB_init and CORBA::ORB.
#
# Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

module CORBA
  # ORB_init(args, orb_id): the ORB, configured by the -ORB options of args
  # (removed from it, as CORBA says) and the environment. The ORB is one per
  # process; a second call gives the same one.
  def self.ORB_init(args = [], _orb_id = "")
    @orb_lock ||= Mutex.new
    @orb_lock.synchronize do
      native, rest = Rustorb::Native.orb_init([$PROGRAM_NAME] + args.map(&:to_s))
      args.replace(rest.drop(1)) if args.is_a?(Array) && !args.frozen?
      @orb = ORB.new(native) if @orb.nil? || @orb._native.nil?
      @orb
    end
  end

  class ORB
    class InvalidName < UserException
      ID = "IDL:omg.org/CORBA/ORB/InvalidName:1.0"
      MEMBERS = [].freeze
    end

    Rustorb::Native::LOCAL_EXCEPTIONS[13] = ->(_) { InvalidName.new }

    def initialize(native)
      @native = native
    end

    def _native
      @native
    end

    def string_to_object(str)
      CORBA::Object._from_ref(@native.string_to_object(str))
    end

    def object_to_string(obj)
      @native.object_to_string(obj&._native_ref)
    end

    def resolve_initial_references(name)
      case name
      when "RootPOA" then PortableServer::POA._wrap(@native.root_poa)
      when "POACurrent" then PortableServer::Current.instance
      else CORBA::Object._from_ref(@native.resolve_initial_references(name))
      end
    end

    def list_initial_services
      (@native.list_initial_services + %w[RootPOA POACurrent]).uniq
    end

    def register_initial_reference(name, obj)
      @native.register_initial_reference(name, obj&._native_ref)
    end

    # Serves requests until shutdown (Ruby threads go on running).
    def run
      PortableServer.ensure_dispatchers
      @native.run
    end

    def shutdown(wait_for_completion = false)
      @native.shutdown(wait_for_completion)
    end

    def destroy
      @native.destroy
    end

    def work_pending
      @native.work_pending
    end

    def perform_work
      @native.perform_work
    end

    # The call timeout of every invocation, in seconds (nil for none).
    def call_timeout=(seconds)
      @native.set_call_timeout(seconds.nil? ? -1 : (seconds * 1_000_000).round)
    end

    def call_timeout
      us = @native.call_timeout
      us.negative? ? nil : us / 1_000_000.0
    end

    def connect_timeout=(seconds)
      @native.set_connect_timeout(seconds.nil? ? -1 : (seconds * 1_000_000).round)
    end

    %i[create_string_tc create_wstring_tc create_fixed_tc create_sequence_tc create_array_tc
       create_alias_tc create_interface_tc create_struct_tc create_exception_tc create_union_tc
       create_enum_tc create_recursive_tc].each do |m|
      define_method(m) { |*a| CORBA::TypeCode.public_send(m, *a) }
    end

    def get_primitive_tc(kind)
      CORBA::TypeCode.simple(kind)
    end
  end
end

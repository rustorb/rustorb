# frozen_string_literal: true

# Portable Interceptors and IOP::Codec (docs/portable-interceptors.md), on
# the PI part of the C ABI.
#
# An interceptor is an object whose class includes
# PortableInterceptor::ClientRequestInterceptor, ServerRequestInterceptor
# or IORInterceptor (one class may include several); it is added by an
# ORBInitializer, registered before CORBA.ORB_init:
#
#   class Tracer
#     include PortableInterceptor::ClientRequestInterceptor
#     def name = "tracer"
#     def send_request(ri)
#       ri.add_request_service_context(IOP::ServiceContext.new(0x52555354, "..."), false)
#     end
#   end
#
#   class Init
#     include PortableInterceptor::ORBInitializer
#     def pre_init(info) = info.add_client_request_interceptor(Tracer.new)
#   end
#
#   PortableInterceptor.register_orb_initializer(Init.new)
#   orb = CORBA.ORB_init(ARGV)
#
# The points run with the GVL on one of the extension's dispatcher threads
# while the thread of the call (client) or of the upcall (server) waits. An
# info object is valid only during its point (BAD_INV_ORDER after). A point
# raises PortableInterceptor::ForwardRequest or a CORBA::SystemException;
# anything else it raises is UNKNOWN.
#
# Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

module IOP
  TAG_INTERNET_IOP = 0
  TAG_MULTIPLE_COMPONENTS = 1

  TAG_ORB_TYPE = 0
  TAG_CODE_SETS = 1
  TAG_POLICIES = 2
  TAG_ALTERNATE_IIOP_ADDRESS = 3
  TAG_COMPLETE_OBJECT_KEY = 5
  TAG_ENDPOINT_ID_POSITION = 6
  TAG_LOCATION_POLICY = 12
  TAG_ASSOCIATION_OPTIONS = 13
  TAG_SEC_NAME = 14
  TAG_SSL_SEC_TRANS = 20
  TAG_JAVA_CODEBASE = 25
  TAG_CSI_SEC_MECH_LIST = 33
  TAG_NULL_TAG = 34
  TAG_TLS_SEC_TRANS = 36

  TransactionService = 0
  CodeSets = 1
  ChainBypassCheck = 2
  ChainBypassInfo = 3
  LogicalThreadId = 4
  BI_DIR_IIOP = 5
  SendingContextRunTime = 6
  INVOCATION_POLICIES = 7
  FORWARDED_IDENTITY = 8
  UnknownExceptionInfo = 9
  RTCorbaPriority = 10
  RTCorbaPriorityRange = 11
  FT_GROUP_VERSION = 12
  FT_REQUEST = 13
  ExceptionDetailMessage = 14

  ENCODING_CDR_ENCAPS = 0

  def self._struct(name, members)
    klass = Class.new(::Rustorb::Struct)
    const_set(name, klass)
    id = "IDL:omg.org/IOP/#{name}:1.0"
    klass.const_set(:ID, id)
    klass.const_set(:MEMBERS, members.map(&:first).freeze)
    members.each { |m, _| klass.attr_accessor(m) }
    klass.define_singleton_method(:_tc) do
      @_tc ||= ::CORBA::TypeCode.create_struct_tc(id, name, members.map { |m, tc| [m, tc.call] })
    end
    klass
  end
  private_class_method :_struct

  octets = -> { ::CORBA::TypeCode.create_sequence_tc(0, ::CORBA._tc_octet) }
  _struct("TaggedProfile", [["tag", -> { ::CORBA._tc_ulong }], ["profile_data", octets]])
  _struct("TaggedComponent", [["tag", -> { ::CORBA._tc_ulong }], ["component_data", octets]])
  _struct("ServiceContext", [["context_id", -> { ::CORBA._tc_ulong }], ["context_data", octets]])
  _struct("Encoding", [["format", -> { ::CORBA._tc_short }], ["major_version", -> { ::CORBA._tc_octet }],
                       ["minor_version", -> { ::CORBA._tc_octet }]])

  # Octets (a String or an Array of Integers) as a binary String.
  def self._octets(data)
    data.is_a?(Array) ? data.pack("C*") : data.to_s.b
  end

  # IOP::Codec: CDR encapsulations of anys (encode / decode) and of values
  # alone (encode_value / decode_value). Octets are binary Strings.
  class Codec < ::CORBA::LocalObject
    ID = "IDL:omg.org/IOP/Codec:1.0"
    ::PortableServer.local_exception(self, "InvalidTypeForEncoding", 24)
    ::PortableServer.local_exception(self, "FormatMismatch", 25)
    ::PortableServer.local_exception(self, "TypeMismatch", 26)

    def initialize(native)
      super()
      @native = native
    end

    def encode(data) = @native.encode(data._native, false)
    def encode_value(data) = @native.encode(data._native, true)
    def decode(data) = ::CORBA::Any._from_native(@native.decode(IOP._octets(data), nil))

    def decode_value(data, tc)
      ::CORBA::Any._from_native(@native.decode(IOP._octets(data), tc.resolve.native))
    end
  end

  # IOP::CodecFactory (orb.resolve_initial_references("CodecFactory"), or
  # ORBInitInfo#codec_factory).
  class CodecFactory < ::CORBA::LocalObject
    ID = "IDL:omg.org/IOP/CodecFactory:1.0"
    ::PortableServer.local_exception(self, "UnknownEncoding", 23)

    # source: a Native::ORB or a Native::ORBInitInfo.
    def initialize(source)
      super()
      @source = source
    end

    def create_codec(enc)
      Codec.new(@source.create_codec(enc.format, enc.major_version, enc.minor_version))
    end
  end
end

module Dynamic
  # Dynamic::Parameter: argument (a CORBA::Any) and mode
  # (CORBA::PARAM_IN, PARAM_OUT or PARAM_INOUT).
  class Parameter < ::Rustorb::Struct
    ID = "IDL:omg.org/Dynamic/Parameter:1.0"
    MEMBERS = %w[argument mode].freeze
    attr_accessor :argument, :mode
  end
end

module CORBA
  PARAM_IN = 0
  PARAM_OUT = 1
  PARAM_INOUT = 2
end

module PortableInterceptor
  # ReplyStatus
  SUCCESSFUL = 0
  SYSTEM_EXCEPTION = 1
  USER_EXCEPTION = 2
  LOCATION_FORWARD = 3
  TRANSPORT_RETRY = 4
  UNKNOWN = 5

  # AdapterState
  HOLDING = 0
  ACTIVE = 1
  DISCARDING = 2
  INACTIVE = 3
  NON_EXISTENT = 4

  # Raised by a point to send the request to forward instead.
  class ForwardRequest < ::CORBA::UserException
    ID = "IDL:omg.org/PortableInterceptor/ForwardRequest:1.0"
    MEMBERS = %w[forward].freeze
    attr_accessor :forward
  end

  ::PortableServer.local_exception(self, "InvalidSlot", 21)

  # --- the interceptors ---

  # name is "" (an anonymous interceptor) unless the class defines it.
  module Interceptor
    ID = "IDL:omg.org/PortableInterceptor/Interceptor:1.0"

    def name = ""
    def destroy; end
  end

  module ClientRequestInterceptor
    include Interceptor
    ID = "IDL:omg.org/PortableInterceptor/ClientRequestInterceptor:1.0"

    def send_request(_ri); end
    def send_poll(_ri); end
    def receive_reply(_ri); end
    def receive_exception(_ri); end
    def receive_other(_ri); end
  end

  module ServerRequestInterceptor
    include Interceptor
    ID = "IDL:omg.org/PortableInterceptor/ServerRequestInterceptor:1.0"

    def receive_request_service_contexts(_ri); end
    def receive_request(_ri); end
    def send_reply(_ri); end
    def send_exception(_ri); end
    def send_other(_ri); end
  end

  module IORInterceptor
    include Interceptor
    ID = "IDL:omg.org/PortableInterceptor/IORInterceptor:1.0"

    def establish_components(_info); end
  end

  module ORBInitializer
    ID = "IDL:omg.org/PortableInterceptor/ORBInitializer:1.0"

    def pre_init(_info); end
    def post_init(_info); end
  end

  # For every ORB that CORBA.ORB_init creates after it in the process.
  def self.register_orb_initializer(init)
    Rustorb::Native.register_orb_initializer(init)
    nil
  end

  def self._policy(type, raw)
    klass = if type.between?(::PortableServer::THREAD_POLICY_ID, ::PortableServer::REQUEST_PROCESSING_POLICY_ID)
              ::PortableServer::POLICY_CLASSES[type - ::PortableServer::THREAD_POLICY_ID]
            else
              ::PortableServer::Policy
            end
    klass.new(type, raw)
  end

  def self._ref(native) = native && ::CORBA::Object._from_ref(native)

  # --- the info objects ---

  # What ClientRequestInfo and ServerRequestInfo share.
  class RequestInfo < ::CORBA::LocalObject
    ID = "IDL:omg.org/PortableInterceptor/RequestInfo:1.0"

    def initialize(native)
      super()
      @native = native
    end

    def request_id = @native.request_id
    def operation = @native.operation
    def response_expected = @native.response_expected
    def sync_scope = @native.sync_scope
    def reply_status = @native.reply_status
    def forward_reference = PortableInterceptor._ref(@native.forward_reference)
    def get_slot(id) = ::CORBA::Any._from_native(@native.get_slot(id))

    def arguments
      @native.arguments.map { |a, m| ::Dynamic::Parameter.new(::CORBA::Any._from_native(a), m) }
    end

    def exceptions = @native.exceptions.map { |tc| ::CORBA::TypeCode._wrap(tc) }
    def contexts = []
    def operation_context = []

    def result
      r = @native.result
      r && ::CORBA::Any._from_native(r)
    end

    def get_request_service_context(id)
      ::IOP::ServiceContext.new(id, @native.get_request_service_context(id))
    end

    def get_reply_service_context(id)
      ::IOP::ServiceContext.new(id, @native.get_reply_service_context(id))
    end
  end

  class ClientRequestInfo < RequestInfo
    ID = "IDL:omg.org/PortableInterceptor/ClientRequestInfo:1.0"

    def target = PortableInterceptor._ref(@native.target)
    def effective_target = PortableInterceptor._ref(@native.effective_target)
    def effective_profile = ::IOP::TaggedProfile.new(*@native.effective_profile)
    def received_exception = ::CORBA::Any._from_native(@native.received_exception)
    def received_exception_id = @native.received_exception_id

    def get_effective_component(id)
      ::IOP::TaggedComponent.new(id, @native.get_effective_component(id))
    end

    def get_effective_components(id)
      @native.get_effective_components(id).map { |t, d| ::IOP::TaggedComponent.new(t, d) }
    end

    # The policy of the type in effect, or nil.
    def get_request_policy(type)
      raw = @native.get_request_policy(type)
      raw && PortableInterceptor._policy(type, raw)
    end

    def add_request_service_context(service_context, replace)
      @native.add_request_service_context(service_context.context_id,
                                          ::IOP._octets(service_context.context_data), replace)
      nil
    end
  end

  class ServerRequestInfo < RequestInfo
    ID = "IDL:omg.org/PortableInterceptor/ServerRequestInfo:1.0"

    def sending_exception = ::CORBA::Any._from_native(@native.sending_exception)
    def sending_exception_id = @native.sending_exception_id
    def r_object_id = @native.object_id
    def adapter_id = @native.adapter_id
    def adapter_name = @native.adapter_name
    def orb_id = @native.orb_id
    def server_id = @native.server_id
    def target_most_derived_interface = @native.target_most_derived_interface
    def target_is_a(id) = @native.target_is_a(id)
    def get_server_policy(type) = PortableInterceptor._policy(type, @native.get_server_policy(type))

    def set_slot(id, data)
      @native.set_slot(id, data._native)
      nil
    end

    def add_reply_service_context(service_context, replace)
      @native.add_reply_service_context(service_context.context_id,
                                        ::IOP._octets(service_context.context_data), replace)
      nil
    end
  end

  class IORInfo < ::CORBA::LocalObject
    ID = "IDL:omg.org/PortableInterceptor/IORInfo:1.0"

    def initialize(native)
      super()
      @native = native
    end

    def get_effective_policy(type) = PortableInterceptor._policy(type, @native.get_effective_policy(type))

    # The names of the POAs from the RootPOA down (rustorb).
    def adapter_name = @native.adapter_name

    def add_ior_component(a_component)
      @native.add_ior_component(a_component.tag, ::IOP._octets(a_component.component_data))
      nil
    end

    def add_ior_component_to_profile(a_component, profile_id)
      @native.add_ior_component_to_profile(a_component.tag, ::IOP._octets(a_component.component_data),
                                           profile_id)
      nil
    end
  end

  # PICurrent (orb.resolve_initial_references("PICurrent")): the slots of
  # ORBInitInfo#allocate_slot_id, as the calling thread sees them (in an
  # upcall, those the server interceptors set).
  class Current < ::CORBA::LocalObject
    ID = "IDL:omg.org/PortableInterceptor/Current:1.0"

    def initialize(native)
      super()
      @native = native
    end

    def get_slot(id) = ::CORBA::Any._from_native(@native.get_slot(id))

    def set_slot(id, data)
      @native.set_slot(id, data._native)
      nil
    end
  end

  class ORBInitInfo < ::CORBA::LocalObject
    ID = "IDL:omg.org/PortableInterceptor/ORBInitInfo:1.0"

    class DuplicateName < ::CORBA::UserException
      ID = "IDL:omg.org/PortableInterceptor/ORBInitInfo/DuplicateName:1.0"
      MEMBERS = %w[name].freeze
      attr_accessor :name
    end

    class InvalidName < ::CORBA::UserException
      ID = "IDL:omg.org/PortableInterceptor/ORBInitInfo/InvalidName:1.0"
      MEMBERS = [].freeze
    end

    def initialize(native)
      super()
      @native = native
    end

    def arguments = @native.arguments
    def orb_id = @native.orb_id
    def codec_factory = ::IOP::CodecFactory.new(@native)
    def allocate_slot_id = @native.allocate_slot_id

    # A local object is kept for the ORB being made (CORBA.ORB_init).
    def register_initial_reference(id, obj)
      raise InvalidName if id.to_s.empty?

      if obj.is_a?(::CORBA::LocalObject)
        raise InvalidName if PortableInterceptor._locals.key?(id)

        PortableInterceptor._locals[id] = obj
        return nil
      end
      _named { @native.register_initial_reference(id, obj&._native_ref) }
      nil
    end

    # Only in post_init (BAD_INV_ORDER in pre_init).
    def resolve_initial_references(id)
      local = PortableInterceptor._locals[id]
      if local || %w[PICurrent CodecFactory].include?(id)
        # The core raises BAD_INV_ORDER in pre_init (and InvalidName for "").
        begin
          @native.resolve_initial_references("")
        rescue ::CORBA::ORB::InvalidName
          nil
        end
        return local if local
        return Current.new(@native.pi_current) if id == "PICurrent"

        return codec_factory
      end
      _named { PortableInterceptor._ref(@native.resolve_initial_references(id)) }
    end

    def add_client_request_interceptor(interceptor)
      _add(:add_client_request_interceptor, ClientRequestInterceptor, interceptor)
    end

    def add_server_request_interceptor(interceptor)
      _add(:add_server_request_interceptor, ServerRequestInterceptor, interceptor)
    end

    def add_ior_interceptor(interceptor)
      _add(:add_ior_interceptor, IORInterceptor, interceptor)
    end

    private

    def _named
      yield
    rescue ::CORBA::ORB::InvalidName
      raise InvalidName
    end

    def _add(method, kind, interceptor)
      unless interceptor.is_a?(kind)
        raise ::CORBA::BAD_PARAM.new("#{interceptor.class} does not include #{kind}", 0, ::CORBA::COMPLETED_NO)
      end

      name = interceptor.name.to_s
      begin
        @native.public_send(method, name, interceptor)
      rescue DuplicateName
        raise DuplicateName.new(name)
      end
      nil
    end
  end
  ::Rustorb::Native::LOCAL_EXCEPTIONS[22] = ->(_) { ORBInitInfo::DuplicateName.new("") }

  # The local initial references ORBInitInfo#register_initial_reference
  # keeps, for the ORB being made.
  def self._locals
    Thread.current[:__rustorb_pi_locals] ||= {}
  end

  def self._take_locals
    locals = _locals
    Thread.current[:__rustorb_pi_locals] = nil
    locals
  end
end

module Rustorb
  # What the extension calls (rustorb_ext.c, the PI section).
  module PI
    INFO = {
      Rustorb::Native::ClientRequestInfo => PortableInterceptor::ClientRequestInfo,
      Rustorb::Native::ServerRequestInfo => PortableInterceptor::ServerRequestInfo,
      Rustorb::Native::IORInfo => PortableInterceptor::IORInfo,
      Rustorb::Native::ORBInitInfo => PortableInterceptor::ORBInitInfo
    }.freeze
    UNKNOWN_ID = "IDL:omg.org/CORBA/UNKNOWN:1.0"

    # Runs obj.point(info); nil, [:forward, ref] or [:system, id, minor,
    # completed].
    def self.point(obj, point, native)
      obj.public_send(point, INFO.fetch(native.class).new(native))
      nil
    rescue PortableInterceptor::ForwardRequest => e
      ref = e.forward
      return [:system, "IDL:omg.org/CORBA/BAD_PARAM:1.0", 0, CORBA::COMPLETED_NO] unless ref.is_a?(CORBA::Object)

      begin
        [:forward, ref._native_ref]
      rescue CORBA::SystemException
        [:system, "IDL:omg.org/CORBA/BAD_PARAM:1.0", 0, CORBA::COMPLETED_NO]
      end
    rescue CORBA::SystemException => e
      [:system, e._id, e.minor, e.completed]
    rescue ::Exception => e # rubocop:disable Lint/RescueException - every error is UNKNOWN
      raise if e.is_a?(SystemExit) || e.is_a?(Interrupt) || e.is_a?(NoMemoryError)

      warn "rustorb: interceptor #{point} raised #{e.class}: #{e.message}" if $VERBOSE
      [:system, UNKNOWN_ID, 0, CORBA::COMPLETED_MAYBE]
    end

    def self.destroy_interceptor(obj)
      obj.destroy
    end
  end
end

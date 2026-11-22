# frozen_string_literal: true

# Portable Interceptors and IOP::Codec: an ORB of its own (CORBA.ORB_init
# with an id and initializers) whose interceptors pass a service context
# and a slot from the client to the servant and back, add an IOR
# component, forward and fail requests. run-pi-probe.sh runs the probe
# against the other ORBs.

require_relative "helper"
require "echo_servant"

module PiCase
  CONTEXT = 0x52425049
  TAG = 0x52425049

  class << self
    attr_accessor :codec, :slot, :log, :forward_to, :fail_with
  end
  self.log = Queue.new

  def self.encode(s) = codec.encode_value(CORBA::Any.to_any(s, CORBA._tc_string))
  def self.decode(d) = codec.decode_value(d, CORBA._tc_string).value

  class Probe
    include PortableInterceptor::ClientRequestInterceptor
    include PortableInterceptor::ServerRequestInterceptor
    include PortableInterceptor::IORInterceptor

    def name = "rb-probe"

    def send_request(ri)
      if ri.operation == "echo_long" && PiCase.forward_to
        to = PiCase.forward_to
        PiCase.forward_to = nil
        raise PortableInterceptor::ForwardRequest.new(to)
      end
      raise PiCase.fail_with if ri.operation == "echo_short" && PiCase.fail_with

      ri.add_request_service_context(IOP::ServiceContext.new(CONTEXT, PiCase.encode("from client")), false)
    end

    def receive_reply(ri)
      return unless ri.operation == "echo_string"

      PiCase.log << [:reply_context, PiCase.decode(ri.get_reply_service_context(CONTEXT).context_data)]
      PiCase.log << [:component, PiCase.decode(ri.get_effective_component(TAG).component_data)]
      PiCase.log << [:reply_status, ri.reply_status]
    end

    def receive_exception(ri)
      PiCase.log << [:exception, ri.received_exception_id]
    end

    def receive_request_service_contexts(ri)
      return unless ri.operation == "echo_string"

      got = PiCase.decode(ri.get_request_service_context(CONTEXT).context_data)
      ri.set_slot(PiCase.slot, CORBA::Any.to_any(got, CORBA._tc_string))
    end

    def receive_request(ri)
      return unless ri.operation == "echo_string"

      PiCase.log << [:server, ri.r_object_id, ri.adapter_name, ri.target_is_a(InteropTest::Echo::ID),
                     ri.target_most_derived_interface]
    end

    def send_reply(ri)
      return unless ri.operation == "echo_string"

      got = ri.get_slot(PiCase.slot).value
      ri.add_reply_service_context(IOP::ServiceContext.new(CONTEXT, PiCase.encode("server got: #{got}")), false)
    end

    def establish_components(info)
      info.add_ior_component(IOP::TaggedComponent.new(TAG, PiCase.encode("rb component")))
    end
  end

  class Init
    include PortableInterceptor::ORBInitializer

    def pre_init(info)
      PiCase.codec = info.codec_factory.create_codec(IOP::Encoding.new(IOP::ENCODING_CDR_ENCAPS, 1, 2))
      PiCase.slot = info.allocate_slot_id
      probe = Probe.new
      info.add_client_request_interceptor(probe)
      info.add_server_request_interceptor(probe)
      info.add_ior_interceptor(probe)
      PiCase.log << [:duplicate, begin
        info.add_client_request_interceptor(probe)
      rescue PortableInterceptor::ORBInitInfo::DuplicateName => e
        e.name
      end]
      PiCase.log << [:pre_init, info.orb_id]
    end

    def post_init(info)
      PiCase.log << [:post_init, info.resolve_initial_references("PICurrent").class]
    end
  end

  # The servant sees in PICurrent the slot the server interceptor set.
  class SlotEcho < EchoServant
    def echo_string(v)
      cur = @orb.resolve_initial_references("PICurrent")
      "#{v} / #{cur.get_slot(PiCase.slot).value}"
    end
  end

  def self.orb
    @orb ||= CORBA.ORB_init([], "rustorbrb-pi-test", initializers: [Init.new]).tap do |orb|
      orb.resolve_initial_references("RootPOA").the_POAManager.activate
    end
  end

  def self.ref(servant)
    poa = orb.resolve_initial_references("RootPOA")
    InteropTest::Echo._narrow(orb.string_to_object(orb.object_to_string(poa.servant_to_reference(servant))))
  end

  def self.take(kind)
    deadline = Process.clock_gettime(Process::CLOCK_MONOTONIC) + 10
    loop do
      until log.empty?
        e = log.pop
        return e[1..] if e[0] == kind
      end
      raise "no #{kind}" if Process.clock_gettime(Process::CLOCK_MONOTONIC) > deadline

      sleep 0.005
    end
  end
end

class PiTest < Minitest::Test
  def setup
    @orb = PiCase.orb
    @echo = PiCase.ref(PiCase::SlotEcho.new(@orb))
  end

  def test_initializer
    PiCase.log.clear
    CORBA.ORB_init([], "rustorbrb-pi-init", initializers: [PiCase::Init.new])
    assert_equal ["rb-probe"], PiCase.take(:duplicate)
    assert_equal ["rustorbrb-pi-init"], PiCase.take(:pre_init)
    assert_equal [PortableInterceptor::Current], PiCase.take(:post_init)
    assert_same @orb, CORBA.ORB_init([], "rustorbrb-pi-test")
  end

  def test_contexts_slots_and_components
    PiCase.log.clear
    assert_equal "hi / from client", @echo.echo_string("hi")
    oid, adapter, is_a, mdi = PiCase.take(:server)
    refute_empty oid
    assert_equal ["RootPOA"], adapter
    assert is_a
    assert_equal InteropTest::Echo::ID, mdi
    assert_equal ["server got: from client"], PiCase.take(:reply_context)
    assert_equal ["rb component"], PiCase.take(:component)
    assert_equal [PortableInterceptor::SUCCESSFUL], PiCase.take(:reply_status)
  end

  def test_forward_and_fail
    other = Class.new(EchoServant) { def echo_long(v) = v + 1000 }.new(@orb)
    PiCase.forward_to = PiCase.ref(other)
    assert_equal 1005, @echo.echo_long(5)
    PiCase.fail_with = CORBA::NO_PERMISSION.new("", 3, CORBA::COMPLETED_NO)
    e = assert_raises(CORBA::NO_PERMISSION) { @echo.echo_short(1) }
    assert_equal 3, e.minor
    PiCase.fail_with = RuntimeError.new("a raising interceptor")
    assert_raises(CORBA::UNKNOWN) { @echo.echo_short(1) }
  ensure
    PiCase.fail_with = nil
    PiCase.forward_to = nil
  end

  def test_receive_exception
    PiCase.log.clear
    assert_raises(InteropTest::DivideByZero) { @echo.divide(1, 0) }
    assert_equal [InteropTest::DivideByZero::ID], PiCase.take(:exception)
  end

  def test_current_and_codec
    cur = @orb.resolve_initial_references("PICurrent")
    assert_equal CORBA::TK_NULL, cur.get_slot(PiCase.slot)._tc.kind
    cur.set_slot(PiCase.slot, CORBA::Any.to_any(3, CORBA._tc_long))
    assert_equal 3, cur.get_slot(PiCase.slot).value
    assert_equal CORBA::TK_NULL, Thread.new { cur.get_slot(PiCase.slot)._tc.kind }.value
    assert_raises(PortableInterceptor::InvalidSlot) { cur.get_slot(99) }
    cur.set_slot(PiCase.slot, CORBA::Any.to_any(nil, CORBA._tc_null))

    factory = @orb.resolve_initial_references("CodecFactory")
    assert_raises(IOP::CodecFactory::UnknownEncoding) { factory.create_codec(IOP::Encoding.new(9, 1, 2)) }
    codec = factory.create_codec(IOP::Encoding.new(IOP::ENCODING_CDR_ENCAPS, 1, 2))
    p = InteropTest::Point.new(1, 2)
    a = codec.decode(codec.encode(CORBA::Any.to_any(p, InteropTest::Point._tc)))
    assert_equal p, a.value
    assert_raises(IOP::Codec::FormatMismatch) { codec.decode("\x00".b) }
  end
end

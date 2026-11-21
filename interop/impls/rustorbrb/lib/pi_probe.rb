# frozen_string_literal: true

# The rustorb Ruby side of run-pi-probe.sh: the probe of
# impls/rustorb/src/pi_probe.rs on PortableInterceptor, printing the same
# lines.
#
#   pi_probe.rb server [ORB options]       prints the IOR of an Echo object
#                                          (DSI), then serves
#   pi_probe.rb client IOR [ORB options]   calls echo (DII)
#
# Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

here = File.expand_path("..", __dir__)
$LOAD_PATH.unshift(File.join(here, "lib"), File.join(here, "rt"))

require "corba"

CONTEXT = 0x50495052
TAG = 0x50495052
NAME = "rustorbrb"
ECHO_ID = "IDL:PiProbe/Echo:1.0"

module Probe
  class << self
    attr_accessor :codec, :slot
  end
  SEEN = Queue.new

  def self.encode(s) = codec.encode_value(CORBA::Any.to_any(s, CORBA._tc_string))

  def self.decode(data)
    codec.decode_value(data, CORBA._tc_string).value
  rescue CORBA::Exception => e
    "<#{e._id}>"
  end

  class Interceptor
    include PortableInterceptor::ClientRequestInterceptor
    include PortableInterceptor::ServerRequestInterceptor
    include PortableInterceptor::IORInterceptor

    def name = "pi-probe"

    def send_request(ri)
      ri.add_request_service_context(IOP::ServiceContext.new(CONTEXT, Probe.encode("from #{NAME}")), false)
    end

    def receive_reply(ri)
      return unless ri.operation == "echo"

      SEEN << "reply context: #{text { ri.get_reply_service_context(CONTEXT).context_data }}"
      SEEN << "component: #{text { ri.get_effective_component(TAG).component_data }}"
    end

    def receive_request_service_contexts(ri)
      got = text { ri.get_request_service_context(CONTEXT).context_data }
      # For clients that cannot read the reply context (run-pi-probe.sh).
      puts "request context: #{got}"
      $stdout.flush
      ri.set_slot(Probe.slot, CORBA::Any.to_any(got, CORBA._tc_string))
    end

    def send_reply(ri)
      got = ri.get_slot(Probe.slot).value
      ri.add_reply_service_context(IOP::ServiceContext.new(CONTEXT, Probe.encode("#{NAME} got: #{got}")), false)
    end

    def establish_components(info)
      info.add_ior_component(IOP::TaggedComponent.new(TAG, Probe.encode("#{NAME} component")))
    end

    private

    def text
      Probe.decode(yield)
    rescue CORBA::SystemException => e
      e._id
    end
  end

  class Init
    include PortableInterceptor::ORBInitializer

    def pre_init(info)
      Probe.codec = info.codec_factory.create_codec(IOP::Encoding.new(IOP::ENCODING_CDR_ENCAPS, 1, 2))
      Probe.slot = info.allocate_slot_id
      i = Interceptor.new
      info.add_client_request_interceptor(i)
      info.add_server_request_interceptor(i)
      info.add_ior_interceptor(i)
    end
  end
end

class Echo < PortableServer::DynamicImplementation
  def initialize(orb)
    super()
    @orb = orb
  end

  def _primary_interface(_oid, _poa) = ECHO_ID

  def invoke(request)
    raise CORBA::BAD_OPERATION.new("", 0, CORBA::COMPLETED_NO) unless request.operation == "echo"

    args = @orb.create_list(1)
    args.add_value("s", CORBA::Any.to_any(nil, CORBA._tc_string), CORBA::ARG_IN)
    request.arguments(args)
    request.set_result(args.item(0).argument)
  end
end

mode = ARGV.shift
unless mode == "server" || (mode == "client" && ARGV.size >= 1)
  warn "usage: pi-probe server|client IOR [ORB options]"
  exit 2
end
ior = ARGV.shift if mode == "client"
PortableInterceptor.register_orb_initializer(Probe::Init.new)
orb = CORBA.ORB_init(ARGV)
if mode == "server"
  poa = orb.resolve_initial_references("RootPOA")
  poa.the_POAManager.activate
  obj = poa.id_to_reference(poa.activate_object(Echo.new(orb)))
  puts orb.object_to_string(obj)
  $stdout.flush
  orb.run
else
  obj = orb.string_to_object(ior)
  req = obj._request("echo")
  req.add_in_arg(CORBA::Any.to_any("hello", CORBA._tc_string))
  req.set_return_type(CORBA._tc_string)
  result = begin
    req.invoke
    req.return_value.value
  rescue CORBA::Exception => e
    e._id
  end
  puts Probe::SEEN.pop until Probe::SEEN.empty?
  puts "result: #{result}"
  orb.destroy
end

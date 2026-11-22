# frozen_string_literal: true

# The rustorbrb-dii implementation: the rustorb Ruby client and server of
# this directory with every call made through the DII and every servant of
# the server run through the DSI (as impls/rustorbpy/dii_adapters.py).
#
# DiiAdapters.install (from common.rb, when RUSTORB_DII is set) replaces
# CORBA::Object#_invoke, which the generated stubs call, with a
# CORBA::Request built from the operation's TypeCodes, and makes the POA
# activate each static servant behind a PortableServer::DynamicImplementation
# that reads the arguments with CORBA::ServerRequest#arguments and calls the
# servant's method.
#
# Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

module DiiAdapters
  MODES = { in: CORBA::ARG_IN, out: CORBA::ARG_OUT, inout: CORBA::ARG_INOUT }.freeze

  # CORBA::Object#_invoke through a CORBA::Request.
  module Invoke
    def _invoke(op, args)
      req = _request(op.name)
      ins = op.in_params
      raise ArgumentError, "#{op.name}: #{args.size} arguments for #{ins.size}" unless args.size == ins.size

      i = 0
      op.params.each do |mode, tc|
        if mode == :out
          req.add_out_arg(tc)
        else
          req.arguments.add_value("", CORBA::Any.to_any(args[i], tc), MODES[mode])
          i += 1
        end
      end
      if op.oneway
        req.send_oneway
        return nil
      end
      req.set_return_type(op.result) if op.result
      op.exceptions.each { |e| req.add_exception(e._tc) }
      req.invoke
      values = []
      values << req.return_value.value if op.result
      req.arguments.each { |nv| values << nv.argument.value unless nv.arg_modes == CORBA::ARG_IN }
      op.returned(values)
    end
  end

  # A static servant behind the DSI.
  class DsiServant < PortableServer::DynamicImplementation
    attr_reader :inner

    def initialize(inner)
      super()
      @inner = inner
      dsi = self
      # References the servant makes of itself are those of this one.
      inner.define_singleton_method(:_this) { dsi._this }
    end

    def _primary_interface(_oid, _poa) = @inner.class::IDS.first
    def _ids = @inner.class::IDS
    def _non_existent = @inner._non_existent
    def _default_POA = @inner._default_POA

    def invoke(request)
      entry = @inner.class._operations[request.operation]
      raise CORBA::BAD_OPERATION.new("", 0x4f4d0002, CORBA::COMPLETED_NO) unless entry

      meth, op = entry
      params = CORBA::NVList.new
      op.params.each do |mode, tc|
        params.add_value("", CORBA::Any.to_any(nil, tc), MODES[mode])
      end
      request.arguments(params)
      raise CORBA::NO_IMPLEMENT.new("#{@inner.class}##{meth}", 0, CORBA::COMPLETED_NO) unless @inner.respond_to?(meth)

      args = params.reject { |nv| nv.arg_modes == CORBA::ARG_OUT }.map { |nv| nv.argument.value }
      begin
        r = @inner.public_send(meth, *args)
      rescue CORBA::UserException => e
        klass = op.find_exception(e._id)
        raise CORBA::UNKNOWN.new("undeclared #{e._id}", 0, CORBA::COMPLETED_MAYBE) unless klass

        request.set_exception(CORBA::Any.to_any(e, klass._tc))
        return
      end
      return if op.oneway

      n = (op.result ? 1 : 0) + op.out_params.size
      return if n.zero?

      values = n > 1 ? Array(r) : [r]
      if op.result
        request.set_result(CORBA::Any.to_any(values.shift, op.result))
      end
      params.each_with_index do |nv, k|
        next if nv.arg_modes == CORBA::ARG_IN

        nv.argument = CORBA::Any.to_any(values.shift, op.params[k][1])
      end
    end
  end

  # The POA with each static servant behind a DsiServant.
  module Poa
    def self.wrap(servant)
      return servant if servant.is_a?(PortableServer::DynamicImplementation) || !servant.is_a?(PortableServer::Servant)

      @wrappers ||= ObjectSpace::WeakMap.new
      @wrappers[servant] ||= DsiServant.new(servant)
    end

    def self.unwrap(servant) = servant.is_a?(DsiServant) ? servant.inner : servant

    %i[activate_object servant_to_id servant_to_reference set_servant].each do |m|
      define_method(m) { |s| super(Poa.wrap(s)) }
    end

    def activate_object_with_id(oid, s) = super(oid, Poa.wrap(s))

    %i[reference_to_servant id_to_servant get_servant].each do |m|
      define_method(m) { |*a| Poa.unwrap(super(*a)) }
    end
  end

  def self.install
    CORBA::Object.prepend(Invoke)
    PortableServer::POA.prepend(Poa)
  end
end

# frozen_string_literal: true

# The DII and the DSI: CORBA::NamedValue, NVList, Request and ServerRequest,
# and PortableServer::DynamicImplementation, on Rustorb::Native::Request
# and ServerRequest (rorb_request / rorb_server_request). Values are
# CORBA::Any. See docs/dii-dsi.md and docs/ruby-binding.md.
#
# Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

module CORBA
  ARG_IN = 1
  ARG_OUT = 2
  ARG_INOUT = 3

  # An argument: its name, value (argument, a CORBA::Any; for an out one
  # only its type before the call) and mode (arg_modes, ARG_IN...).
  class NamedValue
    attr_accessor :name, :argument, :arg_modes

    def initialize(name, argument, arg_modes)
      @name = name
      @argument = argument
      @arg_modes = arg_modes
    end

    def inspect = "#<CORBA::NamedValue #{@name.inspect} #{@argument.inspect} #{@arg_modes}>"
  end

  class Bounds < UserException
    ID = "IDL:omg.org/CORBA/Bounds:1.0"
    MEMBERS = [].freeze
  end

  # A list of named values.
  class NVList
    include Enumerable

    def initialize(items = [])
      @items = items.to_a
    end

    def count = @items.size
    alias size count
    alias length count

    def add(flags) = add_item("", flags)

    def add_item(item_name, flags)
      add_value(item_name, CORBA::Any.to_any(nil, CORBA._tc_null), flags)
    end

    def add_value(item_name, value, flags)
      nv = NamedValue.new(item_name, value, flags)
      @items << nv
      nv
    end

    def item(index)
      raise Bounds unless index.between?(0, @items.size - 1)

      @items[index]
    end
    alias [] item

    def remove(index)
      raise Bounds unless index.between?(0, @items.size - 1)

      @items.delete_at(index)
      nil
    end

    def each(&block)
      @items.each(&block)
    end

    def _check_modes
      @items.each do |nv|
        unless [ARG_IN, ARG_OUT, ARG_INOUT].include?(nv.arg_modes & 3)
          raise BAD_PARAM.new("bad arg_modes #{nv.arg_modes}", 0, COMPLETED_NO)
        end
      end
    end
  end

  class ORB
    def create_list(_count = 0) = NVList.new
    def create_named_value(name, value, flags) = NamedValue.new(name, value, flags)
  end

  # A call built at run time, from obj._request(operation) or
  # obj._create_request(ctx, operation, arg_list, result).
  #
  # Arguments are added with add_in_arg(any), add_inout_arg(any) and
  # add_out_arg(tc), or are the NVList given to _create_request;
  # set_return_type(tc) gives the result type (void by default) and
  # add_exception(tc) the user exceptions it may raise. After invoke (or
  # send_deferred and get_response) the out and inout arguments hold the
  # values sent back and return_value the result. A listed user exception
  # is raised; an unlisted one is UNKNOWN.
  class Request
    attr_reader :target, :operation, :arguments, :result

    def initialize(target, operation, arguments = nil, result = nil, exceptions = [])
      @target = target
      @operation = operation
      @arguments = arguments || NVList.new
      @result = result || NamedValue.new("", CORBA::Any.to_any(nil, CORBA._tc_void), 0)
      @exceptions = exceptions.to_a.dup
      @timeout = nil
    end

    def exceptions = @exceptions.dup
    def ctx(*) = raise(NO_IMPLEMENT.new("contexts", 0, COMPLETED_NO))
    alias contexts ctx

    def add_in_arg(value, name = "") = @arguments.add_value(name, value, ARG_IN)
    def add_inout_arg(value, name = "") = @arguments.add_value(name, value, ARG_INOUT)
    def add_out_arg(tc, name = "") = @arguments.add_value(name, CORBA::Any.to_any(nil, tc), ARG_OUT)

    def set_return_type(tc)
      @result.argument = CORBA::Any.to_any(nil, tc)
    end

    def add_exception(tc) = @exceptions << tc
    def return_value = @result.argument

    # The call timeout in seconds (rustorb), nil for the policies'.
    def _timeout=(seconds)
      @timeout = seconds
    end

    def invoke(_invoke_flags = 0)
      n = _new_native
      _collect(n, n.invoke)
    end

    def send_oneway
      _new_native.send_oneway
      nil
    end

    def send_deferred
      _new_native.send_deferred
      nil
    end

    def send(_invoke_flags = 0) = send_deferred

    def poll_response
      raise BAD_INV_ORDER.new("nothing sent deferred", 0, COMPLETED_NO) unless @native

      @native.poll_response
    end

    def get_response(_response_flags = 0)
      raise BAD_INV_ORDER.new("nothing sent deferred", 0, COMPLETED_NO) unless @native

      _collect(@native, @native.get_response)
    end

    private

    def self._native_any(any)
      raise BAD_PARAM.new("a CORBA::Any expected, got #{any.inspect}", 0, COMPLETED_NO) unless any.is_a?(CORBA::Any)

      any._native
    end

    # A new native request for each call: the arguments may change.
    def _new_native
      @arguments._check_modes
      n = Rustorb::Native::Request.create(@target._native_ref, @operation)
      @arguments.each do |nv|
        mode = nv.arg_modes & 3
        native = if mode == ARG_OUT
                   tc = nv.argument.is_a?(CORBA::TypeCode) ? nv.argument : nv.argument._tc
                   Rustorb::Native::Any.typed(tc.resolve.native)
                 else
                   Request._native_any(nv.argument)
                 end
        n.add_arg(nv.name.to_s, mode, native)
      end
      rt = @result.argument._tc
      n.set_return_type(rt.resolve.native) unless rt.unaliased.kind == TK_VOID
      @exceptions.each { |tc| n.add_exception(tc.resolve.native) }
      n.set_timeout(@timeout ? (@timeout * 1_000_000).round : -1)
      @native = n
    end

    def _collect(n, ok)
      unless ok
        any = CORBA::Any._from_native(n.exception)
        e = any.value
        raise e if e.is_a?(::Exception)

        raise UNKNOWN.new("user exception #{any._tc.id}", 0x4f4d0001, COMPLETED_YES)
      end
      @arguments.each_with_index do |nv, i|
        next if nv.arg_modes & 3 == ARG_IN

        raw = n.argument(i)
        nv.argument = CORBA::Any._from_native(raw) if raw
      end
      rt = @result.argument._tc
      unless rt.unaliased.kind == TK_VOID
        raw = n.result
        @result.argument = CORBA::Any._from_native(raw) if raw
      end
      nil
    end
  end

  class Object
    def _request(operation) = Request.new(self, operation)

    def _create_request(_ctx, operation, arg_list, result, exceptions = [], _ctxlist = nil)
      Request.new(self, operation, arg_list, result, exceptions)
    end
  end

  # A request to a PortableServer::DynamicImplementation, given to its
  # invoke. arguments(nvlist) reads the in and inout arguments into the
  # NVList, whose TypeCodes and modes describe the parameters (the values
  # of its anys are not looked at). The servant then sets the argument of
  # the out and inout items and calls set_result(any) or set_exception(any)
  # (an any of a user exception). A system exception it raises is the
  # reply.
  class ServerRequest
    def initialize(native, operation)
      @native = native
      @operation = operation
      @params = nil
      @raised = false
    end

    attr_reader :operation
    alias op_name operation

    def arguments(nvlist)
      raise BAD_INV_ORDER.new("arguments already read", 0, COMPLETED_NO) if @params

      nvlist._check_modes
      nvlist.each do |nv|
        tc = nv.argument.is_a?(CORBA::TypeCode) ? nv.argument : nv.argument._tc
        @native.add_param(nv.name.to_s, nv.arg_modes & 3, tc.resolve.native)
      end
      @native.arguments
      @unset = []
      nvlist.each_with_index do |nv, i|
        raw = nv.arg_modes & 3 == ARG_OUT ? nil : @native.argument(i)
        nv.argument = CORBA::Any._from_native(raw) if raw
        @unset << (raw ? nil : nv.argument)
      end
      @params = nvlist
      nvlist
    end
    alias params arguments

    def set_result(value)
      @native.set_result(Request._native_any(value))
    end
    alias result= set_result

    def set_exception(value)
      @native.set_exception(Request._native_any(value))
      @raised = true
    end
    alias except= set_exception

    def ctx = raise(NO_IMPLEMENT.new("contexts", 0, COMPLETED_NO))

    # The out and inout values the servant set, then the reply.
    def _reply(reply)
      if @params && !@raised
        @params.each_with_index do |nv, i|
          next if nv.arg_modes & 3 == ARG_IN || nv.argument.equal?(@unset[i])

          @native.set_argument(i, Request._native_any(nv.argument))
        end
      end
      @native.reply(reply)
    end
  end
end

module PortableServer
  # A servant that gets every request as a CORBA::ServerRequest (the DSI):
  # subclasses define invoke(request) and _primary_interface(oid, poa).
  # The ORB answers _is_a with _primary_interface(nil, nil), asked when the
  # servant is first activated (as the C and Python bindings do), or with
  # the class's IDS when it has them.
  class DynamicImplementation < Servant
    def invoke(_request)
      raise CORBA::NO_IMPLEMENT.new("#{self.class}#invoke", 0, CORBA::COMPLETED_NO)
    end

    def _primary_interface(_oid, _poa)
      raise CORBA::NO_IMPLEMENT.new("#{self.class}#_primary_interface", 0, CORBA::COMPLETED_NO)
    end

    def _ids
      return self.class::IDS unless self.class::IDS.empty?

      [_primary_interface(nil, nil)]
    end

    def _is_a?(id)
      _ids.include?(id) || id == "IDL:omg.org/CORBA/Object:1.0"
    end

    def _repository_id = _ids.first

    def _servant_ref
      PortableServer.ensure_dispatchers
      @__rustorb_servant_ref ||= Rustorb::Native.servant_ref(self, _ids)
    end

    def _narrow_ref(ref)
      mod = Rustorb.find_interface(_ids.first)
      mod ? mod::Stub._from_ref(ref) : CORBA::Object._from_ref(ref)
    end

    def _rustorb_dispatch(op_name, inp, reply)
      begin
        req = CORBA::ServerRequest.new(Rustorb::Native::ServerRequest.create(inp, op_name), op_name)
        invoke(req)
        req._reply(reply)
        true
      rescue CORBA::SystemException => e
        _system_reply(reply, e)
      end
    rescue StandardError, ScriptError => e
      warn "rustorb: #{self.class}#invoke raised #{e.class}: #{e.message}" if $VERBOSE
      _system_reply(reply, CORBA::UNKNOWN.new(e.message, 0, CORBA::COMPLETED_MAYBE))
    end
  end
end

# frozen_string_literal: true

# Callback AMI (docs/ami.md): Messaging::ReplyHandler, the base of the
# AMI_<I>Handler interfaces rustorb-idl -bruby -Wbami writes,
# Messaging::ExceptionHolder, and the sendc_ requests of the stubs
# (CORBA::Object#_sendc, on rorb_object_sendc).
#
# Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

module Messaging
  module ReplyHandler
    ID = "IDL:omg.org/Messaging/ReplyHandler:1.0"
    extend ::Rustorb::InterfaceModule

    def self._tc
      @_tc ||= ::CORBA::TypeCode.create_interface_tc(ID, "ReplyHandler")
    end

    def self._ops = {}.freeze

    ::Rustorb.stub_class(self)
  end

  # The exception a handler's <op>_excep gets. raise_exception raises it;
  # raise_exception_with_list(tcs) raises a user exception only when its
  # TypeCode is in tcs (else UNKNOWN). The state (is_system_exception,
  # byte_order, marshaled_exception: the exception in CDR, its repository id
  # first) is what is sent, so a handler in another process gets it too.
  class ExceptionHolder
    include ::CORBA::ValueBase
    ID = "IDL:omg.org/Messaging/ExceptionHolder:1.0"
    TRUNCATABLE_IDS = [ID].freeze
    MEMBERS = %w[is_system_exception byte_order marshaled_exception].freeze
    attr_accessor :is_system_exception, :byte_order, :marshaled_exception

    def self._tc
      ::Rustorb.lazy_tc(self, ID) do
        ::CORBA::TypeCode.create_value_tc(ID, "ExceptionHolder", ::CORBA::VM_NONE, nil, [
          ["is_system_exception", ::CORBA._tc_boolean, ::CORBA::PRIVATE_MEMBER],
          ["byte_order", ::CORBA._tc_boolean, ::CORBA::PRIVATE_MEMBER],
          ["marshaled_exception", ::CORBA::TypeCode.create_sequence_tc(0, ::CORBA._tc_octet),
           ::CORBA::PRIVATE_MEMBER]
        ])
      end
    end

    UNKNOWN_MINOR = 0x4f4d0001
    LITTLE = [1].pack("s") == [1].pack("s<")

    # A holder of the exception e (a CORBA::SystemException, or a user
    # exception of the class klass).
    def self._of(e, klass = nil)
      h = new
      h.is_system_exception = e.is_a?(::CORBA::SystemException)
      h.byte_order = LITTLE
      h.marshaled_exception = ::Rustorb::Native.cdr_encode(LITTLE, lambda do |out|
        out.write_string(e._id)
        if h.is_system_exception
          out.write_ulong(e.minor)
          out.write_ulong(e.completed)
        else
          ::Rustorb::Marshal.write(out, klass._tc, e)
        end
      end)
      h.instance_variable_set(:@__rustorb_exception, e)
      h
    end

    def raise_exception
      e = @__rustorb_exception || _decode(nil)
      raise e
    end

    def raise_exception_with_list(exc_list)
      e = @__rustorb_exception
      if e.is_a?(::CORBA::UserException) && exc_list.none? { |tc| tc.id == e._id }
        e = ::CORBA::UNKNOWN.new("user exception #{e._id}", UNKNOWN_MINOR, ::CORBA::COMPLETED_YES)
      end
      raise(e || _decode(exc_list))
    end

    private

    # The exception of the state; user exceptions by the registered classes
    # (or the TypeCodes of list, when given).
    def _decode(list)
      bytes = marshaled_exception.is_a?(Array) ? marshaled_exception.pack("C*") : marshaled_exception.to_s
      ::Rustorb::Native.cdr_decode(bytes, byte_order, lambda do |inp|
        id = inp.read_string
        if is_system_exception
          minor = inp.read_ulong
          completed = inp.read_ulong
          raise ::CORBA::MARSHAL.new("bad marshaled_exception", 0, ::CORBA::COMPLETED_NO) if inp.failed?

          next ::Rustorb::Native.system_exception(id, minor, completed)
        end
        klass = ::Rustorb.find_type(id)
        tc = if list
               list.find { |t| t.id == id }
             elsif klass.is_a?(Class) && klass <= ::CORBA::UserException
               klass._tc
             end
        next ::CORBA::UNKNOWN.new("user exception #{id}", UNKNOWN_MINOR, ::CORBA::COMPLETED_YES) unless tc

        e = ::Rustorb::Marshal.read(inp, tc)
        raise ::CORBA::MARSHAL.new("bad marshaled_exception", 0, ::CORBA::COMPLETED_NO) if inp.failed?

        e.is_a?(::Exception) ? e : ::CORBA::UNKNOWN.new("user exception #{id}", UNKNOWN_MINOR, ::CORBA::COMPLETED_YES)
      end)
    end
  end
  ::Rustorb::Values.register(ExceptionHolder, default: true)
end

module POA
  module Messaging
    class ReplyHandler < ::PortableServer::Servant
      IDS = [::Messaging::ReplyHandler::ID].freeze

      def self._operations = {}.freeze
    end
  end
end

module CORBA
  class Object
    # A sendc_ request (generated code): the in and inout values of op are
    # copied now; the reply goes to handler.<reply>(result, inout and out
    # values) or handler.<excep>(Messaging::ExceptionHolder), called on a
    # thread of the ORB's reply pool. A nil handler drops the reply. What
    # the handler raises is dropped.
    def _sendc(op, reply, excep, handler, args)
      ins = op.in_params
      raise ArgumentError, "#{op.name}: #{args.size} arguments for #{ins.size}" unless args.size == ins.size
      if !handler.nil? && !handler.is_a?(CORBA::Object)
        raise CORBA::BAD_PARAM.new("the handler is not an object reference", 0, CORBA::COMPLETED_NO)
      end

      anys = ins.each_with_index.map do |p, i|
        tc = p[1]
        Rustorb::Native::Any.insert(tc.resolve.native, ->(out) { Rustorb::Marshal.write(out, tc, args[i]) })
      end
      values = nil
      failure = nil
      user = nil
      reader = if op.result.nil? && op.out_params.empty?
                 nil
               else
                 lambda do |inp|
                   v = []
                   v << Rustorb::Marshal.read(inp, op.result) if op.result
                   op.out_params.each { |p| v << Rustorb::Marshal.read(inp, p[1]) }
                   raise CORBA::MARSHAL.new("bad reply of #{op.name}", 0, CORBA::COMPLETED_YES) if inp.failed?

                   values = v
                 rescue ::Exception => e # rubocop:disable Lint/RescueException - kept for the handler
                   failure = e
                   raise
                 end
               end
      user_exc = if op.exceptions.empty?
                   nil
                 else
                   lambda do |id, inp|
                     klass = op.find_exception(id)
                     next nil unless klass

                     e = Rustorb::Marshal.read(inp, klass._tc)
                     raise CORBA::MARSHAL.new("bad #{id}", 0, CORBA::COMPLETED_YES) if inp.failed?

                     user = [e, klass]
                     e
                   rescue ::Exception => e # rubocop:disable Lint/RescueException - kept for the handler
                     failure = e
                     raise
                   end
                 end
      # status: RORB_OK (0), RORB_SYSTEM_EXCEPTION (1, sys is [id, minor,
      # completed]), RORB_USER_EXCEPTION (2) or RORB_CALLBACK_FAILED (4).
      done = lambda do |status, sys|
        next if handler.nil?

        begin
          case status
          when 0 then handler.public_send(reply, *(values || []))
          when 2 then handler.public_send(excep, Messaging::ExceptionHolder._of(*user))
          else
            e = if sys
                  Rustorb::Native.system_exception(*sys)
                elsif failure.is_a?(CORBA::SystemException)
                  failure
                else
                  CORBA::UNKNOWN.new(failure&.message.to_s, 0, CORBA::COMPLETED_MAYBE)
                end
            handler.public_send(excep, Messaging::ExceptionHolder._of(e))
          end
        rescue ::Exception => e # rubocop:disable Lint/RescueException - nowhere to report it
          raise if e.is_a?(SystemExit) || e.is_a?(Interrupt)

          warn "rustorb: the AMI handler of #{op.name} raised #{e.class}: #{e.message}" if $VERBOSE
        end
      end
      @_ref.sendc(op.name, anys, nil, reader, user_exc, done)
      nil
    end
  end
end

# frozen_string_literal: true

# CORBA::Object (a reference: the stub side of every interface) and what
# generated stubs and skeletons describe operations with.
#
# An IDL interface is a Ruby module (Mod::Iface) holding the stub methods;
# Iface._narrow gives a reference of the class Iface::Stub, which is a
# CORBA::Object including the module.
#
# Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

module Rustorb
  # An operation or attribute accessor: name (on the wire), result
  # (TypeCode, nil for void), params ([[mode, TypeCode]], mode :in, :out or
  # :inout), exceptions (UserException classes), oneway.
  class Operation
    attr_reader :name, :result, :params, :exceptions, :oneway

    def initialize(name, result: nil, params: [], exceptions: [], oneway: false)
      @name = name
      @result = result
      @params = params
      @exceptions = exceptions
      @oneway = oneway
      @in_params = params.reject { |p| p[0] == :out }
      @out_params = params.reject { |p| p[0] == :in }
    end

    attr_reader :in_params, :out_params

    def find_exception(id)
      @exceptions.find { |e| e::ID == id }
    end

    # What a call returns: nil, the one value, or [result, out values...].
    def returned(values)
      case values.size
      when 0 then nil
      when 1 then values[0]
      else values
      end
    end
  end

  @interfaces = {}

  class << self
    # The stub class of an interface module (Iface::Stub).
    def stub_class(mod)
      klass = Class.new(CORBA::Object) { include mod }
      klass.define_singleton_method(:_tc) { mod._tc }
      mod.const_set(:Stub, klass)
      @types_lock.synchronize { @interfaces[mod::ID] = mod }
      klass
    end

    def find_interface(id)
      @types_lock.synchronize { @interfaces[id] }
    end
  end

  # What an interface module extends itself with: _narrow and friends.
  module InterfaceModule
    def _narrow(obj)
      return nil if obj.nil?
      raise CORBA::BAD_PARAM.new("#{obj.inspect} is not a CORBA::Object") unless obj.is_a?(CORBA::Object)
      return obj if obj.is_a?(self)
      raise CORBA::BAD_PARAM.new("the object is not a #{self::ID}", 0, CORBA::COMPLETED_NO) unless obj._is_a?(self::ID)

      self::Stub._from_ref(obj._native_ref)
    end

    def _unchecked_narrow(obj)
      return nil if obj.nil?
      return obj if obj.is_a?(self)

      self::Stub._from_ref(obj._native_ref)
    end

    def _tc
      @_tc ||= CORBA::TypeCode.create_interface_tc(self::ID, name.split("::").last)
    end
  end
end

module CORBA
  class Object
    # ref: a Rustorb::Native::Ref.
    def self._from_ref(ref, tc = nil)
      return nil if ref.nil?

      klass = self
      if klass == CORBA::Object && tc
        mod = Rustorb.find_interface(tc.unaliased.id)
        klass = mod::Stub if mod
      end
      obj = klass.allocate
      obj.instance_variable_set(:@_ref, ref)
      obj
    end

    def _native_ref
      @_ref
    end

    def _is_a?(id)
      @_ref.is_a(id)
    end

    def _non_existent?
      @_ref.non_existent
    end
    alias _non_existent _non_existent?

    def _validate_connection
      @_ref.validate_connection
    end

    def _is_equivalent?(other)
      other.is_a?(CORBA::Object) && @_ref.equivalent(other._native_ref)
    end
    alias _is_equivalent _is_equivalent?

    def _hash(maximum = 2**31 - 1)
      @_ref.hash_value(maximum)
    end

    def _repository_id
      @_ref.type_id
    end

    def _duplicate
      self.class._from_ref(@_ref.dup)
    end

    def _release; end

    def _is_nil?
      false
    end

    # The call timeout of this reference, in seconds (nil: the ORB's).
    def _call_timeout=(seconds)
      @_ref.set_call_timeout(seconds.nil? ? -1 : (seconds * 1_000_000).round)
    end

    def _call_timeout
      us = @_ref.call_timeout
      us.negative? ? nil : us / 1_000_000.0
    end

    def ==(other)
      _is_equivalent?(other)
    end
    alias eql? ==

    def hash
      _hash
    end

    def inspect
      "#<#{self.class.name || 'CORBA::Object'} #{_repository_id}>"
    end
    alias to_s inspect

    # Calls op (a Rustorb::Operation) with the in and inout arguments.
    def _invoke(op, args)
      ins = op.in_params
      unless args.size == ins.size
        raise ArgumentError, "#{op.name}: #{args.size} arguments for #{ins.size}"
      end

      writer = if ins.empty?
                 nil
               else
                 lambda do |out|
                   ins.each_with_index { |p, i| Rustorb::Marshal.write(out, p[1], args[i]) }
                 end
               end
      values = []
      reader = if op.oneway || (op.result.nil? && op.out_params.empty?)
                 nil
               else
                 lambda do |inp|
                   values << Rustorb::Marshal.read(inp, op.result) if op.result
                   op.out_params.each { |p| values << Rustorb::Marshal.read(inp, p[1]) }
                   raise CORBA::MARSHAL.new("bad reply of #{op.name}", 0, CORBA::COMPLETED_YES) if inp.failed?
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

                     e
                   end
                 end
      @_ref.invoke(op.name, op.oneway, nil, writer, reader, user_exc)
      op.returned(values)
    end
  end
end

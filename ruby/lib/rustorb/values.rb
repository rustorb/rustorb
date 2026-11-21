# frozen_string_literal: true

# Valuetypes (CORBA 3.4 part 1, 9; the CDR of part 2, 9.3.4) and abstract
# interfaces, on the value functions of the C ABI (rorb_out_begin_value,
# rorb_in_read_value_header...). See docs/ruby-binding.md.
#
#   class Point                      # generated
#     include ::CORBA::ValueBase
#     ID = "IDL:Demo/Point:1.0"
#     MEMBERS = %w[x y].freeze
#     ...
#   end
#
# A value is written with its repository ids (its own, then those of the
# bases it is truncatable to), chunked when it is truncatable or custom; an
# instance written again in the same message is an indirection, so sharing
# and cycles survive the trip. A value is read by the factory of the first
# repository id received that has one (ORB#register_value_factory, else the
# generated class of a valuetype without operations); without one the
# stream fails (MARSHAL).
#
# Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

module CORBA
  # What every value includes. A generated valuetype class has ID,
  # TRUNCATABLE_IDS (its own id, then those of the bases it is truncatable
  # to), MEMBERS (its own state members) and _tc; new takes the state
  # members, those of the bases first.
  module ValueBase
    def initialize(*args)
      names = Rustorb::Values.member_names(self.class)
      raise ArgumentError, "#{self.class}: #{args.size} values for #{names.size} members" if args.size > names.size

      names.each_with_index { |m, i| instance_variable_set("@#{m}", args[i]) }
    end

    def _truncatable_ids = self.class::TRUNCATABLE_IDS
    def _tc = self.class._tc
    def _add_ref = self
    def _remove_ref = nil
    def _refcount_value = 1

    # Values seen again (cycles, sharing) are shown as #<Class ...>.
    def inspect
      seen = Thread.current[:__rustorb_inspecting] ||= {}.compare_by_identity
      return "#<#{self.class.name} ...>" if seen.key?(self)

      seen[self] = true
      begin
        names = Rustorb::Values.member_names(self.class)
        "#<#{self.class.name} #{names.map { |m| "#{m}=#{instance_variable_get("@#{m}").inspect}" }.join(', ')}>"
      ensure
        seen.delete(self)
      end
    end
    alias to_s inspect
  end

  # A custom valuetype's implementation defines marshal(os) and
  # unmarshal(is) (a CORBA::DataOutputStream and DataInputStream).
  module CustomMarshal
    def marshal(_os)
      raise CORBA::NO_IMPLEMENT.new("#{self.class}#marshal", 0, CORBA::COMPLETED_NO)
    end

    def unmarshal(_is)
      raise CORBA::NO_IMPLEMENT.new("#{self.class}#unmarshal", 0, CORBA::COMPLETED_NO)
    end
  end

  # A value factory: create_for_unmarshal returns a new instance, whose
  # state the ORB then reads. ORB#register_value_factory also takes a
  # class (its instances are made with allocate) or a block.
  class ValueFactoryBase
    def initialize(&block)
      @block = block
    end

    def create_for_unmarshal
      return @block.call if @block

      raise CORBA::NO_IMPLEMENT.new("#{self.class}#create_for_unmarshal", 0, CORBA::COMPLETED_NO)
    end
  end
  ValueFactory = ValueFactoryBase

  # The streams of custom marshaling: write_<type>(v) and read_<type> for
  # the basic types, write_<type>_array(seq, offset, length) and
  # read_<type>_array(seq, offset, length) (which fills seq), write_Value /
  # read_Value, write_Abstract / read_Abstract, write_Object / read_Object,
  # write_any / read_any, write_TypeCode / read_TypeCode.
  module DataStreamTypes
    BASIC = {
      "boolean" => :_tc_boolean, "octet" => :_tc_octet, "char" => :_tc_char, "wchar" => :_tc_wchar,
      "short" => :_tc_short, "ushort" => :_tc_ushort, "long" => :_tc_long, "ulong" => :_tc_ulong,
      "longlong" => :_tc_longlong, "ulonglong" => :_tc_ulonglong, "float" => :_tc_float,
      "double" => :_tc_double, "longdouble" => :_tc_longdouble
    }.freeze
    OTHER = {
      "string" => :_tc_string, "wstring" => :_tc_wstring, "any" => :_tc_any,
      "TypeCode" => :_tc_TypeCode, "Object" => :_tc_Object
    }.freeze
  end

  class DataOutputStream
    def initialize(out)
      @out = out
    end

    def _out = @out

    DataStreamTypes::BASIC.merge(DataStreamTypes::OTHER).each do |name, tc|
      define_method("write_#{name}") { |v| Rustorb::Marshal.write(@out, CORBA.public_send(tc), v) }
    end

    DataStreamTypes::BASIC.each do |name, tc|
      define_method("write_#{name}_array") do |seq, offset = 0, length = nil|
        length ||= seq.size - offset
        element = CORBA.public_send(tc)
        if seq.is_a?(String)
          Rustorb::Marshal.write(@out, CORBA::TypeCode.create_array_tc(length, element), seq.byteslice(offset, length))
        else
          seq[offset, length].each { |v| Rustorb::Marshal.write(@out, element, v) }
        end
      end
    end

    def write_Value(value, tc = nil)
      if tc && tc.unaliased.kind == CORBA::TK_VALUE_BOX
        Rustorb::Values.write_box(@out, tc.unaliased, value)
      else
        Rustorb::Values.write(@out, value)
      end
    end

    def write_Abstract(value) = Rustorb::Values.write_abstract(@out, value)
    def write_fixed(value, digits, scale) = Rustorb::Marshal.write(@out, CORBA::TypeCode.create_fixed_tc(digits, scale), value)
  end

  class DataInputStream
    def initialize(inp)
      @in = inp
    end

    def _in = @in

    DataStreamTypes::BASIC.merge(DataStreamTypes::OTHER).each do |name, tc|
      define_method("read_#{name}") { Rustorb::Marshal.read(@in, CORBA.public_send(tc)) }
    end

    DataStreamTypes::BASIC.each do |name, tc|
      define_method("read_#{name}_array") do |seq, offset, length|
        element = CORBA.public_send(tc)
        length.times { |i| seq[offset + i] = Rustorb::Marshal.read(@in, element) }
        seq
      end
    end

    # A value of any type, or of the value box tc.
    def read_Value(tc = nil)
      if tc && tc.unaliased.kind == CORBA::TK_VALUE_BOX
        Rustorb::Values.read_box(@in, tc.unaliased)
      else
        Rustorb::Values.read(@in, tc && tc.unaliased.id)
      end
    end

    def read_Abstract(tc = nil) = Rustorb::Values.read_abstract(@in, tc)
    def read_fixed(digits, scale) = Rustorb::Marshal.read(@in, CORBA::TypeCode.create_fixed_tc(digits, scale))
  end

  class ORB
    # Registers factory (a ValueFactoryBase, an object with
    # create_for_unmarshal, or a class) for the valuetype id; returns the one
    # it replaces.
    def register_value_factory(id, factory = nil, &block)
      factory ||= CORBA::ValueFactoryBase.new(&block) if block
      unless factory.is_a?(Class) || factory.respond_to?(:create_for_unmarshal)
        raise CORBA::BAD_PARAM.new("not a value factory: #{factory.inspect}", 0, CORBA::COMPLETED_NO)
      end

      Rustorb::Values.register_factory(id, factory)
    end

    def unregister_value_factory(id)
      Rustorb::Values.unregister_factory(id)
      nil
    end

    # The registered factory, else the generated class of a valuetype
    # without operations, else nil.
    def lookup_value_factory(id) = Rustorb::Values.factory(id)
  end
end

module Rustorb
  # What a generated abstract interface module extends itself with: _narrow
  # keeps a value that includes the module and makes a stub of a reference.
  module AbstractInterfaceModule
    include InterfaceModule

    def _narrow(obj)
      return obj if obj.nil? || obj.is_a?(self)
      return super if obj.is_a?(CORBA::Object)

      raise CORBA::BAD_PARAM.new("neither a #{self::ID} value nor an object", 0, CORBA::COMPLETED_NO)
    end

    def _unchecked_narrow(obj)
      return obj unless obj.is_a?(CORBA::Object)

      super
    end
  end

  module Values
    @lock = Mutex.new
    @factories = {}
    @defaults = {}
    @members = {}

    module_function

    # Called by the generated code for each valuetype and value box: its
    # TypeCode is then known to the core (for values in anys) and, with
    # default, the class is its factory.
    def register(klass, default: false)
      Rustorb.register_type(klass)
      @lock.synchronize { @defaults[klass::ID] = klass } if default
      klass
    end

    def register_factory(id, factory)
      @lock.synchronize do
        old = @factories[id]
        @factories[id] = factory
        old
      end
    end

    def unregister_factory(id)
      @lock.synchronize do
        raise CORBA::BAD_PARAM.new("no value factory for #{id}", 0, CORBA::COMPLETED_NO) unless @factories.key?(id)

        @factories.delete(id)
      end
    end

    def factory(id)
      @lock.synchronize { @factories[id] || @defaults[id] }
    end

    def make(factory)
      factory.is_a?(Class) ? factory.allocate : factory.create_for_unmarshal
    end

    # [names, TypeCode] of each valuetype of klass's state, the concrete
    # bases first: the Ruby names (MEMBERS of the class with the type's ID)
    # and the TypeCode with the member types.
    def state(klass)
      cached = @lock.synchronize { @members[klass] }
      return cached if cached

      chain = []
      tc = klass._tc.resolve
      while tc
        chain.unshift(tc)
        tc = tc.concrete_base_type&.resolve
      end
      ancestors = klass.ancestors
      list = chain.map do |t|
        owner = ancestors.find { |k| k.const_defined?(:ID, false) && k::ID == t.id && k.const_defined?(:MEMBERS, false) }
        names = owner ? owner::MEMBERS : Array.new(t.member_count) { |i| t.member_name(i) }
        [names, t]
      end.freeze
      @lock.synchronize { @members[klass] = list }
    end

    def member_names(klass)
      return [] unless klass.respond_to?(:_tc)

      state(klass).flat_map(&:first)
    end

    def write(out, v)
      return out.write_null_value if v.nil?

      unless v.is_a?(CORBA::ValueBase)
        raise CORBA::BAD_PARAM.new("a value expected, got #{v.inspect}", 0, CORBA::COMPLETED_NO)
      end

      custom = v.is_a?(CORBA::CustomMarshal)
      ids = v._truncatable_ids
      return unless out.begin_value(v, ids, ids.size > 1 || custom)

      if custom
        v.marshal(CORBA::DataOutputStream.new(out))
      else
        state(v.class).each do |names, tc|
          names.each_with_index do |m, i|
            Rustorb::Marshal.write(out, tc.member_type(i), v.instance_variable_get("@#{m}"))
          end
        end
      end
      out.end_value
    end

    # expected: the repository id of the declared type (for a value sent
    # without its ids).
    def read(inp, expected)
      kind, slot, ids = inp.read_value_header
      case kind
      when 1
        v = inp.value_at(slot)
        inp.fail if v.nil?
        return v
      when 2 then nil
      else return nil
      end

      ids = [expected] if ids.empty? && expected
      f = nil
      ids.each { |id| break if (f = factory(id)) }
      return inp.fail && nil unless f

      v = make(f)
      return inp.fail && nil unless v.is_a?(CORBA::ValueBase)

      inp.keep_value(slot, v)
      if v.is_a?(CORBA::CustomMarshal)
        v.unmarshal(CORBA::DataInputStream.new(inp))
      else
        state(v.class).each do |names, tc|
          names.each_with_index do |m, i|
            v.instance_variable_set("@#{m}", Rustorb::Marshal.read(inp, tc.member_type(i)))
          end
        end
      end
      inp.end_value
      v
    end

    # A value box: the boxed value itself, nil for null.
    def write_box(out, tc, v)
      return out.write_null_value if v.nil?
      return unless out.begin_value(v, [tc.id], false)

      Rustorb::Marshal.write(out, tc.content_type, v)
      out.end_value
    end

    def read_box(inp, tc)
      kind, slot, = inp.read_value_header
      return inp.value_at(slot) if kind == 1
      return nil unless kind == 2

      v = Rustorb::Marshal.read(inp, tc.content_type)
      inp.keep_value(slot, v)
      inp.end_value
      v
    end

    # An abstract interface: TRUE and a reference (nil is a nil one), or
    # FALSE and a value.
    def write_abstract(out, v)
      if v.nil? || v.is_a?(CORBA::Object)
        out.write_boolean(true)
        out.write_object(v&._native_ref)
      elsif v.is_a?(CORBA::ValueBase)
        out.write_boolean(false)
        write(out, v)
      else
        raise CORBA::BAD_PARAM.new("neither an object nor a value: #{v.inspect}", 0, CORBA::COMPLETED_NO)
      end
    end

    def read_abstract(inp, tc)
      return CORBA::Object._from_ref(inp.read_object, tc) if inp.read_boolean

      read(inp, nil)
    end
  end

  # The TypeCode of the valuetype id, for the core (values in anys).
  def self.resolve_value_tc(id)
    klass = find_type(id)
    klass.respond_to?(:_tc) ? klass._tc.resolve.native : nil
  end
end

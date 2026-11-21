# frozen_string_literal: true

# What generated types derive from (Rustorb::Struct, Rustorb::Union),
# CORBA::Any and CORBA::LongDouble.
#
# Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

module Rustorb
  # A struct: attr_accessor members (MEMBERS, in IDL order) and a
  # positional initialize.
  class Struct
    def initialize(*args)
      members = self.class::MEMBERS
      raise ArgumentError, "#{self.class}: #{args.size} values for #{members.size} members" if args.size > members.size

      members.each_with_index { |m, i| instance_variable_set("@#{m}", args[i]) }
    end

    def _values
      self.class::MEMBERS.map { |m| instance_variable_get("@#{m}") }
    end

    def _tc
      self.class._tc
    end

    def ==(other)
      other.class == self.class && other._values == _values
    end
    alias eql? ==

    def hash
      [self.class, _values].hash
    end

    def inspect
      "#<#{self.class.name} #{self.class::MEMBERS.zip(_values).map { |m, v| "#{m}=#{v.inspect}" }.join(', ')}>"
    end
    alias to_s inspect
  end

  # A union: the discriminator (_disc), the value (_value) and an accessor
  # per member, which reads the value if the member is selected (else
  # CORBA::BAD_PARAM) or selects the member with its first label.
  class Union
    attr_reader :_disc, :_value

    def _set(disc, value)
      @_disc = disc
      @_value = value
      self
    end

    def _tc
      self.class._tc
    end

    # The IDL name of the selected member (nil for none).
    def _member
      m = Rustorb::Marshal.union_member(self.class._tc, @_disc)
      m && m[1]
    end

    # Sets the discriminator to another label of the selected member.
    def _disc=(disc)
      now = _member
      unless now.nil? || Rustorb::Marshal.union_member(self.class._tc, disc)&.[](1) == now
        raise CORBA::BAD_PARAM.new("#{disc.inspect} does not select #{now}")
      end

      @_disc = disc
    end

    def _is_at_default?
      m = Rustorb::Marshal.union_member(self.class._tc, @_disc)
      m.nil? || m[0] == :default
    end

    # Selects no member: the discriminator a value no label has (for a
    # union without a default member), or disc.
    def _default(disc = nil)
      @_disc = disc.nil? ? Rustorb::Union.implicit_default(self.class._tc) : disc
      @_value = nil
      self
    end

    def _get(name)
      raise CORBA::BAD_PARAM.new("#{self.class.name}: #{name} is not selected") unless _member == name

      @_value
    end

    # Selects the member name with value; disc must be one of its labels.
    def _put(name, value, disc = nil)
      labels = self.class._tc._union_members.select { |m| m[1] == name }.map(&:first)
      if disc.nil?
        disc = labels.first
        disc = Rustorb::Union.implicit_default(self.class._tc) if disc == :default
      elsif !labels.include?(disc) && !(labels.include?(:default) && Rustorb::Marshal.union_member(self.class._tc, disc)&.[](0) == :default)
        raise CORBA::BAD_PARAM.new("#{disc.inspect} is not a label of #{name}")
      end
      @_disc = disc
      @_value = value
      self
    end

    def ==(other)
      other.class == self.class && other._disc == @_disc && other._value == @_value
    end
    alias eql? ==

    def hash
      [self.class, @_disc, @_value].hash
    end

    def inspect
      "#<#{self.class.name} _disc=#{@_disc.inspect} #{_member}=#{@_value.inspect}>"
    end
    alias to_s inspect

    # A discriminator value that no label of tc has.
    def self.implicit_default(tc)
      used = tc._union_members.map(&:first).reject { |l| l == :default }
      disc = tc.discriminator_type.unaliased
      case disc.kind
      when CORBA::TK_BOOLEAN then !used.include?(false) ? false : true
      when CORBA::TK_CHAR then (0..255).map { |c| c.chr(Encoding::UTF_8) }.find { |c| !used.include?(c) }
      else (0..).find { |v| !used.include?(v) }
      end
    end
  end
end

module CORBA
  # long double, as its IEEE 754 binary128 bits (hi, lo), made from and
  # turned into a Float.
  class LongDouble
    attr_reader :hi, :lo

    def initialize(value = 0.0)
      @hi, @lo = Rustorb::Native.longdouble_from_double(value.to_f)
    end

    def self._from_bits(hi, lo)
      ld = allocate
      ld.instance_variable_set(:@hi, hi)
      ld.instance_variable_set(:@lo, lo)
      ld
    end

    def to_f
      Rustorb::Native.longdouble_to_double(@hi, @lo)
    end

    def ==(other)
      other.is_a?(LongDouble) ? other.hi == @hi && other.lo == @lo : to_f == other
    end

    def inspect
      "#<CORBA::LongDouble #{to_f}>"
    end
    alias to_s inspect
  end

  # An any: a TypeCode and a value of that type. Values read from the wire
  # are extracted when first asked for; a value of a type without a Ruby
  # class here comes out as a Rustorb::GenericStruct / GenericUnion, and
  # the any goes back on the wire as it came.
  class Any
    def self.to_any(value, tc = nil)
      new(value, tc)
    end

    def initialize(value = nil, tc = nil)
      @tc = tc || Any.infer(value)
      @value = value
      @has_value = true
    end

    def self._from_native(native)
      any = allocate
      any.instance_variable_set(:@native, native)
      any.instance_variable_set(:@tc, CORBA::TypeCode._wrap(native.type))
      any.instance_variable_set(:@has_value, false)
      any
    end

    def _tc
      @tc
    end
    alias type _tc

    def value
      unless @has_value
        v = nil
        tc = @tc
        @native.extract(->(inp) { v = Rustorb::Marshal.read(inp, tc) }) unless tc.unaliased.kind == CORBA::TK_NULL
        @value = v
        @has_value = true
      end
      @value
    end

    def _native
      @native ||= if @tc.unaliased.kind == CORBA::TK_NULL
                    Rustorb::Native::Any.empty
                  else
                    tc = @tc
                    v = @value
                    Rustorb::Native::Any.insert(tc.resolve.native, ->(out) { Rustorb::Marshal.write(out, tc, v) })
                  end
    end

    def ==(other)
      other.is_a?(Any) && _native.equal(other._native)
    end

    def inspect
      "#<CORBA::Any #{@tc.inspect} #{value.inspect}>"
    end
    alias to_s inspect

    # The TypeCode of a Ruby value put in an any without one.
    def self.infer(value)
      case value
      when nil then CORBA._tc_null
      when true, false then CORBA._tc_boolean
      when Integer
        if value.between?(-(2**31), 2**31 - 1) then CORBA._tc_long
        elsif value.between?(-(2**63), 2**63 - 1) then CORBA._tc_longlong
        else CORBA._tc_ulonglong
        end
      when Float then CORBA._tc_double
      when String then CORBA._tc_string
      when CORBA::LongDouble then CORBA._tc_longdouble
      when CORBA::Any then CORBA._tc_any
      when CORBA::TypeCode then CORBA._tc_TypeCode
      when CORBA::Object then value.class.respond_to?(:_tc) ? value.class._tc : CORBA._tc_Object
      when BigDecimal
        digits = value.abs.to_s("F").delete(".").sub(/\A0+(?=\d)/, "")
        scale = value.frac.zero? ? 0 : value.to_s("F").split(".")[1].size
        CORBA::TypeCode.create_fixed_tc([digits.size, scale, 1].max, scale)
      else
        return value._tc if value.respond_to?(:_tc)

        raise CORBA::BAD_PARAM.new("no TypeCode for #{value.inspect}: give one to CORBA::Any.to_any")
      end
    end
  end
end

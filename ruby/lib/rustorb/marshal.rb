# frozen_string_literal: true

require "bigdecimal"

# The TypeCode-driven writer and reader: every value (an argument, a
# result, the value of an any) goes through write / read with its
# TypeCode. A value that does not fit its type is CORBA::MARSHAL
# (COMPLETED_NO), as RCLM says.
#
# Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

module Rustorb
  module Marshal
    # rorb_out_write_array kinds and pack formats (host representation).
    PRIM = {
      CORBA::TK_SHORT => [1, "s*", -(2**15), 2**15 - 1],
      CORBA::TK_USHORT => [2, "S*", 0, 2**16 - 1],
      CORBA::TK_LONG => [3, "l*", -(2**31), 2**31 - 1],
      CORBA::TK_ULONG => [4, "L*", 0, 2**32 - 1],
      CORBA::TK_LONGLONG => [5, "q*", -(2**63), 2**63 - 1],
      CORBA::TK_ULONGLONG => [6, "Q*", 0, 2**64 - 1],
      CORBA::TK_FLOAT => [7, "f*", nil, nil],
      CORBA::TK_DOUBLE => [8, "d*", nil, nil],
      CORBA::TK_OCTET => [0, "C*", 0, 255]
    }.freeze

    MIN_SIZE = {
      CORBA::TK_SHORT => 2, CORBA::TK_USHORT => 2, CORBA::TK_WCHAR => 2, CORBA::TK_LONG => 4,
      CORBA::TK_ULONG => 4, CORBA::TK_FLOAT => 4, CORBA::TK_ENUM => 4, CORBA::TK_STRING => 4,
      CORBA::TK_WSTRING => 4, CORBA::TK_SEQUENCE => 4, CORBA::TK_LONGLONG => 8,
      CORBA::TK_ULONGLONG => 8, CORBA::TK_DOUBLE => 8, CORBA::TK_LONGDOUBLE => 16,
      CORBA::TK_OBJREF => 4, CORBA::TK_TYPECODE => 4, CORBA::TK_ANY => 4
    }.freeze

    MASK64 = 2**64 - 1

    module_function

    def bad(why)
      raise CORBA::MARSHAL.new(why, 0, CORBA::COMPLETED_NO)
    end

    def int(v, lo, hi, what)
      bad("#{what} expected, got #{v.inspect}") unless v.is_a?(Integer)
      bad("#{v} out of range for #{what}") if v < lo || v > hi
      v
    end

    def float(v)
      bad("a number expected, got #{v.inspect}") unless v.is_a?(Numeric)
      v.to_f
    end

    def char_code(v, max, what)
      code = case v
             when Integer then v
             when String
               bad("one character expected, got #{v.inspect}") unless v.length == 1
               v.ord
             else bad("#{what} expected, got #{v.inspect}")
             end
      bad("#{v.inspect} is not a #{what}") if code.negative? || code > max
      code
    end

    # A wstring as UTF-16LE code units.
    def wstring_units(v)
      case v
      when String then v.encode(Encoding::UTF_16LE).b
      when Array
        v.each { |c| int(c, 0, 0xFFFF, "a UTF-16 code unit") }
        v.pack("v*")
      else bad("a wstring (Array of Integer or String) expected, got #{v.inspect}")
      end
    end

    # The union label value as rorb_tc_union takes it.
    def label_bits(label, disc)
      case disc.unaliased.kind
      when CORBA::TK_BOOLEAN then label ? 1 : 0
      when CORBA::TK_CHAR, CORBA::TK_WCHAR then label.is_a?(String) ? label.ord : label
      else label & MASK64
      end
    end

    def write(out, tc, v)
      tc = tc.resolve
      return write_extra(out, tc, v) if tc._extra

      case tc.kind
      when CORBA::TK_NULL, CORBA::TK_VOID then nil
      when CORBA::TK_SHORT then out.write_short(int(v, -(2**15), 2**15 - 1, "short"))
      when CORBA::TK_LONG then out.write_long(int(v, -(2**31), 2**31 - 1, "long"))
      when CORBA::TK_USHORT then out.write_ushort(int(v, 0, 2**16 - 1, "unsigned short"))
      when CORBA::TK_ULONG then out.write_ulong(int(v, 0, 2**32 - 1, "unsigned long"))
      when CORBA::TK_LONGLONG then out.write_longlong(int(v, -(2**63), 2**63 - 1, "long long"))
      when CORBA::TK_ULONGLONG then out.write_ulonglong(int(v, 0, MASK64, "unsigned long long"))
      when CORBA::TK_FLOAT then out.write_float(float(v))
      when CORBA::TK_DOUBLE then out.write_double(float(v))
      when CORBA::TK_BOOLEAN
        bad("true or false expected, got #{v.inspect}") unless [true, false].include?(v)
        out.write_boolean(v)
      when CORBA::TK_CHAR then out.write_char(char_code(v, 255, "char"))
      when CORBA::TK_WCHAR then out.write_wchar(char_code(v, 0xFFFF, "wchar"))
      when CORBA::TK_OCTET then out.write_octet(int(v, 0, 255, "octet"))
      when CORBA::TK_STRING
        bad("a String expected, got #{v.inspect}") unless v.is_a?(String)
        bound = tc.length
        bad("string longer than its bound #{bound}") if bound.positive? && v.length > bound
        out.write_string(v)
      when CORBA::TK_WSTRING
        units = wstring_units(v)
        bound = tc.length
        bad("wstring longer than its bound #{bound}") if bound.positive? && units.bytesize / 2 > bound
        out.write_wstring(units)
      when CORBA::TK_ENUM
        count = tc._enumerators.size
        out.write_ulong(int(v, 0, count - 1, "enumerator of #{tc.name}"))
      when CORBA::TK_STRUCT, CORBA::TK_EXCEPT
        members = tc._struct_members
        bad("#{tc.name} expected, got #{v.inspect}") unless v.respond_to?(:_values)
        values = v._values
        check_ranges(v.class, values) if v.class.const_defined?(:RANGES) && !v.class::RANGES.empty?
        members.each_with_index { |m, i| write(out, m[1], values[i]) }
      when CORBA::TK_UNION then write_union(out, tc, v)
      when CORBA::TK_SEQUENCE
        bound = tc.length
        n = v.is_a?(String) ? v.bytesize : (v.respond_to?(:size) ? v.size : bad("an Array expected, got #{v.inspect}"))
        bad("sequence longer than its bound #{bound}") if bound.positive? && n > bound
        out.write_ulong(n)
        write_elements(out, tc.content_type, v)
      when CORBA::TK_ARRAY
        n = v.is_a?(String) ? v.bytesize : (v.respond_to?(:size) ? v.size : bad("an Array expected, got #{v.inspect}"))
        bad("array of #{tc.length} elements expected, got #{n}") unless n == tc.length
        write_elements(out, tc.content_type, v)
      when CORBA::TK_ALIAS then write(out, tc.content_type, v)
      when CORBA::TK_LOCAL_INTERFACE
        raise CORBA::MARSHAL.new("#{tc.id} is a local interface", 4, CORBA::COMPLETED_NO)
      when CORBA::TK_OBJREF
        bad("an object reference expected, got #{v.inspect}") unless v.nil? || v.is_a?(CORBA::Object)
        out.write_object(v&._native_ref)
      when CORBA::TK_TYPECODE
        bad("a TypeCode expected, got #{v.inspect}") unless v.is_a?(CORBA::TypeCode)
        out.write_typecode(v.resolve.native)
      when CORBA::TK_ANY
        any = v.is_a?(CORBA::Any) ? v : CORBA::Any.to_any(v)
        out.write_any(any._native)
      when CORBA::TK_LONGDOUBLE
        ld = v.is_a?(CORBA::LongDouble) ? v : CORBA::LongDouble.new(float(v))
        out.write_longdouble(ld.hi, ld.lo)
      when CORBA::TK_FIXED then write_fixed(out, tc, v)
      else
        raise CORBA::NO_IMPLEMENT.new("TypeCode kind #{tc.kind} is not supported by rustorb's Ruby binding yet")
      end
    end

    # IDL 4 int8 and bitsets (rustorb), sent as integers.
    def write_extra(out, tc, v)
      extra = tc._extra
      if extra == :int8
        out.write_octet(int(v, -128, 127, "int8") & 0xFF)
      else
        bad("#{extra} expected, got #{v.inspect}") unless v.is_a?(extra)
        write(out, CORBA::TypeCode.simple(tc.kind), v._bits)
      end
    end

    def read_extra(inp, tc)
      extra = tc._extra
      if extra == :int8
        v = inp.read_octet
        v >= 128 ? v - 256 : v
      else
        extra._from_bits(read(inp, CORBA::TypeCode.simple(tc.kind)))
      end
    end

    # @range / @min / @max of a struct's members: BAD_PARAM out of them.
    def check_ranges(klass, values)
      klass::RANGES.each do |m, (lo, hi)|
        v = values[klass::MEMBERS.index(m)]
        next unless v.is_a?(Numeric)
        next unless (lo && v < lo) || (hi && v > hi)

        raise CORBA::BAD_PARAM.new("#{klass.name}##{m} #{v} is out of its @range", 0, CORBA::COMPLETED_NO)
      end
    end

    # The members of a struct as read: those of the generated class when
    # there is one (a TypeCode from the wire, as in an any, does not say
    # which octets are int8 or which integers bitsets).
    def struct_members(tc, klass)
      members = tc._struct_members
      return members unless klass.respond_to?(:_tc)

      own = klass._tc.resolve
      own.kind == tc.kind && own._struct_members.size == members.size ? own._struct_members : members
    end

    def in_ranges?(klass, values)
      klass::RANGES.all? do |m, (lo, hi)|
        v = values[klass::MEMBERS.index(m)]
        !v.is_a?(Numeric) || !((lo && v < lo) || (hi && v > hi))
      end
    end

    def write_elements(out, content, v)
      c = content.unaliased
      if c._extra == :int8 && v.is_a?(Array)
        v.each { |e| int(e, -128, 127, "int8") }
        out.write_array(0, v.pack("c*"))
        return
      end
      prim = !c._extra && PRIM[c.kind]
      if prim
        if c.kind == CORBA::TK_OCTET && v.is_a?(String)
          out.write_array(0, v.b)
          return
        end
        bad("an Array expected, got #{v.inspect}") unless v.is_a?(Array)
        if prim[2]
          v.each { |e| int(e, prim[2], prim[3], c.kind == CORBA::TK_OCTET ? "octet" : "integer") }
        else
          v.each { |e| float(e) }
        end
        out.write_array(prim[0], v.pack(prim[1]))
      elsif c.kind == CORBA::TK_CHAR && v.is_a?(String)
        v.each_char { |ch| write(out, c, ch) }
      else
        bad("an Array expected, got #{v.inspect}") unless v.respond_to?(:each)
        v.each { |e| write(out, content, e) }
      end
    end

    def union_member(tc, disc)
      default = nil
      tc._union_members.each do |m|
        if m[0] == :default
          default = m
        elsif m[0] == disc
          return m
        end
      end
      default
    end

    def write_union(out, tc, v)
      bad("#{tc.name} expected, got #{v.inspect}") unless v.respond_to?(:_disc)
      disc_tc = tc.discriminator_type
      disc = v._disc
      write(out, disc_tc, disc)
      m = union_member(tc, disc)
      write(out, m[2], v._value) if m
    end

    def write_fixed(out, tc, v)
      digits = tc.fixed_digits
      scale = tc.fixed_scale
      d = case v
          when BigDecimal then v
          when Integer then BigDecimal(v)
          when String, Float then BigDecimal(v.to_s)
          else bad("a BigDecimal expected, got #{v.inspect}")
          end
      bad("#{v} is not finite") unless d.finite?
      unscaled = d * (BigDecimal(10)**scale)
      bad("#{d.to_s('F')} has more than #{scale} decimal places") unless unscaled.frac.zero?
      text = unscaled.abs.to_i.to_s
      bad("#{d.to_s('F')} does not fit fixed<#{digits},#{scale}>") if text.size > digits
      text = text.rjust(digits, "0")
      out.write_fixed(text.bytes.map { |b| b - 48 }.pack("C*"), d.negative?)
    end

    def min_size(tc)
      MIN_SIZE[tc.unaliased.kind] || 1
    end

    def read(inp, tc)
      tc = tc.resolve
      return read_extra(inp, tc) if tc._extra

      case tc.kind
      when CORBA::TK_NULL, CORBA::TK_VOID then nil
      when CORBA::TK_SHORT then inp.read_short
      when CORBA::TK_LONG then inp.read_long
      when CORBA::TK_USHORT then inp.read_ushort
      when CORBA::TK_ULONG then inp.read_ulong
      when CORBA::TK_LONGLONG then inp.read_longlong
      when CORBA::TK_ULONGLONG then inp.read_ulonglong
      when CORBA::TK_FLOAT then inp.read_float
      when CORBA::TK_DOUBLE then inp.read_double
      when CORBA::TK_BOOLEAN then inp.read_boolean
      when CORBA::TK_CHAR then inp.read_char.chr(Encoding::UTF_8)
      when CORBA::TK_WCHAR then inp.read_wchar
      when CORBA::TK_OCTET then inp.read_octet
      when CORBA::TK_STRING
        s = inp.read_string
        inp.fail if tc.length.positive? && s.length > tc.length
        s
      when CORBA::TK_WSTRING
        units = inp.read_wstring.unpack("v*")
        inp.fail if tc.length.positive? && units.size > tc.length
        units
      when CORBA::TK_ENUM
        v = inp.read_ulong
        inp.fail if v >= tc._enumerators.size
        v
      when CORBA::TK_STRUCT, CORBA::TK_EXCEPT
        klass = Rustorb.find_type(tc.id)
        values = struct_members(tc, klass).map { |m| read(inp, m[1]) }
        inp.fail if klass && klass.const_defined?(:RANGES) && !klass::RANGES.empty? && !in_ranges?(klass, values)
        klass ? klass.new(*values) : GenericStruct.new(tc, values)
      when CORBA::TK_UNION
        disc = read(inp, tc.discriminator_type)
        m = union_member(tc, disc)
        value = m ? read(inp, m[2]) : nil
        klass = Rustorb.find_type(tc.id) || GenericUnion
        u = klass == GenericUnion ? GenericUnion.new(tc) : klass.new
        u._set(disc, value)
        u
      when CORBA::TK_SEQUENCE
        content = tc.content_type
        n = inp.read_length(min_size(content))
        inp.fail if tc.length.positive? && n > tc.length
        read_elements(inp, content, n)
      when CORBA::TK_ARRAY then read_elements(inp, tc.content_type, tc.length)
      when CORBA::TK_ALIAS then read(inp, tc.content_type)
      when CORBA::TK_OBJREF then CORBA::Object._from_ref(inp.read_object, tc)
      when CORBA::TK_LOCAL_INTERFACE
        raise CORBA::MARSHAL.new("#{tc.id} is a local interface", 4, CORBA::COMPLETED_NO)
      when CORBA::TK_TYPECODE then CORBA::TypeCode._wrap(inp.read_typecode)
      when CORBA::TK_ANY
        native = inp.read_any
        native && CORBA::Any._from_native(native)
      when CORBA::TK_LONGDOUBLE
        hi, lo = inp.read_longdouble
        CORBA::LongDouble._from_bits(hi, lo)
      when CORBA::TK_FIXED
        digits, negative = inp.read_fixed(tc.fixed_digits)
        text = digits.bytes.map { |b| (b + 48).chr }.join
        d = BigDecimal(text.empty? ? "0" : text) / (BigDecimal(10)**tc.fixed_scale)
        negative ? -d : d
      else
        raise CORBA::NO_IMPLEMENT.new("TypeCode kind #{tc.kind} is not supported by rustorb's Ruby binding yet")
      end
    end

    def read_elements(inp, content, n)
      c = content.unaliased
      return inp.read_array(0, n).unpack("c*") if c._extra == :int8

      prim = !c._extra && PRIM[c.kind]
      return inp.read_array(prim[0], n).unpack(prim[1]) if prim

      Array.new(n) { read(inp, content) }
    end
  end

  # A struct or exception value of a type without a Ruby class here (in an
  # any from elsewhere).
  class GenericStruct
    attr_reader :_tc, :_values

    def initialize(tc, values)
      @_tc = tc
      @_values = values
    end

    def [](name)
      i = @_tc._struct_members.index { |m| m[0] == name.to_s }
      i && @_values[i]
    end

    def ==(other)
      other.is_a?(GenericStruct) && other._tc.equal?(@_tc) && other._values == @_values
    end

    def inspect
      "#<#{@_tc.name} #{@_tc._struct_members.map(&:first).zip(@_values).map { |n, v| "#{n}=#{v.inspect}" }.join(' ')}>"
    end
  end

  class GenericUnion
    attr_reader :_tc, :_disc, :_value

    def initialize(tc)
      @_tc = tc
    end

    def _set(disc, value)
      @_disc = disc
      @_value = value
    end

    def ==(other)
      other.is_a?(GenericUnion) && other._disc == @_disc && other._value == @_value
    end
  end
end

# frozen_string_literal: true

# CORBA::TypeCode: a native TypeCode, with what the marshaller needs kept on
# the Ruby side. TypeCodes made here (by the generated code, the ORB's
# create_*_tc) keep their member TypeCodes; those read from the wire ask the
# native one, lazily.
#
# Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

module CORBA
  TK_NULL = 0
  TK_VOID = 1
  TK_SHORT = 2
  TK_LONG = 3
  TK_USHORT = 4
  TK_ULONG = 5
  TK_FLOAT = 6
  TK_DOUBLE = 7
  TK_BOOLEAN = 8
  TK_CHAR = 9
  TK_OCTET = 10
  TK_ANY = 11
  TK_TYPECODE = 12
  TK_PRINCIPAL = 13
  TK_OBJREF = 14
  TK_STRUCT = 15
  TK_UNION = 16
  TK_ENUM = 17
  TK_STRING = 18
  TK_SEQUENCE = 19
  TK_ARRAY = 20
  TK_ALIAS = 21
  TK_EXCEPT = 22
  TK_LONGLONG = 23
  TK_ULONGLONG = 24
  TK_LONGDOUBLE = 25
  TK_WCHAR = 26
  TK_WSTRING = 27
  TK_FIXED = 28
  TK_VALUE = 29
  TK_VALUE_BOX = 30
  TK_NATIVE = 31
  TK_ABSTRACT_INTERFACE = 32
  TK_LOCAL_INTERFACE = 33
  TK_COMPONENT = 34
  TK_HOME = 35
  TK_EVENT = 36

  class TypeCode
    class BadKind < UserException
      ID = "IDL:omg.org/CORBA/TypeCode/BadKind:1.0"
      MEMBERS = [].freeze
    end

    class Bounds < UserException
      ID = "IDL:omg.org/CORBA/TypeCode/Bounds:1.0"
      MEMBERS = [].freeze
    end

    Rustorb::Native::LOCAL_EXCEPTIONS[14] = ->(_) { BadKind.new }
    Rustorb::Native::LOCAL_EXCEPTIONS[15] = ->(_) { Bounds.new }

    attr_reader :native

    # native: a Rustorb::Native::TC. members: [[name, TypeCode]] (struct,
    # except), [[label, name, TypeCode]] (union, label :default), or names
    # (enum); content: the TypeCode of a sequence, array or alias.
    # extra: what the native TypeCode does not say, for the IDL 4 types
    # sent as integers (rustorb): :int8 (an octet read as signed), or the
    # class of a bitset (sent as the unsigned integer of the TypeCode).
    def initialize(native, members: nil, content: nil, discriminator: nil, recursive_id: nil,
                   extra: nil)
      @native = native
      @kind = native&.kind
      @members = members
      @content = content
      @discriminator = discriminator
      @recursive_id = recursive_id
      @extra = extra
    end

    def _extra
      @extra
    end

    def self._wrap(native)
      native && new(native)
    end

    # --- the queries of CORBA::TypeCode ---

    def kind
      return resolve.kind if @recursive_id

      @kind
    end

    def id
      @id ||= @recursive_id || native.id
    end

    def name
      @name ||= native.name
    end

    def member_count
      @member_count ||= native.member_count
    end

    def member_name(index)
      native.member_name(index)
    end

    def member_type(index)
      if @members && kind != TK_ENUM
        m = @members[index] or raise Bounds
        return kind == TK_UNION ? m[2] : m[1]
      end
      @member_types ||= {}
      @member_types[index] ||= TypeCode._wrap(native.member_type(index))
    end

    # The label of a union member, as a Ruby value (:default for the
    # default member).
    def member_label(index)
      raise BadKind unless kind == TK_UNION
      return @members[index][0] if @members

      return :default if index == default_index

      CORBA::Any._from_native(native.member_label(index)).value
    end

    def discriminator_type
      @discriminator ||= TypeCode._wrap(native.discriminator_type)
    end

    def default_index
      @default_index ||= native.default_index
    end

    def length
      @length ||= native.length
    end

    def content_type
      @content ||= TypeCode._wrap(native.content_type)
    end

    def fixed_digits
      @fixed_digits ||= native.fixed_digits
    end

    def fixed_scale
      @fixed_scale ||= native.fixed_scale
    end

    def equal?(other)
      other.is_a?(TypeCode) && resolve.native.equal(other.resolve.native)
    end

    def equivalent?(other)
      other.is_a?(TypeCode) && resolve.native.equivalent(other.resolve.native)
    end

    def ==(other)
      equal?(other)
    end

    def get_compact_typecode
      TypeCode._wrap(native.compact)
    end

    # The TypeCode without its aliases.
    def unaliased
      tc = resolve
      tc = tc.content_type while tc.kind == TK_ALIAS
      tc
    end

    # A placeholder of create_recursive_tc stands for the type with its id
    # once that is made (Rustorb.find_tc).
    def resolve
      return self unless @recursive_id

      @resolved ||= Rustorb.find_tc(@recursive_id) or
        raise CORBA::BAD_TYPECODE.new("unresolved recursive TypeCode #{@recursive_id}")
    end

    # Enumerator names of an enum.
    def _enumerators
      @enumerators ||= @members || Array.new(member_count) { |i| member_name(i) }
    end

    # [label, name, TypeCode] of each union member.
    def _union_members
      @union_members ||= @members || Array.new(member_count) do |i|
        [member_label(i), member_name(i), member_type(i)]
      end
    end

    # [name, TypeCode] of each struct or exception member.
    def _struct_members
      @struct_members ||= @members || Array.new(member_count) { |i| [member_name(i), member_type(i)] }
    end

    def inspect
      "#<CORBA::TypeCode kind=#{kind}#{@recursive_id || ([TK_STRUCT, TK_UNION, TK_ENUM, TK_ALIAS, TK_EXCEPT, TK_OBJREF].include?(kind) ? " #{id}" : '')}>"
    end
    alias to_s inspect

    # --- constructors (ORB::create_*_tc; generated code) ---

    SIMPLE = {}

    def self.simple(kind)
      SIMPLE[kind] ||= new(Rustorb::Native::TC.simple(kind))
    end

    def self.create_string_tc(bound)
      bound.zero? ? CORBA._tc_string : new(Rustorb::Native::TC.string(false, bound))
    end

    def self.create_wstring_tc(bound)
      bound.zero? ? CORBA._tc_wstring : new(Rustorb::Native::TC.string(true, bound))
    end

    def self.create_fixed_tc(digits, scale)
      new(Rustorb::Native::TC.fixed(digits, scale))
    end

    def self.create_sequence_tc(bound, content)
      new(Rustorb::Native::TC.sequence(content.resolve_for_build, bound), content: content)
    end

    def self.create_array_tc(length, content)
      new(Rustorb::Native::TC.array(content.resolve_for_build, length), content: content)
    end

    def self.create_alias_tc(id, name, content)
      new(Rustorb::Native::TC.alias(TK_ALIAS, id, name, content.resolve_for_build), content: content)
    end

    def self.create_interface_tc(id, name)
      new(Rustorb::Native::TC.interface(TK_OBJREF, id, name))
    end

    def self.create_struct_tc(id, name, members)
      make_struct(TK_STRUCT, id, name, members)
    end

    def self.create_exception_tc(id, name, members)
      make_struct(TK_EXCEPT, id, name, members)
    end

    def self.make_struct(kind, id, name, members)
      native = Rustorb::Native::TC.struct(kind, id, name, members.map(&:first),
                                          members.map { |m| m[1].resolve_for_build })
      Rustorb.register_tc(new(native, members: members))
    end

    # members: [[label, name, TypeCode]]; the label :default marks the
    # default member.
    def self.create_union_tc(id, name, discriminator, members)
      default = members.index { |m| m[0] == :default } || -1
      labels = members.map { |m| m[0] == :default ? 0 : Rustorb::Marshal.label_bits(m[0], discriminator) }
      native = Rustorb::Native::TC.union(id, name, discriminator.resolve_for_build, labels, default,
                                         members.map { |m| m[1] }, members.map { |m| m[2].resolve_for_build })
      Rustorb.register_tc(new(native, members: members, discriminator: discriminator))
    end

    def self.create_enum_tc(id, name, names)
      Rustorb.register_tc(new(Rustorb::Native::TC.enum(id, name, names), members: names))
    end

    # IDL 4 int8 (rustorb): the TypeCode of octet, read and written as a
    # signed value.
    def self.create_int8_tc
      @int8_tc ||= new(Rustorb::Native::TC.simple(TK_OCTET), extra: :int8)
    end

    # An IDL 4 bitset (rustorb): the TypeCode of the unsigned integer kind
    # that holds its bits; values are instances of klass (a Rustorb::Bitset).
    def self._bitset_tc(kind, klass)
      new(Rustorb::Native::TC.simple(kind), extra: klass)
    end

    def self.create_recursive_tc(id)
      new(Rustorb::Native::TC.recursive(id), recursive_id: id)
    end

    # The native TypeCode to build a containing one with: a recursive
    # placeholder stays one (the core replaces it).
    def resolve_for_build
      native
    end
  end

  NULL_TC_KINDS = {
    null: TK_NULL, void: TK_VOID, short: TK_SHORT, long: TK_LONG, ushort: TK_USHORT,
    ulong: TK_ULONG, float: TK_FLOAT, double: TK_DOUBLE, boolean: TK_BOOLEAN, char: TK_CHAR,
    octet: TK_OCTET, any: TK_ANY, TypeCode: TK_TYPECODE, longlong: TK_LONGLONG,
    ulonglong: TK_ULONGLONG, longdouble: TK_LONGDOUBLE, wchar: TK_WCHAR, string: TK_STRING,
    wstring: TK_WSTRING
  }.freeze

  NULL_TC_KINDS.each do |name, kind|
    define_singleton_method("_tc_#{name}") { TypeCode.simple(kind) }
  end

  def self._tc_int8
    TypeCode.create_int8_tc
  end

  def self._tc_Object
    @tc_object ||= TypeCode.create_interface_tc("IDL:omg.org/CORBA/Object:1.0", "Object")
  end
end

require "monitor"

module Rustorb
  @tcs = {}
  @types = {}
  @types_lock = Mutex.new
  @tc_monitor = Monitor.new
  @tc_building = []

  # The version of the generated code this runtime reads (rustorb-idl
  # -bruby writes Rustorb.check_generated_code(N)).
  GENERATED_CODE_VERSION = 1

  class << self
    # Named TypeCodes by repository id: what a recursive placeholder stands
    # for.
    def register_tc(tc)
      @types_lock.synchronize { @tcs[tc.id] = tc }
      tc
    end

    def check_generated_code(version)
      return if version == GENERATED_CODE_VERSION

      raise LoadError, "code generated for rustorb's Ruby runtime #{version}, this runtime reads #{GENERATED_CODE_VERSION}: regenerate it with a matching rustorb-idl"
    end

    # The TypeCode of a generated type (holder: its class or module), made
    # by the block on first use. A type met again while its own TypeCode is
    # being made (a recursive struct or union) is a create_recursive_tc
    # placeholder; a TypeCode holding a placeholder of an enclosing type is
    # not kept. An alias (recursive: false) is never a placeholder: a cycle
    # through it goes through a struct or union too.
    def lazy_tc(holder, id, recursive: true)
      tc = holder.instance_variable_get(:@_tc)
      return tc if tc

      @tc_monitor.synchronize do
        tc = holder.instance_variable_get(:@_tc)
        return tc if tc

        i = recursive && @tc_building.index { |f| f[0] == id }
        if i
          ((i + 1)...@tc_building.size).each { |j| @tc_building[j][1] = true }
          return CORBA::TypeCode.create_recursive_tc(id)
        end

        @tc_building.push([id, false])
        begin
          tc = yield
        ensure
          frame = @tc_building.pop
        end
        holder.instance_variable_set(:@_tc, tc) unless frame[1]
        tc
      end
    end

    def find_tc(id)
      @types_lock.synchronize { @tcs[id] }
    end

    # The Ruby classes of named types (struct, union, exception), by
    # repository id: what values read from the wire are made of.
    def register_type(klass)
      @types_lock.synchronize { @types[klass::ID] = klass }
      klass
    end

    def find_type(id)
      @types_lock.synchronize { @types[id] }
    end
  end
end

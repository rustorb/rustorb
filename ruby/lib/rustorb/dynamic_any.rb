# frozen_string_literal: true

# DynamicAny: building and taking apart values of types known only by
# their TypeCode.
#
#   factory = orb.resolve_initial_references("DynAnyFactory")
#   d = factory.create_dyn_any_from_type_code(tc)   # a DynStruct...
#   d.insert_long(1)
#   d.next
#   d.insert_string("x")
#   any = d.to_any
#
# The factory gives the DynAny of the class its type calls for: DynFixed,
# DynEnum, DynStruct (structs and exceptions), DynUnion, DynSequence,
# DynArray, DynValue, DynValueBox or plain DynAny. The operations have
# their IDL names; values are Ruby values, anys CORBA::Any, TCKinds the
# CORBA::TK_ Integers. Components share their state with the DynAny they
# belong to. See docs/dynany.md.
#
# Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

module DynamicAny
  # A member and its value: id (the name) and value (a CORBA::Any).
  class NameValuePair < Rustorb::Struct
    ID = "IDL:omg.org/DynamicAny/NameValuePair:1.0"
    MEMBERS = %w[id value].freeze
    attr_accessor :id, :value
  end

  # A member and its DynAny: id and value.
  class NameDynAnyPair < Rustorb::Struct
    ID = "IDL:omg.org/DynamicAny/NameDynAnyPair:1.0"
    MEMBERS = %w[id value].freeze
    attr_accessor :id, :value
  end

  def self._exception(scope, name, id, code)
    klass = Class.new(CORBA::UserException)
    scope.const_set(name, klass)
    klass.const_set(:ID, id)
    klass.const_set(:MEMBERS, [].freeze)
    Rustorb::Native::LOCAL_EXCEPTIONS[code] = ->(_) { klass.new }
    klass
  end

  # Ruby values as anys of the native layer, and back.
  module Values
    module_function

    def any(value, tc) = CORBA::Any.to_any(value, tc)._native

    def value(native) = CORBA::Any._from_native(native).value

    def any_of(a)
      raise CORBA::BAD_PARAM.new("a CORBA::Any expected, got #{a.inspect}") unless a.is_a?(CORBA::Any)

      a._native
    end
  end

  class DynAny
    ID = "IDL:omg.org/DynamicAny/DynAny:1.0"

    def initialize(native)
      @n = native
    end

    def _n = @n

    def self._wrap(native)
      native && CLASSES.fetch(native.kind, DynAny).new(native)
    end

    def _is_equivalent(other) = other.is_a?(DynAny) && @n.same(other._n)
    alias _is_equivalent? _is_equivalent

    def _is_a(id) = self.class.ancestors.any? { |k| k.const_defined?(:ID, false) && k::ID == id }
    alias _is_a? _is_a

    def type = CORBA::TypeCode._wrap(@n.type)
    def assign(dyn_any) = @n.assign(dyn_any._n)
    def from_any(value) = @n.from_any(Values.any_of(value))
    def to_any = CORBA::Any._from_native(@n.to_any)
    def equal(dyn_any) = @n.equal(dyn_any._n)
    def destroy = @n.destroy
    def copy = DynAny._wrap(@n.copy)

    def insert_reference(value) = @n.insert(CORBA::TK_OBJREF, Values.any(value, CORBA._tc_Object))
    def get_reference = Values.value(@n.get(CORBA::TK_OBJREF))
    def insert_any(value) = @n.insert(CORBA::TK_ANY, Values.any(value, CORBA._tc_any))
    def get_any = Values.value(@n.get(CORBA::TK_ANY))
    def insert_typecode(value) = @n.insert(CORBA::TK_TYPECODE, Values.any(value, CORBA._tc_TypeCode))
    def get_typecode = Values.value(@n.get(CORBA::TK_TYPECODE))
    def insert_dyn_any(value) = @n.insert_dyn_any(value._n)
    def get_dyn_any = DynAny._wrap(@n.get_dyn_any)

    def insert_val(value)
      tc = value.respond_to?(:_tc) ? value._tc : CORBA::Any.infer(value)
      @n.insert(CORBA::TK_VALUE, Values.any(value, tc))
    end

    def get_val = Values.value(@n.get(CORBA::TK_VALUE))
    def insert_abstract(_value) = @n.no_implement
    def get_abstract = @n.no_implement

    def seek(index) = @n.seek(index)
    def rewind = @n.rewind
    def next = @n.next
    def component_count = @n.component_count
    def current_component = DynAny._wrap(@n.current_component)

    BASIC = {
      "boolean" => :_tc_boolean, "octet" => :_tc_octet, "char" => :_tc_char,
      "short" => :_tc_short, "ushort" => :_tc_ushort, "long" => :_tc_long,
      "ulong" => :_tc_ulong, "float" => :_tc_float, "double" => :_tc_double,
      "longdouble" => :_tc_longdouble, "longlong" => :_tc_longlong,
      "ulonglong" => :_tc_ulonglong, "wchar" => :_tc_wchar
    }.freeze

    (BASIC.to_a + [%w[string _tc_string], %w[wstring _tc_wstring]]).each do |name, tc_name|
      define_method("insert_#{name}") do |value|
        tc = CORBA.public_send(tc_name)
        @n.insert(tc.kind, Values.any(value, tc))
      end
      define_method("get_#{name}") { Values.value(@n.get(CORBA.public_send(tc_name).kind)) }
    end

    BASIC.each do |name, tc_name|
      define_method("insert_#{name}_seq") do |value|
        tc = CORBA.public_send(tc_name)
        @n.insert_seq(tc.kind, Values.any(value, CORBA::TypeCode.create_sequence_tc(0, tc)))
      end
      define_method("get_#{name}_seq") { Values.value(@n.get_seq(CORBA.public_send(tc_name).kind)) }
    end
  end

  DynamicAny._exception(DynAny, :InvalidValue, "IDL:omg.org/DynamicAny/DynAny/InvalidValue:1.0", 17)
  DynamicAny._exception(DynAny, :TypeMismatch, "IDL:omg.org/DynamicAny/DynAny/TypeMismatch:1.0", 18)
  DynamicAny._exception(self, :MustTruncate, "IDL:omg.org/DynamicAny/MustTruncate:1.0", 20)

  # DynFixed: the value as a String, such as "-12.5".
  class DynFixed < DynAny
    ID = "IDL:omg.org/DynamicAny/DynFixed:1.0"

    def get_value = @n.get_fixed_value
    def set_value(val) = @n.set_fixed_value(val.is_a?(BigDecimal) ? val.to_s("F") : val.to_s)
  end

  class DynEnum < DynAny
    ID = "IDL:omg.org/DynamicAny/DynEnum:1.0"

    def get_as_string = @n.get_as_string
    def set_as_string(value) = @n.set_as_string(value)
    def get_as_ulong = @n.get_as_ulong
    def set_as_ulong(value) = @n.set_as_ulong(value)
  end

  # The member operations of DynStruct and DynValue.
  module Members
    def current_member_name = @n.current_member_name
    def current_member_kind = @n.current_member_kind

    def get_members
      Array.new(@n.member_count) do |i|
        name, d = @n.member_at(i)
        NameValuePair.new(name, CORBA::Any._from_native(d.to_any))
      end
    end

    def set_members(value)
      @n.set_members(value.map(&:id), value.map { |p| Values.any_of(p.value) })
    end

    def get_members_as_dyn_any
      Array.new(@n.member_count) do |i|
        name, d = @n.member_at(i)
        NameDynAnyPair.new(name, DynAny._wrap(d))
      end
    end

    def set_members_as_dyn_any(value)
      @n.set_members_as_dyn_any(value.map(&:id), value.map { |p| p.value._n })
    end
  end

  # For structs and exceptions.
  class DynStruct < DynAny
    ID = "IDL:omg.org/DynamicAny/DynStruct:1.0"
    include Members
  end

  class DynUnion < DynAny
    ID = "IDL:omg.org/DynamicAny/DynUnion:1.0"

    def get_discriminator = DynAny._wrap(@n.get_discriminator)
    def set_discriminator(d) = @n.set_discriminator(d._n)
    def set_to_default_member = @n.set_to_default_member
    def set_to_no_active_member = @n.set_to_no_active_member
    def has_no_active_member = @n.has_no_active_member
    def discriminator_kind = @n.discriminator_kind
    def member = DynAny._wrap(@n.member)
    def member_name = @n.member_name
    def member_kind = @n.member_kind
  end

  # The element operations of DynSequence and DynArray.
  module Elements
    def get_elements = Array.new(@n.element_count) { |i| CORBA::Any._from_native(@n.element_at(i).to_any) }
    def set_elements(value) = @n.set_elements(value.map { |a| Values.any_of(a) })
    def get_elements_as_dyn_any = Array.new(@n.element_count) { |i| DynAny._wrap(@n.element_at(i)) }
    def set_elements_as_dyn_any(value) = @n.set_elements_as_dyn_any(value.map(&:_n))
  end

  class DynSequence < DynAny
    ID = "IDL:omg.org/DynamicAny/DynSequence:1.0"
    include Elements

    def get_length = @n.get_length
    def set_length(len) = @n.set_length(len)
  end

  class DynArray < DynAny
    ID = "IDL:omg.org/DynamicAny/DynArray:1.0"
    include Elements
  end

  class DynValueCommon < DynAny
    ID = "IDL:omg.org/DynamicAny/DynValueCommon:1.0"

    def is_null = @n.is_null
    def set_to_null = @n.set_to_null
    def set_to_value = @n.set_to_value
  end

  # For valuetypes and eventtypes.
  class DynValue < DynValueCommon
    ID = "IDL:omg.org/DynamicAny/DynValue:1.0"
    include Members
  end

  class DynValueBox < DynValueCommon
    ID = "IDL:omg.org/DynamicAny/DynValueBox:1.0"

    def get_boxed_value = CORBA::Any._from_native(@n.get_boxed_value)
    def set_boxed_value(boxed) = @n.set_boxed_value(Values.any_of(boxed))
    def get_boxed_value_as_dyn_any = DynAny._wrap(@n.get_boxed_value_as_dyn_any)
    def set_boxed_value_as_dyn_any(boxed) = @n.set_boxed_value_as_dyn_any(boxed._n)
  end

  DynAny::CLASSES = {
    CORBA::TK_FIXED => DynFixed, CORBA::TK_ENUM => DynEnum, CORBA::TK_STRUCT => DynStruct,
    CORBA::TK_EXCEPT => DynStruct, CORBA::TK_UNION => DynUnion, CORBA::TK_SEQUENCE => DynSequence,
    CORBA::TK_ARRAY => DynArray, CORBA::TK_VALUE => DynValue, CORBA::TK_EVENT => DynValue,
    CORBA::TK_VALUE_BOX => DynValueBox
  }.freeze

  # The initial reference "DynAnyFactory".
  class DynAnyFactory
    ID = "IDL:omg.org/DynamicAny/DynAnyFactory:1.0"

    def self.instance = @instance ||= new

    def _is_a(id) = id == ID
    alias _is_a? _is_a

    def _non_existent = false
    alias _non_existent? _non_existent

    def create_dyn_any(value) = DynAny._wrap(Rustorb::Native::DynAny.create(Values.any_of(value), true))
    def create_dyn_any_from_type_code(type) = DynAny._wrap(Rustorb::Native::DynAny.from_type_code(type.resolve.native))

    def create_dyn_any_without_truncation(value)
      DynAny._wrap(Rustorb::Native::DynAny.create(Values.any_of(value), false))
    end

    def create_multiple_dyn_anys(values, allow_truncate)
      values.map { |v| DynAny._wrap(Rustorb::Native::DynAny.create(Values.any_of(v), allow_truncate)) }
    end

    def create_multiple_anys(values) = values.map(&:to_any)
  end

  DynamicAny._exception(DynAnyFactory, :InconsistentTypeCode,
                        "IDL:omg.org/DynamicAny/DynAnyFactory/InconsistentTypeCode:1.0", 19)
end

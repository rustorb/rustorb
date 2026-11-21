# frozen_string_literal: true

# DynamicAny on values of generated types (AnyTest.idl); the DynAny probe
# (interop/impls/rustorbrb/lib/dynany_probe.rb) covers the rest.

require_relative "helper"
require "AnyTestC"

class DynAnyTest < Minitest::Test
  A = AnyTest

  def factory = TestORB.orb.resolve_initial_references("DynAnyFactory")

  def test_classes
    assert_includes TestORB.orb.list_initial_services, "DynAnyFactory"
    f = factory
    assert_instance_of DynamicAny::DynStruct, f.create_dyn_any_from_type_code(A::Point._tc)
    assert_instance_of DynamicAny::DynUnion, f.create_dyn_any_from_type_code(A::Choice._tc)
    assert_instance_of DynamicAny::DynEnum, f.create_dyn_any_from_type_code(A::Shade._tc)
    assert_instance_of DynamicAny::DynSequence, f.create_dyn_any_from_type_code(A::LongSeq._tc)
    assert_instance_of DynamicAny::DynArray, f.create_dyn_any_from_type_code(A::ShortArray._tc)
    assert_instance_of DynamicAny::DynFixed, f.create_dyn_any_from_type_code(A::Money._tc)
    assert_instance_of DynamicAny::DynAny, f.create_dyn_any_from_type_code(CORBA._tc_long)
    assert f.create_dyn_any_from_type_code(A::Point._tc)._is_a?("IDL:omg.org/DynamicAny/DynAny:1.0")
  end

  def test_struct
    d = factory.create_dyn_any(CORBA::Any.to_any(A::Point.new(1, 2.5)))
    assert_equal 2, d.component_count
    assert_equal "x", d.current_member_name
    assert_equal 1, d.get_long
    d.next
    d.insert_double(-0.5)
    assert_equal A::Point.new(1, -0.5), d.to_any.value
    m = d.get_members
    assert_equal %w[x y], m.map(&:id)
    assert_equal(-0.5, m[1].value.value)
    m[0].value = CORBA::Any.to_any(9, CORBA._tc_long)
    d.set_members(m)
    assert_equal A::Point.new(9, -0.5), d.to_any.value
    assert_raises(DynamicAny::DynAny::TypeMismatch) { d.insert_string("x") }
    d.destroy
  end

  def test_union_enum_sequence
    f = factory
    u = f.create_dyn_any(CORBA::Any.to_any(A::Choice.new._put("s", "three", 3)))
    assert_equal "s", u.member_name
    assert_equal 3, u.get_discriminator.get_long
    assert_equal "three", u.member.get_string
    e = f.create_dyn_any_from_type_code(A::Shade._tc)
    e.set_as_string("DIM")
    assert_equal A::DIM, e.to_any.value
    assert_raises(DynamicAny::DynAny::InvalidValue) { e.set_as_ulong(7) }
    s = f.create_dyn_any_from_type_code(A::LongSeq._tc)
    s.insert_long_seq([4, 5, 6])
    assert_equal [4, 5, 6], s.get_long_seq
    assert_equal [4, 5, 6], s.get_elements.map(&:value)
    assert_equal [4, 5, 6], s.to_any.value
    fx = f.create_dyn_any_from_type_code(A::Money._tc)
    assert fx.set_value(BigDecimal("-1.25"))
    assert_equal "-1.25", fx.get_value
  end

  def test_errors
    assert_raises(DynamicAny::DynAnyFactory::InconsistentTypeCode) do
      factory.create_dyn_any_from_type_code(CORBA._tc_Principal)
    end
    d = factory.create_dyn_any_from_type_code(CORBA._tc_long)
    d.destroy
    assert_raises(CORBA::OBJECT_NOT_EXIST) { d.get_long }
  end
end

# frozen_string_literal: true

# The code rustorb-idl -bruby generates for cpp/tests/idl/CxxMapping.idl
# (the IDL every binding's mapping test uses), called through the ORB.

require_relative "helper"
require "CxxMappingS"

M = CxxMapping

class TypesServant < POA::CxxMapping::Types
  %w[tree node by_bool by_char by_enum by_ulonglong with_array holder outer keywords global
     short_string price measure].each { |t| define_method("echo_#{t}") { |v| v } }

  def echo_money(m, total) = [m, total + m, m.truncate]

  def echo_quad(q, twice) = [q, CORBA::LongDouble.new(twice.to_f * 2), CORBA::LongDouble.new(-q.to_f)]

  def fail(code)
    raise M::Empty if code.zero?

    raise M::Failure.new(code, "failed", M::CHERRY)
  end

  def laters(n) = Array.new(n) { nil }

  def delete(what)
    @deleted = what
  end

  attr_reader :deleted
end

class MultiServant < POA::CxxMapping::Multi
  attr_accessor :label
  attr_reader :notes

  def initialize
    super
    @label = "start"
    @notes = Queue.new
  end

  def id = 3

  def base_op(fail)
    raise M::Base::BaseError.new("asked to") if fail
  end

  def derived_op(a, b) = [a + b, b * 2, a - b]
  def notify(msg) = @notes << msg
  def as_base = _this
  def other_op = "other"
  def multi_op(info) = M::Base::BaseInfo.new(info.id + 1)
end

class MappingTest < Minitest::Test
  def setup
    @types = TestORB.ref(TypesServant.new, M::Types)
  end

  def test_constants
    assert_equal 10, GLOBAL_LIMIT
    assert_equal(-3, M::C_SHORT)
    assert_equal 18_000_000_000_000_000_000, M::C_ULONGLONG
    assert_equal "q", M::C_CHAR
    assert_equal "w".ord, M::C_WCHAR
    assert_equal "wide".encode("UTF-16LE").unpack("v*"), M::C_WSTRING
    assert_equal M::BANANA, M::C_ENUM
    assert_equal BigDecimal("12.50"), M::C_FIXED
    assert_equal 2.5, M::C_QUAD.to_f
    assert_equal 7, M::Base::BASE_CONST
    assert_equal 1, M::REOPENED
    assert_equal [0, 1, 2], [M::Fruit::APPLE, M::Fruit::BANANA, M::Fruit::CHERRY]
    assert_equal %w[APPLE BANANA CHERRY], M::Fruit::ENUMERATORS
  end

  def test_recursive_types
    t = M::Tree.new("root", [M::Tree.new("a", []), M::Tree.new("b", [M::Tree.new("c", [])])])
    assert_equal t, @types.echo_tree(t)
    leaf = M::Node.new.tap { |n| n.leaf = 5 }
    n = M::Node.new.tap { |b| b.branch = [leaf, M::Node.new.tap { |b2| b2.branch = [] }] }
    assert_equal n, @types.echo_node(n)
    tc = M::Tree._tc
    assert_equal CORBA::TK_STRUCT, tc.kind
    assert_equal CORBA::TK_SEQUENCE, tc.member_type(1).kind
    assert_equal CORBA::TK_STRUCT, tc.member_type(1).content_type.kind
  end

  def test_unions
    yes = M::ByBool.new.tap { |u| u.yes = 3 }
    assert_equal yes, @types.echo_by_bool(yes)
    no = M::ByBool.new.tap { |u| u.no = "n" }
    assert_equal "n", @types.echo_by_bool(no).no
    c = M::ByChar.new._put("bc", "cee", "c")
    got = @types.echo_by_char(c)
    assert_equal ["c", "cee"], [got._disc, got.bc]
    assert_raises(CORBA::BAD_PARAM) { got.a }
    other = M::ByEnum.new._put("other", true, M::BANANA)
    got = @types.echo_by_enum(other)
    assert_equal [M::BANANA, true], [got._disc, got.other]
    assert got._is_at_default?
    dflt = M::ByEnum.new.tap { |u| u.other = false }
    assert_equal M::BANANA, dflt._disc
    none = M::ByULongLong.new._default
    got = @types.echo_by_ulonglong(none)
    assert_nil got._member
    seven = M::ByULongLong.new.tap { |u| u.seven = 0.5 }
    assert_equal 0.5, @types.echo_by_ulonglong(seven).seven
    arr = M::WithArray.new.tap { |u| u.cells = [[1, 2, 3], [4, 5, 6]] }
    assert_equal arr, @types.echo_with_array(arr)
  end

  def holder(**over)
    h = M::Holder.new([1, 2, 3], [M::Pair.new(1, 2)], [[1, 2, 3], [4, 5, 6]], "tag", [1, 2],
                      "wide".encode("UTF-16LE").unpack("v*"), [true, false], [0x3042], [["a"], []],
                      nil, M::ByEnum.new.tap { |u| u.apple_count = 4 })
    over.each { |k, v| h.public_send("#{k}=", v) }
    h
  end

  def test_struct_members
    h = holder
    assert_equal h, @types.echo_holder(h)
    with_ref = holder(target: @types)
    got = @types.echo_holder(with_ref)
    assert got.target._is_equivalent?(@types)
    o = M::Outer.new(M::Outer::Inner.new(1), M::Outer::OFF)
    assert_equal o, @types.echo_outer(o)
    k = M::Keywords.new(1, "ns")
    assert_equal k, @types.echo_keywords(k)
    assert_equal 1, @types.echo_keywords(k).r_class
    g = GlobalPoint.new(1, 2)
    assert_equal g, @types.echo_global(g)
  end

  def test_bounds_and_bad_values
    assert_equal "12345678", @types.echo_short_string("12345678")
    assert_raises(CORBA::MARSHAL) { @types.echo_short_string("123456789") }
    assert_raises(CORBA::MARSHAL) { @types.echo_holder(holder(bounded: [1, 2, 3, 4, 5, 6])) }
    assert_raises(CORBA::MARSHAL) { @types.echo_holder(holder(wide: [1, 2, 3, 4, 5])) }
    assert_raises(CORBA::MARSHAL) { @types.echo_holder(holder(numbers: [1, 2])) }
    assert_raises(CORBA::MARSHAL) { @types.echo_holder(holder(tag: 5)) }
    assert_raises(CORBA::MARSHAL) { @types.echo_outer(M::Outer.new(M::Outer::Inner.new(1), 7)) }
  end

  def test_fixed_and_long_double
    m, total, whole = @types.echo_money(BigDecimal("12.34"), BigDecimal("1.00"))
    assert_equal [BigDecimal("12.34"), BigDecimal("13.34"), BigDecimal("12")], [m, total, whole]
    p = M::Price.new(BigDecimal("-0.05"), BigDecimal("0.0000000000000000000000000000001"),
                     [BigDecimal("1"), BigDecimal("999.99")])
    assert_equal p, @types.echo_price(p)
    q, twice, neg = @types.echo_quad(CORBA::LongDouble.new(1.5), CORBA::LongDouble.new(2.0))
    assert_equal [1.5, 4.0, -1.5], [q.to_f, twice.to_f, neg.to_f]
    ms = M::Measure.new(CORBA::LongDouble.new(0.25), [CORBA::LongDouble.new(1.0)])
    assert_equal ms, @types.echo_measure(ms)
  end

  def test_exceptions_and_misc
    e = assert_raises(M::Failure) { @types.fail(3) }
    assert_equal [3, "failed", M::CHERRY], [e.code, e.message, e.kind]
    assert_raises(M::Empty) { @types.fail(0) }
    assert_equal [nil, nil], @types.laters(2)
    @types.delete("x")
  end

  def test_inheritance
    servant = MultiServant.new
    multi = TestORB.ref(servant, M::Multi)
    assert_kind_of M::Base, multi
    assert_kind_of M::Other, multi
    assert_equal 3, multi.id
    assert_equal "start", multi.label
    multi.label = "x"
    assert_equal "x", servant.label
    assert_nil multi.base_op(false)
    assert_equal "asked to", assert_raises(M::Base::BaseError) { multi.base_op(true) }.why
    assert_equal [5, 4, 1], multi.derived_op(3, 2)
    multi.notify("hi")
    assert_equal "hi", servant.notes.pop
    base = multi.as_base
    assert_kind_of M::Base, base
    assert base._is_a?(M::Multi::ID)
    narrowed = M::Multi._narrow(base)
    assert_equal "other", narrowed.other_op
    assert_equal 6, multi.multi_op(M::Base::BaseInfo.new(5)).id
    assert_raises(CORBA::BAD_PARAM) { M::Multi._narrow(@types) }
    assert_kind_of M::Types, M::Types._unchecked_narrow(base)
  end

  def test_typecodes
    tc = M::Holder._tc
    assert_equal "IDL:rustorb.org/CxxMapping/Holder:1.0", tc.id
    assert_equal "Holder", tc.name
    assert_equal 11, tc.member_count
    assert_equal "choice", tc.member_name(10)
    assert_equal CORBA::TK_ALIAS, M::ShortString._tc.kind
    assert_equal 8, M::ShortString._tc.content_type.length
    assert_equal CORBA::TK_OBJREF, M::Later._tc.kind
    assert M::Holder._tc.equal?(M::Holder._tc)
    any = CORBA::Any.to_any(holder)
    assert_equal holder, any.value
    assert any._tc.equal?(M::Holder._tc)
  end
end

# frozen_string_literal: true

# The stage 2 tests (idl/AnyTest.idl) of the rustorb Ruby client and server
# (rustorbrb): the AnyEcho servant, the table of values and the client
# cases, the same as impls/rustorbpy/anytest.py.
#
# Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

require "AnyTestS"

module AnyCases
  # The names of the table of AnyTest.idl, in the order the client runs them.
  NAMES = %w[
    long short ulonglong float double longdouble boolean char octet string bounded count
    money fixed point shade choice_l choice_s choice_default longseq octets shortarray node
    nvlist any typecode objref nil oops empty
  ].freeze

  # Values of the types only the clients know (AnyTestHidden.idl), with what
  # AnyEcho::describe answers for them.
  HIDDEN = %w[secret pick moods].freeze
  HIDDEN_DESCRIPTION = {
    "secret" => "struct IDL:AnyTestHidden/Secret:1.0",
    "pick" => "union IDL:AnyTestHidden/Pick:1.0",
    "moods" => "alias IDL:AnyTestHidden/Moods:1.0"
  }.freeze

  KIND_NAMES = CORBA.constants.grep(/\ATK_/).to_h { |c| [CORBA.const_get(c), c.to_s.delete_prefix("TK_").downcase] }

  module_function

  def node(name, *children) = AnyTest::Node.new(name, children)

  def any(value, tc) = CORBA::Any.to_any(value, tc)

  # The value named name as a CORBA::Any; echo is the AnyEcho object (for
  # "objref").
  def value(name, echo)
    a = AnyTest
    case name
    when "long" then any(42, CORBA._tc_long)
    when "short" then any(-7, CORBA._tc_short)
    when "ulonglong" then any(2**64 - 1, CORBA._tc_ulonglong)
    when "float" then any(0.25, CORBA._tc_float)
    when "double" then any(1.5, CORBA._tc_double)
    when "longdouble" then any(CORBA::LongDouble.new(0.1), CORBA._tc_longdouble)
    when "boolean" then any(true, CORBA._tc_boolean)
    when "char" then any("x", CORBA._tc_char)
    when "octet" then any(255, CORBA._tc_octet)
    when "string" then any("hello any", CORBA._tc_string)
    when "bounded" then any("abc", a::Short5._tc)
    when "count" then any(7, a::Count._tc)
    when "money" then any(BigDecimal("-123.45"), a::Money._tc)
    when "fixed"
      any(BigDecimal("123456789012345678901234567.8901"), CORBA::TypeCode.create_fixed_tc(31, 4))
    when "point" then any(a::Point.new(1, 2.5), a::Point._tc)
    when "shade" then any(a::DIM, a::Shade._tc)
    when "choice_l" then any(a::Choice.new._put("l", 10, 1), a::Choice._tc)
    when "choice_s" then any(a::Choice.new._put("s", "three", 3), a::Choice._tc)
    when "choice_default" then any(a::Choice.new._put("b", true, 9), a::Choice._tc)
    when "longseq" then any([1, 2, 3], a::LongSeq._tc)
    when "octets" then any([0, 1, 255], a::Octets._tc)
    when "shortarray" then any([1, -2, 3], a::ShortArray._tc)
    when "node" then any(node("root", node("a"), node("b", node("c"))), a::Node._tc)
    when "nvlist"
      any([a::NameValue.new("a", any(1, CORBA._tc_long)),
           a::NameValue.new("b", any("x", CORBA._tc_string)),
           a::NameValue.new("p", any(a::Point.new(3, -0.5), a::Point._tc))], a::NVList._tc)
    when "any" then any(any(5, CORBA._tc_long), CORBA._tc_any)
    when "typecode" then any(a::Node._tc, CORBA._tc_TypeCode)
    when "objref" then any(echo, a::AnyEcho._tc)
    when "nil" then any(nil, a::AnyEcho._tc)
    when "oops" then any(a::Oops.new("bad", 3), a::Oops._tc)
    when "empty" then any(nil, CORBA._tc_null)
    else hidden(name)
    end
  end

  def hidden(name)
    a = AnyTest
    h = AnyTestHidden
    case name
    when "secret" then any(h::Secret.new("s", [a::Point.new(1, 2.0)], a::DARK), h::Secret._tc)
    when "pick" then any(h::Pick.new._put("s", h::Secret.new("p", [], a::LIGHT), "b"), h::Pick._tc)
    when "moods" then any([h::SAD, h::HAPPY], h::Moods._tc)
    else raise CORBA::BAD_PARAM.new("no value #{name}")
    end
  end

  # Whether a and b are the same value of type tc, following the TypeCode.
  def same(tc, a, b)
    tc = tc.resolve
    case tc.kind
    when CORBA::TK_ALIAS then same(tc.content_type, a, b)
    when CORBA::TK_NULL, CORBA::TK_VOID then a.nil? && b.nil?
    when CORBA::TK_STRUCT, CORBA::TK_EXCEPT
      a.instance_of?(b.class) && a._values.size == tc.member_count &&
        Array.new(tc.member_count) { |i| same(tc.member_type(i), a._values[i], b._values[i]) }.all?
    when CORBA::TK_UNION
      return false unless a.instance_of?(b.class) && a._disc == b._disc

      m = Rustorb::Marshal.union_member(tc, a._disc)
      m.nil? || same(m[2], a._value, b._value)
    when CORBA::TK_SEQUENCE, CORBA::TK_ARRAY
      return a.b == b.b if a.is_a?(String) && b.is_a?(String)

      a = a.bytes if a.is_a?(String)
      b = b.bytes if b.is_a?(String)
      a.size == b.size && a.zip(b).all? { |x, y| same(tc.content_type, x, y) }
    when CORBA::TK_ANY then same_any(a, b)
    when CORBA::TK_TYPECODE then a.equal?(b)
    # As the Python, C++ and Java clients: as a double (the .NET decimal
    # cannot hold all the bits of the double 0.1).
    when CORBA::TK_LONGDOUBLE then a.to_f == b.to_f
    when CORBA::TK_OBJREF
      a.nil? || b.nil? ? a.nil? && b.nil? : a._is_equivalent(b)
    else a == b
    end
  end

  def same_any(a, b)
    a._tc.equal?(b._tc) && same(a._tc, a.value, b.value)
  end

  # "struct IDL:AnyTestHidden/Secret:1.0": the kind and repository id of the
  # TypeCode of a.
  def describe(a)
    tc = a._tc
    id = begin
      tc.id
    rescue CORBA::TypeCode::BadKind
      ""
    end
    "#{KIND_NAMES.fetch(tc.kind)} #{id}"
  end

  # The get_/insert_ operations of the DynAnys without components, by kind.
  DYN_BASIC = {
    CORBA::TK_SHORT => "short", CORBA::TK_LONG => "long", CORBA::TK_USHORT => "ushort",
    CORBA::TK_ULONG => "ulong", CORBA::TK_FLOAT => "float", CORBA::TK_DOUBLE => "double",
    CORBA::TK_LONGDOUBLE => "longdouble", CORBA::TK_BOOLEAN => "boolean", CORBA::TK_CHAR => "char",
    CORBA::TK_OCTET => "octet", CORBA::TK_TYPECODE => "typecode", CORBA::TK_OBJREF => "reference",
    CORBA::TK_STRING => "string", CORBA::TK_LONGLONG => "longlong", CORBA::TK_ULONGLONG => "ulonglong",
    CORBA::TK_WCHAR => "wchar", CORBA::TK_WSTRING => "wstring"
  }.freeze

  # The any a taken apart with a DynAny and built again in a DynAny made
  # from its TypeCode alone, component by component.
  def dyn_rebuild(factory, a)
    src = factory.create_dyn_any(a)
    dst = factory.create_dyn_any_from_type_code(a._tc)
    dyn_copy(factory, src, dst)
    rebuilt = dst.to_any
    src.destroy
    dst.destroy
    rebuilt
  end

  def dyn_copy(factory, src, dst)
    k = src.type.unaliased.kind
    case k
    when CORBA::TK_STRUCT, CORBA::TK_EXCEPT, CORBA::TK_SEQUENCE, CORBA::TK_ARRAY
      dst.set_length(src.get_length) if k == CORBA::TK_SEQUENCE
      src.component_count.times do |i|
        src.seek(i)
        dst.seek(i)
        dyn_copy(factory, src.current_component, dst.current_component)
      end
    when CORBA::TK_UNION
      dst.set_discriminator(src.get_discriminator)
      dyn_copy(factory, src.member, dst.member) unless src.has_no_active_member
    when CORBA::TK_ENUM then dst.set_as_ulong(src.get_as_ulong)
    when CORBA::TK_FIXED then dst.set_value(src.get_value)
    when CORBA::TK_ANY then dst.insert_any(dyn_rebuild(factory, src.get_any))
    else
      op = DYN_BASIC[k]
      dst.public_send("insert_#{op}", src.public_send("get_#{op}")) if op
    end
  end

  def run(r, echo)
    expect = lambda do |name, got|
      want = value(name, echo)
      Cases.check(same_any(want, got), "got #{got.inspect[0, 200]}")
    end
    NAMES.each do |n|
      r.case("make[#{n}]") { expect.call(n, echo.make(n)) }
      r.case("check[#{n}]") { Cases.check(echo.check(n, value(n, echo)) == true) }
      r.case("echo[#{n}]") { expect.call(n, echo.echo(value(n, echo))) }
    end
    NAMES.each do |n|
      r.case("make_tc[#{n}]") { Cases.check(echo.make_tc(n).equal?(value(n, echo)._tc)) }
      r.case("check_tc[#{n}]") { Cases.check(echo.check_tc(n, value(n, echo)._tc) == true) }
      r.case("echo_tc[#{n}]") { Cases.check(echo.echo_tc(value(n, echo)._tc).equal?(value(n, echo)._tc)) }
    end
    %w[point nvlist node].each do |n|
      r.case("out_inout[#{n}]") do
        b, c = echo.out_inout(n, value(n, echo))
        expect.call(n, b)
        expect.call("long", c)
      end
    end
    r.case("echo_nvlist") do
      l = value("nvlist", echo).value
      Cases.check(same(AnyTest::NVList._tc, echo.echo_nvlist(l), l))
    end
    HIDDEN.each do |n|
      r.case("echo[#{n}]") { expect.call(n, echo.echo(value(n, echo))) }
      r.case("describe[#{n}]") do
        got = echo.describe(value(n, echo))
        Cases.check(got == HIDDEN_DESCRIPTION[n], got)
      end
    end
    # The values taken apart and built again with DynAny, from their
    # TypeCodes alone.
    factory = CORBA.ORB_init([]).resolve_initial_references("DynAnyFactory")
    NAMES.each do |n|
      r.case("dyn_check[#{n}]") { Cases.check(echo.check(n, dyn_rebuild(factory, value(n, echo))) == true) }
      r.case("dyn_make[#{n}]") { expect.call(n, dyn_rebuild(factory, echo.make(n))) }
    end
    HIDDEN.each do |n|
      r.case("dyn_echo[#{n}]") { expect.call(n, echo.echo(dyn_rebuild(factory, value(n, echo)))) }
    end
    r.case("describe[long]") { Cases.check(echo.describe(value("long", echo)) == "long ") }
    r.case("describe[node]") do
      Cases.check(echo.describe(value("node", echo)) == "struct IDL:AnyTest/Node:1.0")
    end
    # The server compares TypeCodes with equal: a short 42 or an unaliased
    # long 7 is not the value.
    r.case("check_wrong_type") { Cases.check(echo.check("long", any(42, CORBA._tc_short)) == false) }
    r.case("check_unaliased") { Cases.check(echo.check("count", any(7, CORBA._tc_long)) == false) }
    r.case("check_tc_unaliased") { Cases.check(echo.check_tc("count", CORBA._tc_long) == false) }
    r.case("out_inout_wrong") do
      Cases.expect_raises(CORBA::BAD_PARAM) { echo.out_inout("point", value("long", echo)) }
    end
  end
end

class AnyEchoServant < POA::AnyTest::AnyEcho
  def echo(a) = a
  def make(name) = AnyCases.value(name, _this)
  def check(name, a) = AnyCases.same_any(AnyCases.value(name, _this), a)

  def out_inout(name, c)
    raise CORBA::BAD_PARAM.new("not #{name}", 0, CORBA::COMPLETED_NO) unless AnyCases.same_any(AnyCases.value(name, _this), c)

    [AnyCases.value(name, _this), AnyCases.value("long", nil)]
  end

  def echo_tc(tc) = tc
  def make_tc(name) = AnyCases.value(name, _this)._tc
  def check_tc(name, tc) = AnyCases.value(name, _this)._tc.equal?(tc)
  def echo_nvlist(l) = l
  def describe(a) = AnyCases.describe(a)
end

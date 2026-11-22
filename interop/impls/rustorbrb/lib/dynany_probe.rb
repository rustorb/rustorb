# frozen_string_literal: true

# The DynAny probe of impls/omniorb-cpp/dynany_probe.cc with rustorb's Ruby
# binding (DynamicAny in the rustorb gem): the same cases, printed the same
# way. run-dynany-probe.sh compares the output with
# expected/dynany-probe-rustorb.txt, as the Rust probe's (the same core).
#
# Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

here = File.expand_path("..", __dir__)
$LOAD_PATH.unshift(File.join(here, "rt"))
require "corba"

DA = DynamicAny::DynAny
KINDS = CORBA.constants.grep(/\ATK_/).to_h { |c| [CORBA.const_get(c), "tk_#{c.to_s.delete_prefix('TK_').downcase}"] }

def kase(name)
  out = begin
    r = yield
    r.nil? ? "" : r
  rescue DA::TypeMismatch then "TypeMismatch"
  rescue DA::InvalidValue then "InvalidValue"
  rescue DynamicAny::DynAnyFactory::InconsistentTypeCode then "InconsistentTypeCode"
  rescue DynamicAny::MustTruncate then "MustTruncate"
  rescue CORBA::SystemException => e then e.class.name.split("::").last
  end
  puts "#{name}: #{out}"
end

def b(v) = v ? "true" : "false"
def q(s) = "\"#{s}\""
def kind(k) = KINDS.fetch(k)

# The current position, found by counting the components after it (this
# moves the position to -1, so it comes last in a case).
def position(d)
  return -1 if d.current_component.nil?

  after = 0
  after += 1 while d.next
  d.component_count - 1 - after
end

def any(v, tc) = CORBA::Any.to_any(v, tc)

orb = CORBA.ORB_init([])
f = orb.resolve_initial_references("DynAnyFactory")
NEW = f.method(:create_dyn_any_from_type_code)
def new(tc) = NEW.call(tc)

long = CORBA._tc_long
string = CORBA._tc_string
short = CORBA._tc_short
boolean = CORBA._tc_boolean
count = orb.create_alias_tc("IDL:P/Count:1.0", "Count", long)
s_tc = orb.create_struct_tc("IDL:P/S:1.0", "S", [["a", long], ["b", string], ["c", count]])
e_tc = orb.create_exception_tc("IDL:P/E:1.0", "E", [])
color = orb.create_enum_tc("IDL:P/Color:1.0", "Color", %w[RED GREEN BLUE])
u_tc = orb.create_union_tc("IDL:P/U:1.0", "U", long,
                           [[1, "x", long], [2, "y", string], [3, "y", string], [:default, "z", boolean]])
d_tc = orb.create_union_tc("IDL:P/D:1.0", "D", long, [[:default, "z", boolean], [0, "x", long]])
n_tc = orb.create_union_tc("IDL:P/N:1.0", "N", short, [[5, "x", long], [2, "y", string]])
b_tc = orb.create_union_tc("IDL:P/B:1.0", "B", boolean, [[true, "t", long], [false, "f", string]])
seq_tc = orb.create_sequence_tc(0, long)
bseq_tc = orb.create_sequence_tc(2, long)
arr_tc = orb.create_array_tc(3, long)
money = orb.create_fixed_tc(5, 2)
bstr = orb.create_string_tc(3)
v_tc = orb.create_value_tc("IDL:P/V:1.0", "V", CORBA::VM_NONE, nil, [["n", long, CORBA::PUBLIC_MEMBER]])
box_tc = orb.create_value_box_tc("IDL:P/LBox:1.0", "LBox", long)

def seeked(tc, i)
  d = new(tc)
  d.seek(i)
  d
end

# -- creation from TypeCodes and positions
kase("long.count") { new(long).component_count.to_s }
kase("long.get") { new(long).get_long.to_s }
kase("long.seek0") { b(new(long).seek(0)) }
kase("long.next") { b(new(long).next) }
kase("long.current_component") { b(new(long).current_component.nil?) }
kase("long.get_short") { new(long).get_short && nil }
kase("long.insert_short") { new(long).insert_short(1) }
kase("alias.insert_long") do
  d = new(count)
  d.insert_long(7)
  d.get_long.to_s
end
kase("alias.type") { kind(new(count).type.kind) }
kase("string.get") { q(new(string).get_string) }
kase("bstring.insert_long") { new(bstr).insert_string("abcd") }
kase("bstring.insert_ok") { new(bstr).insert_string("abc").then { b(true) } }
kase("typecode.get") { kind(new(CORBA._tc_TypeCode).get_typecode.kind) }
kase("any.get") { kind(new(CORBA._tc_any).get_any._tc.kind) }
kase("any.count") { new(CORBA._tc_any).component_count.to_s }
kase("objref.get") { b(new(CORBA._tc_Object).get_reference.nil?) }
kase("principal") { kind(new(CORBA._tc_Principal).to_any._tc.kind) }
kase("longdouble") { kind(new(CORBA._tc_longdouble).to_any._tc.kind) }
kase("any.get_dyn_any_shared") do
  d = new(CORBA._tc_any)
  d.insert_any(any(5, long))
  d.get_dyn_any.insert_long(9)
  d.get_any.value.to_s
end

# -- struct
kase("struct.count") { new(s_tc).component_count.to_s }
kase("struct.position") { position(new(s_tc)).to_s }
kase("struct.insert_long") do
  d = new(s_tc)
  d.insert_long(3)
  q(d.current_member_name)
end
kase("struct.insert_long_moves") do
  d = new(s_tc)
  d.insert_long(3)
  b(d.seek(0)) + d.current_component.get_long.to_s
end
kase("struct.insert_string_at_0") { new(s_tc).insert_string("x") }
kase("struct.member_kind_alias") { kind(seeked(s_tc, 2).current_member_kind) }
kase("struct.member_name_end") { q(seeked(s_tc, 5).current_member_name) }
kase("struct.insert_at_end") { seeked(s_tc, 5).insert_long(1) }
kase("struct.next_past_end") do
  d = seeked(s_tc, 2)
  b(d.next) + b(d.current_component.nil?)
end
kase("struct.next_from_minus1") do
  d = seeked(s_tc, -1)
  b(d.next) + position(d).to_s
end
kase("struct.seek_minus1") { b(new(s_tc).seek(-1)) }
kase("struct.component_shared") do
  d = new(s_tc)
  d.current_component.insert_long(11)
  d.get_members[0].value.value.to_s
end
kase("struct.members_as_dyn_any_shared") do
  d = new(s_tc)
  d.get_members_as_dyn_any[0].value.insert_long(12)
  d.current_component.get_long.to_s
end

set_members = lambda do |result = nil, &change|
  lambda do
    d = new(s_tc)
    m = d.get_members
    change.call(m)
    d.set_members(m)
    result
  end
end
kase("struct.set_members_wrong_name", &set_members.call { |m| m[1].id = "zz" })
kase("struct.set_members_empty_names", &set_members.call(b(true)) { |m| m.each { |p| p.id = "" } })
kase("struct.set_members_short", &set_members.call(&:pop))
kase("struct.set_members_wrong_type", &set_members.call { |m| m[0].value = any(1, short) })
kase("struct.set_members_unaliased", &set_members.call(b(true)) { |m| m[2].value = any(4, long) })
kase("struct.set_members_position") do
  d = seeked(s_tc, 2)
  d.set_members(d.get_members)
  position(d).to_s
end
kase("struct.from_any_wrong") { new(s_tc).from_any(any(1, long)) }
kase("struct.from_any_position") do
  d = new(s_tc)
  a = d.to_any
  d.seek(2)
  d.from_any(a)
  position(d).to_s
end
kase("struct.assign_wrong") { new(s_tc).assign(new(long)) }
kase("struct.destroy_component") do
  c = new(s_tc).current_component
  c.destroy
  c.insert_long(2)
  c.get_long.to_s
end
kase("struct.destroyed") do
  d = new(s_tc)
  c = d.current_component
  d.destroy
  c.get_long
  nil
end
kase("struct.destroyed_top") do
  d = new(s_tc)
  d.destroy
  d.component_count
  nil
end
kase("struct.copy_independent") do
  d = new(s_tc)
  d.copy.insert_long(5)
  d.current_component.get_long.to_s
end
kase("struct.equal") do
  d = new(s_tc)
  e = d.copy
  s = b(d.equal(e))
  e.insert_long(1)
  s + b(d.equal(e))
end
kase("except.count") { new(e_tc).component_count.to_s }
kase("except.current_component") { b(new(e_tc).current_component.nil?) }
kase("except.member_name") { q(new(e_tc).current_member_name) }

# -- enum
kase("enum.get") { q(new(color).get_as_string) }
kase("enum.set_as_string_bad") { new(color).set_as_string("PINK") }
kase("enum.set_as_ulong_bad") { new(color).set_as_ulong(3) }
kase("enum.set_as_ulong") do
  d = new(color)
  d.set_as_ulong(2)
  q(d.get_as_string)
end
kase("enum.current_component") { b(new(color).current_component.nil?) }
kase("enum.count") { new(color).component_count.to_s }
kase("enum.insert_ulong") { new(color).insert_ulong(1) }

# -- fixed
fixed = lambda do |lit|
  lambda do
    d = new(money)
    exact = d.set_value(lit)
    b(exact) + q(d.get_value)
  end
end
kase("fixed.get") { q(new(money).get_value) }
kase("fixed.set", &fixed.call("-12.5"))
kase("fixed.set_d", &fixed.call("1.25d"))
kase("fixed.set_blank", &fixed.call(" 1.25 "))
kase("fixed.set_truncates", &fixed.call("1.239"))
kase("fixed.set_zero_tail") { b(new(money).set_value("1.230")) }
kase("fixed.set_too_big") { new(money).set_value("1234.5") }
kase("fixed.set_bad") { new(money).set_value("abc") }
kase("fixed.set_int", &fixed.call("7"))

# -- union
def disc_of(tc, v)
  d = new(tc)
  d.from_any(any(v, tc))
  d
end

kase("union.initial") do
  d = new(u_tc)
  "#{d.get_discriminator.get_long} #{b(d.has_no_active_member)} #{q(d.member_name)}"
end
kase("union.count") { new(u_tc).component_count.to_s }
kase("union.position") { position(new(u_tc)).to_s }
kase("union.default_first") { "#{new(d_tc).get_discriminator.get_long} #{q(new(d_tc).member_name)}" }
kase("union.no_default_initial") { "#{new(n_tc).get_discriminator.get_short} #{q(new(n_tc).member_name)}" }
kase("union.set_discriminator") do
  d = new(u_tc)
  d.set_discriminator(disc_of(long, 3))
  "#{q(d.member_name)} #{position(d)}"
end
kase("union.set_discriminator_keeps") do
  d = new(u_tc)
  disc = disc_of(long, 2)
  d.set_discriminator(disc)
  d.member.insert_string("kept")
  disc.insert_long(3)
  d.set_discriminator(disc)
  q(d.member.get_string)
end
kase("union.set_discriminator_default") do
  d = new(u_tc)
  d.set_discriminator(disc_of(long, 42))
  q(d.member_name)
end
kase("union.set_discriminator_wrong") { new(u_tc).set_discriminator(new(short)) }
kase("union.disc_component") do
  d = new(u_tc)
  d.get_discriminator.insert_long(2)
  "#{q(d.member_name)} #{d.component_count}"
end
kase("union.disc_component_to_none") do
  d = new(n_tc)
  d.get_discriminator.insert_short(9)
  "#{b(d.has_no_active_member)} #{d.component_count}"
end
kase("union.set_to_default_member") do
  d = new(u_tc)
  d.set_to_default_member
  "#{q(d.member_name)} #{d.get_discriminator.get_long} #{position(d)}"
end
kase("union.set_to_default_member_none") { new(n_tc).set_to_default_member }
kase("union.set_to_no_active_member") do
  d = new(n_tc)
  d.set_to_no_active_member
  "#{b(d.has_no_active_member)} #{d.get_discriminator.get_short} #{d.component_count} #{position(d)}"
end
kase("union.set_to_no_active_member_default") { new(u_tc).set_to_no_active_member }
kase("union.set_to_no_active_member_full") { new(b_tc).set_to_no_active_member }
no_member = lambda do |op|
  lambda do
    d = new(n_tc)
    d.set_to_no_active_member
    d.public_send(op)
    nil
  end
end
kase("union.member_none", &no_member.call(:member))
kase("union.member_name_none", &no_member.call(:member_name))
kase("union.discriminator_kind") { kind(new(n_tc).discriminator_kind) }
kase("union.member_kind") { kind(new(u_tc).member_kind) }
kase("union.insert_long") do
  d = new(u_tc)
  d.insert_long(1)
  position(d).to_s
end
kase("union.boolean_initial") do
  "#{b(new(b_tc).get_discriminator.get_boolean)} #{q(new(b_tc).member_name)}"
end

# -- sequence and array
seq = lambda do |&steps|
  lambda do
    steps.call(new(seq_tc))
  end
end
kase("seq.count") { new(seq_tc).component_count.to_s }
kase("seq.current_component") { b(new(seq_tc).current_component.nil?) }
kase("seq.insert_empty") { new(seq_tc).insert_long(1) }
kase("seq.grow_position", &seq.call { |d| d.set_length(3) || position(d).to_s })
kase("seq.grow_keeps_position", &seq.call { |d|
  d.set_length(2)
  d.seek(1)
  d.insert_long(4)
  d.set_length(4)
  d.current_component.get_long.to_s
})
kase("seq.grow_from_minus1", &seq.call { |d|
  d.set_length(2)
  d.seek(-1)
  d.set_length(4)
  s = b(d.current_component.nil?)
  d.insert_long(8)
  e = d.copy
  e.seek(2)
  "#{s} #{e.get_long}"
})
kase("seq.shrink_position", &seq.call { |d|
  d.set_length(4)
  d.seek(3)
  d.set_length(2)
  b(d.current_component.nil?)
})
kase("seq.shrink_keeps", &seq.call { |d|
  d.set_length(4)
  d.seek(1)
  d.insert_long(6)
  d.set_length(2)
  d.current_component.get_long.to_s
})
kase("seq.shrink_to_0", &seq.call { |d|
  d.set_length(4)
  d.set_length(0)
  b(d.current_component.nil?)
})
kase("seq.bound") { new(bseq_tc).set_length(3) }
longs = ->(n) { Array.new(n) { |i| any(i, long) } }
kase("seq.set_elements_bound") { new(bseq_tc).set_elements(longs.call(3)) }
kase("seq.set_elements_type") { new(seq_tc).set_elements([any(1, short)]) }
kase("seq.set_elements_position", &seq.call { |d| d.set_elements(longs.call(2)) || position(d).to_s })
kase("seq.insert_long_seq", &seq.call { |d|
  d.insert_long_seq([1, 2])
  "#{d.component_count} #{position(d)}"
})
kase("seq.get_long_seq", &seq.call { |d| d.set_length(2) || d.get_long_seq.size.to_s })
kase("seq.insert_short_seq") { new(seq_tc).insert_short_seq([1]) }
kase("seq.insert_long_seq_bound") { new(bseq_tc).insert_long_seq([0, 0, 0]) }
kase("struct.insert_long_seq") { new(s_tc).insert_long_seq([0]) }
kase("arr.count") do
  d = new(arr_tc)
  "#{d.component_count} #{position(d)}"
end
kase("arr.set_elements_count") { new(arr_tc).set_elements(longs.call(2)) }
kase("arr.insert_long_seq_count") { new(arr_tc).insert_long_seq([0, 0]) }

# -- valuetypes
kase("value.null") do
  d = new(v_tc)
  "#{b(d.is_null)} #{d.component_count}"
end
kase("value.set_to_value") do
  d = new(v_tc)
  d.set_to_value
  "#{b(d.is_null)} #{d.component_count} #{position(d)}"
end
kase("value.get_members_null") { new(v_tc).get_members.size.to_s }
kase("value.member_name_null") { q(new(v_tc).current_member_name) }
kase("value.current_component_null") { b(new(v_tc).current_component.nil?) }
kase("value.set_members") do
  d = new(v_tc)
  d.set_members([DynamicAny::NameValuePair.new("n", any(3, long))])
  "#{b(d.is_null)} #{d.current_component.get_long}"
end
kase("box.null") do
  d = new(box_tc)
  "#{b(d.is_null)} #{d.component_count}"
end
kase("box.get_null") { new(box_tc).get_boxed_value && nil }
kase("box.set") do
  d = new(box_tc)
  d.set_boxed_value(any(4, long))
  "#{b(d.is_null)} #{d.component_count} #{position(d)}"
end
kase("box.set_wrong") { new(box_tc).set_boxed_value(any(4, short)) }
kase("box.set_to_value") do
  d = new(box_tc)
  d.set_to_value
  d.current_component.get_long.to_s
end

orb.destroy

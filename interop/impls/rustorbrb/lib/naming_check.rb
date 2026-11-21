# frozen_string_literal: true

# The naming check of the rustorb Ruby implementation (rustorbrb): a fixed
# sequence of CosNaming operations with one line of output each, compared
# with expected/naming-check.txt (as impls/rustorbpy/naming_check.py).
#
#     naming-check --naming corbaloc::naming:2809/NameService
#
# Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

require_relative "common"
require "CosNamingS"

NC = CosNaming::NamingContext

def fmt_name(name)
  s = name.map { |c| c.id + (c.kind.empty? ? "" : ".#{c.kind}") }.join("/")
  s.empty? ? "<empty>" : s
end

def short(klass) = klass.name.split("::").last

def fmt_exc(e)
  case e
  when NC::NotFound
    "NotFound(#{NC::NotFoundReason::ENUMERATORS[e.why]}, rest=#{fmt_name(e.rest_of_name)})"
  when NC::CannotProceed then "CannotProceed(rest=#{fmt_name(e.rest_of_name)})"
  when CORBA::SystemException
    "#{short(e.class)}(completed=#{%w[COMPLETED_YES COMPLETED_NO COMPLETED_MAYBE][e.completed]})"
  else short(e.class)
  end
end

def step(label, show = nil)
  result = yield
rescue CORBA::Exception => e
  puts format("%-40s -> raises %s", label, fmt_exc(e))
  nil
else
  puts format("%-40s -> %s", label, show ? show.call(result) : "ok")
  result
end

def n(path)
  path.split("/").reject(&:empty?).map do |c|
    id, kind = c.split(".", 2)
    CosNaming::NameComponent.new(id, kind || "")
  end
end

def fmt_bindings(bl)
  items = bl.map { |b| [fmt_name(b.binding_name), CosNaming::BindingType::ENUMERATORS[b.binding_type]] }.sort
  "[#{items.map { |i| i.join(':') }.join(', ')}]"
end

def py(s) = "'#{s}'"

def fmt_components(name) = "[#{name.map { |c| "(#{py(c.id)}, #{py(c.kind)})" }.join(', ')}]"

opts, orb_args = Interop.parse_args(ARGV)
orb = Interop.orb_init(opts.merge(port: 0, host: nil), orb_args)
root = Interop.root_context(orb, opts[:timeout])
ext = begin
  CosNaming::NamingContextExt._narrow(root)
rescue CORBA::BAD_PARAM
  nil
end

t = step("bind_new_context test") { root.bind_new_context(n("test")) }
exit 1 if t.nil?
obj = t
equivalent = ->(o) { o._is_equivalent?(obj) ? "equivalent" : "different object" }
is_context = lambda do |o|
  NC._narrow(o)
  "NamingContext"
rescue CORBA::BAD_PARAM
  "not a context"
end

step("bind test/a") { root.bind(n("test/a"), obj) }
step("bind test/a (again)") { root.bind(n("test/a"), obj) }
step("rebind test/a") { root.rebind(n("test/a"), obj) }
step("resolve test/a", equivalent) { root.resolve(n("test/a")) }
step("bind test/b.kind") { t.bind(n("b.kind"), obj) }
step("resolve test/b (no kind)") { root.resolve(n("test/b")) }
step("resolve test/b.kind", equivalent) { root.resolve(n("test/b.kind")) }
step("resolve test/missing") { root.resolve(n("test/missing")) }
step("resolve test/missing/deeper") { root.resolve(n("test/missing/deeper")) }
step("resolve test/a/x (not a context)") { root.resolve(n("test/a/x")) }
step("resolve <empty>") { root.resolve([]) }
step("bind <empty>") { root.bind([], obj) }

sub = step("bind_new_context test/sub") { root.bind_new_context(n("test/sub")) }
step("bind_new_context test/sub (again)") { root.bind_new_context(n("test/sub")) }
step("bind test/sub/c") { root.bind(n("test/sub/c"), obj) }
step("resolve test/sub (is a context)", is_context) { root.resolve(n("test/sub")) }
loose = step("new_context") { root.new_context }
step("bind_context test/loose") { root.bind_context(n("test/loose"), loose) }
step("rebind_context test/loose") { root.rebind_context(n("test/loose"), loose) }

step("list test (how_many=10)", ->(r) { fmt_bindings(r[0]) + (r[1].nil? ? "" : " + iterator") }) { t.list(10) }
step("list test (how_many=1) + iterator", ->(s) { s }) do
  bl, it = t.list(1)
  names = bl.dup
  steps = ["first=#{bl.size}"]
  until it.nil?
    more, chunk = it.next_n(2)
    steps << "next_n(2)=#{more ? 'True' : 'False'}/#{chunk.size}"
    names += chunk
    unless more
      it.destroy
      break
    end
  end
  "#{fmt_bindings(names)} #{steps.join(' ')}"
end

step("destroy test/sub (not empty)") { sub.destroy }
step("unbind test/sub/c") { root.unbind(n("test/sub/c")) }
step("destroy test/sub") { sub.destroy }
step("resolve test/sub (dangling)", ->(_) { "still bound" }) { root.resolve(n("test/sub")) }
step("unbind test/sub") { root.unbind(n("test/sub")) }
step("unbind test/a") { root.unbind(n("test/a")) }
step("unbind test/a (again)") { root.unbind(n("test/a")) }
step("resolve test/a") { root.resolve(n("test/a")) }

if ext
  nc = CosNaming::NameComponent
  step("to_string [a.b, c/d, .e]", ->(s) { s }) { ext.to_string([nc.new("a", "b"), nc.new("c/d", ""), nc.new("", "e")]) }
  step("to_name a.b/c\\/d/.e", method(:fmt_components)) { ext.to_name("a.b/c\\/d/.e") }
  step("to_url host:2809 a/b", ->(s) { s }) { ext.to_url("host:2809", "a/b") }
  step("to_url :host:2809 a/b c", ->(s) { s }) { ext.to_url(":host:2809", "a/b c") }
  step("resolve_str test/b.kind", equivalent) { ext.resolve_str("test/b.kind") }
  step("to_name <invalid>", method(:fmt_components)) { ext.to_name("a//b") }
else
  puts "root context is not a NamingContextExt"
end

base = opts[:naming].sub("corbaloc:", "corbaname:")
step("corbaname #test/b.kind", equivalent) { orb.string_to_object("#{base}#test/b.kind") }
step("corbaname #test%2Fb%2ekind", equivalent) { orb.string_to_object("#{base}#test%2Fb%2ekind") }
step("corbaname rir: #test/b.kind", equivalent) { orb.string_to_object("corbaname:rir:#test/b.kind") }
step("corbaname (no name)", is_context) { orb.string_to_object(base) }
step("corbaname #test/loose", is_context) { orb.string_to_object("#{base}#test/loose") }
step("corbaname #test/missing") { orb.string_to_object("#{base}#test/missing") }
step("corbaname #test//b (invalid)") { orb.string_to_object("#{base}#test//b") }

step("list test (final)", ->(r) { fmt_bindings(r[0]) }) { t.list(100) }
orb.destroy

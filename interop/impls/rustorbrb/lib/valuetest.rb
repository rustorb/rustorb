# frozen_string_literal: true

# The valuetype tests (idl/ValueTest.idl) of the rustorb Ruby client and
# server (rustorbrb): the ValueEcho servant and the client cases, the same
# as impls/rustorbpy/valuetest.py.
#
# Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

require "ValueTestS"
require "cases"

module ValueCases
  V = ValueTest

  # ValueTest::Tally: also a servant of Counter.
  class TallyImpl < V::Tally
    def count = n
  end

  class AccountImpl < V::Account
    def doubled = balance * 2
  end

  class ValueEchoServant < POA::ValueTest::ValueEcho
    def initialize(orb)
      super()
      @orb = orb
    end

    def echo_point(p) = p
    def echo_pair(p) = p
    def echo_node(n) = n
    def echo_shape(s) = s
    def echo_points(s) = s
    def same(a, b) = a.equal?(b)

    def count_nodes(n)
      seen = {}.compare_by_identity
      until n.nil? || seen.key?(n)
        seen[n] = true
        n = n.r_next
      end
      seen.size
    end

    def echo_long_box(b) = b
    def echo_string_box(b) = b
    def echo_pair_box(b) = b
    def echo_any(a) = a
    def echo_tally(t) = t
    def call_count(c) = c.count
    def account_doubled(a) = a.doubled
    # The factory of the implementation is its class.
    def make_account(balance) = AccountImpl.new(balance)
    def shutdown = @orb.shutdown(false)
  end

  module_function

  # The value factories of the valuetypes with operations.
  def register_factories(orb)
    orb.register_value_factory(V::Tally::ID, TallyImpl)
    orb.register_value_factory(V::Account::ID, AccountImpl)
  end

  def check(cond, msg = "check failed") = Cases.check(cond, msg)

  # A list of n nodes, ids 0 to n - 1.
  def chain(n)
    head = nil
    (n - 1).downto(0) { |i| head = V::Node.new(i, head) }
    head
  end

  # Two nodes that refer to each other.
  def ring
    a = V::Node.new(1, nil)
    b = V::Node.new(2, a)
    a.r_next = b
    a
  end

  def point(p, x, y)
    check(p.instance_of?(V::Point), "got #{p.inspect}")
    check([p.x, p.y] == [x, y], "got #{[p.x, p.y].inspect}")
  end

  def point3(p)
    check(p.instance_of?(V::Point3), "got #{p.inspect}")
    check([p.x, p.y, p.z, p.label] == [1, 2, 3, "three"], "got #{p.inspect}")
  end

  def run(r, echo)
    r.case("echo_point[plain]") { point(echo.echo_point(V::Point.new(1, -2)), 1, -2) }
    r.case("echo_point[null]") { check(echo.echo_point(nil).nil?) }
    r.case("echo_point[derived]") { point3(echo.echo_point(V::Point3.new(1, 2, 3, "three"))) }
    r.case("echo_pair[shared]") do
      p = V::Point.new(5, 6)
      got = echo.echo_pair(V::Pair.new(p, p))
      point(got.first, 5, 6)
      check(got.first.equal?(got.second), "not one instance")
    end
    r.case("echo_pair[distinct]") do
      got = echo.echo_pair(V::Pair.new(V::Point.new(5, 6), V::Point.new(5, 6)))
      point(got.first, 5, 6)
      point(got.second, 5, 6)
      check(!got.first.equal?(got.second), "one instance")
    end
    r.case("echo_pair[null]") do
      got = echo.echo_pair(V::Pair.new(nil, V::Point3.new(1, 2, 3, "three")))
      check(got.first.nil?)
      point3(got.second)
    end
    r.case("same[shared]") do
      p = V::Point.new(0, 0)
      check(echo.same(p, p) == true)
    end
    r.case("same[distinct]") { check(echo.same(V::Point.new(0, 0), V::Point.new(0, 0)) == false) }
    r.case("echo_node[list]") do
      got = echo.echo_node(chain(20))
      ids = []
      until got.nil?
        ids << got.id
        got = got.r_next
      end
      check(ids == (0...20).to_a, "got #{ids.inspect}")
    end
    r.case("echo_node[ring]") do
      got = echo.echo_node(ring)
      check([got.id, got.r_next.id] == [1, 2], "got #{[got.id, got.r_next.id].inspect}")
      check(got.r_next.r_next.equal?(got), "the cycle is lost")
    end
    r.case("count_nodes[list]") { check(echo.count_nodes(chain(20)) == 20) }
    r.case("count_nodes[ring]") { check(echo.count_nodes(ring) == 2) }
    r.case("echo_shape") do
      c = V::Point.new(0, 0)
      got = echo.echo_shape(V::Shape.new("square", [V::Point.new(1, 1), c], c, 1.5))
      check([got.name, got.area] == ["square", 1.5], "got #{[got.name, got.area].inspect}")
      check(got.points.size == 2, "got #{got.points.inspect}")
      point(got.points[0], 1, 1)
      point(got.center, 0, 0)
      check(got.points[1].equal?(got.center), "not one instance")
    end
    r.case("echo_points") do
      p = V::Point.new(7, 8)
      q = V::Point3.new(1, 2, 3, "three")
      got = echo.echo_points([p, q, p, nil, q])
      check(got.size == 5, "got #{got.inspect}")
      point(got[0], 7, 8)
      point3(got[1])
      check(got[0].equal?(got[2]) && got[1].equal?(got[4]), "sharing lost")
      check(got[3].nil?)
    end
    r.case("echo_long_box[value]") { check(echo.echo_long_box(7) == 7) }
    r.case("echo_long_box[null]") { check(echo.echo_long_box(nil).nil?) }
    r.case("echo_string_box[value]") { check(echo.echo_string_box("boxed") == "boxed") }
    r.case("echo_string_box[null]") { check(echo.echo_string_box(nil).nil?) }
    r.case("echo_pair_box") do
      got = echo.echo_pair_box(V::Pair.new(V::Point.new(1, 2), nil))
      point(got.first, 1, 2)
      check(got.second.nil?)
    end
    r.case("echo_any[ring]") do
      got = echo.echo_any(CORBA::Any.to_any(ring, V::Node._tc))
      check(got._tc.equal?(V::Node._tc), "got #{got._tc.inspect}")
      n = got.value
      check([n.id, n.r_next.id] == [1, 2] && n.r_next.r_next.equal?(n), "got #{n.inspect}")
    end
    r.case("echo_any[derived]") do
      got = echo.echo_any(CORBA::Any.to_any(V::Point3.new(1, 2, 3, "three"), V::Point._tc))
      check(got._tc.equal?(V::Point._tc), "got #{got._tc.inspect}")
      point3(got.value)
    end
    r.case("echo_any[box]") do
      got = echo.echo_any(CORBA::Any.to_any("in an any", V::StringBox._tc))
      check(got._tc.equal?(V::StringBox._tc), "got #{got._tc.inspect}")
      check(got.value == "in an any", "got #{got.value.inspect}")
    end
    r.case("echo_tally") do
      got = echo.echo_tally(TallyImpl.new(3))
      check(got.is_a?(TallyImpl), "got #{got.inspect}")
      check(got.n == 3 && got.count == 3, "got #{got.inspect}")
    end
    r.case("call_count") do
      t = TallyImpl.new(5)
      check(echo.call_count(t._this) == 5, "wrong count")
    end
    r.case("account_doubled") { check(echo.account_doubled(AccountImpl.new(21)) == 42) }
    r.case("make_account") do
      got = echo.make_account(7)
      check(got.is_a?(AccountImpl), "got #{got.inspect}")
      check(got.balance == 7 && got.doubled == 14, "got #{got.inspect}")
    end
  end
end

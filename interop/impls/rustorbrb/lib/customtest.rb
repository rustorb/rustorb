# frozen_string_literal: true

# The custom valuetype tests (idl/CustomTest.idl) of the rustorb Ruby
# client and server (rustorbrb): the Blob implementation, the CustomEcho
# servant and the client cases, the same as impls/rustorbpy/customtest.py.
#
# Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

require "CustomTestS"
require "cases"

module CustomCases
  C = CustomTest
  V = ValueTest

  # CustomTest::Blob, marshaled as CustomTest.idl says.
  class Blob < C::Blob
    def marshal(os)
      os.write_long(n)
      os.write_string(label)
      os.write_Value(pt)
      os.write_ulong(values.size)
      os.write_long_array(values, 0, values.size)
    end

    def unmarshal(is)
      self.n = is.read_long
      self.label = is.read_string
      self.pt = is.read_Value
      count = is.read_ulong
      self.values = is.read_long_array([], 0, count)
    end
  end

  class CustomEchoServant < POA::CustomTest::CustomEcho
    def initialize(orb)
      super()
      @orb = orb
    end

    def echo_blob(b) = b
    def echo_blob_pair(p) = p
    def echo_blobs(s) = s
    def shutdown = @orb.shutdown(false)
  end

  module_function

  # The value factory of Blob.
  def register(orb)
    orb.register_value_factory(C::Blob::ID, Blob)
  end

  def check(cond, msg = "check failed") = Cases.check(cond, msg)

  def blob(b, n, label, values)
    check(b.is_a?(Blob), "got #{b.inspect}")
    check([b.n, b.label, b.values.to_a] == [n, label, values], "got #{[b.n, b.label, b.values].inspect}")
  end

  def run(r, echo)
    r.case("echo_blob[plain]") do
      got = echo.echo_blob(Blob.new(7, "seven", V::Point.new(1, -2), [1, 2, 3]))
      blob(got, 7, "seven", [1, 2, 3])
      check(got.pt.instance_of?(V::Point) && [got.pt.x, got.pt.y] == [1, -2], "got #{got.pt.inspect}")
    end
    r.case("echo_blob[null]") { check(echo.echo_blob(nil).nil?) }
    r.case("echo_blob[empty]") do
      got = echo.echo_blob(Blob.new(0, "", nil, []))
      blob(got, 0, "", [])
      check(got.pt.nil?, "got #{got.pt.inspect}")
    end
    r.case("echo_blob[derived]") do
      got = echo.echo_blob(Blob.new(3, "three", V::Point3.new(1, 2, 3, "three"), [-1]))
      blob(got, 3, "three", [-1])
      p = got.pt
      check(p.instance_of?(V::Point3), "got #{p.inspect}")
      check([p.x, p.y, p.z, p.label] == [1, 2, 3, "three"], "got #{p.inspect}")
    end
    r.case("echo_blob_pair[shared]") do
      p = V::Point.new(5, 6)
      got = echo.echo_blob_pair(C::BlobPair.new(Blob.new(1, "one", p, [4, 5]), p))
      blob(got.data, 1, "one", [4, 5])
      check(got.data.pt.equal?(got.pt), "not one instance")
      check([got.pt.x, got.pt.y] == [5, 6], "got #{got.pt.inspect}")
    end
    r.case("echo_blob_pair[distinct]") do
      got = echo.echo_blob_pair(C::BlobPair.new(Blob.new(1, "one", V::Point.new(5, 6), []), V::Point.new(5, 6)))
      check(!got.data.pt.equal?(got.pt), "one instance")
    end
    r.case("echo_blobs") do
      b = Blob.new(2, "two", nil, (0...100).to_a)
      c = Blob.new(9, "nine", V::Point.new(9, 9), [])
      got = echo.echo_blobs([b, c, b, nil])
      check(got.size == 4, "got #{got.inspect}")
      blob(got[0], 2, "two", (0...100).to_a)
      blob(got[1], 9, "nine", [])
      check(got[0].equal?(got[2]), "sharing lost")
      check(!got[0].equal?(got[1]) && got[3].nil?, "got #{got.inspect}")
    end
  end
end

# frozen_string_literal: true

# The InteropTest cases of the interop client (interop/impls/rustorbrb)
# against its Echo servant, in one process.

require_relative "helper"
require "echo_servant"
require "cases"
require "stringio"

class InteropTest_ < Minitest::Test
  def test_cases
    servant = EchoServant.new(TestORB.orb)
    echo = TestORB.ref(servant, InteropTest::Echo)
    poa = TestORB.root_poa.create_POA("InteropTestClient", nil, [
      TestORB.root_poa.create_lifespan_policy(PortableServer::PERSISTENT),
      TestORB.root_poa.create_id_assignment_policy(PortableServer::USER_ID)
    ])
    poa.the_POAManager.activate
    out = StringIO.new
    r = Runner.new(out)
    Cases.run(r, echo, poa)
    assert_empty r.failed, out.string
    assert_equal 79, r.passed
  end
end

# The stage 2 (AnyTest.idl) cases of the interop client against its
# AnyEcho servant, in one process.
require "anytest"
require "AnyTestHiddenC"

class AnyTest_ < Minitest::Test
  def test_cases
    echo = TestORB.ref(AnyEchoServant.new, AnyTest::AnyEcho)
    out = StringIO.new
    r = Runner.new(out)
    AnyCases.run(r, echo)
    assert_empty r.failed, out.string
    assert_equal 6 * AnyCases::NAMES.size + 3 + 1 + 2 * AnyCases::HIDDEN.size + 6, r.passed
  end
end

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

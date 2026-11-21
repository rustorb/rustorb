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
    assert_equal 8 * AnyCases::NAMES.size + 3 + 1 + 3 * AnyCases::HIDDEN.size + 6, r.passed
  end
end

# The same cases with every call through the DII and every servant through
# the DSI (interop/impls/rustorbrb-dii), in a child process: the adapters
# change CORBA::Object and the POA for the whole process.
class DiiInterop_ < Minitest::Test
  def test_cases
    script = <<~'RUBY'
      require "dii_adapters"
      DiiAdapters.install
      require "echo_servant"
      require "cases"
      require "anytest"
      require "AnyTestHiddenC"
      orb = TestORB.orb
      poa = TestORB.root_poa
      basic = Runner.new
      echo = TestORB.ref(EchoServant.new(orb), InteropTest::Echo)
      client_poa = poa.create_POA("InteropTestClient", nil, [
        poa.create_lifespan_policy(PortableServer::PERSISTENT),
        poa.create_id_assignment_policy(PortableServer::USER_ID)
      ])
      client_poa.the_POAManager.activate
      Cases.run(basic, echo, client_poa)
      any = Runner.new
      AnyCases.run(any, TestORB.ref(AnyEchoServant.new, AnyTest::AnyEcho))
      puts "RESULT #{basic.passed} #{basic.failed.size} #{any.passed} #{any.failed.size}"
    RUBY
    helper = File.expand_path("helper", __dir__)
    out = IO.popen([RbConfig.ruby, "-r", helper, "-e", script],
                   err: %i[child out], &:read)
    m = out.match(/RESULT (\d+) (\d+) (\d+) (\d+)/)
    assert m, out
    assert_equal [79, 0], [m[1].to_i, m[2].to_i], out
    assert_equal 0, m[4].to_i, out
  end
end

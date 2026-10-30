// Leak checks of the C++ binding (issue #49): servants, object references,
// exceptions, TypeCodes (recursive ones too) and any values made and
// dropped many times. Servants are counted; everything else is left to
// LeakSanitizer, which the CI job built with -DRUSTORB_SANITIZE=address
// runs at exit (the native handles are allocated by librustorb_ffi with
// the same malloc). RUSTORB_LEAK_ROUNDS multiplies the number of rounds.

#define ANYTEST_HIDDEN
#include <atomic>
#include <cstdlib>
#include <iostream>
#include <string>
#include <thread>

#include "anytest.h"
#include "echo_impl.h"

static int failures = 0;

#define CHECK(cond)                                                              \
  do {                                                                           \
    if (!(cond)) {                                                               \
      std::cout << "FAIL line " << __LINE__ << ": " #cond << std::endl;          \
      ++failures;                                                                \
    }                                                                            \
  } while (0)

static std::atomic<int> live_servants{0};

class CountedEcho : public EchoImpl {
 public:
  explicit CountedEcho(IDL::traits<CORBA::ORB>::ref_type orb) : EchoImpl(std::move(orb)) {
    ++live_servants;
  }
  ~CountedEcho() override { --live_servants; }
};

using EchoRef = IDL::traits<InteropTest::Echo>::ref_type;
using OrbRef = IDL::traits<CORBA::ORB>::ref_type;
using PoaRef = IDL::traits<PortableServer::POA>::ref_type;

static OrbRef make_orb(const char* name) {
  int argc = 3;
  char arg1[] = "-ORBendPoint", arg2[] = "giop:tcp:127.0.0.1:";
  char* argv[] = {const_cast<char*>(name), arg1, arg2, nullptr};
  return CORBA::ORB_init(argc, argv);
}

static PoaRef root_of(const OrbRef& orb) {
  auto root = IDL::traits<PortableServer::POA>::narrow(orb->resolve_initial_references("RootPOA"));
  root->the_POAManager()->activate();
  return root;
}

template <typename T>
static typename IDL::traits<T>::ref_type remote(const OrbRef& server, const OrbRef& client,
                                                const CORBA::object_reference<CORBA::Object>& obj) {
  return IDL::traits<T>::narrow(client->string_to_object(server->object_to_string(obj)));
}

static void servants(const OrbRef& server, const OrbRef& client, int rounds) {
  auto root = root_of(server);
  for (int i = 0; i < rounds; ++i) {
    auto servant = CORBA::make_reference<CountedEcho>(server);
    auto oid = root->activate_object(servant);
    auto echo = remote<InteropTest::Echo>(server, client, root->id_to_reference(oid));
    CHECK(echo->echo_string("x") == "x");
    root->deactivate_object(oid);
  }
  // Servants in a POA that is destroyed.
  for (int i = 0; i < rounds / 5; ++i) {
    auto poa = root->create_POA("leak" + std::to_string(i), root->the_POAManager(), {});
    for (int j = 0; j < 5; ++j) {
      auto servant = CORBA::make_reference<CountedEcho>(server);
      auto echo = remote<InteropTest::Echo>(server, client,
                                            poa->id_to_reference(poa->activate_object(servant)));
      CHECK(echo->echo_long(j) == j);
    }
    poa->destroy(true, true);
  }
}

static void references_and_exceptions(const OrbRef& server, const OrbRef& client, int rounds) {
  auto root = root_of(server);
  auto servant = CORBA::make_reference<CountedEcho>(server);
  std::string ior = server->object_to_string(root->id_to_reference(root->activate_object(servant)));
  InteropTest::Record rec(1, "r", InteropTest::Color::BLUE, InteropTest::Point(1, 2), {1, 2, 3},
                          true);
  for (int i = 0; i < rounds; ++i) {
    auto echo = IDL::traits<InteropTest::Echo>::narrow(client->string_to_object(ior));
    CHECK(echo->echo_ref(echo)->_is_equivalent(echo));
    CHECK(echo->echo_records({rec, rec}).size() == 2);
    CHECK(echo->echo_octets(InteropTest::OctetSeq(4096, 7)).size() == 4096);
    try {
      echo->fail(i, "reason");
      CHECK(false);
    } catch (const InteropTest::Oops& e) {
      CHECK(e.code() == i);
    }
    try {
      echo->fail_system();
      CHECK(false);
    } catch (const CORBA::BAD_PARAM&) {
    }
  }
}

static void typecodes_and_anys(const OrbRef& server, const OrbRef& client, int rounds) {
  auto root = root_of(server);
  auto servant = CORBA::make_reference<anytest::AnyEchoImpl>();
  auto e = remote<AnyTest::AnyEcho>(server, client,
                                    root->id_to_reference(root->activate_object(servant)));
  for (int i = 0; i < rounds; ++i) {
    std::string id = "IDL:Leak/Node" + std::to_string(i) + ":1.0";
    auto node_seq = client->create_sequence_tc(0, client->create_recursive_tc(id));
    auto node = client->create_struct_tc(id, "Node", {{"name", CORBA::_tc_string},
                                                      {"children", node_seq}});
    CHECK(e->echo_tc(node)->equal(node));
    CHECK(e->echo_tc(AnyTest::_tc_Node)->equal(AnyTest::_tc_Node));
    for (const auto& name : anytest::names()) {
      CORBA::Any a = anytest::make(name, e);
      CHECK(anytest::same(name, e->echo(a), e));
    }
  }
}

int main() {
  int scale = 1;
  if (const char* s = std::getenv("RUSTORB_LEAK_ROUNDS")) scale = std::max(1, std::atoi(s));
  try {
    auto server = make_orb("leak-server");
    auto client = make_orb("leak-client");
    std::thread run([&] { server->run(); });
    servants(server, client, 100 * scale);
    // Deactivated servants are released once their last request is over.
    CHECK(live_servants == 0);
    references_and_exceptions(server, client, 200 * scale);
    typecodes_and_anys(server, client, 100 * scale);
    server->shutdown(true);
    run.join();
    server->destroy();
    client->destroy();
    // Destroying the ORB releases the servants still active.
    CHECK(live_servants == 0);
  } catch (const CORBA::Exception& e) {
    std::cout << "CORBA exception: " << e << std::endl;
    return 1;
  }
  std::cout << (failures ? "FAILED" : "ok") << std::endl;
  return failures ? 1 : 0;
}

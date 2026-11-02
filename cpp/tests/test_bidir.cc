// Bidirectional GIOP (docs/bidir-giop.md) in the C++ binding: the
// BiDirPolicy module, ORB::create_policy, POAs with the BidirectionalPolicy,
// the ORB's PolicyManager, and a callback over the connection the client
// opened.
//
// Two ORBs in this process, a server that accepts bidirectional GIOP and a
// client that offers it. The client's IORs name an address nobody listens
// on, so the callback works only over the client's own connection.

#include <iostream>
#include <string>
#include <thread>

#include "cases.h"
#include "echo_impl.h"

static int failures = 0;

#define EXPECT(cond)                                                             \
  do {                                                                           \
    if (!(cond)) {                                                               \
      std::cout << "FAIL line " << __LINE__ << ": " #cond << std::endl;          \
      ++failures;                                                                \
    }                                                                            \
  } while (0)

using PolicyRef = CORBA::object_reference<CORBA::Policy>;
using BiDirPolicy::BidirectionalPolicy;

static PolicyRef bidir(const CORBA::object_reference<CORBA::ORB>& orb,
                       BiDirPolicy::BidirectionalPolicyValue v) {
  CORBA::Any a;
  a <<= v;
  return orb->create_policy(BiDirPolicy::BIDIRECTIONAL_POLICY_TYPE, a);
}

static int value_of(const PolicyRef& p) {
  auto b = rustorb::detail::narrow<BidirectionalPolicy>(p);
  return b ? b->value() : -1;
}

static void create_policy(const CORBA::object_reference<CORBA::ORB>& orb) {
  PolicyRef p = bidir(orb, BiDirPolicy::BOTH);
  EXPECT(p->policy_type() == 37);
  EXPECT(value_of(p) == BiDirPolicy::BOTH);
  EXPECT(value_of(p->copy()) == BiDirPolicy::BOTH);
  EXPECT(value_of(bidir(orb, BiDirPolicy::NORMAL)) == BiDirPolicy::NORMAL);
  try {
    bidir(orb, 2);
    EXPECT(false);
  } catch (const CORBA::PolicyError& e) {
    EXPECT(e.reason() == CORBA::BAD_POLICY_VALUE);
  }
  CORBA::Any ulong;
  ulong <<= uint32_t{1};
  try {
    orb->create_policy(BiDirPolicy::BIDIRECTIONAL_POLICY_TYPE, ulong);
    EXPECT(false);
  } catch (const CORBA::PolicyError& e) {
    EXPECT(e.reason() == CORBA::BAD_POLICY_TYPE);
  }
}

static void policy_manager(const CORBA::object_reference<CORBA::ORB>& orb) {
  auto manager = IDL::traits<CORBA::PolicyManager>::narrow(
      orb->resolve_initial_references("ORBPolicyManager"));
  manager->set_policy_overrides({bidir(orb, BiDirPolicy::BOTH)},
                                CORBA::SetOverrideType::ADD_OVERRIDE);
  auto got = manager->get_policy_overrides({BiDirPolicy::BIDIRECTIONAL_POLICY_TYPE});
  EXPECT(got.size() == 1 && value_of(got[0]) == BiDirPolicy::BOTH);
  manager->set_policy_overrides({}, CORBA::SetOverrideType::SET_OVERRIDE);
  EXPECT(manager->get_policy_overrides({BiDirPolicy::BIDIRECTIONAL_POLICY_TYPE}).empty());
  // Only the ORB can hold it.
  auto current = IDL::traits<CORBA::PolicyCurrent>::narrow(
      orb->resolve_initial_references("PolicyCurrent"));
  try {
    current->set_policy_overrides({bidir(orb, BiDirPolicy::BOTH)},
                                  CORBA::SetOverrideType::ADD_OVERRIDE);
    EXPECT(false);
  } catch (const CORBA::NO_PERMISSION&) {
  }
}

static IDL::traits<InteropTest::Callback>::ref_type callback(
    const CORBA::object_reference<CORBA::ORB>& orb,
    const CORBA::object_reference<PortableServer::POA>& root, const std::string& name,
    BiDirPolicy::BidirectionalPolicyValue v) {
  auto poa = root->create_POA(name, root->the_POAManager(), {bidir(orb, v)});
  auto oid = poa->activate_object(CORBA::make_reference<CallbackImpl>());
  return IDL::traits<InteropTest::Callback>::narrow(poa->id_to_reference(oid));
}

int main() {
  try {
    int sargc = 5;
    char sarg0[] = "server", sarg1[] = "-ORBendPoint", sarg2[] = "giop:tcp:127.0.0.1:",
         sarg3[] = "-ORBacceptBiDirectionalGIOP", sarg4[] = "1";
    char* sargv[] = {sarg0, sarg1, sarg2, sarg3, sarg4, nullptr};
    auto server_orb = CORBA::ORB_init(sargc, sargv);
    auto root = IDL::traits<PortableServer::POA>::narrow(
        server_orb->resolve_initial_references("RootPOA"));
    root->the_POAManager()->activate();
    auto oid = root->activate_object(CORBA::make_reference<EchoImpl>(server_orb));
    std::string ior = server_orb->object_to_string(root->id_to_reference(oid));
    std::thread server([&] { server_orb->run(); });

    int cargc = 7;
    char carg0[] = "client", carg1[] = "-ORBendPoint", carg2[] = "giop:tcp:127.0.0.1:",
         carg3[] = "-ORBendPointPublish", carg4[] = "giop:tcp:127.0.0.1:1",
         carg5[] = "-ORBofferBiDirectionalGIOP", carg6[] = "1";
    char* cargv[] = {carg0, carg1, carg2, carg3, carg4, carg5, carg6, nullptr};
    auto client_orb = CORBA::ORB_init(cargc, cargv);
    auto client_root = IDL::traits<PortableServer::POA>::narrow(
        client_orb->resolve_initial_references("RootPOA"));
    client_root->the_POAManager()->activate();
    auto echo = IDL::traits<InteropTest::Echo>::narrow(client_orb->string_to_object(ior));

    create_policy(client_orb);
    policy_manager(client_orb);

    auto cb = callback(client_orb, client_root, "bidir", BiDirPolicy::BOTH);
    EXPECT(echo->call_back(cb, "hi") == "ack:hi");
    EXPECT(echo->call_back(cb, "again") == "ack:again");

    auto cb_normal = callback(client_orb, client_root, "normal", BiDirPolicy::NORMAL);
    try {
      echo->call_back(cb_normal, "hi");
      EXPECT(false);
    } catch (const CORBA::NO_PERMISSION&) {
    }

    echo->shutdown();
    server.join();
    client_orb->destroy();
    server_orb->destroy();
  } catch (const CORBA::Exception& e) {
    std::cout << "CORBA exception: " << e << std::endl;
    return 1;
  }
  if (failures == 0) std::cout << "all bidir tests passed" << std::endl;
  return failures == 0 ? 0 : 1;
}

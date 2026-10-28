// any and TypeCode: the stage 2 interop cases (interop/impls/rustorbcpp/
// anytest.h) in one process, between a server ORB and a client ORB over
// IIOP on the loopback interface, and checks of the TypeCode API.

#define ANYTEST_HIDDEN
#include <iostream>
#include <sstream>
#include <thread>

#include "anytest.h"

struct CheckFailed {
  std::string message;
};

#define CHECK(cond)                                                                       \
  do {                                                                                    \
    if (!(cond)) throw CheckFailed{"line " + std::to_string(__LINE__) + ": " #cond}; \
  } while (0)

class Runner {
 public:
  void run(const std::string& name, const std::function<void()>& fn) {
    try {
      fn();
      ++passed_;
      std::cout << "ok   " << name << std::endl;
      return;
    } catch (const CheckFailed& e) {
      fail(name, e.message);
    } catch (const CORBA::Exception& e) {
      std::ostringstream os;
      os << "unexpected " << e;
      fail(name, os.str());
    } catch (const std::exception& e) {
      fail(name, e.what());
    }
  }

  bool ok() const { return failed_ == 0; }
  void summary() const {
    std::cout << "\n" << passed_ << " passed, " << failed_ << " failed" << std::endl;
  }

 private:
  void fail(const std::string& name, const std::string& why) {
    ++failed_;
    std::cout << "FAIL " << name << ": " << why << std::endl;
  }
  int passed_ = 0;
  int failed_ = 0;
};

using anytest::TypeCodeRef;

// TypeCode operations, ORB::create_*_tc and the rules of <<= and >>=.
static void run_local_cases(Runner& r, const IDL::traits<CORBA::ORB>::ref_type& orb) {
  r.run("tc_queries", [&]() {
    auto tc = AnyTest::_tc_Choice;
    CHECK(tc->kind() == CORBA::TCKind::tk_union);
    CHECK(tc->id() == "IDL:AnyTest/Choice:1.0");
    CHECK(tc->name() == "Choice");
    CHECK(tc->member_count() == 4);
    CHECK(tc->member_name(1) == "s");
    CHECK(tc->member_type(1)->equal(CORBA::_tc_string));
    CHECK(tc->discriminator_type()->equal(CORBA::_tc_long));
    CHECK(tc->default_index() == 3);
    int32_t label = 0;
    CHECK(tc->member_label(2) >>= label);
    CHECK(label == 3);
    uint8_t zero = 1;
    CHECK(tc->member_label(3) >>= zero);
    CHECK(zero == 0);
    CHECK(AnyTest::_tc_ShortArray->content_type()->length() == 3);
    CHECK(AnyTest::_tc_Short5->content_type()->length() == 5);
  });
  r.run("tc_exceptions", [&]() {
    bool bad_kind = false, bounds = false;
    try {
      CORBA::_tc_long->member_count();
    } catch (const CORBA::TypeCode::BadKind&) {
      bad_kind = true;
    }
    try {
      AnyTest::_tc_Point->member_name(2);
    } catch (const CORBA::TypeCode::Bounds&) {
      bounds = true;
    }
    CHECK(bad_kind);
    CHECK(bounds);
  });
  r.run("tc_recursive", [&]() {
    // Node's children are a sequence of Node: the member type of the
    // sequence is Node itself.
    auto seq = AnyTest::_tc_Node->member_type(1)->content_type();
    CHECK(seq->kind() == CORBA::TCKind::tk_sequence);
    CHECK(seq->content_type()->equal(AnyTest::_tc_Node));
  });
  r.run("tc_equal_equivalent", [&]() {
    CHECK(!AnyTest::_tc_Count->equal(CORBA::_tc_long));
    CHECK(AnyTest::_tc_Count->equivalent(CORBA::_tc_long));
    auto compact = AnyTest::_tc_Point->get_compact_typecode();
    CHECK(compact->name().empty());
    CHECK(compact->member_name(0).empty());
    CHECK(compact->equivalent(AnyTest::_tc_Point));
    CHECK(!compact->equal(AnyTest::_tc_Point));
  });
  r.run("create_tc", [&]() {
    CORBA::StructMemberSeq members{{"x", CORBA::_tc_long}, {"y", CORBA::_tc_double}};
    auto point = orb->create_struct_tc("IDL:AnyTest/Point:1.0", "Point", members);
    CHECK(point->equal(AnyTest::_tc_Point));
    auto shade = orb->create_enum_tc("IDL:AnyTest/Shade:1.0", "Shade", {"LIGHT", "DARK", "DIM"});
    CHECK(shade->equal(AnyTest::_tc_Shade));
    CORBA::Any l1, l2, l3, dflt;
    l1 <<= int32_t{1};
    l2 <<= int32_t{2};
    l3 <<= int32_t{3};
    dflt <<= uint8_t{0};
    CORBA::UnionMemberSeq cases{{"l", l1, CORBA::_tc_long},
                                {"s", l2, CORBA::_tc_string},
                                {"s", l3, CORBA::_tc_string},
                                {"b", dflt, CORBA::_tc_boolean}};
    auto choice = orb->create_union_tc("IDL:AnyTest/Choice:1.0", "Choice", CORBA::_tc_long, cases);
    CHECK(choice->equal(AnyTest::_tc_Choice));
    auto node_seq = orb->create_alias_tc(
        "IDL:AnyTest/NodeSeq:1.0", "NodeSeq",
        orb->create_sequence_tc(0, orb->create_recursive_tc("IDL:AnyTest/Node:1.0")));
    auto node = orb->create_struct_tc("IDL:AnyTest/Node:1.0", "Node",
                                      {{"name", CORBA::_tc_string}, {"children", node_seq}});
    CHECK(node->equal(AnyTest::_tc_Node));
    CHECK(orb->create_array_tc(3, CORBA::_tc_short)->equal(AnyTest::_tc_ShortArray->content_type()));
    CHECK(orb->create_string_tc(5)->equal(AnyTest::_tc_Short5->content_type()));
    bool bad_param = false;
    try {
      orb->create_struct_tc("IDL:X:1.0", "X", {{"a", CORBA::_tc_long}, {"a", CORBA::_tc_long}});
    } catch (const CORBA::BAD_PARAM&) {
      bad_param = true;
    }
    CHECK(bad_param);
  });
  r.run("any_rules", [&]() {
    CORBA::Any a;
    CHECK(a.type()->kind() == CORBA::TCKind::tk_null);
    a <<= int32_t{7};
    int16_t s = 0;
    CHECK(!(a >>= s));
    a.type(AnyTest::_tc_Count);
    int32_t v = 0;
    CHECK((a >>= v) && v == 7);
    bool bad_typecode = false;
    try {
      a.type(CORBA::_tc_short);
    } catch (const CORBA::BAD_TYPECODE&) {
      bad_typecode = true;
    }
    CHECK(bad_typecode);
    // A bound exceeded is BAD_PARAM when the value goes in.
    bool bad_param = false;
    try {
      a <<= AnyTest::Short5("too long");
    } catch (const CORBA::BAD_PARAM&) {
      bad_param = true;
    }
    CHECK(bad_param);
    CHECK((a >>= v) && v == 7);
    // Copies and moves.
    CORBA::Any b = a;
    CORBA::Any c = std::move(a);
    CHECK(b == c);
    CHECK(a.type()->kind() == CORBA::TCKind::tk_null);
    c <<= "other";
    CHECK(b != c);
  });
}

int main() {
  try {
    int sargc = 3;
    char sarg0[] = "server", sarg1[] = "-ORBendPoint", sarg2[] = "giop:tcp:127.0.0.1:";
    char* sargv[] = {sarg0, sarg1, sarg2, nullptr};
    auto server_orb = CORBA::ORB_init(sargc, sargv);
    auto root = IDL::traits<PortableServer::POA>::narrow(
        server_orb->resolve_initial_references("RootPOA"));
    root->the_POAManager()->activate();
    auto servant = CORBA::make_reference<anytest::AnyEchoImpl>();
    auto oid = root->activate_object(servant);
    std::string ior = server_orb->object_to_string(root->id_to_reference(oid));
    std::thread server([&] { server_orb->run(); });

    int cargc = 1;
    char carg0[] = "client";
    char* cargv[] = {carg0, nullptr};
    auto client_orb = CORBA::ORB_init(cargc, cargv);
    auto echo = IDL::traits<AnyTest::AnyEcho>::narrow(client_orb->string_to_object(ior));
    if (!echo) {
      std::cout << "not an AnyTest::AnyEcho: " << ior << std::endl;
      return 1;
    }

    Runner r;
    run_local_cases(r, client_orb);
    anytest::run_cases(r, echo, [](bool cond, const std::string& what) {
      if (!cond) throw CheckFailed{what};
    });
    server_orb->shutdown(true);
    server.join();
    r.summary();

    server_orb->destroy();
    client_orb->destroy();
    return r.ok() ? 0 : 1;
  } catch (const CORBA::Exception& e) {
    std::cerr << "CORBA exception: " << e << std::endl;
    return 1;
  }
}

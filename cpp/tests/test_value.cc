// Valuetypes: the value interop cases (interop/impls/rustorbcpp/valuetest.h)
// in one process, over IIOP on the loopback interface, then truncation,
// unknown types, ORB::create_value_tc and _copy_value; then the abstract
// interface cases (abstracttest.h) and narrowing of abstract interfaces.

#include <iostream>
#include <thread>

#include "abstracttest.h"
#include "cases.h"
#include "valuetest.h"

int main() {
  try {
    int sargc = 3;
    char sarg0[] = "server", sarg1[] = "-ORBendPoint", sarg2[] = "giop:tcp:127.0.0.1:";
    char* sargv[] = {sarg0, sarg1, sarg2, nullptr};
    auto server_orb = CORBA::ORB_init(sargc, sargv);
    auto root = IDL::traits<PortableServer::POA>::narrow(
        server_orb->resolve_initial_references("RootPOA"));
    root->the_POAManager()->activate();
    auto oid = root->activate_object(CORBA::make_reference<valuetest::ValueEchoImpl>(server_orb));
    std::string ior = server_orb->object_to_string(root->id_to_reference(oid));
    auto abstract_oid =
        root->activate_object(CORBA::make_reference<abstracttest::AbstractEchoImpl>(server_orb));
    std::string abstract_ior = server_orb->object_to_string(root->id_to_reference(abstract_oid));
    std::thread server([&] { server_orb->run(); });

    int cargc = 1;
    char carg0[] = "client";
    char* cargv[] = {carg0, nullptr};
    auto orb = CORBA::ORB_init(cargc, cargv);
    auto echo = IDL::traits<ValueTest::ValueEcho>::narrow(orb->string_to_object(ior));
    valuetest::register_factories(orb);
    abstracttest::register_factories(orb);
    auto abstract_echo = IDL::traits<AbstractTest::AbstractEcho>::narrow(
        orb->string_to_object(abstract_ior));

    Runner r;
    auto check = [](bool cond, const std::string& what) {
      if (!cond) throw CheckFailed{what};
    };
    valuetest::run_cases(r, echo, check);

    using namespace ValueTest;
    r.run("factories", [&]() {
      check(orb->lookup_value_factory(Point::_rorb_repository_id) != nullptr, "no Point factory");
      check(orb->lookup_value_factory("IDL:nothing:1.0") == nullptr, "a factory for nothing");
    });
    // Without a factory for Point3 (the ORBs share them), a Point3 is read
    // as its truncatable base Point, and the derived state is skipped.
    r.run("truncation", [&]() {
      auto old = orb->register_value_factory(Point3::_rorb_repository_id, nullptr);
      orb->unregister_value_factory(Point3::_rorb_repository_id);
      auto got = echo->echo_point(valuetest::point3());
      orb->register_value_factory(Point3::_rorb_repository_id, old);
      check(valuetest::is_point(got, 1, 2), "not truncated to a Point");
    });
    // A Node has no truncatable base: without its factory, MARSHAL.
    r.run("no factory", [&]() {
      auto old = orb->register_value_factory(Node::_rorb_repository_id, nullptr);
      orb->unregister_value_factory(Node::_rorb_repository_id);
      bool marshal = false;
      try {
        echo->echo_node(valuetest::chain(2));
      } catch (const CORBA::MARSHAL&) {
        marshal = true;
      }
      orb->register_value_factory(Node::_rorb_repository_id, old);
      check(marshal, "no MARSHAL");
    });
    r.run("create_value_tc", [&]() {
      CORBA::ValueMemberSeq members{{"x", CORBA::_tc_long, CORBA::PUBLIC_MEMBER},
                                    {"y", CORBA::_tc_long, CORBA::PUBLIC_MEMBER}};
      auto tc = orb->create_value_tc(Point::_rorb_repository_id, "Point", CORBA::VM_NONE,
                                     nullptr, members);
      check(tc->equal(_tc_Point), "not the TypeCode of Point");
      check(_tc_Point3->type_modifier() == CORBA::VM_TRUNCATABLE, "not truncatable");
      check(_tc_Point3->concrete_base_type()->equal(_tc_Point), "wrong base");
      check(_tc_Point3->member_visibility(1) == CORBA::PRIVATE_MEMBER, "label not private");
      check(_tc_LongBox->kind() == CORBA::TCKind::tk_value_box, "not a value box");
    });
    r.run("_copy_value", [&]() {
      auto p = valuetest::point(3, 4);
      auto q = IDL::traits<Point>::narrow(p->_copy_value());
      check(q && q != p && valuetest::is_point(q, 3, 4), "wrong copy");
    });

    abstracttest::run_cases(r, abstract_echo, check);
    r.run("abstract narrow", [&]() {
      using namespace AbstractTest;
      auto person = abstract_echo->make_person("carol");
      IDL::traits<Named>::ref_type named = person;
      check(IDL::traits<Person>::narrow(named) == person, "Named to Person");
      // A plain object reference narrows to the stub of the abstract
      // interface, which narrows on to Person.
      IDL::traits<CORBA::Object>::ref_type obj = person;
      auto n = IDL::traits<Named>::narrow(obj);
      check(n && n->name() == "carol", "Object to Named");
      auto p = IDL::traits<Person>::narrow(n);
      check(p && p->age() == 42, "Named to Person");
      // A value narrows to its valuetype, and not to an interface.
      auto c = abstracttest::card("ten");
      check(IDL::traits<Card>::narrow(c) != nullptr, "Named to Card");
      check(IDL::traits<Person>::narrow(c) == nullptr, "a value narrowed to Person");
      check(_tc_Named->kind() == CORBA::TCKind::tk_abstract_interface, "wrong TypeCode kind");
    });

    echo->shutdown();
    server.join();
    r.summary();
    server_orb->destroy();
    orb->destroy();
    return r.failed().empty() ? 0 : 1;
  } catch (const CORBA::Exception& e) {
    std::cerr << "CORBA exception: " << e << std::endl;
    return 1;
  }
}

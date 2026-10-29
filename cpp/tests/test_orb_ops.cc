// The ORB operations of #39 in the C++ binding: ORB::id, create_policy,
// register_initial_reference, get_service_information, the policy
// overrides of references, of the ORB (ORBPolicyManager) and of the thread
// (PolicyCurrent), and Object::_validate_connection.

#include <iostream>
#include <string>
#include <thread>

#include "rustorb/cdr.h"

static int failures = 0;

#define CHECK(cond)                                                              \
  do {                                                                           \
    if (!(cond)) {                                                               \
      std::cout << "FAIL line " << __LINE__ << ": " #cond << std::endl;          \
      ++failures;                                                                \
    }                                                                            \
  } while (0)

using ObjectRef = CORBA::object_reference<CORBA::Object>;
using PolicyRef = CORBA::object_reference<CORBA::Policy>;
using Messaging::RelativeRoundtripTimeoutPolicy;

// The PolicyError reason create_policy raises, or -1.
template <typename T>
static int policy_error(const CORBA::object_reference<CORBA::ORB>& orb, CORBA::PolicyType type,
                        T value) {
  CORBA::Any a;
  a <<= value;
  try {
    orb->create_policy(type, a);
    return -1;
  } catch (const CORBA::PolicyError& e) {
    return e.reason();
  }
}

static uint64_t expiry(const PolicyRef& p) {
  auto rt = rustorb::detail::narrow<RelativeRoundtripTimeoutPolicy>(p);
  return rt ? rt->relative_expiry() : 0;
}

class Nothing : public virtual PortableServer::ServantBase {
 public:
  const char* _rorb_repository_id() const override { return "IDL:test/Nothing:1.0"; }
  rorb_status _rorb_dispatch(const char*, size_t, rorb_in*, rorb_server_reply*) override {
    return RORB_OK;
  }
};

static void create_policy(const CORBA::object_reference<CORBA::ORB>& orb) {
  CORBA::Any a;
  a <<= PortableServer::LifespanPolicyValue::PERSISTENT;
  PolicyRef p = orb->create_policy(PortableServer::LIFESPAN_POLICY_ID, a);
  auto lifespan = rustorb::detail::narrow<PortableServer::LifespanPolicy>(p);
  CHECK(lifespan && lifespan->value() == PortableServer::LifespanPolicyValue::PERSISTENT);
  CHECK(p->policy_type() == 17);

  a <<= uint64_t{2000000};
  PolicyRef rt = orb->create_policy(Messaging::RELATIVE_RT_TIMEOUT_POLICY_TYPE, a);
  CHECK(expiry(rt) == 2000000);
  CHECK(expiry(rt->copy()) == 2000000);

  CHECK(policy_error(orb, PortableServer::LIFESPAN_POLICY_ID, uint32_t{1}) ==
        CORBA::BAD_POLICY_TYPE);
  CHECK(policy_error(orb, PortableServer::LIFESPAN_POLICY_ID,
                     PortableServer::ThreadPolicyValue::ORB_CTRL_MODEL) ==
        CORBA::BAD_POLICY_TYPE);
  CHECK(policy_error(orb, Messaging::RELATIVE_RT_TIMEOUT_POLICY_TYPE, int32_t{1}) ==
        CORBA::BAD_POLICY_TYPE);
  CHECK(policy_error(orb, 9999, int32_t{1}) == CORBA::BAD_POLICY);

  // A policy from create_policy works in create_POA.
  auto root = IDL::traits<PortableServer::POA>::narrow(orb->resolve_initial_references("RootPOA"));
  a <<= PortableServer::IdAssignmentPolicyValue::USER_ID;
  auto child = root->create_POA("CreatePolicy", nullptr,
                                {orb->create_policy(PortableServer::ID_ASSIGNMENT_POLICY_ID, a)});
  try {
    child->activate_object(CORBA::make_reference<Nothing>());
    CHECK(false);
  } catch (const PortableServer::POA::WrongPolicy&) {
  }
}

static void initial_references(const CORBA::object_reference<CORBA::ORB>& orb) {
  CHECK(orb->id() == "rustorb");
  ObjectRef bar = orb->string_to_object("corbaloc::127.0.0.1:2/Bar");
  auto invalid = [&](const std::string& name, const ObjectRef& obj) {
    try {
      orb->register_initial_reference(name, obj);
      return false;
    } catch (const CORBA::ORB::InvalidName&) {
      return true;
    }
  };
  CHECK(invalid("", bar));
  CHECK(invalid("Foo", bar));  // -ORBInitRef
  CHECK(invalid("RootPOA", bar));
  CHECK(invalid("PolicyCurrent", bar));
  try {
    orb->register_initial_reference("Nil", nullptr);
    CHECK(false);
  } catch (const CORBA::BAD_PARAM& e) {
    CHECK(e.minor() == 0x4f4d001b);
  }
  orb->register_initial_reference("Bar", bar);
  CHECK(invalid("Bar", bar));
  CHECK(orb->resolve_initial_references("Bar")->_is_equivalent(bar));
  // A local object.
  ObjectRef manager = orb->resolve_initial_references("ORBPolicyManager");
  orb->register_initial_reference("Manager", manager);
  CHECK(invalid("Manager", bar));
  CHECK(orb->resolve_initial_references("Manager") == manager);

  auto names = orb->list_initial_services();
  for (const char* n : {"Foo", "Bar", "Manager", "RootPOA", "POACurrent", "ORBPolicyManager",
                        "PolicyCurrent"}) {
    bool found = false;
    for (auto& name : names) found = found || name == n;
    CHECK(found);
  }

  CORBA::ServiceInformation info;
  CHECK(!orb->get_service_information(1, info));
}

static void overrides(const CORBA::object_reference<CORBA::ORB>& orb) {
  ObjectRef obj = orb->string_to_object("corbaloc::127.0.0.1:1/x");
  auto rt = [](uint64_t t) {
    return PolicyRef(std::make_shared<RelativeRoundtripTimeoutPolicy>(t));
  };
  CHECK(!obj->_get_client_policy(Messaging::RELATIVE_RT_TIMEOUT_POLICY_TYPE));
  CHECK(!obj->_get_policy(PortableServer::LIFESPAN_POLICY_ID));

  ObjectRef with = obj->_set_policy_overrides({rt(30)}, CORBA::SetOverrideType::SET_OVERRIDE);
  CHECK(with->_is_equivalent(obj));
  CHECK(obj->_get_policy_overrides({}).empty());
  auto list = with->_get_policy_overrides({});
  CHECK(list.size() == 1 && expiry(list[0]) == 30);
  CHECK(with->_get_policy_overrides({PortableServer::LIFESPAN_POLICY_ID}).empty());
  CHECK(expiry(with->_get_policy(Messaging::RELATIVE_RT_TIMEOUT_POLICY_TYPE)) == 30);
  CHECK(rorb_object_call_timeout(with->_rorb_handle()) == 3);  // microseconds

  try {
    obj->_set_policy_overrides({rt(1), rt(2)}, CORBA::SetOverrideType::SET_OVERRIDE);
    CHECK(false);
  } catch (const CORBA::BAD_PARAM& e) {
    CHECK(e.minor() == 0x4f4d001e);
  }
  try {
    obj->_set_policy_overrides(
        {CORBA::make_reference<PortableServer::LifespanPolicy>(
            PortableServer::LifespanPolicyValue::PERSISTENT)},
        CORBA::SetOverrideType::ADD_OVERRIDE);
    CHECK(false);
  } catch (const CORBA::NO_PERMISSION&) {
  }

  auto manager = rustorb::detail::narrow<CORBA::PolicyManager>(
      orb->resolve_initial_references("ORBPolicyManager"));
  auto current = rustorb::detail::narrow<CORBA::PolicyCurrent>(
      orb->resolve_initial_references("PolicyCurrent"));
  CHECK(manager && current);
  manager->set_policy_overrides({rt(50)}, CORBA::SetOverrideType::SET_OVERRIDE);
  CHECK(expiry(obj->_get_client_policy(Messaging::RELATIVE_RT_TIMEOUT_POLICY_TYPE)) == 50);
  current->set_policy_overrides({rt(40)}, CORBA::SetOverrideType::ADD_OVERRIDE);
  CHECK(expiry(obj->_get_client_policy(Messaging::RELATIVE_RT_TIMEOUT_POLICY_TYPE)) == 40);
  CHECK(expiry(with->_get_client_policy(Messaging::RELATIVE_RT_TIMEOUT_POLICY_TYPE)) == 30);
  std::thread([&] {
    CHECK(current->get_policy_overrides({}).empty());
    CHECK(expiry(obj->_get_client_policy(Messaging::RELATIVE_RT_TIMEOUT_POLICY_TYPE)) == 50);
  }).join();
  current->set_policy_overrides({}, CORBA::SetOverrideType::SET_OVERRIDE);
  CHECK(current->get_policy_overrides({}).empty());
  CHECK(manager->get_policy_overrides({}).size() == 1);
  manager->set_policy_overrides({}, CORBA::SetOverrideType::SET_OVERRIDE);
  CHECK(manager->get_policy_overrides({}).empty());
  CHECK(!obj->_get_client_policy(Messaging::RELATIVE_RT_TIMEOUT_POLICY_TYPE));
}

static void validate_connection(const CORBA::object_reference<CORBA::ORB>& orb) {
  auto root = IDL::traits<PortableServer::POA>::narrow(orb->resolve_initial_references("RootPOA"));
  root->the_POAManager()->activate();
  ObjectRef obj = root->servant_to_reference(CORBA::make_reference<Nothing>());
  CORBA::PolicyList inconsistent;
  CHECK(obj->_validate_connection(inconsistent) && inconsistent.empty());
  ObjectRef missing = root->create_reference("IDL:test/Nothing:1.0");
  try {
    missing->_validate_connection(inconsistent);
    CHECK(false);
  } catch (const CORBA::OBJECT_NOT_EXIST&) {
  }
  try {
    orb->string_to_object("corbaloc::127.0.0.1:1/x")->_validate_connection(inconsistent);
    CHECK(false);
  } catch (const CORBA::TRANSIENT&) {
  }
  try {
    root->_validate_connection(inconsistent);
    CHECK(false);
  } catch (const CORBA::NO_IMPLEMENT&) {
  }
}

int main() {
  char a0[] = "test_orb_ops", a1[] = "-ORBInitRef", a2[] = "Foo=corbaloc::127.0.0.1:1/Foo",
       a3[] = "-ORBendPoint", a4[] = "giop:tcp:127.0.0.1:";
  char* argv[] = {a0, a1, a2, a3, a4, nullptr};
  int argc = 5;
  auto orb = CORBA::ORB_init(argc, argv);
  try {
    create_policy(orb);
    initial_references(orb);
    overrides(orb);
    validate_connection(orb);
  } catch (const CORBA::Exception& e) {
    std::cout << "FAIL: exception " << e._name() << std::endl;
    ++failures;
  }
  orb->destroy();
  if (failures) {
    std::cout << failures << " failures" << std::endl;
    return 1;
  }
  std::cout << "ok" << std::endl;
  return 0;
}

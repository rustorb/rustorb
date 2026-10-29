// Servant managers (ServantActivator, ServantLocator), adapter activators
// and default servants of the C++ binding, called over GIOP from a second
// ORB in the same process.

#include <algorithm>
#include <chrono>
#include <iostream>
#include <memory>
#include <mutex>
#include <stdexcept>
#include <string>
#include <thread>
#include <vector>

#include "CxxMapping.hh"

static int failures = 0;

#define CHECK(cond)                                                              \
  do {                                                                           \
    if (!(cond)) {                                                               \
      std::cout << "FAIL line " << __LINE__ << ": " #cond << std::endl;          \
      ++failures;                                                                \
    }                                                                            \
  } while (0)

template <typename E, typename F> static bool raises(F fn) {
  try {
    fn();
  } catch (const E&) {
    return true;
  } catch (const CORBA::Exception& e) {
    std::cout << "unexpected exception: " << e << std::endl;
  }
  return false;
}

using namespace CxxMapping;
using PortableServer::ObjectId;
using POARef = IDL::traits<PortableServer::POA>::ref_type;

static const char* const kMultiId = "IDL:rustorb.org/CxxMapping/Multi:1.0";

// What the servant managers saw, in order.
class Log {
 public:
  void add(const std::string& s) {
    std::lock_guard<std::mutex> lock(m_);
    events_.push_back(s);
  }
  std::vector<std::string> take() {
    std::lock_guard<std::mutex> lock(m_);
    return std::move(events_);
  }
  // Waits for an event that arrives after the reply (etherealize).
  bool wait_for(const std::string& s) {
    for (int i = 0; i < 100; ++i) {
      {
        std::lock_guard<std::mutex> lock(m_);
        auto it = std::find(events_.begin(), events_.end(), s);
        if (it != events_.end()) {
          events_.erase(it);
          return true;
        }
      }
      std::this_thread::sleep_for(std::chrono::milliseconds(20));
    }
    return false;
  }

 private:
  std::mutex m_;
  std::vector<std::string> events_;
};

static Log events;

class MultiImpl : public CORBA::servant_traits<Multi>::base_type {
 public:
  explicit MultiImpl(std::string label) : label_(std::move(label)) {}
  int32_t id() override { return 5; }
  std::string label() override { return label_; }
  void label(const std::string& v) override { label_ = v; }
  void base_op(bool) override {}
  int32_t derived_op(int32_t a, int32_t&, int32_t&) override { return a; }
  void notify(const std::string&) override {}
  IDL::traits<CxxMapping::Base>::ref_type as_base() override { return _this(); }
  std::string other_op() override { return "other"; }
  CxxMapping::Base::BaseInfo multi_op(const CxxMapping::Base::BaseInfo& info) override {
    return info;
  }

 private:
  std::string label_;
};

static std::string label_of(const PortableServer::Servant& s) {
  auto* m = dynamic_cast<MultiImpl*>(s.get());
  return m ? m->label() : "?";
}

// A servant per object id, made when it is first called. "bad", "fwd",
// "nil" and "throw" fail in different ways.
class Activator : public PortableServer::ServantActivator {
 public:
  explicit Activator(IDL::traits<CORBA::Object>::ref_type forward) : forward_(std::move(forward)) {}

  PortableServer::Servant incarnate(const ObjectId& oid, POARef adapter) override {
    std::string name = PortableServer::ObjectId_to_string(oid);
    events.add("incarnate " + name + " in " + adapter->the_name());
    if (name == "bad") throw CORBA::OBJECT_NOT_EXIST(7, CORBA::CompletionStatus::COMPLETED_NO);
    if (name == "fwd") throw PortableServer::ForwardRequest(forward_);
    if (name == "nil") return nullptr;
    if (name == "throw") throw std::runtime_error("incarnate failed");
    return CORBA::make_reference<MultiImpl>(name);
  }

  void etherealize(const ObjectId& oid, POARef, PortableServer::Servant serv,
                   bool cleanup_in_progress, bool remaining_activations) override {
    events.add("etherealize " + PortableServer::ObjectId_to_string(oid) + " " + label_of(serv) +
               (cleanup_in_progress ? " cleanup" : "") +
               (remaining_activations ? " remaining" : ""));
  }

 private:
  IDL::traits<CORBA::Object>::ref_type forward_;
};

// A servant for every request; the cookie counts the calls.
class Locator : public PortableServer::ServantLocator {
 public:
  explicit Locator(IDL::traits<CORBA::Object>::ref_type forward) : forward_(std::move(forward)) {}

  PortableServer::Servant preinvoke(const ObjectId& oid, POARef, const std::string& operation,
                                    Cookie& the_cookie) override {
    std::string name = PortableServer::ObjectId_to_string(oid);
    events.add("preinvoke " + name + " " + operation);
    if (name == "bad") throw CORBA::OBJECT_NOT_EXIST(8, CORBA::CompletionStatus::COMPLETED_NO);
    if (name == "fwd") throw PortableServer::ForwardRequest(forward_);
    the_cookie = new int(++calls_);
    return CORBA::make_reference<MultiImpl>(name);
  }

  void postinvoke(const ObjectId& oid, POARef, const std::string& operation, Cookie the_cookie,
                  PortableServer::Servant the_servant) override {
    std::unique_ptr<int> n(static_cast<int*>(the_cookie));
    events.add("postinvoke " + PortableServer::ObjectId_to_string(oid) + " " + operation + " " +
               label_of(the_servant) + " cookie " + std::to_string(n ? *n : -1));
  }

 private:
  IDL::traits<CORBA::Object>::ref_type forward_;
  int calls_ = 0;
};

static CORBA::PolicyList persistent_user(const POARef& root) {
  return {root->create_lifespan_policy(PortableServer::LifespanPolicyValue::PERSISTENT),
          root->create_id_assignment_policy(PortableServer::IdAssignmentPolicyValue::USER_ID)};
}

// Creates POAs named lazy* on demand, with a default servant.
class LazyActivator : public PortableServer::AdapterActivator {
 public:
  bool unknown_adapter(POARef parent, const std::string& name) override {
    events.add("unknown_adapter " + name);
    if (name.compare(0, 4, "lazy") != 0) return false;
    CORBA::PolicyList policies = persistent_user(parent);
    policies.push_back(parent->create_servant_retention_policy(
        PortableServer::ServantRetentionPolicyValue::NON_RETAIN));
    policies.push_back(parent->create_request_processing_policy(
        PortableServer::RequestProcessingPolicyValue::USE_DEFAULT_SERVANT));
    policies.push_back(parent->create_id_uniqueness_policy(
        PortableServer::IdUniquenessPolicyValue::MULTIPLE_ID));
    auto poa = parent->create_POA(name, parent->the_POAManager(), policies);
    poa->set_servant(CORBA::make_reference<MultiImpl>("default of " + name));
    return true;
  }
};

static IDL::traits<Multi>::ref_type remote(const IDL::traits<CORBA::ORB>::ref_type& client,
                                           const IDL::traits<CORBA::ORB>::ref_type& server,
                                           const POARef& poa, const std::string& oid) {
  auto ref = poa->create_reference_with_id(PortableServer::string_to_ObjectId(oid), kMultiId);
  return IDL::traits<Multi>::narrow(client->string_to_object(server->object_to_string(ref)));
}

static void check_activator(const IDL::traits<CORBA::ORB>::ref_type& client,
                            const IDL::traits<CORBA::ORB>::ref_type& server, const POARef& root,
                            const IDL::traits<CORBA::Object>::ref_type& target,
                            std::weak_ptr<Activator>& weak) {
  using namespace PortableServer;
  CORBA::PolicyList policies = persistent_user(root);
  policies.push_back(
      root->create_request_processing_policy(RequestProcessingPolicyValue::USE_SERVANT_MANAGER));
  auto poa = root->create_POA("activator", root->the_POAManager(), policies);

  CHECK(poa->get_servant_manager() == nullptr);
  auto activator = CORBA::make_reference<Activator>(target);
  weak = activator._rorb_shared();
  // A locator does not fit a RETAIN POA.
  CHECK(raises<CORBA::OBJ_ADAPTER>([&] {
    poa->set_servant_manager(CORBA::make_reference<Locator>(target));
  }));
  poa->set_servant_manager(activator);
  CHECK(poa->get_servant_manager() == activator);
  CHECK(raises<CORBA::BAD_INV_ORDER>([&] { poa->set_servant_manager(activator); }));
  CHECK(raises<POA::WrongPolicy>([&] { root->get_servant_manager(); }));
  CHECK(raises<POA::WrongPolicy>([&] { root->set_servant_manager(activator); }));

  auto a = remote(client, server, poa, "a");
  CHECK(a->label() == "a");
  a->label("A");
  CHECK(a->label() == "A");
  // Incarnated once, then in the active object map.
  auto seen = events.take();
  CHECK(seen == std::vector<std::string>{"incarnate a in activator"});
  CHECK(label_of(poa->id_to_servant(string_to_ObjectId("a"))) == "A");

  CHECK(remote(client, server, poa, "fwd")->label() == "target");
  try {
    remote(client, server, poa, "bad")->label();
    CHECK(false);
  } catch (const CORBA::OBJECT_NOT_EXIST& e) {
    CHECK(e.minor() == 7);
  }
  CHECK(raises<CORBA::OBJ_ADAPTER>([&] { remote(client, server, poa, "nil")->label(); }));
  CHECK(raises<CORBA::UNKNOWN>([&] { remote(client, server, poa, "throw")->label(); }));
  events.take();

  poa->deactivate_object(string_to_ObjectId("a"));
  CHECK(events.wait_for("etherealize a A"));
  // Incarnated again after the deactivation.
  CHECK(a->label() == "a");
  CHECK(events.take() == std::vector<std::string>{"incarnate a in activator"});

  // Activated objects of a destroyed POA are etherealized with
  // cleanup_in_progress.
  poa->destroy(true, true);
  CHECK(events.wait_for("etherealize a a cleanup"));
}

static void check_locator(const IDL::traits<CORBA::ORB>::ref_type& client,
                          const IDL::traits<CORBA::ORB>::ref_type& server, const POARef& root,
                          const IDL::traits<CORBA::Object>::ref_type& target) {
  using namespace PortableServer;
  CORBA::PolicyList policies = persistent_user(root);
  policies.push_back(
      root->create_request_processing_policy(RequestProcessingPolicyValue::USE_SERVANT_MANAGER));
  policies.push_back(
      root->create_servant_retention_policy(ServantRetentionPolicyValue::NON_RETAIN));
  auto poa = root->create_POA("locator", root->the_POAManager(), policies);
  CHECK(raises<CORBA::OBJ_ADAPTER>([&] {
    poa->set_servant_manager(CORBA::make_reference<Activator>(target));
  }));
  auto locator = CORBA::make_reference<Locator>(target);
  poa->set_servant_manager(locator);
  CHECK(poa->get_servant_manager() == locator);

  auto x = remote(client, server, poa, "x");
  CHECK(x->label() == "x");
  CHECK(x->other_op() == "other");
  CHECK((events.take() == std::vector<std::string>{
             "preinvoke x _get_label", "postinvoke x _get_label x cookie 1",
             "preinvoke x other_op", "postinvoke x other_op x cookie 2"}));
  CHECK(remote(client, server, poa, "fwd")->label() == "target");
  try {
    remote(client, server, poa, "bad")->label();
    CHECK(false);
  } catch (const CORBA::OBJECT_NOT_EXIST& e) {
    CHECK(e.minor() == 8);
  }
  // postinvoke is not called when preinvoke failed.
  CHECK((events.take() == std::vector<std::string>{"preinvoke fwd _get_label",
                                                   "preinvoke bad _get_label"}));
}

static void check_adapter_activator(const IDL::traits<CORBA::ORB>::ref_type& client,
                                    const IDL::traits<CORBA::ORB>::ref_type& server,
                                    const POARef& root,
                                    std::weak_ptr<LazyActivator>& weak) {
  CHECK(root->the_activator() == nullptr);
  auto activator = CORBA::make_reference<LazyActivator>();
  weak = activator._rorb_shared();
  root->the_activator(activator);
  CHECK(root->the_activator() == activator);

  auto lazy = root->find_POA("lazy1", true);
  CHECK(lazy->the_name() == "lazy1");
  CHECK(raises<PortableServer::POA::AdapterNonExistent>([&] { root->find_POA("nope", true); }));
  CHECK(raises<PortableServer::POA::AdapterNonExistent>([&] { root->find_POA("lazy9", false); }));
  CHECK((events.take() ==
         std::vector<std::string>{"unknown_adapter lazy1", "unknown_adapter nope"}));

  // A request for a POA that does not exist (any more) makes it again.
  auto made = root->find_POA("lazy2", true);
  auto obj = remote(client, server, made, "o");
  made->destroy(false, true);
  events.take();
  CHECK(obj->label() == "default of lazy2");
  CHECK(events.take() == std::vector<std::string>{"unknown_adapter lazy2"});
  CHECK(lazy->get_servant() != nullptr);

  root->the_activator(nullptr);
  CHECK(root->the_activator() == nullptr);
  CHECK(raises<PortableServer::POA::AdapterNonExistent>([&] { root->find_POA("lazy3", true); }));
  CHECK(events.take().empty());
}

int main() {
  std::weak_ptr<Activator> activator;
  std::weak_ptr<LazyActivator> adapter_activator;
  try {
    int sargc = 3;
    char sarg0[] = "server", sarg1[] = "-ORBendPoint", sarg2[] = "giop:tcp:127.0.0.1:";
    char* sargv[] = {sarg0, sarg1, sarg2, nullptr};
    auto server_orb = CORBA::ORB_init(sargc, sargv);
    auto root = IDL::traits<PortableServer::POA>::narrow(
        server_orb->resolve_initial_references("RootPOA"));
    root->the_POAManager()->activate();
    auto target = CORBA::make_reference<MultiImpl>("target")->_this();
    std::thread server([&] { server_orb->run(); });

    int cargc = 1;
    char carg0[] = "client";
    char* cargv[] = {carg0, nullptr};
    auto client_orb = CORBA::ORB_init(cargc, cargv);

    check_activator(client_orb, server_orb, root, target, activator);
    check_locator(client_orb, server_orb, root, target);
    check_adapter_activator(client_orb, server_orb, root, adapter_activator);
    // The same through colocated calls.
    check_locator(server_orb, server_orb, root->find_POA("lazy1", false), target);

    server_orb->shutdown(true);
    server.join();
    server_orb->destroy();
    client_orb->destroy();
  } catch (const CORBA::Exception& e) {
    std::cout << "CORBA exception: " << e << std::endl;
    return 1;
  }
  // The POAs let the managers go.
  CHECK(activator.expired());
  CHECK(adapter_activator.expired());
  std::cout << (failures ? "FAILED" : "ok") << std::endl;
  return failures ? 1 : 0;
}

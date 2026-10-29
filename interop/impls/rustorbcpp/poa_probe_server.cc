// rustorb C++ server for the POA and thread model probes (PoaProbe.idl).
//
//   poa_probe_server --host HOST --port N --ior-file FILE [ORB options]
//
// The same objects and POAs as impls/omniorbpy/poa_probe_server.py, on the
// IDL to C++11 mapping; see run-poa-probe.sh.

#include <chrono>
#include <cstdio>
#include <cstdlib>
#include <fstream>
#include <iostream>
#include <map>
#include <mutex>
#include <string>
#include <thread>
#include <vector>

#include "PoaProbe.hh"

namespace {

using Clock = std::chrono::steady_clock;
const Clock::time_point kStart = Clock::now();
std::thread::id g_main_thread;
const char* const kTargetId = "IDL:rustorb.org/PoaProbe/Target:1.0";

using POARef = IDL::traits<PortableServer::POA>::ref_type;
using CurrentRef = IDL::traits<PortableServer::Current>::ref_type;
using ObjectRef = IDL::traits<CORBA::Object>::ref_type;

double now() { return std::chrono::duration<double>(Clock::now() - kStart).count(); }

std::mutex g_mutex;
std::map<std::thread::id, std::string> g_labels;
std::vector<std::string> g_events;

std::string thread_label() {
  auto id = std::this_thread::get_id();
  if (id == g_main_thread) return "main";
  std::lock_guard<std::mutex> lock(g_mutex);
  auto it = g_labels.find(id);
  if (it != g_labels.end()) return it->second;
  std::string label = "T" + std::to_string(g_labels.size() + 1);
  g_labels[id] = label;
  return label;
}

void event(const std::string& text) {
  std::lock_guard<std::mutex> lock(g_mutex);
  g_events.push_back(text);
}

PortableServer::ObjectId to_oid(const std::string& s) {
  return PortableServer::string_to_ObjectId(s);
}

std::string poa_path(POARef poa) {
  std::vector<std::string> names;
  while (poa) {
    POARef parent = poa->the_parent();
    if (!parent) break;
    names.insert(names.begin(), poa->the_name());
    poa = parent;
  }
  if (names.empty()) return "RootPOA";
  std::string path;
  for (auto& n : names) path += (path.empty() ? "" : "/") + n;
  return path;
}

class TargetImpl : public CORBA::servant_traits<PoaProbe::Target>::base_type {
 public:
  explicit TargetImpl(CurrentRef current) : current_(std::move(current)) {}

  PoaProbe::CallInfo work(int32_t ms) override {
    double start = now();
    std::string thread = thread_label();
    std::string oid = PortableServer::ObjectId_to_string(current_->get_object_id());
    std::string poa = poa_path(current_->get_POA());
    if (ms > 0) std::this_thread::sleep_for(std::chrono::milliseconds(ms));
    return PoaProbe::CallInfo(thread, oid, poa, start, now());
  }

  void post(int32_t seq, int32_t ms) override {
    if (ms > 0) std::this_thread::sleep_for(std::chrono::milliseconds(ms));
    event("post " + std::to_string(seq));
  }

  PoaProbe::CallInfo relay(IDL::traits<PoaProbe::Target>::ref_type other, int32_t ms,
                           std::string& outer_thread) override {
    outer_thread = thread_label();
    return other->work(ms);
  }

 private:
  CurrentRef current_;
};

class Activator : public PortableServer::ServantActivator {
 public:
  Activator(PortableServer::Servant servant, ObjectRef forward)
      : servant_(std::move(servant)), forward_(std::move(forward)) {}

  PortableServer::Servant incarnate(const PortableServer::ObjectId& oid, POARef) override {
    std::string name = PortableServer::ObjectId_to_string(oid);
    event("incarnate " + name);
    if (name == "bad") throw CORBA::OBJECT_NOT_EXIST(0, CORBA::CompletionStatus::COMPLETED_NO);
    if (name == "fwd") throw PortableServer::ForwardRequest(forward_);
    return servant_;
  }

  void etherealize(const PortableServer::ObjectId& oid, POARef, PortableServer::Servant, bool,
                   bool) override {
    event("etherealize " + PortableServer::ObjectId_to_string(oid));
  }

 private:
  PortableServer::Servant servant_;
  ObjectRef forward_;
};

class Locator : public PortableServer::ServantLocator {
 public:
  Locator(PortableServer::Servant servant, ObjectRef forward)
      : servant_(std::move(servant)), forward_(std::move(forward)) {}

  PortableServer::Servant preinvoke(const PortableServer::ObjectId& oid, POARef,
                                    const std::string& operation, Cookie&) override {
    std::string name = PortableServer::ObjectId_to_string(oid);
    event("preinvoke " + name + " " + operation);
    if (name == "bad") throw CORBA::OBJECT_NOT_EXIST(0, CORBA::CompletionStatus::COMPLETED_NO);
    if (name == "fwd") throw PortableServer::ForwardRequest(forward_);
    return servant_;
  }

  void postinvoke(const PortableServer::ObjectId& oid, POARef, const std::string& operation,
                  Cookie, PortableServer::Servant) override {
    event("postinvoke " + PortableServer::ObjectId_to_string(oid) + " " + operation);
  }

 private:
  PortableServer::Servant servant_;
  ObjectRef forward_;
};

// Creates POAs named lazy* on demand, with a default servant.
class AdapterActivatorImpl : public PortableServer::AdapterActivator {
 public:
  explicit AdapterActivatorImpl(PortableServer::Servant servant) : servant_(std::move(servant)) {}

  bool unknown_adapter(POARef parent, const std::string& name) override {
    using namespace PortableServer;
    event("unknown_adapter " + name);
    if (name.rfind("lazy", 0) != 0) return false;
    CORBA::PolicyList pl{
        parent->create_lifespan_policy(LifespanPolicyValue::PERSISTENT),
        parent->create_id_assignment_policy(IdAssignmentPolicyValue::USER_ID),
        parent->create_servant_retention_policy(ServantRetentionPolicyValue::NON_RETAIN),
        parent->create_request_processing_policy(RequestProcessingPolicyValue::USE_DEFAULT_SERVANT),
        parent->create_id_uniqueness_policy(IdUniquenessPolicyValue::MULTIPLE_ID)};
    POARef poa = parent->create_POA(name, parent->the_POAManager(), pl);
    poa->set_servant(servant_);
    return true;
  }

 private:
  PortableServer::Servant servant_;
};

class ControlImpl : public CORBA::servant_traits<PoaProbe::Control>::base_type {
 public:
  ControlImpl(IDL::traits<CORBA::ORB>::ref_type orb, POARef root,
              IDL::traits<PortableServer::POAManager>::ref_type managed, CurrentRef current,
              std::map<std::string, ObjectRef> system_refs)
      : orb_(std::move(orb)),
        root_(std::move(root)),
        managed_(std::move(managed)),
        current_(std::move(current)),
        system_refs_(std::move(system_refs)) {}

  std::string reference(const std::string& poa, const std::string& oid) override {
    auto it = system_refs_.find(poa);
    if (it != system_refs_.end()) return orb_->object_to_string(it->second);
    return orb_->object_to_string(find(poa)->create_reference_with_id(to_oid(oid), kTargetId));
  }

  void set_state(const std::string& s) override {
    try {
      if (s == "active") managed_->activate();
      else if (s == "holding") managed_->hold_requests(false);
      else if (s == "discarding") managed_->discard_requests(false);
      else if (s == "inactive") managed_->deactivate(false, false);
      else throw PoaProbe::Failed("unknown state " + s);
    } catch (const PortableServer::POAManager::AdapterInactive&) {
      throw PoaProbe::Failed("AdapterInactive");
    }
  }

  void deactivate_object(const std::string& poa, const std::string& oid) override {
    try {
      find(poa)->deactivate_object(to_oid(oid));
    } catch (const PortableServer::POA::ObjectNotActive&) {
      throw PoaProbe::Failed("ObjectNotActive");
    }
  }

  void activate_object(const std::string& poa, const std::string& oid) override {
    try {
      find(poa)->activate_object_with_id(to_oid(oid), CORBA::make_reference<TargetImpl>(current_));
    } catch (const PortableServer::POA::ObjectAlreadyActive&) {
      throw PoaProbe::Failed("ObjectAlreadyActive");
    } catch (const PortableServer::POA::ServantAlreadyActive&) {
      throw PoaProbe::Failed("ServantAlreadyActive");
    }
  }

  void destroy_poa(const std::string& poa) override { find(poa)->destroy(true, false); }

  PoaProbe::StringSeq events() override {
    std::lock_guard<std::mutex> lock(g_mutex);
    PoaProbe::StringSeq seq(g_events.begin(), g_events.end());
    g_events.clear();
    return seq;
  }

  std::string shutdown_inside(bool wait_for_completion) override {
    try {
      orb_->shutdown(wait_for_completion);
    } catch (const CORBA::SystemException& e) {
      char buf[160];
      std::snprintf(buf, sizeof buf, "%s minor 0x%08x", e._rep_id(),
                    static_cast<unsigned>(e.minor()));
      return buf;
    }
    return "";
  }

  void shutdown_later(int32_t delay_ms) override {
    auto orb = orb_;
    std::thread([orb, delay_ms]() {
      std::this_thread::sleep_for(std::chrono::milliseconds(delay_ms));
      orb->shutdown(false);
    }).detach();
  }

 private:
  POARef find(const std::string& s) {
    POARef poa = root_;
    size_t start = 0;
    while (start < s.size()) {
      size_t end = s.find('/', start);
      if (end == std::string::npos) end = s.size();
      std::string name = s.substr(start, end - start);
      if (!name.empty() && name != "RootPOA") {
        try {
          poa = poa->find_POA(name, true);
        } catch (const PortableServer::POA::AdapterNonExistent&) {
          throw PoaProbe::Failed("no POA " + s);
        }
      }
      start = end + 1;
    }
    return poa;
  }

  IDL::traits<CORBA::ORB>::ref_type orb_;
  POARef root_;
  IDL::traits<PortableServer::POAManager>::ref_type managed_;
  CurrentRef current_;
  std::map<std::string, ObjectRef> system_refs_;
};

}  // namespace

int main(int argc, char** argv) {
  g_main_thread = std::this_thread::get_id();
  std::string host, ior_file;
  int port = 0;
  std::vector<std::string> orb_args{argv[0]};
  for (int i = 1; i < argc; ++i) {
    std::string a = argv[i];
    if (a == "--host" && i + 1 < argc) host = argv[++i];
    else if (a == "--port" && i + 1 < argc) port = std::atoi(argv[++i]);
    else if (a == "--ior-file" && i + 1 < argc) ior_file = argv[++i];
    else orb_args.push_back(a);
  }
  if (ior_file.empty()) {
    std::cerr << "usage: " << argv[0] << " --ior-file FILE [--host H] [--port N]" << std::endl;
    return 2;
  }
  if (port) {
    orb_args.push_back("-ORBendPoint");
    orb_args.push_back("giop:tcp:" + host + ":" + std::to_string(port));
  }
  std::vector<char*> cargv;
  for (auto& a : orb_args) cargv.push_back(const_cast<char*>(a.c_str()));
  cargv.push_back(nullptr);
  int cargc = static_cast<int>(orb_args.size());

  try {
    using namespace PortableServer;
    auto orb = CORBA::ORB_init(cargc, cargv.data());
    auto root = IDL::traits<POA>::narrow(orb->resolve_initial_references("RootPOA"));
    auto current = IDL::traits<Current>::narrow(orb->resolve_initial_references("POACurrent"));
    auto root_manager = root->the_POAManager();
    root_manager->activate();

    auto target = [&]() { return CORBA::make_reference<TargetImpl>(current); };
    auto policies = [&](LifespanPolicyValue lifespan, IdAssignmentPolicyValue ids,
                        CORBA::PolicyList extra = {}) {
      CORBA::PolicyList pl{root->create_lifespan_policy(lifespan),
                           root->create_id_assignment_policy(ids)};
      pl.insert(pl.end(), extra.begin(), extra.end());
      return pl;
    };
    auto persistent_user_policies = [&](CORBA::PolicyList extra = {}) {
      return policies(LifespanPolicyValue::PERSISTENT, IdAssignmentPolicyValue::USER_ID,
                      std::move(extra));
    };
    auto create = [&](const POARef& parent, const std::string& name,
                      const CORBA::PolicyList& pl) {
      return parent->create_POA(name, root_manager, pl);
    };
    std::map<std::string, ObjectRef> system_refs;

    auto servant = target();
    // RootPOA: TRANSIENT, SYSTEM_ID, IMPLICIT_ACTIVATION.
    system_refs["RootPOA"] = servant->_this();

    auto persistent_user = create(root, "persistent_user", persistent_user_policies());
    persistent_user->activate_object_with_id(to_oid("obj"), servant);
    persistent_user->activate_object_with_id(to_oid("victim"), target());
    auto nested = create(persistent_user, "nested", persistent_user_policies());
    nested->activate_object_with_id(to_oid("obj"), servant);

    auto persistent_system = create(
        root, "persistent_system",
        policies(LifespanPolicyValue::PERSISTENT, IdAssignmentPolicyValue::SYSTEM_ID));
    auto oid = persistent_system->activate_object(target());
    system_refs["persistent_system"] = persistent_system->id_to_reference(oid);

    auto transient_user = create(
        root, "transient_user",
        policies(LifespanPolicyValue::TRANSIENT, IdAssignmentPolicyValue::USER_ID));
    transient_user->activate_object_with_id(to_oid("obj"), servant);

    auto transient_system = create(
        root, "transient_system",
        policies(LifespanPolicyValue::TRANSIENT, IdAssignmentPolicyValue::SYSTEM_ID));
    oid = transient_system->activate_object(target());
    system_refs["transient_system"] = transient_system->id_to_reference(oid);

    for (const char* name : {"single", "single2"}) {
      auto poa = create(root, name, persistent_user_policies({root->create_thread_policy(
                                        ThreadPolicyValue::SINGLE_THREAD_MODEL)}));
      poa->activate_object_with_id(to_oid("a"), servant);
      poa->activate_object_with_id(to_oid("b"), target());
    }

    auto main_poa = create(root, "main", persistent_user_policies({root->create_thread_policy(
                                             ThreadPolicyValue::MAIN_THREAD_MODEL)}));
    main_poa->activate_object_with_id(to_oid("a"), servant);
    main_poa->activate_object_with_id(to_oid("b"), target());

    auto managed = root->create_POA("managed", nullptr, persistent_user_policies());
    auto managed_manager = managed->the_POAManager();
    managed->activate_object_with_id(to_oid("obj"), servant);

    auto doomed = create(root, "doomed", persistent_user_policies());
    doomed->activate_object_with_id(to_oid("obj"), servant);

    auto forward = persistent_user->id_to_reference(to_oid("obj"));
    auto activator = create(root, "activator",
                            persistent_user_policies({root->create_request_processing_policy(
                                RequestProcessingPolicyValue::USE_SERVANT_MANAGER)}));
    activator->set_servant_manager(CORBA::make_reference<Activator>(servant, forward));
    auto locator = create(
        root, "locator",
        persistent_user_policies(
            {root->create_request_processing_policy(
                 RequestProcessingPolicyValue::USE_SERVANT_MANAGER),
             root->create_servant_retention_policy(ServantRetentionPolicyValue::NON_RETAIN)}));
    locator->set_servant_manager(CORBA::make_reference<Locator>(servant, forward));

    auto dflt = create(
        root, "default",
        persistent_user_policies(
            {root->create_request_processing_policy(
                 RequestProcessingPolicyValue::USE_DEFAULT_SERVANT),
             root->create_servant_retention_policy(ServantRetentionPolicyValue::NON_RETAIN),
             root->create_id_uniqueness_policy(IdUniquenessPolicyValue::MULTIPLE_ID)}));
    dflt->set_servant(servant);

    root->the_activator(CORBA::make_reference<AdapterActivatorImpl>(servant));

    // The Control object has its own POAManager, so that it keeps working
    // whatever the probe does to the others.
    auto control_poa = root->create_POA("control", nullptr, persistent_user_policies());
    control_poa->activate_object_with_id(
        to_oid("Control"),
        CORBA::make_reference<ControlImpl>(orb, root, managed_manager, current, system_refs));
    control_poa->the_POAManager()->activate();
    managed_manager->activate();

    std::string ior = orb->object_to_string(control_poa->id_to_reference(to_oid("Control")));
    {
      std::ofstream f(ior_file + ".tmp");
      f << ior << "\n";
    }
    std::rename((ior_file + ".tmp").c_str(), ior_file.c_str());
    std::cout << "server ready" << std::endl;

    orb->run();
    std::cout << "server stopped" << std::endl;
    orb->destroy();
  } catch (const CORBA::Exception& e) {
    std::cerr << "CORBA exception " << e << std::endl;
    return 1;
  }
  return 0;
}

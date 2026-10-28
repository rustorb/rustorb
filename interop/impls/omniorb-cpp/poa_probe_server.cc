// omniORB C++ server for the POA and thread model probes (PoaProbe.idl).
//
//   poa_probe_server --host HOST --port N --ior-file FILE [ORB options]
//
// The same objects and POAs as impls/omniorbpy/poa_probe_server.py; see
// run-poa-probe.sh.

#include <omniORB4/CORBA.h>
#include <omniORB4/omniORB.h>

#include <chrono>
#include <cstdio>
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
const char* kTargetId = "IDL:rustorb.org/PoaProbe/Target:1.0";

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

std::string oid_string(const PortableServer::ObjectId& oid) {
  return std::string(reinterpret_cast<const char*>(oid.get_buffer()), oid.length());
}

PortableServer::ObjectId_var to_oid(const char* s) {
  return PortableServer::string_to_ObjectId(s);
}

std::string poa_path(PortableServer::POA_ptr poa) {
  std::vector<std::string> names;
  PortableServer::POA_var p = PortableServer::POA::_duplicate(poa);
  while (!CORBA::is_nil(p)) {
    CORBA::String_var name = p->the_name();
    PortableServer::POA_var parent = p->the_parent();
    if (CORBA::is_nil(parent)) break;
    names.insert(names.begin(), name.in());
    p = parent;
  }
  if (names.empty()) return "RootPOA";
  std::string path;
  for (auto& n : names) path += (path.empty() ? "" : "/") + n;
  return path;
}

class TargetImpl : public POA_PoaProbe::Target {
 public:
  explicit TargetImpl(PortableServer::Current_ptr current)
      : current_(PortableServer::Current::_duplicate(current)) {}

  PoaProbe::CallInfo* work(CORBA::Long ms) override {
    PoaProbe::CallInfo_var info = new PoaProbe::CallInfo;
    info->start = now();
    info->thread = thread_label().c_str();
    PortableServer::ObjectId_var oid = current_->get_object_id();
    PortableServer::POA_var poa = current_->get_POA();
    info->object_id = oid_string(oid.in()).c_str();
    info->poa = poa_path(poa).c_str();
    if (ms > 0) std::this_thread::sleep_for(std::chrono::milliseconds(ms));
    info->end = now();
    return info._retn();
  }

  void post(CORBA::Long seq, CORBA::Long ms) override {
    if (ms > 0) std::this_thread::sleep_for(std::chrono::milliseconds(ms));
    event("post " + std::to_string(seq));
  }

  PoaProbe::CallInfo* relay(PoaProbe::Target_ptr other, CORBA::Long ms,
                            CORBA::String_out outer_thread) override {
    outer_thread = CORBA::string_dup(thread_label().c_str());
    omniORB::setClientCallTimeout(other, 3000);
    return other->work(ms);
  }

 private:
  PortableServer::Current_var current_;
};

class Activator : public POA_PortableServer::ServantActivator {
 public:
  Activator(PortableServer::Servant servant, CORBA::Object_ptr forward)
      : servant_(servant), forward_(CORBA::Object::_duplicate(forward)) {}

  PortableServer::Servant incarnate(const PortableServer::ObjectId& oid,
                                    PortableServer::POA_ptr) override {
    std::string name = oid_string(oid);
    event("incarnate " + name);
    if (name == "bad") throw CORBA::OBJECT_NOT_EXIST(0, CORBA::COMPLETED_NO);
    if (name == "fwd") throw PortableServer::ForwardRequest(forward_);
    servant_->_add_ref();
    return servant_;
  }

  void etherealize(const PortableServer::ObjectId& oid, PortableServer::POA_ptr,
                   PortableServer::Servant, CORBA::Boolean, CORBA::Boolean) override {
    event("etherealize " + oid_string(oid));
  }

 private:
  PortableServer::Servant servant_;
  CORBA::Object_var forward_;
};

class Locator : public POA_PortableServer::ServantLocator {
 public:
  Locator(PortableServer::Servant servant, CORBA::Object_ptr forward)
      : servant_(servant), forward_(CORBA::Object::_duplicate(forward)) {}

  PortableServer::Servant preinvoke(const PortableServer::ObjectId& oid,
                                    PortableServer::POA_ptr, const char* operation,
                                    PortableServer::ServantLocator::Cookie&) override {
    std::string name = oid_string(oid);
    event("preinvoke " + name + " " + operation);
    if (name == "bad") throw CORBA::OBJECT_NOT_EXIST(0, CORBA::COMPLETED_NO);
    if (name == "fwd") throw PortableServer::ForwardRequest(forward_);
    return servant_;
  }

  void postinvoke(const PortableServer::ObjectId& oid, PortableServer::POA_ptr,
                  const char* operation, PortableServer::ServantLocator::Cookie,
                  PortableServer::Servant) override {
    event("postinvoke " + oid_string(oid) + " " + operation);
  }

 private:
  PortableServer::Servant servant_;
  CORBA::Object_var forward_;
};

// Creates POAs named lazy* on demand, with a default servant.
class AdapterActivatorImpl : public POA_PortableServer::AdapterActivator {
 public:
  explicit AdapterActivatorImpl(PortableServer::Servant servant) : servant_(servant) {}

  CORBA::Boolean unknown_adapter(PortableServer::POA_ptr parent, const char* name) override {
    event(std::string("unknown_adapter ") + name);
    if (std::string(name).rfind("lazy", 0) != 0) return false;
    CORBA::PolicyList pl;
    pl.length(5);
    pl[0] = parent->create_lifespan_policy(PortableServer::PERSISTENT);
    pl[1] = parent->create_id_assignment_policy(PortableServer::USER_ID);
    pl[2] = parent->create_servant_retention_policy(PortableServer::NON_RETAIN);
    pl[3] = parent->create_request_processing_policy(PortableServer::USE_DEFAULT_SERVANT);
    pl[4] = parent->create_id_uniqueness_policy(PortableServer::MULTIPLE_ID);
    PortableServer::POAManager_var manager = parent->the_POAManager();
    PortableServer::POA_var poa = parent->create_POA(name, manager, pl);
    poa->set_servant(servant_);
    return true;
  }

 private:
  PortableServer::Servant servant_;
};

class ControlImpl : public POA_PoaProbe::Control {
 public:
  ControlImpl(CORBA::ORB_ptr orb, PortableServer::POA_ptr root,
              PortableServer::POAManager_ptr managed, PortableServer::Current_ptr current,
              std::map<std::string, CORBA::Object_var>* system_refs)
      : orb_(CORBA::ORB::_duplicate(orb)),
        root_(PortableServer::POA::_duplicate(root)),
        managed_(PortableServer::POAManager::_duplicate(managed)),
        current_(PortableServer::Current::_duplicate(current)),
        system_refs_(system_refs) {}

  char* reference(const char* poa, const char* oid) override {
    auto it = system_refs_->find(poa);
    if (it != system_refs_->end()) return orb_->object_to_string(it->second);
    PortableServer::POA_var p = find(poa);
    CORBA::Object_var ref = p->create_reference_with_id(to_oid(oid), kTargetId);
    return orb_->object_to_string(ref);
  }

  void set_state(const char* state) override {
    std::string s = state;
    try {
      if (s == "active") managed_->activate();
      else if (s == "holding") managed_->hold_requests(false);
      else if (s == "discarding") managed_->discard_requests(false);
      else if (s == "inactive") managed_->deactivate(false, false);
      else throw PoaProbe::Failed(("unknown state " + s).c_str());
    } catch (PortableServer::POAManager::AdapterInactive&) {
      throw PoaProbe::Failed("AdapterInactive");
    }
  }

  void deactivate_object(const char* poa, const char* oid) override {
    try {
      find(poa)->deactivate_object(to_oid(oid));
    } catch (PortableServer::POA::ObjectNotActive&) {
      throw PoaProbe::Failed("ObjectNotActive");
    }
  }

  void activate_object(const char* poa, const char* oid) override {
    TargetImpl* servant = new TargetImpl(current_);
    try {
      find(poa)->activate_object_with_id(to_oid(oid), servant);
    } catch (PortableServer::POA::ObjectAlreadyActive&) {
      throw PoaProbe::Failed("ObjectAlreadyActive");
    }
    servant->_remove_ref();
  }

  void destroy_poa(const char* poa) override { find(poa)->destroy(true, false); }

  PoaProbe::StringSeq* events() override {
    std::lock_guard<std::mutex> lock(g_mutex);
    PoaProbe::StringSeq_var seq = new PoaProbe::StringSeq;
    seq->length(static_cast<CORBA::ULong>(g_events.size()));
    for (CORBA::ULong i = 0; i < seq->length(); ++i) seq[i] = g_events[i].c_str();
    g_events.clear();
    return seq._retn();
  }

  char* shutdown_inside(CORBA::Boolean wait_for_completion) override {
    try {
      orb_->shutdown(wait_for_completion);
    } catch (CORBA::SystemException& e) {
      char buf[160];
      std::snprintf(buf, sizeof buf, "%s minor 0x%08x", e._rep_id(),
                    static_cast<unsigned>(e.minor()));
      return CORBA::string_dup(buf);
    }
    return CORBA::string_dup("");
  }

  void shutdown_later(CORBA::Long delay_ms) override {
    CORBA::ORB_var orb = CORBA::ORB::_duplicate(orb_);
    std::thread([orb, delay_ms]() {
      std::this_thread::sleep_for(std::chrono::milliseconds(delay_ms));
      orb->shutdown(false);
    }).detach();
  }

 private:
  PortableServer::POA_ptr find(const char* path) {
    PortableServer::POA_var poa = PortableServer::POA::_duplicate(root_);
    std::string s = path;
    size_t start = 0;
    while (start < s.size()) {
      size_t end = s.find('/', start);
      if (end == std::string::npos) end = s.size();
      std::string name = s.substr(start, end - start);
      if (!name.empty() && name != "RootPOA") {
        try {
          poa = poa->find_POA(name.c_str(), true);
        } catch (PortableServer::POA::AdapterNonExistent&) {
          throw PoaProbe::Failed(("no POA " + s).c_str());
        }
      }
      start = end + 1;
    }
    return poa._retn();
  }

  CORBA::ORB_var orb_;
  PortableServer::POA_var root_;
  PortableServer::POAManager_var managed_;
  PortableServer::Current_var current_;
  std::map<std::string, CORBA::Object_var>* system_refs_;
};

CORBA::PolicyList policies(PortableServer::POA_ptr root,
                           PortableServer::LifespanPolicyValue lifespan,
                           PortableServer::IdAssignmentPolicyValue ids,
                           std::vector<CORBA::Policy_ptr> extra = {}) {
  CORBA::PolicyList pl;
  pl.length(static_cast<CORBA::ULong>(2 + extra.size()));
  pl[0] = root->create_lifespan_policy(lifespan);
  pl[1] = root->create_id_assignment_policy(ids);
  for (size_t i = 0; i < extra.size(); ++i) pl[static_cast<CORBA::ULong>(2 + i)] = extra[i];
  return pl;
}

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
  int cargc = static_cast<int>(cargv.size());
  char** pargv = cargv.data();

  try {
    using namespace PortableServer;
    CORBA::ORB_var orb = CORBA::ORB_init(cargc, pargv);
    CORBA::Object_var obj = orb->resolve_initial_references("RootPOA");
    POA_var root = POA::_narrow(obj);
    obj = orb->resolve_initial_references("POACurrent");
    Current_var current = Current::_narrow(obj);
    POAManager_var root_manager = root->the_POAManager();
    root_manager->activate();

    auto target = [&]() { return new TargetImpl(current); };
    auto create = [&](POA_ptr parent, const char* name, const CORBA::PolicyList& pl) {
      return parent->create_POA(name, root_manager, pl);
    };
    std::map<std::string, CORBA::Object_var> system_refs;

    TargetImpl* servant = target();
    // RootPOA: TRANSIENT, SYSTEM_ID, IMPLICIT_ACTIVATION.
    system_refs["RootPOA"] = servant->_this();

    POA_var persistent_user = create(root, "persistent_user", policies(root, PERSISTENT, USER_ID));
    persistent_user->activate_object_with_id(to_oid("obj"), servant);
    persistent_user->activate_object_with_id(to_oid("victim"), target());
    POA_var nested = create(persistent_user, "nested", policies(root, PERSISTENT, USER_ID));
    nested->activate_object_with_id(to_oid("obj"), servant);

    POA_var persistent_system =
        create(root, "persistent_system", policies(root, PERSISTENT, SYSTEM_ID));
    ObjectId_var oid = persistent_system->activate_object(target());
    system_refs["persistent_system"] = persistent_system->id_to_reference(oid);

    POA_var transient_user = create(root, "transient_user", policies(root, TRANSIENT, USER_ID));
    transient_user->activate_object_with_id(to_oid("obj"), servant);

    POA_var transient_system =
        create(root, "transient_system", policies(root, TRANSIENT, SYSTEM_ID));
    oid = transient_system->activate_object(target());
    system_refs["transient_system"] = transient_system->id_to_reference(oid);

    for (const char* name : {"single", "single2"}) {
      POA_var poa = create(root, name, policies(root, PERSISTENT, USER_ID,
                                               {root->create_thread_policy(SINGLE_THREAD_MODEL)}));
      poa->activate_object_with_id(to_oid("a"), servant);
      poa->activate_object_with_id(to_oid("b"), target());
    }

    POA_var main_poa = create(root, "main", policies(root, PERSISTENT, USER_ID,
                                                    {root->create_thread_policy(MAIN_THREAD_MODEL)}));
    main_poa->activate_object_with_id(to_oid("a"), servant);
    main_poa->activate_object_with_id(to_oid("b"), target());

    POA_var managed = root->create_POA("managed", POAManager::_nil(),
                                       policies(root, PERSISTENT, USER_ID));
    POAManager_var managed_manager = managed->the_POAManager();
    managed->activate_object_with_id(to_oid("obj"), servant);

    POA_var doomed = create(root, "doomed", policies(root, PERSISTENT, USER_ID));
    doomed->activate_object_with_id(to_oid("obj"), servant);

    CORBA::Object_var forward = persistent_user->id_to_reference(to_oid("obj"));
    POA_var activator = create(root, "activator", policies(root, PERSISTENT, USER_ID,
        {root->create_request_processing_policy(USE_SERVANT_MANAGER)}));
    Activator* act = new Activator(servant, forward);
    ServantActivator_var act_ref = act->_this();
    activator->set_servant_manager(act_ref);
    POA_var locator = create(root, "locator", policies(root, PERSISTENT, USER_ID,
        {root->create_request_processing_policy(USE_SERVANT_MANAGER),
         root->create_servant_retention_policy(NON_RETAIN)}));
    Locator* loc = new Locator(servant, forward);
    ServantLocator_var loc_ref = loc->_this();
    locator->set_servant_manager(loc_ref);

    POA_var dflt = create(root, "default", policies(root, PERSISTENT, USER_ID,
        {root->create_request_processing_policy(USE_DEFAULT_SERVANT),
         root->create_servant_retention_policy(NON_RETAIN),
         root->create_id_uniqueness_policy(MULTIPLE_ID)}));
    dflt->set_servant(servant);

    AdapterActivatorImpl* aa = new AdapterActivatorImpl(servant);
    AdapterActivator_var aa_ref = aa->_this();
    root->the_activator(aa_ref);

    // The Control object has its own POAManager, so that it keeps working
    // whatever the probe does to the others.
    POA_var control_poa = root->create_POA("control", POAManager::_nil(),
                                           policies(root, PERSISTENT, USER_ID));
    ControlImpl* control = new ControlImpl(orb, root, managed_manager, current, &system_refs);
    control_poa->activate_object_with_id(to_oid("Control"), control);
    POAManager_var control_manager = control_poa->the_POAManager();
    control_manager->activate();
    managed_manager->activate();

    CORBA::Object_var control_ref = control_poa->id_to_reference(to_oid("Control"));
    CORBA::String_var ior = orb->object_to_string(control_ref);
    {
      std::ofstream f(ior_file + ".tmp");
      f << ior.in() << "\n";
    }
    std::rename((ior_file + ".tmp").c_str(), ior_file.c_str());
    std::cout << "server ready" << std::endl;

    orb->run();
    std::cout << "server stopped" << std::endl;
    orb->destroy();
  } catch (CORBA::SystemException& e) {
    std::cerr << "CORBA system exception " << e._name() << " minor " << e.minor() << std::endl;
    return 1;
  } catch (CORBA::Exception& e) {
    std::cerr << "CORBA exception " << e._name() << std::endl;
    return 1;
  }
  return 0;
}

// Helpers shared by the omniORB C++ server and client.

#ifndef INTEROP_COMMON_H
#define INTEROP_COMMON_H

#include <omniORB4/CORBA.h>
#include <omniORB4/Naming.hh>

#include <chrono>
#include <cstdlib>
#include <cstring>
#include <iostream>
#include <string>
#include <thread>
#include <vector>

#include "InteropTest.hh"

struct Options {
  std::string naming;
  std::string name;
  std::string host;
  int port = 0;
  double timeout = 30.0;
  bool shutdown_server = false;
  // "basic" (InteropTest.idl) or "any" (AnyTest.idl).
  std::string suite = "basic";
};

// Parses the options shared with the omniORBpy implementation and builds the
// ORB_init argument vector. Unknown arguments are passed to ORB_init.
inline Options parse_args(int argc, char** argv, std::vector<std::string>& orb_args) {
  Options opt;
  orb_args.push_back(argv[0]);
  for (int i = 1; i < argc; ++i) {
    std::string a = argv[i];
    auto value = [&]() -> std::string {
      if (i + 1 >= argc) {
        std::cerr << "missing value for " << a << std::endl;
        std::exit(2);
      }
      return argv[++i];
    };
    if (a == "--naming") opt.naming = value();
    else if (a == "--name") opt.name = value();
    else if (a == "--host") opt.host = value();
    else if (a == "--port") opt.port = std::atoi(value().c_str());
    else if (a == "--timeout") opt.timeout = std::atof(value().c_str());
    else if (a == "--shutdown-server") opt.shutdown_server = true;
    else if (a == "--suite") opt.suite = value();
    else orb_args.push_back(a);
  }
  if (opt.naming.empty() || opt.name.empty()) {
    std::cerr << "usage: " << argv[0]
              << " --naming CORBALOC --name PATH [--host H] [--port N] [--timeout S]"
              << " [--shutdown-server] [--suite basic|any] [ORB options]" << std::endl;
    std::exit(2);
  }
  orb_args.push_back("-ORBInitRef");
  orb_args.push_back("NameService=" + opt.naming);
  if (opt.port) {
    orb_args.push_back("-ORBendPoint");
    orb_args.push_back("giop:tcp:" + opt.host + ":" + std::to_string(opt.port));
  }
  return opt;
}

inline CORBA::ORB_ptr orb_init(std::vector<std::string>& orb_args) {
  static std::vector<char*> argv;
  for (auto& a : orb_args) argv.push_back(const_cast<char*>(a.c_str()));
  int argc = static_cast<int>(argv.size());
  char** p = argv.data();
  return CORBA::ORB_init(argc, p);
}

inline CosNaming::Name to_name(const std::string& path) {
  CosNaming::Name name;
  size_t start = 0;
  while (start <= path.size()) {
    size_t end = path.find('/', start);
    if (end == std::string::npos) end = path.size();
    if (end > start) {
      CORBA::ULong n = name.length();
      name.length(n + 1);
      name[n].id = CORBA::string_dup(path.substr(start, end - start).c_str());
      name[n].kind = CORBA::string_dup("");
    }
    start = end + 1;
  }
  return name;
}

// Calls fn until it stops throwing "not there yet" exceptions or the timeout
// passes.
template <typename F>
auto retry(const char* what, F fn, double timeout) -> decltype(fn()) {
  auto deadline = std::chrono::steady_clock::now() +
                  std::chrono::milliseconds(static_cast<long>(timeout * 1000));
  for (;;) {
    try {
      return fn();
    } catch (CORBA::TRANSIENT&) {
    } catch (CORBA::COMM_FAILURE&) {
    } catch (CORBA::OBJECT_NOT_EXIST&) {
    } catch (CosNaming::NamingContext::NotFound&) {
    }
    if (std::chrono::steady_clock::now() >= deadline) {
      std::cerr << "timed out waiting for " << what << std::endl;
      std::exit(1);
    }
    std::this_thread::sleep_for(std::chrono::milliseconds(200));
  }
}

inline CosNaming::NamingContext_ptr root_context(CORBA::ORB_ptr orb, double timeout) {
  return retry("the naming service", [&]() {
    CORBA::Object_var obj = orb->resolve_initial_references("NameService");
    CosNaming::NamingContext_var ctx = CosNaming::NamingContext::_narrow(obj);
    if (CORBA::is_nil(ctx)) throw CORBA::TRANSIENT();
    return ctx._retn();
  }, timeout);
}

// Creates a PERSISTENT / USER_ID child POA, so that object keys are fixed and
// GIOP captures are reproducible between runs.
inline PortableServer::POA_ptr persistent_poa(CORBA::ORB_ptr orb, const char* name) {
  CORBA::Object_var obj = orb->resolve_initial_references("RootPOA");
  PortableServer::POA_var root = PortableServer::POA::_narrow(obj);
  PortableServer::POAManager_var mgr = root->the_POAManager();
  CORBA::PolicyList policies;
  policies.length(2);
  policies[0] = root->create_lifespan_policy(PortableServer::PERSISTENT);
  policies[1] = root->create_id_assignment_policy(PortableServer::USER_ID);
  PortableServer::POA_var poa = root->create_POA(name, mgr, policies);
  mgr->activate();
  return poa._retn();
}

#endif

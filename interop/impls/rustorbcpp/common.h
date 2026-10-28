// Helpers shared by the rustorb C++ server and client (and the C++ tests in
// cpp/tests): the same options and naming service handling as the omniORB
// C++ implementation, on the IDL to C++11 mapping.

#ifndef RUSTORBCPP_COMMON_H
#define RUSTORBCPP_COMMON_H

#include <chrono>
#include <cstdlib>
#include <iostream>
#include <string>
#include <thread>
#include <vector>

#include "CosNaming.hh"
#include "InteropTest.hh"

struct Options {
  std::string naming;
  std::string name;
  std::string host;
  int port = 0;
  double timeout = 30.0;
  bool shutdown_server = false;
};

// Parses the options shared with the other implementations and builds the
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
    else if (a == "--suite") {
      // Only the InteropTest suite: the C++ binding has no Any yet.
      if (value() != "basic") {
        std::cerr << "only --suite basic is supported" << std::endl;
        std::exit(2);
      }
    } else orb_args.push_back(a);
  }
  if (opt.naming.empty() || opt.name.empty()) {
    std::cerr << "usage: " << argv[0]
              << " --naming CORBALOC --name PATH [--host H] [--port N] [--timeout S]"
              << " [--shutdown-server] [--suite basic] [ORB options]" << std::endl;
    std::exit(2);
  }
  orb_args.push_back("-ORBInitRef");
  orb_args.push_back("NameService=" + opt.naming);
  if (opt.port) {
    orb_args.push_back("-ORBendPoint");
    orb_args.push_back("giop:tcp:" + opt.host + ":" + std::to_string(opt.port));
  }
  // The omniORB C++ client takes the GIOP limit from the environment
  // (ORBmaxGIOPVersion, set by compose.yaml); rustorb takes an option.
  if (const char* giop = std::getenv("ORBmaxGIOPVersion")) {
    if (*giop) {
      orb_args.push_back("-ORBmaxGIOPVersion");
      orb_args.push_back(giop);
    }
  }
  return opt;
}

inline IDL::traits<CORBA::ORB>::ref_type orb_init(std::vector<std::string>& orb_args) {
  static std::vector<char*> argv;
  for (auto& a : orb_args) argv.push_back(const_cast<char*>(a.c_str()));
  argv.push_back(nullptr);
  int argc = static_cast<int>(orb_args.size());
  return CORBA::ORB_init(argc, argv.data());
}

inline CosNaming::Name to_name(const std::string& path) {
  CosNaming::Name name;
  size_t start = 0;
  while (start <= path.size()) {
    size_t end = path.find('/', start);
    if (end == std::string::npos) end = path.size();
    if (end > start) name.emplace_back(path.substr(start, end - start), "");
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
    } catch (const CORBA::TRANSIENT&) {
    } catch (const CORBA::COMM_FAILURE&) {
    } catch (const CORBA::OBJECT_NOT_EXIST&) {
    } catch (const CosNaming::NamingContext::NotFound&) {
    }
    if (std::chrono::steady_clock::now() >= deadline) {
      std::cerr << "timed out waiting for " << what << std::endl;
      std::exit(1);
    }
    std::this_thread::sleep_for(std::chrono::milliseconds(200));
  }
}

inline IDL::traits<CosNaming::NamingContext>::ref_type root_context(
    IDL::traits<CORBA::ORB>::ref_type orb, double timeout) {
  return retry("the naming service", [&]() {
    auto obj = orb->resolve_initial_references("NameService");
    auto ctx = IDL::traits<CosNaming::NamingContext>::narrow(obj);
    if (!ctx) throw CORBA::TRANSIENT();
    return ctx;
  }, timeout);
}

// Creates a PERSISTENT / USER_ID child POA, so that object keys are fixed and
// GIOP captures are reproducible between runs.
inline IDL::traits<PortableServer::POA>::ref_type persistent_poa(
    IDL::traits<CORBA::ORB>::ref_type orb, const std::string& name) {
  auto root = IDL::traits<PortableServer::POA>::narrow(orb->resolve_initial_references("RootPOA"));
  auto mgr = root->the_POAManager();
  CORBA::PolicyList policies;
  policies.push_back(root->create_lifespan_policy(PortableServer::LifespanPolicyValue::PERSISTENT));
  policies.push_back(root->create_id_assignment_policy(PortableServer::IdAssignmentPolicyValue::USER_ID));
  auto poa = root->create_POA(name, mgr, policies);
  mgr->activate();
  return poa;
}

#endif

// rustorb C++: runs the CosNaming operation sequence of
// impls/omniorbpy/naming_check.py with the generated CosNaming stubs,
// printing the same lines, so that the output can be compared with
// expected/naming-check.txt (run-naming-check.sh).
//
//   naming_check --naming corbaloc::naming:2809/NameService [--timeout S]

#include <algorithm>
#include <chrono>
#include <cstdio>
#include <cstdlib>
#include <functional>
#include <iostream>
#include <string>
#include <thread>
#include <utility>
#include <vector>

#include "CosNaming.hh"

namespace {

using ObjectRef = IDL::traits<CORBA::Object>::ref_type;
using ContextRef = IDL::traits<CosNaming::NamingContext>::ref_type;
using NC = CosNaming::NamingContext;

std::string fmt_name(const CosNaming::Name& name) {
  if (name.empty()) return "<empty>";
  std::string s;
  for (const auto& c : name) {
    if (!s.empty()) s += "/";
    s += c.id();
    if (!c.kind().empty()) s += "." + c.kind();
  }
  return s;
}

const char* why_name(NC::NotFoundReason why) {
  switch (why) {
    case NC::NotFoundReason::missing_node: return "missing_node";
    case NC::NotFoundReason::not_context: return "not_context";
    case NC::NotFoundReason::not_object: return "not_object";
  }
  return "?";
}

std::string fmt_system(const CORBA::SystemException& e) {
  static const char* const completed[] = {"COMPLETED_YES", "COMPLETED_NO", "COMPLETED_MAYBE"};
  return std::string(e._name()) + "(completed=" +
         completed[static_cast<uint32_t>(e.completed()) % 3] + ")";
}

std::string fmt_user(const CORBA::UserException& e) {
  if (auto* nf = dynamic_cast<const NC::NotFound*>(&e))
    return std::string("NotFound(") + why_name(nf->why()) + ", rest=" +
           fmt_name(nf->rest_of_name()) + ")";
  if (auto* cp = dynamic_cast<const NC::CannotProceed*>(&e))
    return "CannotProceed(rest=" + fmt_name(cp->rest_of_name()) + ")";
  return e._name();
}

// Python's repr of a string, for the plain strings used here.
std::string py_repr(const std::string& s) {
  std::string r = "'";
  for (char c : s) {
    if (c == '\\' || c == '\'') r += '\\';
    r += c;
  }
  return r + "'";
}

std::string fmt_components(const CosNaming::Name& name) {
  std::string s = "[";
  for (size_t i = 0; i < name.size(); ++i) {
    if (i) s += ", ";
    s += "(" + py_repr(name[i].id()) + ", " + py_repr(name[i].kind()) + ")";
  }
  return s + "]";
}

std::string fmt_bindings(const CosNaming::BindingList& bl) {
  std::vector<std::pair<std::string, std::string>> items;
  for (const auto& b : bl)
    items.emplace_back(fmt_name(b.binding_name()),
                       b.binding_type() == CosNaming::BindingType::nobject ? "nobject"
                                                                            : "ncontext");
  std::sort(items.begin(), items.end());
  std::string s = "[";
  for (size_t i = 0; i < items.size(); ++i) {
    if (i) s += ", ";
    s += items[i].first + ":" + items[i].second;
  }
  return s + "]";
}

// `path` as a name: components split at '/', id and kind at the first '.'.
CosNaming::Name n(const std::string& path) {
  CosNaming::Name name;
  size_t start = 0;
  while (start <= path.size()) {
    size_t end = path.find('/', start);
    if (end == std::string::npos) end = path.size();
    if (end > start) {
      std::string c = path.substr(start, end - start);
      size_t dot = c.find('.');
      if (dot == std::string::npos) name.emplace_back(c, "");
      else name.emplace_back(c.substr(0, dot), c.substr(dot + 1));
    }
    start = end + 1;
  }
  return name;
}

// Runs f, which returns what to print, and prints the line; false if it
// raised.
bool step(const std::string& label, const std::function<std::string()>& f) {
  std::string out;
  bool ok = true;
  try {
    out = f();
  } catch (const CORBA::UserException& e) {
    out = "raises " + fmt_user(e);
    ok = false;
  } catch (const CORBA::SystemException& e) {
    out = "raises " + fmt_system(e);
    ok = false;
  }
  std::printf("%-40s -> %s\n", label.c_str(), out.c_str());
  return ok;
}

ContextRef root_context(IDL::traits<CORBA::ORB>::ref_type orb, double timeout) {
  auto deadline = std::chrono::steady_clock::now() +
                  std::chrono::milliseconds(static_cast<long>(timeout * 1000));
  for (;;) {
    try {
      auto ctx = IDL::traits<NC>::narrow(orb->resolve_initial_references("NameService"));
      if (ctx) return ctx;
    } catch (const CORBA::TRANSIENT&) {
    } catch (const CORBA::COMM_FAILURE&) {
    } catch (const CORBA::OBJECT_NOT_EXIST&) {
    }
    if (std::chrono::steady_clock::now() >= deadline) {
      std::cerr << "timed out waiting for the naming service" << std::endl;
      std::exit(1);
    }
    std::this_thread::sleep_for(std::chrono::milliseconds(200));
  }
}

int run(int argc, char** argv) {
  std::string naming;
  double timeout = 30.0;
  std::vector<std::string> orb_args{argv[0]};
  for (int i = 1; i < argc; ++i) {
    std::string a = argv[i];
    if (a == "--naming" && i + 1 < argc) naming = argv[++i];
    else if (a == "--timeout" && i + 1 < argc) timeout = std::atof(argv[++i]);
    else orb_args.push_back(a);
  }
  if (naming.empty()) {
    std::cerr << "usage: " << argv[0] << " --naming CORBALOC [--timeout S]" << std::endl;
    return 2;
  }
  orb_args.push_back("-ORBInitRef");
  orb_args.push_back("NameService=" + naming);
  std::vector<char*> cargv;
  for (auto& a : orb_args) cargv.push_back(const_cast<char*>(a.c_str()));
  cargv.push_back(nullptr);
  int cargc = static_cast<int>(orb_args.size());
  auto orb = CORBA::ORB_init(cargc, cargv.data());

  ContextRef root = root_context(orb, timeout);
  auto ext = IDL::traits<CosNaming::NamingContextExt>::narrow(root);
  auto ok = [](auto&&...) { return std::string("ok"); };

  ContextRef t;
  if (!step("bind_new_context test", [&] { t = root->bind_new_context(n("test")); return ok(); }))
    return 1;
  // Any object reference will do. The test context is used rather than the
  // root (as the omniORBpy check does) because its IOR comes from the naming
  // service, while the root's was made from the corbaloc URL, and
  // _is_equivalent compares IORs (as the Rust check).
  ObjectRef obj = t;
  auto equivalent = [&](ObjectRef o) {
    return std::string(o && o->_is_equivalent(obj) ? "equivalent" : "different object");
  };
  auto is_context = [&](ObjectRef o) {
    return std::string(IDL::traits<NC>::narrow(o) ? "NamingContext" : "not a context");
  };

  step("bind test/a", [&] { root->bind(n("test/a"), obj); return ok(); });
  step("bind test/a (again)", [&] { root->bind(n("test/a"), obj); return ok(); });
  step("rebind test/a", [&] { root->rebind(n("test/a"), obj); return ok(); });
  step("resolve test/a", [&] { return equivalent(root->resolve(n("test/a"))); });
  step("bind test/b.kind", [&] { t->bind(n("b.kind"), obj); return ok(); });
  step("resolve test/b (no kind)", [&] { root->resolve(n("test/b")); return ok(); });
  step("resolve test/b.kind", [&] { return equivalent(root->resolve(n("test/b.kind"))); });
  step("resolve test/missing", [&] { root->resolve(n("test/missing")); return ok(); });
  step("resolve test/missing/deeper", [&] { root->resolve(n("test/missing/deeper")); return ok(); });
  step("resolve test/a/x (not a context)", [&] { root->resolve(n("test/a/x")); return ok(); });
  step("resolve <empty>", [&] { root->resolve({}); return ok(); });
  step("bind <empty>", [&] { root->bind({}, obj); return ok(); });

  ContextRef sub, loose;
  step("bind_new_context test/sub", [&] { sub = root->bind_new_context(n("test/sub")); return ok(); });
  step("bind_new_context test/sub (again)", [&] { root->bind_new_context(n("test/sub")); return ok(); });
  step("bind test/sub/c", [&] { root->bind(n("test/sub/c"), obj); return ok(); });
  step("resolve test/sub (is a context)", [&] { return is_context(root->resolve(n("test/sub"))); });
  step("new_context", [&] { loose = root->new_context(); return ok(); });
  step("bind_context test/loose", [&] { root->bind_context(n("test/loose"), loose); return ok(); });
  step("rebind_context test/loose", [&] { root->rebind_context(n("test/loose"), loose); return ok(); });

  step("list test (how_many=10)", [&] {
    CosNaming::BindingList bl;
    IDL::traits<CosNaming::BindingIterator>::ref_type it;
    t->list(10, bl, it);
    return fmt_bindings(bl) + (it ? " + iterator" : "");
  });
  step("list test (how_many=1) + iterator", [&] {
    CosNaming::BindingList bl;
    IDL::traits<CosNaming::BindingIterator>::ref_type it;
    t->list(1, bl, it);
    CosNaming::BindingList names = bl;
    std::string steps = "first=" + std::to_string(bl.size());
    while (it) {
      CosNaming::BindingList chunk;
      bool more = it->next_n(2, chunk);
      steps += std::string(" next_n(2)=") + (more ? "True" : "False") + "/" +
               std::to_string(chunk.size());
      names.insert(names.end(), chunk.begin(), chunk.end());
      if (!more) {
        it->destroy();
        break;
      }
    }
    return fmt_bindings(names) + " " + steps;
  });

  step("destroy test/sub (not empty)", [&] { sub->destroy(); return ok(); });
  step("unbind test/sub/c", [&] { root->unbind(n("test/sub/c")); return ok(); });
  step("destroy test/sub", [&] { sub->destroy(); return ok(); });
  step("resolve test/sub (dangling)", [&] { root->resolve(n("test/sub")); return std::string("still bound"); });
  step("unbind test/sub", [&] { root->unbind(n("test/sub")); return ok(); });
  step("unbind test/a", [&] { root->unbind(n("test/a")); return ok(); });
  step("unbind test/a (again)", [&] { root->unbind(n("test/a")); return ok(); });
  step("resolve test/a", [&] { root->resolve(n("test/a")); return ok(); });

  if (ext) {
    step("to_string [a.b, c/d, .e]", [&] {
      CosNaming::Name name{CosNaming::NameComponent("a", "b"), CosNaming::NameComponent("c/d", ""),
                           CosNaming::NameComponent("", "e")};
      return ext->to_string(name);
    });
    step("to_name a.b/c\\/d/.e", [&] { return fmt_components(ext->to_name("a.b/c\\/d/.e")); });
    step("to_url host:2809 a/b", [&] { return ext->to_url("host:2809", "a/b"); });
    step("to_url :host:2809 a/b c", [&] { return ext->to_url(":host:2809", "a/b c"); });
    step("resolve_str test/b.kind", [&] { return equivalent(ext->resolve_str("test/b.kind")); });
    step("to_name <invalid>", [&] { return fmt_components(ext->to_name("a//b")); });
  } else {
    std::printf("root context is not a NamingContextExt\n");
  }

  // corbaname: URLs, resolved by the ORB through the naming service.
  std::string base = naming;
  if (base.rfind("corbaloc:", 0) == 0) base = "corbaname:" + base.substr(9);
  auto url = [&](const std::string& s) { return orb->string_to_object(s); };
  step("corbaname #test/b.kind", [&] { return equivalent(url(base + "#test/b.kind")); });
  step("corbaname #test%2Fb%2ekind", [&] { return equivalent(url(base + "#test%2Fb%2ekind")); });
  step("corbaname rir: #test/b.kind", [&] { return equivalent(url("corbaname:rir:#test/b.kind")); });
  step("corbaname (no name)", [&] { return is_context(url(base)); });
  step("corbaname #test/loose", [&] { return is_context(url(base + "#test/loose")); });
  step("corbaname #test/missing", [&] { url(base + "#test/missing"); return ok(); });
  step("corbaname #test//b (invalid)", [&] { url(base + "#test//b"); return ok(); });

  step("list test (final)", [&] {
    CosNaming::BindingList bl;
    IDL::traits<CosNaming::BindingIterator>::ref_type it;
    t->list(100, bl, it);
    return fmt_bindings(bl);
  });
  std::fflush(stdout);
  orb->destroy();
  return 0;
}

}  // namespace

int main(int argc, char** argv) {
  try {
    return run(argc, argv);
  } catch (const CORBA::Exception& e) {
    std::cerr << "CORBA exception " << e << std::endl;
    return 1;
  }
}

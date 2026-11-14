// The DII and the DSI of the C++ binding (#155), in one process: the
// interop cases of the basic, any and value suites through the DII
// proxies and the DSI servants of interop/impls/rustorbcpp/dii_adapters.h
// (both ways, and each against the static side), and the API itself:
// deferred and multiple requests, _create_request, the exception list, the
// order of the ServerRequest calls, SyncScope for oneway requests, and what
// the Portable Interceptors see of DII and DSI requests.

#define ANYTEST_HIDDEN
#include <atomic>
#include <iostream>
#include <mutex>
#include <set>
#include <sstream>
#include <string>
#include <thread>

#include "anytest.h"
#include "cases.h"
#include "dii_adapters.h"
#include "echo_impl.h"
#include "rustorb/pi.h"
#include "valuetest.h"

namespace PI = PortableInterceptor;
using ObjectRef = IDL::traits<CORBA::Object>::ref_type;
using EchoRef = IDL::traits<InteropTest::Echo>::ref_type;

// A case whose failure is known (docs/dii-dsi.md): C++ anys copy their
// valuetypes, so one instance in two anys arrives as two.
static const std::set<std::string> known_failures = {"same[shared]"};

// Runs the cases of a suite, counting the known failures apart.
static int run_suite(const std::string& label, const std::function<void(Runner&)>& cases) {
  Runner r;
  std::cout << "--- " << label << std::endl;
  int failed = 0;
  try {
    cases(r);
  } catch (const CORBA::Exception& e) {
    std::cout << "FAIL " << label << ": " << e << std::endl;
    ++failed;
  }
  for (const auto& name : r.failed()) {
    if (!known_failures.count(name)) ++failed;
  }
  std::cout << label << ": " << r.passed() << " passed, " << r.failed().size() << " failed ("
            << failed << " unexpected)" << std::endl;
  return failed;
}

template <typename E, typename F> static bool raises(const F& f) {
  try {
    f();
  } catch (const E&) {
    return true;
  } catch (const CORBA::Exception& e) {
    std::cout << "  raised " << e << std::endl;
    return false;
  }
  return false;
}

template <typename E, typename F> static uint32_t minor_of(const F& f) {
  try {
    f();
  } catch (const E& e) {
    return e.minor();
  } catch (const CORBA::Exception& e) {
    std::cout << "  raised " << e << std::endl;
  }
  return 0xffffffff;
}

template <typename T> static T value_of(const CORBA::Any& a) {
  T v{};
  if (!(a >>= v)) throw CheckFailed{"an any of another type"};
  return v;
}

// --- a DSI servant for the order of the ServerRequest calls -----------------------

class Probe : public PortableServer::DynamicImplementation {
 public:
  std::string _primary_interface(const PortableServer::ObjectId&,
                                 CORBA::object_reference<PortableServer::POA>) override {
    return "IDL:rustorb.org/test/Probe:1.0";
  }

  void invoke(CORBA::object_reference<CORBA::ServerRequest> r) override {
    std::string op = r->operation();
    auto params = CORBA::make_reference<CORBA::NVList>();
    if (op == "twice") {
      // arguments() twice is BAD_INV_ORDER; returns whether it was.
      params->add_value("a", dii::any_of(int32_t()), CORBA::ARG_IN);
      r->arguments(params);
      bool bad = raises<CORBA::BAD_INV_ORDER>([&] { r->arguments(params); });
      r->set_result(dii::any_of(bad));
    } else if (op == "result_first") {
      // set_result before arguments() is BAD_INV_ORDER.
      bool bad = raises<CORBA::BAD_INV_ORDER>([&] { r->set_result(dii::any_of(true)); });
      r->arguments(params);
      r->set_result(dii::any_of(bad));
    } else if (op == "not_an_exception") {
      r->arguments(params);
      bool bad = raises<CORBA::BAD_PARAM>([&] { r->set_exception(dii::any_of(int32_t(1))); });
      r->set_result(dii::any_of(bad));
    } else if (op == "unset_out") {
      // An out parameter left without a value: the reply is BAD_PARAM.
      params->add_value("x", rustorb::detail::typed_any(CORBA::_tc_long), CORBA::ARG_OUT);
      r->arguments(params);
    } else if (op == "wrong_out") {
      // An out value of another type: BAD_PARAM.
      params->add_value("x", dii::any_of(int32_t()), CORBA::ARG_OUT);
      r->arguments(params);
      params->item(0)->value() <<= std::string("no");
    } else if (op == "unknown_user") {
      // A user exception raised as UnknownUserException.
      r->arguments(params);
      throw CORBA::UnknownUserException(dii::any_of(InteropTest::Oops(3, "dsi")));
    } else if (op == "bad_args") {
      // Parameters that the arguments do not fit: MARSHAL.
      params->add_value("s", dii::any_of(std::string()), CORBA::ARG_IN);
      r->arguments(params);
    } else if (op == "add") {
      // inout and out values, set through the NVList.
      params->add_value("a", dii::any_of(int32_t()), CORBA::ARG_INOUT);
      params->add_value("b", dii::any_of(int32_t()), CORBA::ARG_IN);
      params->add_value("sum", dii::any_of(int32_t()), CORBA::ARG_OUT);
      r->arguments(params);
      int32_t a = value_of<int32_t>(params->item(0)->value());
      int32_t b = value_of<int32_t>(params->item(1)->value());
      params->item(0)->value() <<= int32_t(a * 10);
      params->item(2)->value() <<= int32_t(a + b);
      r->set_result(dii::any_of(std::string(r->operation())));
    } else if (op == "ctx") {
      r->arguments(params);
      r->set_result(dii::any_of(raises<CORBA::NO_IMPLEMENT>([&] { r->ctx(); })));
    } else {
      throw CORBA::BAD_OPERATION(0x4f4d0002, CORBA::CompletionStatus::COMPLETED_NO);
    }
  }
};

// --- what the interceptors see -------------------------------------------------------

struct Seen {
  std::mutex m;
  // Client: divide through the DII.
  size_t client_args = 0;
  std::vector<CORBA::ParameterMode> client_modes;
  int32_t client_remainder = -1, client_result = -1;
  size_t client_exceptions = 0;
  std::string client_received_id;
  // Server: divide through the DSI.
  size_t server_args = 0;
  int32_t server_a = -1, server_remainder = -1, server_result = -1;
  std::string server_sending_id;
  bool static_unavailable = false;
};
static Seen seen;

class ClientPi : public PI::ClientRequestInterceptor {
 public:
  std::string name() override { return "dii"; }
  void send_request(CORBA::object_reference<PI::ClientRequestInfo> ri) override {
    if (ri->operation() != "divide") return;
    std::lock_guard<std::mutex> lock(seen.m);
    try {
      auto args = ri->arguments();
      seen.client_args = args.size();
      seen.client_modes.clear();
      for (const auto& p : args) seen.client_modes.push_back(p.mode());
      seen.client_exceptions = ri->exceptions().size();
    } catch (const CORBA::NO_RESOURCES&) {
      seen.client_args = 0;
    }
  }
  void receive_reply(CORBA::object_reference<PI::ClientRequestInfo> ri) override {
    if (ri->operation() != "divide") return;
    std::lock_guard<std::mutex> lock(seen.m);
    try {
      ri->arguments()[2].argument() >>= seen.client_remainder;
      ri->result() >>= seen.client_result;
    } catch (const CORBA::NO_RESOURCES&) {
    }
  }
  void receive_exception(CORBA::object_reference<PI::ClientRequestInfo> ri) override {
    if (ri->operation() != "divide") return;
    std::lock_guard<std::mutex> lock(seen.m);
    seen.client_received_id = ri->received_exception_id();
  }
};

class ServerPi : public PI::ServerRequestInterceptor {
 public:
  std::string name() override { return "dsi"; }
  void send_reply(CORBA::object_reference<PI::ServerRequestInfo> ri) override {
    std::string op = ri->operation();
    std::lock_guard<std::mutex> lock(seen.m);
    if (op == "echo_long") {
      seen.static_unavailable = raises<CORBA::NO_RESOURCES>([&] { ri->arguments(); });
    }
    if (op != "divide") return;
    try {
      auto args = ri->arguments();
      seen.server_args = args.size();
      args[0].argument() >>= seen.server_a;
      args[2].argument() >>= seen.server_remainder;
      ri->result() >>= seen.server_result;
    } catch (const CORBA::NO_RESOURCES&) {
      seen.server_args = 0;
    }
  }
  void send_exception(CORBA::object_reference<PI::ServerRequestInfo> ri) override {
    if (ri->operation() != "divide") return;
    std::lock_guard<std::mutex> lock(seen.m);
    try {
      seen.server_sending_id = ri->sending_exception_id();
      // The Any of the user exception, with the TypeCode set_exception had.
      InteropTest::DivideByZero e;
      if (!(ri->sending_exception() >>= e)) seen.server_sending_id += " (no any)";
    } catch (const CORBA::Exception& e) {
      std::ostringstream os;
      os << e;
      seen.server_sending_id = os.str();
    }
  }
};

class Init : public PI::ORBInitializer {
 public:
  void pre_init(CORBA::object_reference<PI::ORBInitInfo> info) override {
    info->add_client_request_interceptor(CORBA::make_reference<ClientPi>());
    info->add_server_request_interceptor(CORBA::make_reference<ServerPi>());
  }
};

// --- the API cases ------------------------------------------------------------------

static void run_api_cases(Runner& r, const IDL::traits<CORBA::ORB>::ref_type& orb,
                          const EchoRef& echo, const ObjectRef& probe, const ObjectRef& missing) {
  using namespace InteropTest;

  r.run("request accessors", [&] {
    auto req = echo->_request("echo_long");
    CHECK(req->operation() == "echo_long");
    CHECK(req->target()->_is_equivalent(echo));
    CHECK(req->arguments()->count() == 0);
    CHECK(req->return_value().type()->kind() == CORBA::TCKind::tk_void);
    req->add_in_arg("v") <<= int32_t(9);
    CHECK(req->arguments()->count() == 1);
    CHECK(req->arguments()->item(0)->name() == "v");
    CHECK(req->arguments()->item(0)->flags() == CORBA::ARG_IN);
    req->set_return_type(CORBA::_tc_long);
    CHECK(req->return_value().type()->equal(CORBA::_tc_long));
    req->invoke();
    CHECK(value_of<int32_t>(req->return_value()) == 9);
    // Again, with another argument.
    req->arguments()->item(0)->value() <<= int32_t(-4);
    req->invoke();
    CHECK(value_of<int32_t>(req->return_value()) == -4);
    CHECK(raises<CORBA::NO_IMPLEMENT>([&] { req->ctx(); }));
    CHECK(raises<CORBA::NO_IMPLEMENT>([&] { req->contexts(); }));
  });

  r.run("create_request", [&] {
    auto args = orb->create_list(3);
    CHECK(args->count() == 0);
    args->add_value("a", dii::any_of(int32_t(17)), CORBA::ARG_IN);
    args->add_value("b", dii::any_of(int32_t(5)), CORBA::ARG_IN);
    args->add_item("remainder", CORBA::ARG_OUT)->value() <<= int32_t();
    auto result = orb->create_named_value();
    result->value() = rustorb::detail::typed_any(CORBA::_tc_long);
    auto excs = orb->create_exception_list();
    excs->add(_tc_DivideByZero);
    IDL::traits<CORBA::Request>::ref_type req;
    echo->_create_request(nullptr, "divide", args, result, excs, nullptr, req, 0);
    req->invoke();
    CHECK(value_of<int32_t>(result->value()) == 3);
    CHECK(value_of<int32_t>(args->item(2)->value()) == 2);
    CHECK(req->result() == result);
    // A user exception of the list: UnknownUserException.
    args->item(1)->value() <<= int32_t(0);
    bool raised = false;
    try {
      req->invoke();
    } catch (const CORBA::UnknownUserException& e) {
      DivideByZero d;
      raised = (e.exception() >>= d) &&
               e.exception().type()->id() == DivideByZero::_rorb_repository_id;
    }
    CHECK(raised);
    // Without the exception list: UNKNOWN, minor 1 (OMG).
    IDL::traits<CORBA::Request>::ref_type bare;
    echo->_create_request(nullptr, "divide", args, nullptr, bare, 0);
    CHECK(minor_of<CORBA::UNKNOWN>([&] { bare->invoke(); }) == 0x4f4d0001);
  });

  r.run("system exception", [&] {
    auto req = echo->_request("fail_system");
    try {
      req->invoke();
      CHECK(false);
    } catch (const CORBA::BAD_PARAM& e) {
      CHECK(e.minor() == 42 && e.completed() == CORBA::CompletionStatus::COMPLETED_NO);
    }
  });

  r.run("deferred", [&] {
    auto req = echo->_request("echo_string");
    req->add_in_arg() <<= std::string("later");
    req->set_return_type(CORBA::_tc_string);
    CHECK(raises<CORBA::BAD_INV_ORDER>([&] { req->poll_response(); }));
    CHECK(raises<CORBA::BAD_INV_ORDER>([&] { req->get_response(); }));
    req->send_deferred();
    CHECK(raises<CORBA::BAD_INV_ORDER>([&] { req->send_deferred(); }));
    while (!req->poll_response()) std::this_thread::sleep_for(std::chrono::milliseconds(1));
    req->get_response();
    CHECK(value_of<std::string>(req->return_value()) == "later");
    CHECK(raises<CORBA::BAD_INV_ORDER>([&] { req->get_response(); }));
    // A user exception comes back from get_response.
    auto div = echo->_request("divide");
    div->add_in_arg() <<= int32_t(1);
    div->add_in_arg() <<= int32_t(0);
    div->add_out_arg() <<= int32_t();
    div->set_return_type(CORBA::_tc_long);
    div->exceptions()->add(_tc_DivideByZero);
    div->send_deferred();
    CHECK(raises<CORBA::UnknownUserException>([&] { div->get_response(); }));
  });

  r.run("multiple requests", [&] {
    std::vector<IDL::traits<CORBA::Request>::ref_type> reqs;
    for (int32_t i = 0; i < 5; ++i) {
      auto req = echo->_request("echo_long");
      req->add_in_arg() <<= i;
      req->set_return_type(CORBA::_tc_long);
      reqs.push_back(req);
    }
    CHECK(raises<CORBA::BAD_INV_ORDER>([&] {
      IDL::traits<CORBA::Request>::ref_type none;
      orb->get_next_response(none);
    }));
    orb->send_multiple_requests_deferred(reqs);
    std::set<int32_t> got;
    for (size_t i = 0; i < reqs.size(); ++i) {
      IDL::traits<CORBA::Request>::ref_type next;
      orb->get_next_response(next);
      next->get_response();
      got.insert(value_of<int32_t>(next->return_value()));
    }
    CHECK(got == (std::set<int32_t>{0, 1, 2, 3, 4}));
    CHECK(!orb->poll_next_response());

    echo->posted();
    std::vector<IDL::traits<CORBA::Request>::ref_type> posts;
    for (const char* m : {"x", "y"}) {
      auto req = echo->_request("post");
      req->add_in_arg() <<= std::string(m);
      posts.push_back(req);
    }
    orb->send_multiple_requests_oneway(posts);
    std::vector<std::string> received;
    for (int i = 0; i < 100 && received.size() < 2; ++i) {
      for (auto& m : echo->posted()) received.push_back(m);
      std::this_thread::sleep_for(std::chrono::milliseconds(20));
    }
    std::sort(received.begin(), received.end());
    CHECK(received == (std::vector<std::string>{"x", "y"}));
  });

  r.run("lists", [&] {
    auto list = CORBA::make_reference<CORBA::NVList>();
    list->add(CORBA::ARG_INOUT);
    list->add_item("b", CORBA::ARG_OUT);
    CHECK(list->count() == 2);
    CHECK(list->item(0)->flags() == CORBA::ARG_INOUT);
    list->remove(0);
    CHECK(list->count() == 1 && list->item(0)->name() == "b");
    CHECK(raises<CORBA::Bounds>([&] { list->item(1); }));
    CHECK(raises<CORBA::Bounds>([&] { list->remove(5); }));
    auto excs = orb->create_exception_list();
    excs->add(_tc_Oops);
    CHECK(excs->count() == 1 && excs->item(0)->equal(_tc_Oops));
    CHECK(raises<CORBA::Bounds>([&] { excs->item(1); }));
    excs->remove(0);
    CHECK(excs->count() == 0);
    // Bad flags: BAD_PARAM when sent.
    auto req = echo->_request("echo_long");
    req->arguments()->add_value("", dii::any_of(int32_t(1)), 0);
    CHECK(raises<CORBA::BAD_PARAM>([&] { req->invoke(); }));
    CHECK(raises<CORBA::NO_IMPLEMENT>([&] { orb->get_default_context(); }));
  });

  r.run("local object", [&] {
    CHECK(raises<CORBA::NO_IMPLEMENT>([&] { orb->_request("x"); }));
  });

  r.run("server request order", [&] {
    auto call = [&](const char* op) {
      auto req = probe->_request(op);
      req->set_return_type(CORBA::_tc_boolean);
      req->invoke();
      return value_of<bool>(req->return_value());
    };
    {
      auto req = probe->_request("twice");
      req->add_in_arg() <<= int32_t(1);
      req->set_return_type(CORBA::_tc_boolean);
      req->invoke();
      CHECK(value_of<bool>(req->return_value()));
    }
    CHECK(call("result_first"));
    CHECK(call("not_an_exception"));
    CHECK(call("ctx"));
    auto out = [&](const char* op) {
      auto req = probe->_request(op);
      req->add_out_arg() <<= int32_t();
      req->invoke();
    };
    CHECK(raises<CORBA::BAD_PARAM>([&] { out("unset_out"); }));
    CHECK(raises<CORBA::BAD_PARAM>([&] { out("wrong_out"); }));
    {
      auto req = probe->_request("unknown_user");
      req->exceptions()->add(_tc_Oops);
      try {
        req->invoke();
        CHECK(false);
      } catch (const CORBA::UnknownUserException& e) {
        Oops o;
        CHECK((e.exception() >>= o) && o.code() == 3 && o.reason() == "dsi");
      }
    }
    {
      auto req = probe->_request("bad_args");
      req->add_in_arg() <<= int32_t(1);
      CHECK(raises<CORBA::MARSHAL>([&] { req->invoke(); }));
    }
    {
      auto req = probe->_request("add");
      req->add_inout_arg() <<= int32_t(4);
      req->add_in_arg() <<= int32_t(5);
      req->add_out_arg() <<= int32_t();
      req->set_return_type(CORBA::_tc_string);
      req->invoke();
      CHECK(value_of<int32_t>(req->arguments()->item(0)->value()) == 40);
      CHECK(value_of<int32_t>(req->arguments()->item(2)->value()) == 9);
      CHECK(value_of<std::string>(req->return_value()) == "add");
    }
    CHECK(minor_of<CORBA::BAD_OPERATION>([&] { probe->_request("nothing")->invoke(); }) ==
          0x4f4d0002);
    CHECK(probe->_is_a("IDL:rustorb.org/test/Probe:1.0"));
  });

  // Oneway requests follow the SyncScope policy of the target: with
  // SYNC_WITH_TARGET the server's OBJECT_NOT_EXIST comes back.
  r.run("oneway sync scope", [&] {
    using namespace Messaging;
    auto send = [&](const ObjectRef& target) { target->_request("ping")->send_oneway(); };
    send(missing);
    auto with_target = missing->_set_policy_overrides(
        {CORBA::make_reference<SyncScopePolicy>(SYNC_WITH_TARGET)},
        CORBA::SetOverrideType::SET_OVERRIDE);
    CHECK(raises<CORBA::OBJECT_NOT_EXIST>([&] { send(with_target); }));
  });

  r.run("interceptors", [&] {
    auto req = echo->_request("divide");
    req->add_in_arg("a") <<= int32_t(23);
    req->add_in_arg("b") <<= int32_t(10);
    req->add_out_arg("remainder") <<= int32_t();
    req->set_return_type(CORBA::_tc_long);
    req->exceptions()->add(_tc_DivideByZero);
    req->invoke();
    {
      std::lock_guard<std::mutex> lock(seen.m);
      CHECK(seen.client_args == 3);
      CHECK(seen.client_modes ==
            (std::vector<CORBA::ParameterMode>{CORBA::ParameterMode::PARAM_IN,
                                               CORBA::ParameterMode::PARAM_IN,
                                               CORBA::ParameterMode::PARAM_OUT}));
      CHECK(seen.client_exceptions == 1);
      CHECK(seen.client_remainder == 3 && seen.client_result == 2);
      CHECK(seen.server_args == 3);
      CHECK(seen.server_a == 23 && seen.server_remainder == 3 && seen.server_result == 2);
    }
    req->arguments()->item(1)->value() <<= int32_t(0);
    CHECK(raises<CORBA::UnknownUserException>([&] { req->invoke(); }));
    {
      std::lock_guard<std::mutex> lock(seen.m);
      CHECK(seen.client_received_id == DivideByZero::_rorb_repository_id);
      CHECK(seen.server_sending_id == DivideByZero::_rorb_repository_id);
    }
  });
}

int main() {
  try {
    PI::register_orb_initializer(CORBA::make_reference<Init>());
    int sargc = 3;
    char sarg0[] = "server", sarg1[] = "-ORBendPoint", sarg2[] = "giop:tcp:127.0.0.1:";
    char* sargv[] = {sarg0, sarg1, sarg2, nullptr};
    auto server_orb = CORBA::ORB_init(sargc, sargv);
    auto root = IDL::traits<PortableServer::POA>::narrow(
        server_orb->resolve_initial_references("RootPOA"));
    root->the_POAManager()->activate();
    auto ref = [&](PortableServer::Servant s) {
      return server_orb->object_to_string(root->id_to_reference(root->activate_object(s)));
    };
    std::string echo_ior = ref(CORBA::make_reference<EchoImpl>(server_orb));
    std::string dsi_echo_ior = ref(CORBA::make_reference<dii::Dsi<EchoImpl>>(server_orb));
    std::string any_ior = ref(CORBA::make_reference<anytest::AnyEchoImpl>());
    std::string dsi_any_ior = ref(CORBA::make_reference<dii::Dsi<anytest::AnyEchoImpl>>());
    std::string value_ior = ref(CORBA::make_reference<valuetest::ValueEchoImpl>(server_orb));
    std::string dsi_value_ior =
        ref(CORBA::make_reference<dii::Dsi<valuetest::ValueEchoImpl>>(server_orb));
    std::string probe_ior = ref(CORBA::make_reference<Probe>());
    std::string missing_ior =
        server_orb->object_to_string(root->create_reference("IDL:rustorb.org/test/Nothing:1.0"));
    std::thread server([&] { server_orb->run(); });

    int cargc = 1;
    char carg0[] = "client";
    char* cargv[] = {carg0, nullptr};
    auto orb = CORBA::ORB_init(cargc, cargv);
    auto poa = IDL::traits<PortableServer::POA>::narrow(orb->resolve_initial_references("RootPOA"));
    poa->the_POAManager()->activate();
    valuetest::register_factories(orb);
    auto narrow = [&](const std::string& ior) {
      return IDL::traits<InteropTest::Echo>::narrow(orb->string_to_object(ior));
    };
    auto narrow_any = [&](const std::string& ior) {
      return IDL::traits<AnyTest::AnyEcho>::narrow(orb->string_to_object(ior));
    };
    auto narrow_value = [&](const std::string& ior) {
      return IDL::traits<ValueTest::ValueEcho>::narrow(orb->string_to_object(ior));
    };
    auto check = [](bool cond, const std::string& what) {
      if (!cond) throw CheckFailed{what};
    };

    int failed = 0;
    // The basic suite: DII to DSI, DII to static, static to DSI.
    struct Pair {
      const char* label;
      std::string ior;
      bool dii;
    };
    for (const Pair& p : {Pair{"basic: DII -> DSI", dsi_echo_ior, true},
                          Pair{"basic: DII -> static", echo_ior, true},
                          Pair{"basic: static -> DSI", dsi_echo_ior, false}}) {
      failed += run_suite(p.label, [&](Runner& r) {
        EchoRef echo = narrow(p.ior);
        if (p.dii) echo = dii::wrap<dii::DiiEcho>(echo);
        run_cases(r, echo, poa);
        // run_cases activates its callback under a fixed id.
        poa->deactivate_object(PortableServer::string_to_ObjectId("Callback"));
      });
    }
    for (const Pair& p : {Pair{"any: DII -> DSI", dsi_any_ior, true},
                          Pair{"any: DII -> static", any_ior, true},
                          Pair{"any: static -> DSI", dsi_any_ior, false}}) {
      failed += run_suite(p.label, [&](Runner& r) {
        auto echo = narrow_any(p.ior);
        if (p.dii) echo = dii::wrap<dii::DiiAnyEcho>(echo);
        anytest::run_cases(r, echo, check);
      });
    }
    for (const Pair& p : {Pair{"value: DII -> DSI", dsi_value_ior, true},
                          Pair{"value: DII -> static", value_ior, true},
                          Pair{"value: static -> DSI", dsi_value_ior, false}}) {
      failed += run_suite(p.label, [&](Runner& r) {
        auto echo = narrow_value(p.ior);
        if (p.dii) echo = dii::wrap<dii::DiiValueEcho>(echo);
        valuetest::run_cases(r, echo, check);
      });
    }
    failed += run_suite("api", [&](Runner& r) {
      run_api_cases(r, orb, narrow(dsi_echo_ior), orb->string_to_object(probe_ior),
                    orb->string_to_object(missing_ior));
    });
    {
      // The static skeleton's arguments are NO_RESOURCES.
      narrow(echo_ior)->echo_long(1);
      std::lock_guard<std::mutex> lock(seen.m);
      if (!seen.static_unavailable) {
        std::cout << "FAIL static arguments available" << std::endl;
        ++failed;
      }
    }

    narrow(echo_ior)->shutdown();
    server.join();
    server_orb->destroy();
    orb->destroy();
    std::cout << (failed ? "FAILED" : "OK") << std::endl;
    return failed ? 1 : 0;
  } catch (const CORBA::Exception& e) {
    std::cerr << "CORBA exception: " << e << std::endl;
    return 1;
  }
}

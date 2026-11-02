// Portable Interceptors and IOP::Codec in the C++ binding (#100): an ORB
// initializer registered for the process, client, server and IOR
// interceptors on a server ORB and a client ORB in one process, service
// contexts both ways, slots from the client thread to the servant,
// ForwardRequest, exceptions raised by interceptors, info objects kept
// past their point, and the Codec.

#include <atomic>
#include <iostream>
#include <mutex>
#include <string>
#include <thread>

#include "echo_impl.h"
#include "rustorb/pi.h"

static int failures = 0;

#define CHECK(cond)                                                     \
  do {                                                                  \
    if (!(cond)) {                                                      \
      std::cout << "FAIL line " << __LINE__ << ": " #cond << std::endl; \
      ++failures;                                                       \
    }                                                                   \
  } while (0)

namespace PI = PortableInterceptor;
using ObjectRef = CORBA::object_reference<CORBA::Object>;

constexpr IOP::ServiceId REQUEST_CONTEXT = 0x52520010;
constexpr IOP::ServiceId REPLY_CONTEXT = 0x52520011;
constexpr IOP::ComponentId COMPONENT = 0x52520001;
constexpr const char* ECHO_ID = "IDL:rustorb.org/InteropTest/Echo:1.0";

static std::vector<uint8_t> octets(const std::string& s) {
  return std::vector<uint8_t>(s.begin(), s.end());
}
static std::string text(const std::vector<uint8_t>& v) { return std::string(v.begin(), v.end()); }

template <typename E, typename F>
static bool raises(const F& f) {
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

template <typename E, typename F>
static uint32_t minor_of(const F& f) {
  try {
    f();
  } catch (const E& e) {
    return e.minor();
  }
  return 0xffffffff;
}

// What the interceptors saw.
struct Seen {
  std::mutex m;
  PI::SlotId slot = 0;
  std::vector<std::string> pre_init_ids, post_init_ids, arguments;
  int send_request = 0, receive_reply = 0, receive_exception = 0, receive_other = 0;
  std::string reply_context, received_exception_id, sending_exception_id;
  bool component_ok = false, arguments_unavailable = false, response_expected = false;
  bool server_orb_id_ok = false, target_is_a = false, adapter_name_ok = false;
  bool kept_ri_invalid = false, duplicate_name = false, anonymous_twice = false;
  bool pre_init_resolve_bad_order = false, post_init_resolve_ok = false;
  bool post_init_invalid_name = false, pi_current_ok = false, codec_factory_ok = false;
  bool poa_current_ok = false;
  ObjectRef forward_from, forward_to, server_forward;
  std::vector<uint8_t> server_forward_oid;
  CORBA::object_reference<PI::ClientRequestInfo> kept_ri;
  CORBA::object_reference<PI::ORBInitInfo> kept_info;
};
static Seen seen;

class Marker : public CORBA::LocalObject {};

class Client : public PI::ClientRequestInterceptor {
 public:
  std::string name() override { return "ctx"; }
  void send_request(CORBA::object_reference<PI::ClientRequestInfo> ri) override {
    std::lock_guard<std::mutex> lock(seen.m);
    ++seen.send_request;
    std::string op = ri->operation();
    if (op == "echo_string") {
      std::string slot;
      CORBA::Any a = ri->get_slot(seen.slot);
      if (a >>= slot)
        ri->add_request_service_context(IOP::ServiceContext(REQUEST_CONTEXT, octets(slot)), false);
      seen.component_ok =
          ri->get_effective_component(COMPONENT).component_data() == std::vector<uint8_t>{1, 2, 3};
      seen.arguments_unavailable = raises<CORBA::NO_RESOURCES>([&] { ri->arguments(); });
      seen.response_expected = ri->response_expected();
      seen.kept_ri = ri;
    }
    if (op == "echo_short" && seen.forward_to &&
        ri->effective_target()->_is_equivalent(seen.forward_from)) {
      throw PI::ForwardRequest(seen.forward_to);
    }
  }
  void receive_reply(CORBA::object_reference<PI::ClientRequestInfo> ri) override {
    std::lock_guard<std::mutex> lock(seen.m);
    ++seen.receive_reply;
    if (ri->operation() == "echo_string" && ri->reply_status() == PI::SUCCESSFUL) {
      seen.reply_context = text(ri->get_reply_service_context(REPLY_CONTEXT).context_data());
    }
  }
  void receive_exception(CORBA::object_reference<PI::ClientRequestInfo> ri) override {
    std::lock_guard<std::mutex> lock(seen.m);
    ++seen.receive_exception;
    seen.received_exception_id = ri->received_exception_id();
  }
  void receive_other(CORBA::object_reference<PI::ClientRequestInfo> ri) override {
    std::lock_guard<std::mutex> lock(seen.m);
    if (ri->reply_status() == PI::LOCATION_FORWARD) ++seen.receive_other;
  }
};

class Server : public PI::ServerRequestInterceptor {
 public:
  std::string name() override { return "ctx"; }
  void receive_request_service_contexts(
      CORBA::object_reference<PI::ServerRequestInfo> ri) override {
    if (ri->operation() != "echo_string") return;
    CORBA::Any a;
    a <<= text(ri->get_request_service_context(REQUEST_CONTEXT).context_data());
    ri->set_slot(seen.slot, a);
  }
  void receive_request(CORBA::object_reference<PI::ServerRequestInfo> ri) override {
    std::string op = ri->operation();
    if (op == "echo_ulong") throw CORBA::NO_PERMISSION(7, CORBA::CompletionStatus::COMPLETED_NO);
    if (op == "echo_octet" && ri->object_id() == seen.server_forward_oid) {
      throw PI::ForwardRequest(seen.server_forward);
    }
    if (op == "echo_string") {
      std::lock_guard<std::mutex> lock(seen.m);
      seen.server_orb_id_ok = ri->orb_id() == "server";
      seen.target_is_a = ri->target_is_a(ECHO_ID) && !ri->target_is_a("IDL:Other:1.0");
      seen.target_is_a = seen.target_is_a && ri->target_most_derived_interface() == ECHO_ID;
      seen.adapter_name_ok = ri->adapter_name() == PI::AdapterName{"RootPOA"};
    }
  }
  void send_reply(CORBA::object_reference<PI::ServerRequestInfo> ri) override {
    ri->add_reply_service_context(
        IOP::ServiceContext(REPLY_CONTEXT, octets("reply:" + ri->operation())), true);
  }
  void send_exception(CORBA::object_reference<PI::ServerRequestInfo> ri) override {
    std::lock_guard<std::mutex> lock(seen.m);
    seen.sending_exception_id = ri->sending_exception_id();
  }
};

class Ior : public PI::IORInterceptor {
 public:
  std::string name() override { return "component"; }
  void establish_components(CORBA::object_reference<PI::IORInfo> info) override {
    info->add_ior_component(IOP::TaggedComponent(COMPONENT, {1, 2, 3}));
  }
};

class Anonymous : public PI::ClientRequestInterceptor {};

class Init : public PI::ORBInitializer {
 public:
  void pre_init(CORBA::object_reference<PI::ORBInitInfo> info) override {
    std::lock_guard<std::mutex> lock(seen.m);
    seen.pre_init_ids.push_back(info->orb_id());
    seen.arguments = info->arguments();
    seen.slot = info->allocate_slot_id();
    info->add_client_request_interceptor(CORBA::make_reference<Client>());
    info->add_server_request_interceptor(CORBA::make_reference<Server>());
    info->add_ior_interceptor(CORBA::make_reference<Ior>());
    seen.duplicate_name = raises<PI::ORBInitInfo::DuplicateName>(
        [&] { info->add_client_request_interceptor(CORBA::make_reference<Client>()); });
    info->add_client_request_interceptor(CORBA::make_reference<Anonymous>());
    info->add_client_request_interceptor(CORBA::make_reference<Anonymous>());
    seen.anonymous_twice = true;
    info->register_initial_reference("PiTestMarker", CORBA::make_reference<Marker>());
    seen.pre_init_resolve_bad_order =
        raises<CORBA::BAD_INV_ORDER>([&] { info->resolve_initial_references("PiTestMarker"); });
  }
  void post_init(CORBA::object_reference<PI::ORBInitInfo> info) override {
    std::lock_guard<std::mutex> lock(seen.m);
    seen.post_init_ids.push_back(info->orb_id());
    seen.post_init_resolve_ok = static_cast<bool>(
        IDL::traits<Marker>::narrow(info->resolve_initial_references("PiTestMarker")));
    seen.post_init_invalid_name = raises<PI::ORBInitInfo::InvalidName>(
        [&] { info->resolve_initial_references("NoSuchName"); });
    // One of the ORB's own (not the RootPOA: a POA made in post_init
    // would not have the components of the IOR interceptors, which only
    // work once the ORB is initialised).
    seen.poa_current_ok = static_cast<bool>(IDL::traits<PortableServer::Current>::narrow(
        info->resolve_initial_references("POACurrent")));
    seen.pi_current_ok = static_cast<bool>(
        IDL::traits<PI::Current>::narrow(info->resolve_initial_references("PICurrent")));
    auto codec = info->codec_factory()->create_codec(IOP::Encoding(IOP::ENCODING_CDR_ENCAPS, 1, 2));
    CORBA::Any a;
    a <<= int32_t{5};
    int32_t back = 0;
    seen.codec_factory_ok = (codec->decode(codec->encode(a)) >>= back) && back == 5;
    seen.kept_info = info;
  }
};

// echo_string returns the slot the server interceptor set from the
// request's service context.
class SlotEcho : public EchoImpl {
 public:
  explicit SlotEcho(IDL::traits<CORBA::ORB>::ref_type orb)
      : EchoImpl(orb),
        current_(IDL::traits<PI::Current>::narrow(orb->resolve_initial_references("PICurrent"))) {}
  std::string echo_string(const std::string& v) override {
    std::string slot;
    CORBA::Any a = current_->get_slot(seen.slot);
    return (a >>= slot) ? slot : "no slot:" + v;
  }

 private:
  CORBA::object_reference<PI::Current> current_;
};

static void codec(const CORBA::object_reference<CORBA::ORB>& orb) {
  auto factory =
      IDL::traits<IOP::CodecFactory>::narrow(orb->resolve_initial_references("CodecFactory"));
  CHECK(factory);
  auto codec = factory->create_codec(IOP::Encoding(IOP::ENCODING_CDR_ENCAPS, 1, 2));
  CORBA::Any a;
  a <<= std::string("hello");
  std::string s;
  CHECK((codec->decode(codec->encode(a)) >>= s) && s == "hello");
  std::vector<uint8_t> value = codec->encode_value(a);
  s.clear();
  CHECK((codec->decode_value(value, CORBA::_tc_string) >>= s) && s == "hello");
  CHECK(raises<IOP::CodecFactory::UnknownEncoding>(
      [&] { factory->create_codec(IOP::Encoding(1, 1, 2)); }));
  CHECK(raises<IOP::CodecFactory::UnknownEncoding>(
      [&] { factory->create_codec(IOP::Encoding(IOP::ENCODING_CDR_ENCAPS, 1, 9)); }));
}

int main() {
  try {
    PI::register_orb_initializer(CORBA::make_reference<Init>());

    int sargc = 4;
    char sarg0[] = "server", sarg1[] = "-ORBendPoint",
         sarg2[] = "giop:tcp:127.0.0.1:", sarg3[] = "extra";
    char* sargv[] = {sarg0, sarg1, sarg2, sarg3, nullptr};
    auto server_orb = CORBA::ORB_init(sargc, sargv, "server");
    CHECK(seen.kept_info);
    CHECK(minor_of<CORBA::OBJECT_NOT_EXIST>([&] { seen.kept_info->orb_id(); }) ==
          (0x4f4d0000u | 10));
    CHECK(seen.arguments.size() >= 1 && seen.arguments.back() == "extra");
    auto root =
        IDL::traits<PortableServer::POA>::narrow(server_orb->resolve_initial_references("RootPOA"));
    root->the_POAManager()->activate();
    auto servant = CORBA::make_reference<SlotEcho>(server_orb);
    auto oid = root->activate_object(servant);
    std::string ior = server_orb->object_to_string(root->id_to_reference(oid));
    auto oid2 = root->activate_object(CORBA::make_reference<SlotEcho>(server_orb));
    std::string ior2 = server_orb->object_to_string(root->id_to_reference(oid2));
    seen.server_forward = root->id_to_reference(oid);
    seen.server_forward_oid = oid2;
    std::thread server([&] { server_orb->run(); });

    int cargc = 3;
    char carg0[] = "client", carg1[] = "-ORBid", carg2[] = "client";
    char* cargv[] = {carg0, carg1, carg2, nullptr};
    auto client_orb = CORBA::ORB_init(cargc, cargv);
    CHECK(seen.pre_init_ids == (std::vector<std::string>{"server", "client"}));
    CHECK(seen.post_init_ids == seen.pre_init_ids);
    CHECK(seen.duplicate_name);
    CHECK(seen.anonymous_twice);
    CHECK(seen.pre_init_resolve_bad_order);
    CHECK(seen.post_init_resolve_ok);
    CHECK(seen.post_init_invalid_name);
    CHECK(seen.pi_current_ok);
    CHECK(seen.poa_current_ok);
    CHECK(seen.codec_factory_ok);

    auto echo = IDL::traits<InteropTest::Echo>::narrow(client_orb->string_to_object(ior));
    CHECK(echo);

    // A slot of the client thread goes to the servant in a service
    // context, and a reply context comes back.
    auto current =
        IDL::traits<PI::Current>::narrow(client_orb->resolve_initial_references("PICurrent"));
    CORBA::Any a;
    a <<= std::string("from the client");
    current->set_slot(seen.slot, a);
    CHECK(echo->echo_string("x") == "from the client");
    CHECK(seen.reply_context == "reply:echo_string");
    CHECK(seen.component_ok);
    CHECK(seen.arguments_unavailable);
    CHECK(seen.response_expected);
    CHECK(seen.server_orb_id_ok);
    CHECK(seen.target_is_a);
    CHECK(seen.adapter_name_ok);
    CHECK(raises<PI::InvalidSlot>([&] { current->get_slot(seen.slot + 100); }));

    // The info kept past its point.
    CHECK(minor_of<CORBA::BAD_INV_ORDER>([&] { seen.kept_ri->operation(); }) == (0x4f4d0000u | 14));
    seen.kept_ri.reset();

    // An exception from receive_request reaches the client.
    int before = seen.receive_exception;
    CHECK(minor_of<CORBA::NO_PERMISSION>([&] { echo->echo_ulong(1); }) == 7);
    CHECK(seen.receive_exception == before + 1);
    CHECK(seen.received_exception_id == "IDL:omg.org/CORBA/NO_PERMISSION:1.0");
    CHECK(seen.sending_exception_id == "IDL:omg.org/CORBA/NO_PERMISSION:1.0");

    // A user exception of the servant.
    CHECK(raises<InteropTest::Oops>([&] { echo->fail(1, "no"); }));
    CHECK(seen.received_exception_id == "IDL:rustorb.org/InteropTest/Oops:1.0");

    // ForwardRequest from send_request: the call goes to the forward
    // reference. The interceptor that raised it gets no ending point.
    auto other = IDL::traits<InteropTest::Echo>::narrow(client_orb->string_to_object(ior2));
    seen.forward_from = client_orb->string_to_object(ior2);
    seen.forward_to = client_orb->string_to_object(ior);
    int sends = seen.send_request, others = seen.receive_other;
    CHECK(other->echo_short(3) == 3);
    CHECK(seen.send_request == sends + 2);
    CHECK(seen.receive_other == others);

    // ForwardRequest from receive_request: the client sees
    // LOCATION_FORWARD in receive_other and follows it.
    auto third = IDL::traits<InteropTest::Echo>::narrow(client_orb->string_to_object(ior2));
    CHECK(third->echo_octet(4) == 4);
    CHECK(seen.receive_other == others + 1);
    seen.server_forward.reset();
    seen.forward_to.reset();
    seen.forward_from.reset();

    codec(client_orb);

    echo->shutdown();
    server.join();
    server_orb->destroy();
    client_orb->destroy();
  } catch (const CORBA::Exception& e) {
    std::cout << "CORBA exception: " << e << std::endl;
    return 1;
  }
  if (failures) {
    std::cout << failures << " failure(s)" << std::endl;
    return 1;
  }
  std::cout << "ok" << std::endl;
  return 0;
}

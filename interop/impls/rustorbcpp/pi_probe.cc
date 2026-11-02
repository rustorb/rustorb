// The rustorb C++ side of run-pi-probe.sh: the probe of
// impls/rustorb/src/pi_probe.rs on <rustorb/pi.h>, printing the same lines.
//
//   pi-probe server [ORB options]       prints the IOR of an Echo object,
//                                       then serves
//   pi-probe client IOR [ORB options]   calls echo

#include <iostream>
#include <mutex>
#include <string>
#include <vector>

#include "PiProbe.hh"
#include "rustorb/pi.h"

namespace PI = PortableInterceptor;

constexpr IOP::ServiceId CONTEXT = 0x50495052;
constexpr IOP::ComponentId TAG = 0x50495052;
const std::string NAME = "rustorbcpp";

static std::vector<uint8_t> encode(IDL::traits<IOP::Codec>::ref_type codec, const std::string& s) {
  CORBA::Any a;
  a <<= s;
  return codec->encode_value(a);
}

static std::string decode(IDL::traits<IOP::Codec>::ref_type codec,
                          const std::vector<uint8_t>& data) {
  try {
    std::string s;
    if (codec->decode_value(data, CORBA::_tc_string) >>= s) return s;
    return "<not a string>";
  } catch (const CORBA::Exception& e) {
    return std::string("<") + e._rep_id() + ">";
  }
}

class Probe : public PI::ClientRequestInterceptor,
              public PI::ServerRequestInterceptor,
              public PI::IORInterceptor {
 public:
  Probe(IDL::traits<IOP::Codec>::ref_type codec, PI::SlotId slot)
      : codec_(std::move(codec)), slot_(slot) {}

  std::string name() override { return "pi-probe"; }

  void send_request(CORBA::object_reference<PI::ClientRequestInfo> ri) override {
    ri->add_request_service_context(IOP::ServiceContext(CONTEXT, encode(codec_, "from " + NAME)),
                                    false);
  }
  void receive_reply(CORBA::object_reference<PI::ClientRequestInfo> ri) override {
    if (ri->operation() != "echo") return;  // not narrow's _is_a
    std::string context, component;
    try {
      context = decode(codec_, ri->get_reply_service_context(CONTEXT).context_data());
    } catch (const CORBA::SystemException& e) {
      context = e._rep_id();
    }
    try {
      component = decode(codec_, ri->get_effective_component(TAG).component_data());
    } catch (const CORBA::SystemException& e) {
      component = e._rep_id();
    }
    std::lock_guard<std::mutex> lock(m_);
    seen_.push_back("reply context: " + context);
    seen_.push_back("component: " + component);
  }
  void receive_request_service_contexts(CORBA::object_reference<PI::ServerRequestInfo> ri) override {
    std::string got;
    try {
      got = decode(codec_, ri->get_request_service_context(CONTEXT).context_data());
    } catch (const CORBA::SystemException& e) {
      got = e._rep_id();
    }
    CORBA::Any a;
    a <<= got;
    ri->set_slot(slot_, a);
  }
  void send_reply(CORBA::object_reference<PI::ServerRequestInfo> ri) override {
    std::string got;
    ri->get_slot(slot_) >>= got;
    ri->add_reply_service_context(
        IOP::ServiceContext(CONTEXT, encode(codec_, NAME + " got: " + got)), false);
  }
  void establish_components(CORBA::object_reference<PI::IORInfo> info) override {
    info->add_ior_component(IOP::TaggedComponent(TAG, encode(codec_, NAME + " component")));
  }

  std::vector<std::string> seen() {
    std::lock_guard<std::mutex> lock(m_);
    return seen_;
  }

 private:
  IDL::traits<IOP::Codec>::ref_type codec_;
  PI::SlotId slot_;
  std::mutex m_;
  std::vector<std::string> seen_;
};

static CORBA::object_reference<Probe> probe;

class Init : public PI::ORBInitializer {
 public:
  void pre_init(CORBA::object_reference<PI::ORBInitInfo> info) override {
    auto codec = info->codec_factory()->create_codec(IOP::Encoding(IOP::ENCODING_CDR_ENCAPS, 1, 2));
    probe = CORBA::make_reference<Probe>(codec, info->allocate_slot_id());
    info->add_client_request_interceptor(probe);
    info->add_server_request_interceptor(probe);
    info->add_ior_interceptor(probe);
  }
};

class Echo : public virtual CORBA::servant_traits<PiProbe::Echo>::base_type {
 public:
  std::string echo(const std::string& s) override { return s; }
};

int main(int argc, char* argv[]) {
  try {
    PI::register_orb_initializer(CORBA::make_reference<Init>());
    auto orb = CORBA::ORB_init(argc, argv);
    if (argc >= 2 && std::string(argv[1]) == "server") {
      auto root =
          IDL::traits<PortableServer::POA>::narrow(orb->resolve_initial_references("RootPOA"));
      root->the_POAManager()->activate();
      auto oid = root->activate_object(CORBA::make_reference<Echo>());
      std::cout << orb->object_to_string(root->id_to_reference(oid)) << std::endl;
      orb->run();
    } else if (argc >= 3 && std::string(argv[1]) == "client") {
      auto echo = IDL::traits<PiProbe::Echo>::narrow(orb->string_to_object(argv[2]));
      std::string result;
      try {
        result = echo->echo("hello");
      } catch (const CORBA::Exception& e) {
        result = e._rep_id();
      }
      for (const auto& line : probe->seen()) std::cout << line << "\n";
      std::cout << "result: " << result << std::endl;
      orb->destroy();
    } else {
      std::cerr << "usage: pi-probe server|client IOR [ORB options]" << std::endl;
      return 2;
    }
  } catch (const CORBA::Exception& e) {
    std::cerr << "CORBA exception: " << e << std::endl;
    return 1;
  }
  return 0;
}

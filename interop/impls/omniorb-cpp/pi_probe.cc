// The omniORB C++ client of run-pi-probe.sh. omniORB has no Portable
// Interceptors in C++; its own clientSendRequest interceptor
// (omniInterceptors, as the omniORB user's guide documents it) adds the
// request service context of impls/rustorb/src/pi_probe.rs. The guide does
// not document how to read reply service contexts or IOR components, and
// the probe does not look for them (clean-room policy): the script checks
// that the server printed the request context instead.
//
//   pi_probe client IOR [ORB options]   calls echo (DII) and prints
//                                       "result: <result>"

#include <omniORB4/CORBA.h>
#include <omniORB4/omniInterceptors.h>

#include <iostream>
#include <string>

namespace {

const CORBA::ULong CONTEXT = 0x50495052;  // "PIPR"
const char* const NAME = "omniorbcpp";

// s as a CORBA string in a big-endian CDR encapsulation (CORBA 3.x 9.3.3):
// the byte order octet, padding to 4, the length with the NUL, the octets.
template <class Octets>
void encode(const std::string& s, Octets& out) {
  CORBA::ULong n = static_cast<CORBA::ULong>(s.size() + 1);
  out.length(8 + n);
  out[0] = 0;
  out[1] = out[2] = out[3] = 0;
  out[4] = static_cast<CORBA::Octet>(n >> 24);
  out[5] = static_cast<CORBA::Octet>(n >> 16);
  out[6] = static_cast<CORBA::Octet>(n >> 8);
  out[7] = static_cast<CORBA::Octet>(n);
  for (CORBA::ULong i = 0; i < n - 1; ++i) out[8 + i] = static_cast<CORBA::Octet>(s[i]);
  out[8 + n - 1] = 0;
}

CORBA::Boolean send_request(omni::omniInterceptors::clientSendRequest_T::info_T& info) {
  CORBA::ULong i = info.service_contexts.length();
  info.service_contexts.length(i + 1);
  info.service_contexts[i].context_id = CONTEXT;
  encode(std::string("from ") + NAME, info.service_contexts[i].context_data);
  return 1;
}

}  // namespace

int main(int argc, char* argv[]) {
  try {
    CORBA::ORB_var orb = CORBA::ORB_init(argc, argv);
    if (argc < 3 || std::string(argv[1]) != "client") {
      std::cerr << "usage: pi_probe client IOR" << std::endl;
      return 2;
    }
    omniORB::getInterceptors()->clientSendRequest.add(send_request);
    CORBA::Object_var obj = orb->string_to_object(argv[2]);
    CORBA::Request_var req = obj->_request("echo");
    req->add_in_arg() <<= "hello";
    req->set_return_type(CORBA::_tc_string);
    std::string result;
    try {
      req->invoke();
      const char* r = nullptr;
      req->return_value() >>= r;
      result = r ? r : "<null>";
    } catch (const CORBA::Exception& e) {
      result = e._rep_id();
    }
    std::cout << "result: " << result << std::endl;
    orb->destroy();
  } catch (const CORBA::Exception& e) {
    std::cerr << "pi_probe: " << e._name() << std::endl;
    return 1;
  }
  return 0;
}

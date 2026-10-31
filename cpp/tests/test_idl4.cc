// The IDL 4 constructs of rustorb-idl -bc++11 (cpp/tests/idl/Idl4.idl):
// int8, bitmask, bitset, @default and @range, locally and through stubs
// and skeletons between two ORBs in one process.

#include <cstdint>
#include <iostream>
#include <string>
#include <thread>
#include <type_traits>

#include "Idl4.hh"

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

using namespace Idl4;

class EchoImpl : public CORBA::servant_traits<Echo>::base_type {
 public:
  Settings echo_settings(const Settings& s) override { return s; }
  Flags echo_flags(Flags f, Wide& w) override {
    w = Wide::LOW | Wide::HIGH;
    return f;
  }
  Header echo_header(Header h) override { return h; }
  int8_t echo_int8(int8_t v, Bytes& b) override {
    b.push_back(v);
    return static_cast<int8_t>(-v);
  }
  CORBA::Any echo_any(const CORBA::Any& a) override { return a; }
};

static void check_types() {
  static_assert(std::is_same<std::underlying_type<Flags>::type, uint8_t>::value, "bit_bound 8");
  static_assert(std::is_same<std::underlying_type<Wide>::type, uint64_t>::value, "bit_bound 40");
  CHECK(SMALLEST == -128);
  CHECK(static_cast<uint8_t>(Flags::READ) == 1);
  CHECK(static_cast<uint8_t>(Flags::WRITE) == 8);
  CHECK(static_cast<uint8_t>(Flags::EXEC) == 16);
  Flags f = Flags::READ | Flags::EXEC;
  CHECK(has(f, Flags::READ) && !has(f, Flags::WRITE));
  f |= Flags::WRITE;
  f &= ~Flags::READ;
  CHECK(f == (Flags::WRITE | Flags::EXEC));
  CHECK(static_cast<uint64_t>(Wide::HIGH) == (uint64_t{1} << 39));

  Settings s;
  CHECK(s.retries() == 7);
  CHECK(s.enabled());
  CHECK(s.ratio() == 2.5);
  CHECK(s.level() == 0 && s.id() == 0);
}

static Settings settings() {
  Settings s;
  s.id(3);
  s.level(10);
  s.step(1);
  s.offset(-5);
  s.raw(250);
  s.flags(Flags::READ | Flags::WRITE);
  s.wide(Wide::HIGH);
  Header h{};
  h.kind = 5;
  h.delta = -3;
  h.last = true;
  s.header(h);
  return s;
}

static void check_calls(IDL::traits<CORBA::ORB>::ref_type orb, const std::string& ior) {
  auto echo = IDL::traits<Echo>::narrow(orb->string_to_object(ior));

  Settings s = settings();
  CHECK(echo->echo_settings(s) == s);
  Settings back = echo->echo_settings(s);
  CHECK(back.header().delta == -3 && back.header().kind == 5 && back.header().last);
  CHECK(back.offset() == -5 && back.raw() == 250);

  // @range / @min / @max: a value outside is not sent.
  Settings bad = settings();
  bad.level(11);
  CHECK(raises<CORBA::BAD_PARAM>([&] { echo->echo_settings(bad); }));
  bad = settings();
  bad.step(0);
  CHECK(raises<CORBA::BAD_PARAM>([&] { echo->echo_settings(bad); }));

  Wide w = Wide::LOW;
  CHECK(echo->echo_flags(Flags::EXEC, w) == Flags::EXEC);
  CHECK(w == (Wide::LOW | Wide::HIGH));

  Header h{};
  h.kind = 7;
  h.delta = 15;
  h.last = false;
  CHECK(echo->echo_header(h) == h);
  h.delta = -16;
  CHECK(echo->echo_header(h).delta == -16);

  Bytes b{1, -2};
  CHECK(echo->echo_int8(-128 + 1, b) == 127);
  CHECK((b == Bytes{1, -2, -127}));

  // In an any: a bitmask and a bitset are the unsigned integers that hold
  // their bits, an int8 is an octet.
  CORBA::Any a;
  a <<= Flags::WRITE;
  CHECK(a.type()->kind() == CORBA::TCKind::tk_octet);
  CORBA::Any ab = echo->echo_any(a);
  Flags got{};
  CHECK((ab >>= got) && got == Flags::WRITE);
  a <<= h;  // 11 bits
  CHECK(a.type()->kind() == CORBA::TCKind::tk_ushort);
  a <<= static_cast<int8_t>(-1);
  CHECK(a.type()->kind() == CORBA::TCKind::tk_octet);
  a <<= Wide::HIGH;
  CHECK(a.type()->kind() == CORBA::TCKind::tk_ulonglong);
}

int main() {
  try {
    check_types();

    int sargc = 3;
    char sarg0[] = "server", sarg1[] = "-ORBendPoint", sarg2[] = "giop:tcp:127.0.0.1:";
    char* sargv[] = {sarg0, sarg1, sarg2, nullptr};
    auto server_orb = CORBA::ORB_init(sargc, sargv);
    auto root = IDL::traits<PortableServer::POA>::narrow(
        server_orb->resolve_initial_references("RootPOA"));
    root->the_POAManager()->activate();
    auto servant = CORBA::make_reference<EchoImpl>();
    std::string ior = server_orb->object_to_string(servant->_this());
    std::thread server([&] { server_orb->run(); });

    int cargc = 1;
    char carg0[] = "client";
    char* cargv[] = {carg0, nullptr};
    auto client_orb = CORBA::ORB_init(cargc, cargv);
    check_calls(client_orb, ior);
    // The same calls on the server ORB: colocated.
    check_calls(server_orb, ior);

    server_orb->shutdown(true);
    server.join();
    server_orb->destroy();
    client_orb->destroy();
  } catch (const CORBA::Exception& e) {
    std::cout << "CORBA exception: " << e << std::endl;
    return 1;
  }
  std::cout << (failures ? "FAILED" : "ok") << std::endl;
  return failures ? 1 : 0;
}

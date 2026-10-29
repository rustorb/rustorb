// omniORB C++ implementation of the InteropTest client.
//
// Resolves the Echo object from the naming service, runs every test case
// against it and exits non-zero if any case fails.

#include <algorithm>
#include <cmath>
#include <functional>
#include <sstream>
#include <string>

#include "common.h"
#define ANYTEST_HIDDEN
#include "anytest.h"
#include "valuetest.h"

using namespace InteropTest;

struct CheckFailed {
  std::string message;
};

#define CHECK(cond)                                                     \
  do {                                                                  \
    if (!(cond)) throw CheckFailed{std::string("line ") +               \
                                   std::to_string(__LINE__) + ": " #cond}; \
  } while (0)

class Runner {
 public:
  void run(const std::string& name, const std::function<void()>& fn) {
    try {
      fn();
      ++passed_;
      std::cout << "ok   " << name << std::endl;
      return;
    } catch (CheckFailed& e) {
      fail(name, e.message);
    } catch (CORBA::Exception& e) {
      fail(name, std::string("unexpected ") + e._name());
    } catch (std::exception& e) {
      fail(name, e.what());
    }
  }

  int passed() const { return passed_; }
  const std::vector<std::string>& failed() const { return failed_; }

 private:
  void fail(const std::string& name, const std::string& why) {
    failed_.push_back(name);
    std::cout << "FAIL " << name << ": " << why << std::endl;
  }

  int passed_ = 0;
  std::vector<std::string> failed_;
};

template <typename F>
void expect_raises(F fn) {
  fn();  // must throw; the caller catches the expected type
  throw CheckFailed{"expected exception was not raised"};
}

class CallbackImpl : public POA_InteropTest::Callback {
 public:
  char* notify(const char* message) override {
    received.push_back(message);
    return CORBA::string_dup(("ack:" + std::string(message)).c_str());
  }
  std::vector<std::string> received;
};

static bool same(const Point& a, const Point& b) { return a.x == b.x && a.y == b.y; }

static bool same(const LongSeq& a, const LongSeq& b) {
  if (a.length() != b.length()) return false;
  for (CORBA::ULong i = 0; i < a.length(); ++i)
    if (a[i] != b[i]) return false;
  return true;
}

static bool same(const Record& a, const Record& b) {
  return a.id == b.id && std::strcmp(a.name, b.name) == 0 && a.hue == b.hue &&
         same(a.pos, b.pos) && same(a.values, b.values) && a.active == b.active;
}

static bool same(const Value& a, const Value& b) {
  if (a._d() != b._d()) return false;
  switch (a._d()) {
    case 1: return a.l() == b.l();
    case 2: return std::strcmp(a.s(), b.s()) == 0;
    case 3: return same(a.p(), b.p());
    case 4:
    case 5: return same(a.seq(), b.seq());
    default: return a.flag() == b.flag();
  }
}

static bool same(const Shade& a, const Shade& b) {
  if (a._d() != b._d()) return false;
  switch (a._d()) {
    case RED: return a.level() == b.level();
    case GREEN: return std::strcmp(a.label(), b.label()) == 0;
    default: return true;
  }
}

static Record make_record(CORBA::Long id, const char* name, Color color, Point pos,
                          std::vector<CORBA::Long> values, bool active) {
  Record r;
  r.id = id;
  r.name = CORBA::string_dup(name);
  r.hue = color;
  r.pos = pos;
  r.values.length(static_cast<CORBA::ULong>(values.size()));
  for (CORBA::ULong i = 0; i < values.size(); ++i) r.values[i] = values[i];
  r.active = active;
  return r;
}

template <typename T, typename F>
static void echo_values(Runner& r, const char* op, std::vector<T> values, F call) {
  for (size_t i = 0; i < values.size(); ++i) {
    T v = values[i];
    r.run(std::string(op) + "[" + std::to_string(i) + "]", [&]() { CHECK(call(v) == v); });
  }
}

static void run_cases(Runner& r, InteropTest::Echo_ptr echo, PortableServer::POA_ptr poa) {
  // --- basic types ---
  echo_values<bool>(r, "echo_boolean", {true, false},
                    [&](bool v) { return static_cast<bool>(echo->echo_boolean(v)); });
  echo_values<CORBA::Octet>(r, "echo_octet", {0, 0x7f, 0xff},
                            [&](CORBA::Octet v) { return echo->echo_octet(v); });
  echo_values<CORBA::Char>(r, "echo_char", {'a', '\0', '~'},
                           [&](CORBA::Char v) { return echo->echo_char(v); });
  echo_values<CORBA::WChar>(r, "echo_wchar", {L'a', L'あ', L'é'},
                            [&](CORBA::WChar v) { return echo->echo_wchar(v); });
  echo_values<CORBA::Short>(r, "echo_short", {0, -32768, 32767},
                            [&](CORBA::Short v) { return echo->echo_short(v); });
  echo_values<CORBA::UShort>(r, "echo_ushort", {0, 65535},
                             [&](CORBA::UShort v) { return echo->echo_ushort(v); });
  echo_values<CORBA::Long>(r, "echo_long", {0, -2147483647 - 1, 2147483647, MAGIC},
                           [&](CORBA::Long v) { return echo->echo_long(v); });
  echo_values<CORBA::ULong>(r, "echo_ulong", {0, 4294967295u},
                            [&](CORBA::ULong v) { return echo->echo_ulong(v); });
  echo_values<CORBA::LongLong>(r, "echo_longlong",
                               {0, -9223372036854775807LL - 1, 9223372036854775807LL},
                               [&](CORBA::LongLong v) { return echo->echo_longlong(v); });
  echo_values<CORBA::ULongLong>(r, "echo_ulonglong", {0, 18446744073709551615ULL},
                                [&](CORBA::ULongLong v) { return echo->echo_ulonglong(v); });
  echo_values<CORBA::Double>(r, "echo_double", {0.0, -1.5, 1e300, std::ldexp(1.0, -1074)},
                             [&](CORBA::Double v) { return echo->echo_double(v); });
  // char strings use the native code set ISO-8859-1, as in omniORBpy.
  echo_values<std::string>(r, "echo_string", {"", "hello", std::string(10000, 'x'), "caf\xe9"},
                           [&](const std::string& v) {
                             CORBA::String_var s = echo->echo_string(v.c_str());
                             return std::string(s.in());
                           });
  // omniORB C++ converts each wchar_t to one UTF-16 code unit and rejects
  // values above 0xFFFF, so U+1F980 is passed as its surrogate pair. On the
  // wire this is the same as the "\U0001F980" sent by the Python client.
  echo_values<std::wstring>(r, "echo_wstring",
                            {L"", L"hello", L"こんにちは",
                             std::wstring{wchar_t(0xD83E), wchar_t(0xDD80)}},
                            [&](const std::wstring& v) {
                              CORBA::WString_var s = echo->echo_wstring(v.c_str());
                              return std::wstring(s.in());
                            });
  echo_values<CORBA::Float>(r, "echo_float", {0.0f, -1.5f, 3.4028234663852886e38f,
                                              1.1754943508222875e-38f},
                            [&](CORBA::Float v) { return echo->echo_float(v); });

  // --- constructed types ---
  Point p{1.25, -2.5};
  Record rec = make_record(7, "seven", GREEN, p, {1, -2, 3}, true);
  Record rec2 = make_record(-1, "", RED, Point{0.0, 0.0}, {}, false);

  r.run("echo_color", [&]() {
    for (Color c : {RED, GREEN, BLUE}) CHECK(echo->echo_color(c) == c);
  });
  r.run("echo_point", [&]() { CHECK(same(echo->echo_point(p), p)); });
  r.run("echo_record", [&]() {
    Record_var got = echo->echo_record(rec);
    CHECK(same(got.in(), rec));
  });
  r.run("echo_records", [&]() {
    RecordSeq seq;
    seq.length(3);
    seq[0] = rec;
    seq[1] = rec2;
    seq[2] = rec;
    RecordSeq_var got = echo->echo_records(seq);
    CHECK(got->length() == 3);
    for (CORBA::ULong i = 0; i < 3; ++i) CHECK(same(got[i], seq[i]));
  });
  r.run("echo_records[empty]", [&]() {
    RecordSeq_var got = echo->echo_records(RecordSeq());
    CHECK(got->length() == 0);
  });
  // Large enough to be sent in several fragments, with doubles after each
  // fragment boundary; record i has a name of varying length so that the
  // doubles land at every alignment.
  r.run("echo_records[large]", [&]() {
    const CORBA::ULong n = 400;
    const Color colors[] = {RED, GREEN, BLUE};
    RecordSeq seq;
    seq.length(n);
    for (CORBA::ULong i = 0; i < n; ++i) {
      CORBA::Long id = static_cast<CORBA::Long>(i);
      seq[i] = make_record(id, ("r" + std::to_string(i)).c_str(), colors[i % 3],
                           Point{id + 0.5, -0.25 * id}, {id, id + 1}, i % 2 == 0);
    }
    RecordSeq_var got = echo->echo_records(seq);
    CHECK(got->length() == n);
    for (CORBA::ULong i = 0; i < n; ++i) CHECK(same(got[i], seq[i]));
  });

  std::vector<Value> values(7);
  values[0].l(-5);
  values[1].s(CORBA::string_dup("union"));
  values[2].p(p);
  {
    LongSeq seq;
    seq.length(2);
    seq[0] = 1;
    seq[1] = 2;
    values[3].seq(seq);
    values[3]._d(4);
    values[4].seq(LongSeq());
    values[4]._d(5);
  }
  values[5].flag(true);
  values[5]._d(99);
  values[6].flag(false);
  values[6]._d(0);
  for (size_t i = 0; i < values.size(); ++i) {
    r.run("echo_value[" + std::to_string(i) + "]", [&]() {
      Value_var got = echo->echo_value(values[i]);
      CHECK(same(got.in(), values[i]));
    });
  }

  std::vector<Shade> shades(3);
  shades[0].level(200);
  shades[1].label(CORBA::string_dup("leaf"));
  shades[2]._default();
  shades[2]._d(BLUE);
  for (size_t i = 0; i < shades.size(); ++i) {
    r.run("echo_shade[" + std::to_string(i) + "]", [&]() {
      Shade_var got = echo->echo_shade(shades[i]);
      CHECK(same(got.in(), shades[i]));
    });
  }

  r.run("echo_long_array", [&]() {
    LongArray a = {1, -1, 2147483647, 0};
    LongArray_var got = echo->echo_long_array(a);
    for (int i = 0; i < 4; ++i) CHECK(got[i] == a[i]);
  });
  r.run("echo_matrix", [&]() {
    Matrix m = {{1.0, 2.0, 3.0}, {-4.0, 0.5, 1e-9}};
    Matrix_var got = echo->echo_matrix(m);
    for (int i = 0; i < 2; ++i)
      for (int j = 0; j < 3; ++j) CHECK(got[i][j] == m[i][j]);
  });
  r.run("echo_long_seq", [&]() {
    LongSeq seq;
    seq.length(1000);
    for (CORBA::ULong i = 0; i < 1000; ++i) seq[i] = static_cast<CORBA::Long>(i) - 500;
    LongSeq_var got = echo->echo_long_seq(seq);
    CHECK(same(got.in(), seq));
  });
  r.run("echo_long_seq[empty]", [&]() {
    LongSeq_var got = echo->echo_long_seq(LongSeq());
    CHECK(got->length() == 0);
  });
  r.run("echo_string_seq", [&]() {
    StringSeq seq;
    seq.length(3);
    seq[0] = CORBA::string_dup("a");
    seq[1] = CORBA::string_dup("");
    seq[2] = CORBA::string_dup("ccc");
    StringSeq_var got = echo->echo_string_seq(seq);
    CHECK(got->length() == 3);
    for (CORBA::ULong i = 0; i < 3; ++i) CHECK(std::strcmp(got[i], seq[i]) == 0);
  });
  r.run("echo_octets", [&]() {
    OctetSeq seq;
    seq.length(256);
    for (CORBA::ULong i = 0; i < 256; ++i) seq[i] = static_cast<CORBA::Octet>(i);
    OctetSeq_var got = echo->echo_octets(seq);
    CHECK(got->length() == 256);
    for (CORBA::ULong i = 0; i < 256; ++i) CHECK(got[i] == seq[i]);
  });

  // --- parameter modes ---
  r.run("swap", [&]() {
    CORBA::Long a = 3, b = -10;
    CHECK(echo->swap(a, b) == -7);
    CHECK(a == -10 && b == 3);
  });
  r.run("divide", [&]() {
    CORBA::Long rem = 0;
    CHECK(echo->divide(17, 5, rem) == 3 && rem == 2);
    CHECK(echo->divide(-17, 5, rem) == -3 && rem == -2);
    try {
      expect_raises([&]() { echo->divide(1, 0, rem); });
    } catch (DivideByZero&) {
    }
  });
  r.run("make", [&]() {
    Point pt;
    Record_var rc;
    echo->make(12, pt, rc.out());
    CHECK(same(pt, Point{12.0, -12.0}));
    CHECK(same(rc.in(), make_record(12, "r12", BLUE, Point{12.0, -12.0}, {12, 13}, true)));
  });

  // --- exceptions ---
  r.run("fail", [&]() {
    try {
      expect_raises([&]() { echo->fail(7, "boom"); });
    } catch (Oops& e) {
      CHECK(e.code == 7 && std::strcmp(e.reason, "boom") == 0);
    }
  });
  r.run("fail_system", [&]() {
    try {
      expect_raises([&]() { echo->fail_system(); });
    } catch (CORBA::BAD_PARAM& e) {
      CHECK(e.minor() == 42 && e.completed() == CORBA::COMPLETED_NO);
    }
  });

  // --- attributes ---
  r.run("attribute counter", [&]() {
    echo->counter(41);
    CHECK(echo->counter() == 41);
    echo->counter(-1);
    CHECK(echo->counter() == -1);
  });
  r.run("attribute name", [&]() {
    CORBA::String_var n = echo->name();
    CHECK(std::strcmp(n, "rustorb-interop-echo") == 0);
  });

  // --- object references ---
  r.run("self_ref", [&]() {
    InteropTest::Echo_var ref = echo->self_ref();
    CHECK(ref->_is_equivalent(echo));
    CHECK(ref->echo_long(5) == 5);
  });
  r.run("echo_ref", [&]() {
    InteropTest::Echo_var a = echo->echo_ref(echo);
    CHECK(a->_is_equivalent(echo));
    InteropTest::Echo_var b = echo->echo_ref(InteropTest::Echo::_nil());
    CHECK(b->_is_equivalent(echo));
  });
  r.run("is_nil", [&]() {
    CHECK(echo->is_nil(InteropTest::Echo::_nil()));
    CHECK(!echo->is_nil(echo));
  });

  // --- callbacks ---
  CallbackImpl* cb_servant = new CallbackImpl;
  PortableServer::ObjectId_var oid = PortableServer::string_to_ObjectId("Callback");
  poa->activate_object_with_id(oid, cb_servant);
  CORBA::Object_var cb_obj = poa->id_to_reference(oid);
  Callback_var cb = Callback::_narrow(cb_obj);

  r.run("call_back", [&]() {
    CORBA::String_var s = echo->call_back(cb, "hi");
    CHECK(std::strcmp(s, "ack:hi") == 0);
  });
  r.run("register_callback/trigger", [&]() {
    echo->register_callback(cb);
    CORBA::String_var s = echo->trigger("later");
    CHECK(std::strcmp(s, "ack:later") == 0);
    CHECK(!cb_servant->received.empty() && cb_servant->received.back() == "later");
  });

  // --- oneway ---
  // Last, because the number of posted() polls depends on timing and would
  // otherwise change the messages after it (request ids, and the stale bytes
  // omniORB leaves in alignment padding), which breaks the golden comparison.
  r.run("oneway post", [&]() {
    StringSeq_var drop = echo->posted();
    std::vector<std::string> sent = {"one", "two", "three"};
    for (auto& m : sent) echo->post(m.c_str());
    std::vector<std::string> got;
    auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(5);
    while (got.size() < sent.size() && std::chrono::steady_clock::now() < deadline) {
      StringSeq_var seq = echo->posted();
      for (CORBA::ULong i = 0; i < seq->length(); ++i) got.push_back(seq[i].in());
      std::this_thread::sleep_for(std::chrono::milliseconds(50));
    }
    // Oneway requests may be dispatched concurrently, so order is not checked.
    std::sort(got.begin(), got.end());
    std::sort(sent.begin(), sent.end());
    CHECK(got == sent);
  });
}

// The stage 2 suite (AnyTest.idl) against the AnyEcho object at opt.name;
// the server's Echo object is at opt.name + "-Echo".
static int run_any(CORBA::ORB_ptr orb, CosNaming::NamingContext_ptr root, const Options& opt) {
  CosNaming::Name name = to_name(opt.name);
  CORBA::Object_var obj = retry("the AnyEcho server", [&]() { return root->resolve(name); },
                                opt.timeout);
  AnyTest::AnyEcho_var echo = AnyTest::AnyEcho::_narrow(obj);
  if (CORBA::is_nil(echo)) {
    std::cout << opt.name << " is not an AnyTest::AnyEcho" << std::endl;
    return 1;
  }
  retry("the AnyEcho server", [&]() { return echo->_non_existent(); }, opt.timeout);

  Runner r;
  anytest::run_cases(r, echo, [](bool cond, const std::string& what) {
    if (!cond) throw CheckFailed{what};
  });

  if (opt.shutdown_server) {
    CORBA::Object_var o = root->resolve(to_name(opt.name + "-Echo"));
    InteropTest::Echo_var server = InteropTest::Echo::_narrow(o);
    server->shutdown();
  }

  std::cout << "\n" << r.passed() << " passed, " << r.failed().size() << " failed" << std::endl;
  if (!r.failed().empty()) {
    std::cout << "failed: ";
    for (size_t i = 0; i < r.failed().size(); ++i)
      std::cout << (i ? ", " : "") << r.failed()[i];
    std::cout << std::endl;
  }
  orb->destroy();
  return r.failed().empty() ? 0 : 1;
}

// The valuetype suite (ValueTest.idl) against the ValueEcho object at
// opt.name.
static int run_value(CORBA::ORB_ptr orb, CosNaming::NamingContext_ptr root, const Options& opt) {
  valuetest::register_factories(orb);
  CosNaming::Name name = to_name(opt.name);
  CORBA::Object_var obj = retry("the ValueEcho server", [&]() { return root->resolve(name); },
                                opt.timeout);
  ValueTest::ValueEcho_var echo = ValueTest::ValueEcho::_narrow(obj);
  if (CORBA::is_nil(echo)) {
    std::cout << opt.name << " is not a ValueTest::ValueEcho" << std::endl;
    return 1;
  }
  retry("the ValueEcho server", [&]() { return echo->_non_existent(); }, opt.timeout);

  Runner r;
  valuetest::run_cases(r, echo, [](bool cond, const std::string& what) {
    if (!cond) throw CheckFailed{what};
  });

  if (opt.shutdown_server) echo->shutdown();

  std::cout << "\n" << r.passed() << " passed, " << r.failed().size() << " failed" << std::endl;
  if (!r.failed().empty()) {
    std::cout << "failed: ";
    for (size_t i = 0; i < r.failed().size(); ++i)
      std::cout << (i ? ", " : "") << r.failed()[i];
    std::cout << std::endl;
  }
  orb->destroy();
  return r.failed().empty() ? 0 : 1;
}

int main(int argc, char** argv) {
  std::vector<std::string> orb_args;
  Options opt = parse_args(argc, argv, orb_args);
  try {
    CORBA::ORB_var orb = orb_init(orb_args);
    PortableServer::POA_var poa = persistent_poa(orb, "InteropTestClient");

    CosNaming::NamingContext_var root = root_context(orb, opt.timeout);
    if (opt.suite == "any") return run_any(orb, root, opt);
    if (opt.suite == "value") return run_value(orb, root, opt);
    CosNaming::Name name = to_name(opt.name);
    CORBA::Object_var obj = retry("the Echo server", [&]() { return root->resolve(name); },
                                  opt.timeout);
    InteropTest::Echo_var echo = InteropTest::Echo::_narrow(obj);
    if (CORBA::is_nil(echo)) {
      std::cout << opt.name << " is not an InteropTest::Echo" << std::endl;
      return 1;
    }
    retry("the Echo server", [&]() { return echo->_non_existent(); }, opt.timeout);

    Runner r;
    run_cases(r, echo, poa);

    if (opt.shutdown_server) echo->shutdown();

    std::cout << "\n" << r.passed() << " passed, " << r.failed().size() << " failed" << std::endl;
    if (!r.failed().empty()) {
      std::cout << "failed: ";
      for (size_t i = 0; i < r.failed().size(); ++i)
        std::cout << (i ? ", " : "") << r.failed()[i];
      std::cout << std::endl;
    }
    orb->destroy();
    return r.failed().empty() ? 0 : 1;
  } catch (CORBA::Exception& e) {
    std::cerr << "CORBA exception: " << e._name() << std::endl;
    return 1;
  }
}

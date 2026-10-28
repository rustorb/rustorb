// The test cases of the rustorb C++ client, the same as those of the other
// clients (case names included), on the IDL to C++11 mapping.

#ifndef RUSTORBCPP_CASES_H
#define RUSTORBCPP_CASES_H

#include <algorithm>
#include <sstream>
#include <chrono>
#include <cmath>
#include <functional>
#include <iostream>
#include <mutex>
#include <string>
#include <thread>
#include <vector>

#include "InteropTest.hh"

struct CheckFailed {
  std::string message;
};

#define CHECK(cond)                                                          \
  do {                                                                       \
    if (!(cond))                                                             \
      throw CheckFailed{std::string("line ") + std::to_string(__LINE__) + \
                        ": " #cond};                                         \
  } while (0)

class Runner {
 public:
  void run(const std::string& name, const std::function<void()>& fn) {
    try {
      fn();
      ++passed_;
      std::cout << "ok   " << name << std::endl;
      return;
    } catch (const CheckFailed& e) {
      fail(name, e.message);
    } catch (const CORBA::Exception& e) {
      std::ostringstream os;
      os << "unexpected " << e;
      fail(name, os.str());
    } catch (const std::exception& e) {
      fail(name, e.what());
    }
  }

  int passed() const { return passed_; }
  const std::vector<std::string>& failed() const { return failed_; }

  void summary() const {
    std::cout << "\n" << passed_ << " passed, " << failed_.size() << " failed" << std::endl;
    if (!failed_.empty()) {
      std::cout << "failed: ";
      for (size_t i = 0; i < failed_.size(); ++i) std::cout << (i ? ", " : "") << failed_[i];
      std::cout << std::endl;
    }
  }

 private:
  void fail(const std::string& name, const std::string& why) {
    failed_.push_back(name);
    std::cout << "FAIL " << name << ": " << why << std::endl;
  }

  int passed_ = 0;
  std::vector<std::string> failed_;
};

template <typename F> void expect_raises(F fn) {
  fn();  // must throw; the caller catches the expected type
  throw CheckFailed{"expected exception was not raised"};
}

class CallbackImpl : public virtual CORBA::servant_traits<InteropTest::Callback>::base_type {
 public:
  std::string notify(const std::string& message) override {
    std::lock_guard<std::mutex> lock(mutex_);
    received_.push_back(message);
    return "ack:" + message;
  }
  std::vector<std::string> received() {
    std::lock_guard<std::mutex> lock(mutex_);
    return received_;
  }

 private:
  std::mutex mutex_;
  std::vector<std::string> received_;
};

inline InteropTest::Record make_record(int32_t id, const std::string& name, InteropTest::Color c,
                                       InteropTest::Point pos, std::vector<int32_t> values,
                                       bool active) {
  return InteropTest::Record(id, name, c, pos, values, active);
}

template <typename T, typename F>
void echo_values(Runner& r, const char* op, std::vector<T> values, F call) {
  for (size_t i = 0; i < values.size(); ++i) {
    T v = values[i];
    r.run(std::string(op) + "[" + std::to_string(i) + "]", [&]() { CHECK(call(v) == v); });
  }
}

// Runs every case against echo; poa is where the callback object goes.
inline void run_cases(Runner& r, IDL::traits<InteropTest::Echo>::ref_type echo,
                      IDL::traits<PortableServer::POA>::ref_type poa) {
  using namespace InteropTest;

  // --- basic types ---
  echo_values<bool>(r, "echo_boolean", {true, false}, [&](bool v) { return echo->echo_boolean(v); });
  echo_values<uint8_t>(r, "echo_octet", {0, 0x7f, 0xff},
                       [&](uint8_t v) { return echo->echo_octet(v); });
  echo_values<char>(r, "echo_char", {'a', '\0', '~'}, [&](char v) { return echo->echo_char(v); });
  echo_values<wchar_t>(r, "echo_wchar", {L'a', L'あ', L'é'},
                       [&](wchar_t v) { return echo->echo_wchar(v); });
  echo_values<int16_t>(r, "echo_short", {0, -32768, 32767},
                       [&](int16_t v) { return echo->echo_short(v); });
  echo_values<uint16_t>(r, "echo_ushort", {0, 65535},
                        [&](uint16_t v) { return echo->echo_ushort(v); });
  echo_values<int32_t>(r, "echo_long", {0, -2147483647 - 1, 2147483647, MAGIC},
                       [&](int32_t v) { return echo->echo_long(v); });
  echo_values<uint32_t>(r, "echo_ulong", {0, 4294967295u},
                        [&](uint32_t v) { return echo->echo_ulong(v); });
  echo_values<int64_t>(r, "echo_longlong", {0, -9223372036854775807LL - 1, 9223372036854775807LL},
                       [&](int64_t v) { return echo->echo_longlong(v); });
  echo_values<uint64_t>(r, "echo_ulonglong", {0, 18446744073709551615ULL},
                        [&](uint64_t v) { return echo->echo_ulonglong(v); });
  echo_values<double>(r, "echo_double", {0.0, -1.5, 1e300, std::ldexp(1.0, -1074)},
                      [&](double v) { return echo->echo_double(v); });
  // std::string holds UTF-8, rustorb's native code set; "café" is the
  // same text the Python clients send.
  echo_values<std::string>(r, "echo_string", {"", "hello", std::string(10000, 'x'), "caf\xc3\xa9"},
                           [&](const std::string& v) { return echo->echo_string(v); });
  // U+1F980 is sent as a surrogate pair, as the other clients do.
  echo_values<std::wstring>(r, "echo_wstring",
                            {L"", L"hello", L"こんにちは", L"\U0001F980"},
                            [&](const std::wstring& v) { return echo->echo_wstring(v); });
  echo_values<float>(r, "echo_float", {0.0f, -1.5f, 3.4028234663852886e38f, 1.1754943508222875e-38f},
                     [&](float v) { return echo->echo_float(v); });

  // --- constructed types ---
  Point p(1.25, -2.5);
  Record rec = make_record(7, "seven", Color::GREEN, p, {1, -2, 3}, true);
  Record rec2 = make_record(-1, "", Color::RED, Point(0.0, 0.0), {}, false);

  r.run("echo_color", [&]() {
    for (Color c : {Color::RED, Color::GREEN, Color::BLUE}) CHECK(echo->echo_color(c) == c);
  });
  r.run("echo_point", [&]() { CHECK(echo->echo_point(p) == p); });
  r.run("echo_record", [&]() { CHECK(echo->echo_record(rec) == rec); });
  r.run("echo_records", [&]() {
    RecordSeq seq{rec, rec2, rec};
    CHECK(echo->echo_records(seq) == seq);
  });
  r.run("echo_records[empty]", [&]() { CHECK(echo->echo_records(RecordSeq()).empty()); });
  // Large enough to be sent in several fragments, with doubles after each
  // fragment boundary; record i has a name of varying length so that the
  // doubles land at every alignment.
  r.run("echo_records[large]", [&]() {
    const int32_t n = 400;
    const Color colors[] = {Color::RED, Color::GREEN, Color::BLUE};
    RecordSeq seq;
    for (int32_t i = 0; i < n; ++i) {
      seq.push_back(make_record(i, "r" + std::to_string(i), colors[i % 3],
                                Point(i + 0.5, -0.25 * i), {i, i + 1}, i % 2 == 0));
    }
    CHECK(echo->echo_records(seq) == seq);
  });

  std::vector<Value> values(7);
  values[0].l(-5);
  values[1].s("union");
  values[2].p(p);
  values[3].seq(LongSeq{1, 2}, 4);
  values[4].seq(LongSeq(), 5);
  values[5].flag(true, 99);
  values[6].flag(false, 0);
  for (size_t i = 0; i < values.size(); ++i) {
    r.run("echo_value[" + std::to_string(i) + "]",
          [&]() { CHECK(echo->echo_value(values[i]) == values[i]); });
  }

  std::vector<Shade> shades(3);
  shades[0].level(200);
  shades[1].label("leaf");
  shades[2]._default();
  shades[2]._d(Color::BLUE);
  for (size_t i = 0; i < shades.size(); ++i) {
    r.run("echo_shade[" + std::to_string(i) + "]",
          [&]() { CHECK(echo->echo_shade(shades[i]) == shades[i]); });
  }

  r.run("echo_long_array", [&]() {
    LongArray a = {1, -1, 2147483647, 0};
    CHECK(echo->echo_long_array(a) == a);
  });
  r.run("echo_matrix", [&]() {
    Matrix m = {{{1.0, 2.0, 3.0}, {-4.0, 0.5, 1e-9}}};
    CHECK(echo->echo_matrix(m) == m);
  });
  r.run("echo_long_seq", [&]() {
    LongSeq seq;
    for (int32_t i = 0; i < 1000; ++i) seq.push_back(i - 500);
    CHECK(echo->echo_long_seq(seq) == seq);
  });
  r.run("echo_long_seq[empty]", [&]() { CHECK(echo->echo_long_seq(LongSeq()).empty()); });
  r.run("echo_string_seq", [&]() {
    StringSeq seq{"a", "", "ccc"};
    CHECK(echo->echo_string_seq(seq) == seq);
  });
  r.run("echo_octets", [&]() {
    OctetSeq seq;
    for (int i = 0; i < 256; ++i) seq.push_back(static_cast<uint8_t>(i));
    CHECK(echo->echo_octets(seq) == seq);
  });

  // --- parameter modes ---
  r.run("swap", [&]() {
    int32_t a = 3, b = -10;
    CHECK(echo->swap(a, b) == -7);
    CHECK(a == -10 && b == 3);
  });
  r.run("divide", [&]() {
    int32_t rem = 0;
    CHECK(echo->divide(17, 5, rem) == 3 && rem == 2);
    CHECK(echo->divide(-17, 5, rem) == -3 && rem == -2);
    try {
      expect_raises([&]() { echo->divide(1, 0, rem); });
    } catch (const DivideByZero&) {
    }
  });
  r.run("make", [&]() {
    Point pt;
    Record rc;
    echo->make(12, pt, rc);
    CHECK(pt == Point(12.0, -12.0));
    CHECK(rc == make_record(12, "r12", Color::BLUE, Point(12.0, -12.0), {12, 13}, true));
  });

  // --- exceptions ---
  r.run("fail", [&]() {
    try {
      expect_raises([&]() { echo->fail(7, "boom"); });
    } catch (const Oops& e) {
      CHECK(e.code() == 7 && e.reason() == "boom");
    }
  });
  r.run("fail_system", [&]() {
    try {
      expect_raises([&]() { echo->fail_system(); });
    } catch (const CORBA::BAD_PARAM& e) {
      CHECK(e.minor() == 42 && e.completed() == CORBA::CompletionStatus::COMPLETED_NO);
    }
  });

  // --- attributes ---
  r.run("attribute counter", [&]() {
    echo->counter(41);
    CHECK(echo->counter() == 41);
    echo->counter(-1);
    CHECK(echo->counter() == -1);
  });
  r.run("attribute name", [&]() { CHECK(echo->name() == "rustorb-interop-echo"); });

  // --- object references ---
  r.run("self_ref", [&]() {
    auto ref = echo->self_ref();
    CHECK(ref->_is_equivalent(echo));
    CHECK(ref->echo_long(5) == 5);
  });
  r.run("echo_ref", [&]() {
    auto a = echo->echo_ref(echo);
    CHECK(a->_is_equivalent(echo));
    auto b = echo->echo_ref(nullptr);
    CHECK(b->_is_equivalent(echo));
  });
  r.run("is_nil", [&]() {
    CHECK(echo->is_nil(nullptr));
    CHECK(!echo->is_nil(echo));
  });

  // --- callbacks ---
  auto cb_servant = CORBA::make_reference<CallbackImpl>();
  auto oid = PortableServer::string_to_ObjectId("Callback");
  poa->activate_object_with_id(oid, cb_servant);
  auto cb = IDL::traits<Callback>::narrow(poa->id_to_reference(oid));

  r.run("call_back", [&]() { CHECK(echo->call_back(cb, "hi") == "ack:hi"); });
  r.run("register_callback/trigger", [&]() {
    echo->register_callback(cb);
    CHECK(echo->trigger("later") == "ack:later");
    auto received = cb_servant->received();
    CHECK(!received.empty() && received.back() == "later");
  });

  // --- oneway ---
  // Last, as in the other clients (the number of posted() polls depends on
  // timing).
  r.run("oneway post", [&]() {
    echo->posted();
    std::vector<std::string> sent = {"one", "two", "three"};
    for (auto& m : sent) echo->post(m);
    std::vector<std::string> got;
    auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(5);
    while (got.size() < sent.size() && std::chrono::steady_clock::now() < deadline) {
      for (auto& m : echo->posted()) got.push_back(m);
      std::this_thread::sleep_for(std::chrono::milliseconds(50));
    }
    // Oneway requests may be dispatched concurrently, so order is not checked.
    std::sort(got.begin(), got.end());
    std::sort(sent.begin(), sent.end());
    CHECK(got == sent);
  });
}

#endif

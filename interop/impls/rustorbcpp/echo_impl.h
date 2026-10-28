// The InteropTest::Echo servant of the rustorb C++ server, on the IDL to
// C++11 mapping. Behaves as the comments in idl/InteropTest.idl say.

#ifndef RUSTORBCPP_ECHO_IMPL_H
#define RUSTORBCPP_ECHO_IMPL_H

#include <mutex>
#include <string>
#include <utility>
#include <vector>

#include "InteropTest.hh"

class EchoImpl : public virtual CORBA::servant_traits<InteropTest::Echo>::base_type {
 public:
  explicit EchoImpl(IDL::traits<CORBA::ORB>::ref_type orb) : orb_(std::move(orb)) {}

  // --- basic types ---
  bool echo_boolean(bool v) override { return v; }
  uint8_t echo_octet(uint8_t v) override { return v; }
  char echo_char(char v) override { return v; }
  wchar_t echo_wchar(wchar_t v) override { return v; }
  int16_t echo_short(int16_t v) override { return v; }
  uint16_t echo_ushort(uint16_t v) override { return v; }
  int32_t echo_long(int32_t v) override { return v; }
  uint32_t echo_ulong(uint32_t v) override { return v; }
  int64_t echo_longlong(int64_t v) override { return v; }
  uint64_t echo_ulonglong(uint64_t v) override { return v; }
  float echo_float(float v) override { return v; }
  double echo_double(double v) override { return v; }
  std::string echo_string(const std::string& v) override { return v; }
  std::wstring echo_wstring(const std::wstring& v) override { return v; }

  // --- constructed types ---
  InteropTest::Color echo_color(InteropTest::Color v) override { return v; }
  InteropTest::Point echo_point(const InteropTest::Point& v) override { return v; }
  InteropTest::Record echo_record(const InteropTest::Record& v) override { return v; }
  InteropTest::RecordSeq echo_records(const InteropTest::RecordSeq& v) override { return v; }
  InteropTest::Value echo_value(const InteropTest::Value& v) override { return v; }
  InteropTest::Shade echo_shade(const InteropTest::Shade& v) override { return v; }
  InteropTest::LongArray echo_long_array(const InteropTest::LongArray& v) override { return v; }
  InteropTest::Matrix echo_matrix(const InteropTest::Matrix& v) override { return v; }
  InteropTest::LongSeq echo_long_seq(const InteropTest::LongSeq& v) override { return v; }
  InteropTest::StringSeq echo_string_seq(const InteropTest::StringSeq& v) override { return v; }
  InteropTest::OctetSeq echo_octets(const InteropTest::OctetSeq& v) override { return v; }

  // --- parameter modes ---
  int32_t swap(int32_t& a, int32_t& b) override {
    int32_t sum = a + b;
    std::swap(a, b);
    return sum;
  }

  int32_t divide(int32_t a, int32_t b, int32_t& remainder) override {
    if (b == 0) throw InteropTest::DivideByZero();
    remainder = a % b;
    return a / b;
  }

  void make(int32_t x, InteropTest::Point& p, InteropTest::Record& r) override {
    p = InteropTest::Point(x, -x);
    r = InteropTest::Record(x, "r" + std::to_string(x), InteropTest::Color::BLUE, p, {x, x + 1},
                            true);
  }

  // --- exceptions ---
  void fail(int32_t code, const std::string& reason) override {
    throw InteropTest::Oops(code, reason);
  }
  void fail_system() override {
    throw CORBA::BAD_PARAM(42, CORBA::CompletionStatus::COMPLETED_NO);
  }

  // --- attributes ---
  int32_t counter() override {
    std::lock_guard<std::mutex> lock(mutex_);
    return counter_;
  }
  void counter(int32_t v) override {
    std::lock_guard<std::mutex> lock(mutex_);
    counter_ = v;
  }
  std::string name() override { return "rustorb-interop-echo"; }

  // --- oneway ---
  void post(const std::string& message) override {
    std::lock_guard<std::mutex> lock(mutex_);
    posted_.push_back(message);
  }

  InteropTest::StringSeq posted() override {
    std::lock_guard<std::mutex> lock(mutex_);
    InteropTest::StringSeq result;
    result.swap(posted_);
    return result;
  }

  // --- object references ---
  IDL::traits<InteropTest::Echo>::ref_type self_ref() override { return _this(); }

  IDL::traits<InteropTest::Echo>::ref_type echo_ref(
      IDL::traits<InteropTest::Echo>::ref_type other) override {
    if (!other) return _this();
    return other;
  }

  bool is_nil(IDL::traits<InteropTest::Echo>::ref_type other) override { return !other; }

  // --- callbacks ---
  std::string call_back(IDL::traits<InteropTest::Callback>::ref_type cb,
                        const std::string& message) override {
    return cb->notify(message);
  }

  void register_callback(IDL::traits<InteropTest::Callback>::ref_type cb) override {
    std::lock_guard<std::mutex> lock(mutex_);
    callback_ = cb;
  }

  std::string trigger(const std::string& message) override {
    IDL::traits<InteropTest::Callback>::ref_type cb;
    {
      std::lock_guard<std::mutex> lock(mutex_);
      cb = callback_;
    }
    if (!cb) throw InteropTest::Oops(1, "no callback");
    return cb->notify(message);
  }

  // --- lifecycle ---
  void shutdown() override { orb_->shutdown(false); }

 private:
  IDL::traits<CORBA::ORB>::ref_type orb_;
  std::mutex mutex_;
  int32_t counter_ = 0;
  std::vector<std::string> posted_;
  IDL::traits<InteropTest::Callback>::ref_type callback_;
};

#endif

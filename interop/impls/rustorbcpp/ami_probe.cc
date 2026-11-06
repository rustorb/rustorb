// The rustorb C++ side of run-ami-probe.sh: callback AMI (docs/ami.md)
// against the Echo server of any implementation, printing the same lines
// as impls/rustorbpy/ami_probe.py. Built by build.sh, from the stubs of
// InteropTest.idl generated with -Wbami.
//
//   ami_probe REFERENCE [ORB options]
//
// REFERENCE names the Echo object, such as corbaname::127.0.0.1:2809#ami/Echo.

#include <chrono>
#include <condition_variable>
#include <deque>
#include <iostream>
#include <mutex>
#include <sstream>
#include <string>
#include <thread>
#include <vector>

#include "InteropTest.hh"

using Holder = IDL::traits<Messaging::ExceptionHolder>::ref_type;
using CORBA::CompletionStatus;

static const auto WAIT = std::chrono::seconds(10);

static const char* completed(CompletionStatus c) {
  switch (c) {
    case CompletionStatus::COMPLETED_YES: return "COMPLETED_YES";
    case CompletionStatus::COMPLETED_NO: return "COMPLETED_NO";
    case CompletionStatus::COMPLETED_MAYBE: return "COMPLETED_MAYBE";
  }
  return "?";
}

// What raise_exception() throws, as a probe line.
static std::string describe(const Holder& holder) {
  if (!holder) return "nil holder";
  std::ostringstream out;
  try {
    holder->raise_exception();
    return "nothing raised";
  } catch (const InteropTest::Oops& e) {
    out << "Oops " << e.code() << " " << e.reason();
  } catch (const InteropTest::DivideByZero&) {
    out << "DivideByZero";
  } catch (const CORBA::SystemException& e) {
    out << e._name() << " " << e.minor() << " " << completed(e.completed());
  } catch (const std::exception& e) {
    out << "unexpected " << e.what();
  }
  return out.str();
}

// Every operation of the handler, each reporting that it was not expected.
class Unexpected : public CORBA::servant_traits<InteropTest::AMI_EchoHandler>::base_type {
 public:
  virtual void unexpected(const char* op) = 0;

  void echo_boolean(bool) override { unexpected("echo_boolean"); }
  void echo_boolean_excep(Holder) override { unexpected("echo_boolean_excep"); }
  void echo_octet(uint8_t) override { unexpected("echo_octet"); }
  void echo_octet_excep(Holder) override { unexpected("echo_octet_excep"); }
  void echo_char(char) override { unexpected("echo_char"); }
  void echo_char_excep(Holder) override { unexpected("echo_char_excep"); }
  void echo_wchar(wchar_t) override { unexpected("echo_wchar"); }
  void echo_wchar_excep(Holder) override { unexpected("echo_wchar_excep"); }
  void echo_short(int16_t) override { unexpected("echo_short"); }
  void echo_short_excep(Holder) override { unexpected("echo_short_excep"); }
  void echo_ushort(uint16_t) override { unexpected("echo_ushort"); }
  void echo_ushort_excep(Holder) override { unexpected("echo_ushort_excep"); }
  void echo_long(int32_t) override { unexpected("echo_long"); }
  void echo_long_excep(Holder) override { unexpected("echo_long_excep"); }
  void echo_ulong(uint32_t) override { unexpected("echo_ulong"); }
  void echo_ulong_excep(Holder) override { unexpected("echo_ulong_excep"); }
  void echo_longlong(int64_t) override { unexpected("echo_longlong"); }
  void echo_longlong_excep(Holder) override { unexpected("echo_longlong_excep"); }
  void echo_ulonglong(uint64_t) override { unexpected("echo_ulonglong"); }
  void echo_ulonglong_excep(Holder) override { unexpected("echo_ulonglong_excep"); }
  void echo_float(float) override { unexpected("echo_float"); }
  void echo_float_excep(Holder) override { unexpected("echo_float_excep"); }
  void echo_double(double) override { unexpected("echo_double"); }
  void echo_double_excep(Holder) override { unexpected("echo_double_excep"); }
  void echo_string(const std::string&) override { unexpected("echo_string"); }
  void echo_string_excep(Holder) override { unexpected("echo_string_excep"); }
  void echo_wstring(const std::wstring&) override { unexpected("echo_wstring"); }
  void echo_wstring_excep(Holder) override { unexpected("echo_wstring_excep"); }
  void echo_color(::InteropTest::Color) override { unexpected("echo_color"); }
  void echo_color_excep(Holder) override { unexpected("echo_color_excep"); }
  void echo_point(const ::InteropTest::Point&) override { unexpected("echo_point"); }
  void echo_point_excep(Holder) override { unexpected("echo_point_excep"); }
  void echo_record(const ::InteropTest::Record&) override { unexpected("echo_record"); }
  void echo_record_excep(Holder) override { unexpected("echo_record_excep"); }
  void echo_records(const ::InteropTest::RecordSeq&) override { unexpected("echo_records"); }
  void echo_records_excep(Holder) override { unexpected("echo_records_excep"); }
  void echo_value(const ::InteropTest::Value&) override { unexpected("echo_value"); }
  void echo_value_excep(Holder) override { unexpected("echo_value_excep"); }
  void echo_shade(const ::InteropTest::Shade&) override { unexpected("echo_shade"); }
  void echo_shade_excep(Holder) override { unexpected("echo_shade_excep"); }
  void echo_long_array(const ::InteropTest::LongArray&) override { unexpected("echo_long_array"); }
  void echo_long_array_excep(Holder) override { unexpected("echo_long_array_excep"); }
  void echo_matrix(const ::InteropTest::Matrix&) override { unexpected("echo_matrix"); }
  void echo_matrix_excep(Holder) override { unexpected("echo_matrix_excep"); }
  void echo_long_seq(const ::InteropTest::LongSeq&) override { unexpected("echo_long_seq"); }
  void echo_long_seq_excep(Holder) override { unexpected("echo_long_seq_excep"); }
  void echo_string_seq(const ::InteropTest::StringSeq&) override { unexpected("echo_string_seq"); }
  void echo_string_seq_excep(Holder) override { unexpected("echo_string_seq_excep"); }
  void echo_octets(const ::InteropTest::OctetSeq&) override { unexpected("echo_octets"); }
  void echo_octets_excep(Holder) override { unexpected("echo_octets_excep"); }
  void swap(int32_t, int32_t, int32_t) override { unexpected("swap"); }
  void swap_excep(Holder) override { unexpected("swap_excep"); }
  void divide(int32_t, int32_t) override { unexpected("divide"); }
  void divide_excep(Holder) override { unexpected("divide_excep"); }
  void make(const ::InteropTest::Point&, const ::InteropTest::Record&) override { unexpected("make"); }
  void make_excep(Holder) override { unexpected("make_excep"); }
  void fail() override { unexpected("fail"); }
  void fail_excep(Holder) override { unexpected("fail_excep"); }
  void fail_system() override { unexpected("fail_system"); }
  void fail_system_excep(Holder) override { unexpected("fail_system_excep"); }
  void get_counter(int32_t) override { unexpected("get_counter"); }
  void get_counter_excep(Holder) override { unexpected("get_counter_excep"); }
  void set_counter() override { unexpected("set_counter"); }
  void set_counter_excep(Holder) override { unexpected("set_counter_excep"); }
  void get_name(const std::string&) override { unexpected("get_name"); }
  void get_name_excep(Holder) override { unexpected("get_name_excep"); }
  void posted(const ::InteropTest::StringSeq&) override { unexpected("posted"); }
  void posted_excep(Holder) override { unexpected("posted_excep"); }
  void self_ref(::IDL::traits<::InteropTest::Echo>::ref_type) override { unexpected("self_ref"); }
  void self_ref_excep(Holder) override { unexpected("self_ref_excep"); }
  void echo_ref(::IDL::traits<::InteropTest::Echo>::ref_type) override { unexpected("echo_ref"); }
  void echo_ref_excep(Holder) override { unexpected("echo_ref_excep"); }
  void is_nil(bool) override { unexpected("is_nil"); }
  void is_nil_excep(Holder) override { unexpected("is_nil_excep"); }
  void call_back(const std::string&) override { unexpected("call_back"); }
  void call_back_excep(Holder) override { unexpected("call_back_excep"); }
  void register_callback() override { unexpected("register_callback"); }
  void register_callback_excep(Holder) override { unexpected("register_callback_excep"); }
  void trigger(const std::string&) override { unexpected("trigger"); }
  void trigger_excep(Holder) override { unexpected("trigger_excep"); }
};

// Puts one line into a queue for each call.
class Handler : public Unexpected {
 public:
  void unexpected(const char* op) override { put(std::string("unexpected ") + op); }
  void echo_long(int32_t ret) override { put("echo_long: " + std::to_string(ret)); }
  void echo_string(const std::string& ret) override { put("echo_string: " + ret); }
  void swap(int32_t ret, int32_t a, int32_t b) override {
    put("swap: " + std::to_string(ret) + " " + std::to_string(a) + " " + std::to_string(b));
  }
  void divide(int32_t ret, int32_t remainder) override {
    put("divide: " + std::to_string(ret) + " " + std::to_string(remainder));
  }
  void divide_excep(Holder h) override { put("divide_excep: " + describe(h)); }
  void fail_excep(Holder h) override { put("fail_excep: " + describe(h)); }
  void fail_system_excep(Holder h) override { put("fail_system_excep: " + describe(h)); }
  void set_counter() override { put("set_counter"); }
  void get_counter(int32_t ret) override { put("get_counter: " + std::to_string(ret)); }
  void get_name(const std::string& ret) override { put("get_name: " + ret); }
  void trigger_excep(Holder h) override { put("trigger_excep: " + describe(h)); }

  // The next line, or "WHAT: no reply" after WAIT.
  std::string next(const std::string& what) {
    std::unique_lock<std::mutex> lock(mutex_);
    if (!cond_.wait_for(lock, WAIT, [&] { return !lines_.empty(); })) return what + ": no reply";
    std::string line = std::move(lines_.front());
    lines_.pop_front();
    return line;
  }

 private:
  void put(std::string line) {
    std::lock_guard<std::mutex> lock(mutex_);
    lines_.push_back(std::move(line));
    cond_.notify_all();
  }

  std::mutex mutex_;
  std::condition_variable cond_;
  std::deque<std::string> lines_;
};

// Answers after 2 s, longer than the call timeout of the last case.
class SlowCallback : public CORBA::servant_traits<InteropTest::Callback>::base_type {
 public:
  std::string notify(const std::string& message) override {
    std::this_thread::sleep_for(std::chrono::seconds(2));
    return "ack:" + message;
  }
};

int main(int argc, char** argv) {
  if (argc < 2) {
    std::cerr << "usage: ami_probe REFERENCE [ORB options]" << std::endl;
    return 2;
  }
  std::vector<std::string> args{argv[0], "-ORBendPoint", "giop:tcp:127.0.0.1:"};
  for (int i = 2; i < argc; ++i) args.push_back(argv[i]);
  std::vector<char*> orb_argv;
  for (auto& a : args) orb_argv.push_back(&a[0]);
  orb_argv.push_back(nullptr);
  int orb_argc = static_cast<int>(args.size());
  int status = 0;
  try {
    auto orb = CORBA::ORB_init(orb_argc, orb_argv.data());
    auto poa = IDL::traits<PortableServer::POA>::narrow(orb->resolve_initial_references("RootPOA"));
    poa->the_POAManager()->activate();
    auto echo = IDL::traits<InteropTest::Echo>::narrow(orb->string_to_object(argv[1]));
    auto handler = CORBA::make_reference<Handler>();
    auto h = IDL::traits<InteropTest::AMI_EchoHandler>::narrow(
        poa->id_to_reference(poa->activate_object(handler)));
    auto show = [&](const std::string& what) {
      std::string line = handler->next(what);
      if (line == what + ": no reply") status = 1;
      std::cout << line << std::endl;
    };

    echo->sendc_echo_long(h, 42);
    show("echo_long");
    echo->sendc_echo_string(h, "hello AMI");
    show("echo_string");
    echo->sendc_swap(h, 1, 2);
    show("swap");
    echo->sendc_divide(h, 7, 2);
    show("divide");
    echo->sendc_divide(h, 1, 0);
    show("divide_excep");
    echo->sendc_fail(h, 7, "bad");
    show("fail_excep");
    echo->sendc_fail_system(h);
    show("fail_system_excep");
    echo->sendc_set_counter(h, 5);
    show("set_counter");
    echo->sendc_get_counter(h);
    show("get_counter");
    echo->sendc_get_name(h);
    show("get_name");
    // A nil handler: the request is sent, the reply dropped.
    echo->sendc_set_counter(nullptr, 9);
    auto deadline = std::chrono::steady_clock::now() + WAIT;
    while (echo->counter() != 9 && std::chrono::steady_clock::now() < deadline) {
      std::this_thread::sleep_for(std::chrono::milliseconds(10));
    }
    std::cout << "nil handler: counter " << echo->counter() << std::endl;
    // The server calls the slow callback; the call times out first.
    auto cb = IDL::traits<InteropTest::Callback>::narrow(
        poa->id_to_reference(poa->activate_object(CORBA::make_reference<SlowCallback>())));
    echo->register_callback(cb);
    rustorb::setClientCallTimeout(echo, 500);
    echo->sendc_trigger(h, "late");
    rustorb::setClientCallTimeout(echo, 0);
    show("trigger_excep");
    // Let the server's call of the callback finish before going away.
    std::this_thread::sleep_for(std::chrono::seconds(2));
    orb->destroy();
  } catch (const CORBA::Exception& e) {
    std::cout << "CORBA exception: " << e << std::endl;
    return 1;
  }
  return status;
}

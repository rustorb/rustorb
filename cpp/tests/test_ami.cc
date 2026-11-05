// Callback AMI in the C++ binding (docs/ami.md): the sendc_ operations of
// code generated with rustorb-idl -bc++11 -Wbami, the AMI_<I>Handler
// servants and Messaging::ExceptionHolder (rustorb/messaging.h).
//
// A server ORB, a client ORB and an ORB for handlers talk over TCP on the
// loopback interface, in this process.

#include <chrono>
#include <condition_variable>
#include <deque>
#include <iostream>
#include <mutex>
#include <string>
#include <thread>
#include <vector>

#include "AmiTest.hh"

static int failures = 0;

#define EXPECT(cond)                                                             \
  do {                                                                           \
    if (!(cond)) {                                                               \
      std::cout << "FAIL line " << __LINE__ << ": " #cond << std::endl;          \
      ++failures;                                                                \
    }                                                                            \
  } while (0)

using OrbRef = CORBA::object_reference<CORBA::ORB>;
using CalcRef = IDL::traits<AmiCpp::Calc>::ref_type;
using HandlerRef = IDL::traits<AmiCpp::AMI_CalcHandler>::ref_type;
using HolderRef = IDL::traits<Messaging::ExceptionHolder>::ref_type;
using CORBA::CompletionStatus;

static const auto WAIT = std::chrono::seconds(10);

class CalcImpl : public CORBA::servant_traits<AmiCpp::Calc>::base_type {
 public:
  int32_t add(int32_t a, int32_t b, int32_t& carry) override {
    if (a < 0) throw AmiCpp::Oops(a, "negative");
    int32_t s = a + b;
    carry = s / 100;
    return s % 100;
  }
  AmiCpp::Pairs swap(AmiCpp::Pairs& ps, const std::string& tag) override {
    AmiCpp::Pairs ret(ps.rbegin(), ps.rend());
    ps = {AmiCpp::Pair(static_cast<int32_t>(ps.size()), tag)};
    return ret;
  }
  void fail(uint32_t minor) override {
    throw CORBA::NO_PERMISSION(minor, CompletionStatus::COMPLETED_YES);
  }
  void sleep(uint32_t ms) override { std::this_thread::sleep_for(std::chrono::milliseconds(ms)); }
  std::string label() override {
    std::lock_guard<std::mutex> lock(mutex_);
    return label_;
  }
  void label(const std::string& value) override {
    std::lock_guard<std::mutex> lock(mutex_);
    label_ = value;
  }
  int32_t count() override { return 7; }
  void fire(int32_t) override {}

 private:
  std::mutex mutex_;
  std::string label_ = "start";
};

// One call of a handler.
struct Call {
  Call(std::string n, std::vector<int32_t> i = {}) : name(std::move(n)), ints(std::move(i)) {}
  std::string name;
  std::vector<int32_t> ints;
  std::string text;
  AmiCpp::Pairs ret, out;
  HolderRef holder;
  std::thread::id thread;
};

// An AMI_CalcHandler servant that queues its calls.
class Handler : public CORBA::servant_traits<AmiCpp::AMI_CalcHandler>::base_type {
 public:
  void add(int32_t ret, int32_t carry) override { record(Call("add", {ret, carry})); }
  void add_excep(HolderRef h) override { record_excep("add_excep", h); }
  void swap(const AmiCpp::Pairs& ret, const AmiCpp::Pairs& ps) override {
    Call c{"swap"};
    c.ret = ret;
    c.out = ps;
    record(std::move(c));
  }
  void swap_excep(HolderRef h) override { record_excep("swap_excep", h); }
  void fail() override { record(Call("fail")); }
  void fail_excep(HolderRef h) override { record_excep("fail_excep", h); }
  void sleep() override { record(Call("sleep")); }
  void sleep_excep(HolderRef h) override { record_excep("sleep_excep", h); }
  void get_label(const std::string& ret) override {
    Call c{"get_label"};
    c.text = ret;
    record(std::move(c));
  }
  void get_label_excep(HolderRef h) override { record_excep("get_label_excep", h); }
  void set_label() override { record(Call("set_label")); }
  void set_label_excep(HolderRef h) override { record_excep("set_label_excep", h); }
  void get_count(int32_t ret) override {
    record(Call("get_count", {ret}));
    if (throw_in_get_count) throw std::runtime_error("dropped");
  }
  void get_count_excep(HolderRef h) override { record_excep("get_count_excep", h); }

  // The next call, or a call named "timeout" after WAIT.
  Call next(std::chrono::milliseconds wait = WAIT) {
    std::unique_lock<std::mutex> lock(mutex_);
    if (!cond_.wait_for(lock, wait, [&] { return !calls_.empty(); })) return Call("timeout");
    Call c = std::move(calls_.front());
    calls_.pop_front();
    return c;
  }

  bool throw_in_get_count = false;

 private:
  void record(Call c) {
    c.thread = std::this_thread::get_id();
    std::lock_guard<std::mutex> lock(mutex_);
    calls_.push_back(std::move(c));
    cond_.notify_all();
  }
  void record_excep(const char* name, HolderRef h) {
    Call c{name};
    c.holder = std::move(h);
    record(std::move(c));
  }

  std::mutex mutex_;
  std::condition_variable cond_;
  std::deque<Call> calls_;
};

static OrbRef server_orb, client_orb, handler_orb;
static CalcRef calc;

static OrbRef make_orb(const char* name) {
  int argc = 3;
  std::string a0 = name, a1 = "-ORBendPoint", a2 = "giop:tcp:127.0.0.1:";
  char* argv[] = {&a0[0], &a1[0], &a2[0], nullptr};
  OrbRef orb = CORBA::ORB_init(argc, argv);
  auto root = IDL::traits<PortableServer::POA>::narrow(orb->resolve_initial_references("RootPOA"));
  root->the_POAManager()->activate();
  return orb;
}

static CORBA::object_reference<CORBA::Object> activate(
    const OrbRef& orb, const CORBA::servant_reference<PortableServer::ServantBase>& servant) {
  auto root = IDL::traits<PortableServer::POA>::narrow(orb->resolve_initial_references("RootPOA"));
  return root->id_to_reference(root->activate_object(servant));
}

// A handler served by the client ORB.
static HandlerRef local_handler(const CORBA::servant_reference<Handler>& h) {
  return IDL::traits<AmiCpp::AMI_CalcHandler>::narrow(activate(client_orb, h));
}

// A handler served by another ORB, as the client sees it.
static HandlerRef remote_handler(const CORBA::servant_reference<Handler>& h) {
  std::string ior = handler_orb->object_to_string(activate(handler_orb, h));
  return IDL::traits<AmiCpp::AMI_CalcHandler>::narrow(client_orb->string_to_object(ior));
}

static void results() {
  auto h = CORBA::make_reference<Handler>();
  calc->sendc_add(local_handler(h), 70, 45);
  Call c = h->next();
  EXPECT(c.name == "add");
  EXPECT((c.ints == std::vector<int32_t>{15, 1}));
  EXPECT(c.thread != std::this_thread::get_id());
}

static void inout_and_out() {
  auto h = CORBA::make_reference<Handler>();
  AmiCpp::Pairs ps{AmiCpp::Pair(1, "one"), AmiCpp::Pair(2, "two")};
  calc->sendc_swap(local_handler(h), ps, "tag");
  // The arguments are copied: changing them later changes nothing.
  ps.push_back(AmiCpp::Pair(3, "three"));
  Call c = h->next();
  EXPECT(c.name == "swap");
  EXPECT((c.ret == AmiCpp::Pairs{AmiCpp::Pair(2, "two"), AmiCpp::Pair(1, "one")}));
  EXPECT((c.out == AmiCpp::Pairs{AmiCpp::Pair(2, "tag")}));
}

static void attributes() {
  auto h = CORBA::make_reference<Handler>();
  HandlerRef ref = local_handler(h);
  calc->sendc_set_label(ref, "new");
  EXPECT(h->next().name == "set_label");
  calc->sendc_get_label(ref);
  Call c = h->next();
  EXPECT(c.name == "get_label" && c.text == "new");
  calc->sendc_get_count(ref);
  c = h->next();
  EXPECT(c.name == "get_count" && c.ints == std::vector<int32_t>{7});
}

static void check_user_exception(const HolderRef& holder) {
  EXPECT(holder);
  if (!holder) return;
  try {
    holder->raise_exception();
    EXPECT(false);
  } catch (const AmiCpp::Oops& e) {
    EXPECT(e.code() == -1 && e.why() == "negative");
  }
  try {
    holder->raise_exception_with_list({AmiCpp::_tc_Oops});
    EXPECT(false);
  } catch (const AmiCpp::Oops&) {
  }
  try {
    holder->raise_exception_with_list({});
    EXPECT(false);
  } catch (const CORBA::UNKNOWN&) {
  }
}

static void check_system_exception(const HolderRef& holder) {
  EXPECT(holder);
  if (!holder) return;
  try {
    holder->raise_exception();
    EXPECT(false);
  } catch (const CORBA::NO_PERMISSION& e) {
    EXPECT(e.minor() == 5);
    EXPECT(e.completed() == CompletionStatus::COMPLETED_YES);
  }
  try {
    holder->raise_exception_with_list({});
    EXPECT(false);
  } catch (const CORBA::NO_PERMISSION&) {
  }
}

static void user_exception() {
  auto h = CORBA::make_reference<Handler>();
  calc->sendc_add(local_handler(h), -1, 0);
  Call c = h->next();
  EXPECT(c.name == "add_excep");
  check_user_exception(c.holder);
}

static void system_exception() {
  auto h = CORBA::make_reference<Handler>();
  calc->sendc_fail(local_handler(h), 5);
  Call c = h->next();
  EXPECT(c.name == "fail_excep");
  check_system_exception(c.holder);
}

// The holder is sent as a valuetype: the other side has only its state,
// marshaled_exception, and reads it with the registered readers.
static void handler_of_another_orb() {
  auto h = CORBA::make_reference<Handler>();
  HandlerRef ref = remote_handler(h);
  calc->sendc_add(ref, 1, 2);
  Call c = h->next();
  EXPECT(c.name == "add" && (c.ints == std::vector<int32_t>{3, 0}));
  calc->sendc_add(ref, -1, 0);
  c = h->next();
  EXPECT(c.name == "add_excep");
  check_user_exception(c.holder);
  calc->sendc_fail(ref, 5);
  c = h->next();
  EXPECT(c.name == "fail_excep");
  check_system_exception(c.holder);
  // A copy keeps the state.
  if (c.holder) {
    auto copy = IDL::traits<Messaging::ExceptionHolder>::narrow(c.holder->_copy_value());
    check_system_exception(copy);
  }
}

static void timeout() {
  auto h = CORBA::make_reference<Handler>();
  rustorb::setClientCallTimeout(calc, 100);
  calc->sendc_sleep(local_handler(h), 2000);
  rustorb::setClientCallTimeout(calc, 0);
  Call c = h->next();
  EXPECT(c.name == "sleep_excep");
  if (!c.holder) return;
  try {
    c.holder->raise_exception();
    EXPECT(false);
  } catch (const CORBA::TIMEOUT& e) {
    EXPECT(e.completed() == CompletionStatus::COMPLETED_MAYBE);
  }
}

// The request is sent; the reply is dropped.
static void nil_handler() {
  calc->sendc_set_label(nullptr, "from nil");
  auto deadline = std::chrono::steady_clock::now() + WAIT;
  while (calc->label() != "from nil") {
    if (std::chrono::steady_clock::now() > deadline) {
      EXPECT(false);
      return;
    }
    std::this_thread::sleep_for(std::chrono::milliseconds(10));
  }
}

static void many() {
  auto h = CORBA::make_reference<Handler>();
  HandlerRef ref = remote_handler(h);
  const int n = 100;
  for (int i = 0; i < n; ++i) calc->sendc_add(ref, i, 1000);
  std::vector<int> seen(n, 0);
  for (int i = 0; i < n; ++i) {
    Call c = h->next();
    EXPECT(c.name == "add" && c.ints.size() == 2);
    if (c.ints.size() != 2) return;
    int a = c.ints[1] * 100 + c.ints[0] - 1000;
    EXPECT(a >= 0 && a < n);
    if (a >= 0 && a < n) ++seen[a];
  }
  EXPECT(seen == std::vector<int>(n, 1));
}

// A handler that throws: the exception is dropped, and later replies still
// come.
static void handler_throws() {
  auto h = CORBA::make_reference<Handler>();
  h->throw_in_get_count = true;
  HandlerRef ref = local_handler(h);
  calc->sendc_get_count(ref);
  calc->sendc_get_count(ref);
  EXPECT(h->next().name == "get_count");
  EXPECT(h->next().name == "get_count");
}

// A target whose ORB is gone: the handler gets the failure.
static void unreachable_target() {
  auto h = CORBA::make_reference<Handler>();
  OrbRef gone = make_orb("ami-gone");
  std::string ior = gone->object_to_string(activate(gone, CORBA::make_reference<CalcImpl>()));
  gone->destroy();
  auto unreachable = IDL::traits<AmiCpp::Calc>::narrow(client_orb->string_to_object(ior));
  unreachable->sendc_add(local_handler(h), 1, 2);
  Call c = h->next();
  EXPECT(c.name == "add_excep");
  if (!c.holder) return;
  try {
    c.holder->raise_exception();
    EXPECT(false);
  } catch (const CORBA::TRANSIENT&) {
  } catch (const CORBA::COMM_FAILURE&) {
  }
}

// A client ORB destroyed while waiting: the handler, called through a
// reference of an ORB still running, gets the failure, once. (A handler
// reference of the destroyed ORB could not be called.) Sending once shut
// down fails at once.
static void shut_down_client() {
  auto h = CORBA::make_reference<Handler>();
  HandlerRef ref = remote_handler(h);
  OrbRef orb = make_orb("ami-short-client");
  auto short_calc = IDL::traits<AmiCpp::Calc>::narrow(
      orb->string_to_object(client_orb->object_to_string(calc)));
  short_calc->sendc_sleep(ref, 1000);
  std::this_thread::sleep_for(std::chrono::milliseconds(100));
  orb->destroy();
  Call c = h->next();
  EXPECT(c.name == "sleep_excep");
  if (c.holder) {
    try {
      c.holder->raise_exception();
      EXPECT(false);
    } catch (const CORBA::COMM_FAILURE&) {
    } catch (const CORBA::TRANSIENT&) {
    }
  }
  EXPECT(h->next(std::chrono::milliseconds(200)).name == "timeout");
  try {
    short_calc->sendc_add(ref, 1, 2);
    EXPECT(false);
  } catch (const CORBA::BAD_INV_ORDER&) {
  }
}

int main() {
  try {
    server_orb = make_orb("ami-server");
    client_orb = make_orb("ami-client");
    handler_orb = make_orb("ami-handlers");
    std::string ior =
        server_orb->object_to_string(activate(server_orb, CORBA::make_reference<CalcImpl>()));
    calc = IDL::traits<AmiCpp::Calc>::narrow(client_orb->string_to_object(ior));

    results();
    inout_and_out();
    attributes();
    user_exception();
    system_exception();
    handler_of_another_orb();
    timeout();
    nil_handler();
    many();
    handler_throws();
    unreachable_target();
    shut_down_client();

    calc = nullptr;
    client_orb->destroy();
    handler_orb->destroy();
    server_orb->destroy();
  } catch (const CORBA::Exception& e) {
    std::cout << "CORBA exception: " << e << std::endl;
    return 1;
  }
  if (failures == 0) std::cout << "all ami tests passed" << std::endl;
  return failures == 0 ? 0 : 1;
}

// The rustorbcpp-dii implementation: the rustorb C++ client and server of
// this directory with every call of the basic, any and value suites made
// through the DII and their servants run through the DSI.
//
// With RUSTORB_DII set, client.cc wraps the reference it tests in a proxy
// below (DiiEcho, DiiAnyEcho, DiiValueEcho), whose operations build a
// CORBA::Request from the TypeCodes of their parameters, and server.cc
// activates its servants as Dsi<Impl>: a PortableServer::DynamicImplementation
// that reads the arguments with CORBA::ServerRequest::arguments and calls
// the static servant's operation. The messages are those of the static
// stubs and skeletons. (The callback objects of the client, and the calls
// the servants make, stay static.)

#ifndef RUSTORBCPP_DII_ADAPTERS_H
#define RUSTORBCPP_DII_ADAPTERS_H

#include <cstdlib>
#include <string>

#include "AnyTest.hh"
#include "InteropTest.hh"
#include "ValueTest.hh"

namespace dii {

inline bool enabled() {
  const char* v = std::getenv("RUSTORB_DII");
  return v && *v && std::string(v) != "0";
}

template <typename T> IDL::traits<CORBA::TypeCode>::ref_type tc() {
  return rustorb::cdr::type_code<T>::get();
}

template <typename T> CORBA::Any any_of(const T& v) {
  CORBA::Any a;
  a <<= v;
  return a;
}

// The value of an any of type T (BAD_PARAM for another type).
template <typename T> T get(const CORBA::Any& a) {
  T v{};
  if (!(a >>= v)) throw CORBA::BAD_PARAM(0, CORBA::CompletionStatus::COMPLETED_NO);
  return v;
}

// Throws the user exception in u if it is an E.
template <typename E> void throw_if(const CORBA::UnknownUserException& u) {
  E e;
  if (u.exception() >>= e) throw e;
}

// --- the client side --------------------------------------------------------------

// A call of an operation through a CORBA::Request.
class Call {
 public:
  Call(CORBA::Object* target, const char* op) : req_(target->_request(op)) {}

  template <typename T> Call& in(const T& v) {
    req_->add_in_arg() <<= v;
    return *this;
  }
  template <typename T> Call& inout(const T& v) {
    req_->add_inout_arg() <<= v;
    return *this;
  }
  // The value only gives the type.
  template <typename T> Call& out() {
    req_->add_out_arg() = rustorb::detail::typed_any(tc<T>());
    return *this;
  }
  template <typename R> Call& returns() {
    req_->set_return_type(tc<R>());
    return *this;
  }
  // Invokes, raising the user exceptions E... as themselves.
  template <typename... E> Call& invoke() {
    (req_->exceptions()->add(tc<E>()), ...);
    try {
      req_->invoke();
    } catch (const CORBA::UnknownUserException& u) {
      (throw_if<E>(u), ...);
      throw;
    }
    return *this;
  }
  void oneway() { req_->send_oneway(); }

  template <typename R> R result() { return get<R>(req_->return_value()); }
  template <typename T> T arg(uint32_t i) { return get<T>(req_->arguments()->item(i)->value()); }

 private:
  IDL::traits<CORBA::Request>::ref_type req_;
};

// A proxy of type P (derived from the generated interface class) for the
// object of ref, or nil.
template <typename P, typename R> R wrap(const R& ref) {
  if (!ref) return ref;
  return R(std::make_shared<P>(rorb_object_clone(ref->_rorb_handle())));
}

#define DII_ECHO(NAME, T) \
  T NAME(T v) override { return Call(this, #NAME).in(v).returns<T>().invoke().result<T>(); }
#define DII_ECHO_REF(NAME, T)                                                  \
  T NAME(const T& v) override {                                               \
    return Call(this, #NAME).in(v).returns<T>().invoke().result<T>();          \
  }

class DiiEcho : public InteropTest::Echo {
 public:
  using Ref = IDL::traits<InteropTest::Echo>::ref_type;
  using CbRef = IDL::traits<InteropTest::Callback>::ref_type;

  explicit DiiEcho(rorb_object* h)
      : CORBA::Object(rustorb::detail::adopt, h), InteropTest::Echo(rustorb::detail::adopt, h) {}

  DII_ECHO(echo_boolean, bool)
  DII_ECHO(echo_octet, uint8_t)
  DII_ECHO(echo_char, char)
  DII_ECHO(echo_wchar, wchar_t)
  DII_ECHO(echo_short, int16_t)
  DII_ECHO(echo_ushort, uint16_t)
  DII_ECHO(echo_long, int32_t)
  DII_ECHO(echo_ulong, uint32_t)
  DII_ECHO(echo_longlong, int64_t)
  DII_ECHO(echo_ulonglong, uint64_t)
  DII_ECHO(echo_float, float)
  DII_ECHO(echo_double, double)
  DII_ECHO_REF(echo_string, std::string)
  DII_ECHO_REF(echo_wstring, std::wstring)
  DII_ECHO(echo_color, InteropTest::Color)
  DII_ECHO_REF(echo_point, InteropTest::Point)
  DII_ECHO_REF(echo_record, InteropTest::Record)
  DII_ECHO_REF(echo_records, InteropTest::RecordSeq)
  DII_ECHO_REF(echo_value, InteropTest::Value)
  DII_ECHO_REF(echo_shade, InteropTest::Shade)
  DII_ECHO_REF(echo_long_array, InteropTest::LongArray)
  DII_ECHO_REF(echo_matrix, InteropTest::Matrix)
  DII_ECHO_REF(echo_long_seq, InteropTest::LongSeq)
  DII_ECHO_REF(echo_string_seq, InteropTest::StringSeq)
  DII_ECHO_REF(echo_octets, InteropTest::OctetSeq)

  int32_t swap(int32_t& a, int32_t& b) override {
    Call c(this, "swap");
    c.inout(a).inout(b).returns<int32_t>().invoke();
    a = c.arg<int32_t>(0);
    b = c.arg<int32_t>(1);
    return c.result<int32_t>();
  }
  int32_t divide(int32_t a, int32_t b, int32_t& remainder) override {
    Call c(this, "divide");
    c.in(a).in(b).out<int32_t>().returns<int32_t>().invoke<InteropTest::DivideByZero>();
    remainder = c.arg<int32_t>(2);
    return c.result<int32_t>();
  }
  void make(int32_t x, InteropTest::Point& p, InteropTest::Record& r) override {
    Call c(this, "make");
    c.in(x).out<InteropTest::Point>().out<InteropTest::Record>().invoke();
    p = c.arg<InteropTest::Point>(1);
    r = c.arg<InteropTest::Record>(2);
  }
  void fail(int32_t code, const std::string& reason) override {
    Call(this, "fail").in(code).in(reason).invoke<InteropTest::Oops>();
  }
  void fail_system() override { Call(this, "fail_system").invoke(); }
  int32_t counter() override {
    return Call(this, "_get_counter").returns<int32_t>().invoke().result<int32_t>();
  }
  void counter(int32_t value) override { Call(this, "_set_counter").in(value).invoke(); }
  std::string name() override {
    return Call(this, "_get_name").returns<std::string>().invoke().result<std::string>();
  }
  void post(const std::string& message) override { Call(this, "post").in(message).oneway(); }
  InteropTest::StringSeq posted() override {
    return Call(this, "posted")
        .returns<InteropTest::StringSeq>()
        .invoke()
        .result<InteropTest::StringSeq>();
  }
  Ref self_ref() override {
    return wrap<DiiEcho>(Call(this, "self_ref").returns<Ref>().invoke().result<Ref>());
  }
  Ref echo_ref(Ref other) override {
    return wrap<DiiEcho>(Call(this, "echo_ref").in(other).returns<Ref>().invoke().result<Ref>());
  }
  bool is_nil(Ref other) override {
    return Call(this, "is_nil").in(other).returns<bool>().invoke().result<bool>();
  }
  std::string call_back(CbRef cb, const std::string& message) override {
    return Call(this, "call_back")
        .in(cb)
        .in(message)
        .returns<std::string>()
        .invoke()
        .result<std::string>();
  }
  void register_callback(CbRef cb) override { Call(this, "register_callback").in(cb).invoke(); }
  std::string trigger(const std::string& message) override {
    return Call(this, "trigger")
        .in(message)
        .returns<std::string>()
        .invoke<InteropTest::Oops>()
        .result<std::string>();
  }
  void shutdown() override { Call(this, "shutdown").oneway(); }
};

class DiiAnyEcho : public AnyTest::AnyEcho {
 public:
  using TcRef = IDL::traits<CORBA::TypeCode>::ref_type;

  explicit DiiAnyEcho(rorb_object* h)
      : CORBA::Object(rustorb::detail::adopt, h), AnyTest::AnyEcho(rustorb::detail::adopt, h) {}

  CORBA::Any echo(const CORBA::Any& a) override {
    return Call(this, "echo").in(a).returns<CORBA::Any>().invoke().result<CORBA::Any>();
  }
  CORBA::Any make(const std::string& name) override {
    return Call(this, "make").in(name).returns<CORBA::Any>().invoke().result<CORBA::Any>();
  }
  bool check(const std::string& name, const CORBA::Any& a) override {
    return Call(this, "check").in(name).in(a).returns<bool>().invoke().result<bool>();
  }
  void out_inout(const std::string& name, CORBA::Any& b, CORBA::Any& c) override {
    Call call(this, "out_inout");
    call.in(name).out<CORBA::Any>().inout(c).invoke();
    b = call.arg<CORBA::Any>(1);
    c = call.arg<CORBA::Any>(2);
  }
  TcRef echo_tc(TcRef tc) override {
    return Call(this, "echo_tc").in(tc).returns<TcRef>().invoke().result<TcRef>();
  }
  TcRef make_tc(const std::string& name) override {
    return Call(this, "make_tc").in(name).returns<TcRef>().invoke().result<TcRef>();
  }
  bool check_tc(const std::string& name, TcRef tc) override {
    return Call(this, "check_tc").in(name).in(tc).returns<bool>().invoke().result<bool>();
  }
  AnyTest::NVList echo_nvlist(const AnyTest::NVList& l) override {
    return Call(this, "echo_nvlist")
        .in(l)
        .returns<AnyTest::NVList>()
        .invoke()
        .result<AnyTest::NVList>();
  }
  std::string describe(const CORBA::Any& a) override {
    return Call(this, "describe").in(a).returns<std::string>().invoke().result<std::string>();
  }
};

class DiiValueEcho : public ValueTest::ValueEcho {
 public:
  template <typename T> using R = typename IDL::traits<T>::ref_type;

  explicit DiiValueEcho(rorb_object* h)
      : CORBA::Object(rustorb::detail::adopt, h), ValueTest::ValueEcho(rustorb::detail::adopt, h) {}

#define DII_VALUE_ECHO(NAME, T)                                                  \
  R<T> NAME(R<T> v) override {                                                   \
    return Call(this, #NAME).in(v).returns<R<T>>().invoke().result<R<T>>();       \
  }
  DII_VALUE_ECHO(echo_point, ValueTest::Point)
  DII_VALUE_ECHO(echo_node, ValueTest::Node)
  DII_VALUE_ECHO(echo_shape, ValueTest::Shape)
  DII_VALUE_ECHO(echo_long_box, ValueTest::LongBox)
  DII_VALUE_ECHO(echo_string_box, ValueTest::StringBox)
  DII_VALUE_ECHO(echo_pair_box, ValueTest::PairBox)
  DII_VALUE_ECHO(echo_tally, ValueTest::Tally)
#undef DII_VALUE_ECHO

  ValueTest::Pair echo_pair(const ValueTest::Pair& p) override {
    return Call(this, "echo_pair").in(p).returns<ValueTest::Pair>().invoke().result<ValueTest::Pair>();
  }
  ValueTest::PointSeq echo_points(const ValueTest::PointSeq& s) override {
    return Call(this, "echo_points")
        .in(s)
        .returns<ValueTest::PointSeq>()
        .invoke()
        .result<ValueTest::PointSeq>();
  }
  bool same(R<ValueTest::Point> a, R<ValueTest::Point> b) override {
    return Call(this, "same").in(a).in(b).returns<bool>().invoke().result<bool>();
  }
  int32_t count_nodes(R<ValueTest::Node> n) override {
    return Call(this, "count_nodes").in(n).returns<int32_t>().invoke().result<int32_t>();
  }
  CORBA::Any echo_any(const CORBA::Any& a) override {
    return Call(this, "echo_any").in(a).returns<CORBA::Any>().invoke().result<CORBA::Any>();
  }
  int32_t call_count(R<ValueTest::Counter> c) override {
    return Call(this, "call_count").in(c).returns<int32_t>().invoke().result<int32_t>();
  }
  int32_t account_doubled(R<ValueTest::Account> a) override {
    return Call(this, "account_doubled").in(a).returns<int32_t>().invoke().result<int32_t>();
  }
  R<ValueTest::Account> make_account(int32_t balance) override {
    return Call(this, "make_account")
        .in(balance)
        .returns<R<ValueTest::Account>>()
        .invoke()
        .result<R<ValueTest::Account>>();
  }
  void shutdown() override { Call(this, "shutdown").oneway(); }
};

#undef DII_ECHO
#undef DII_ECHO_REF

// --- the server side --------------------------------------------------------------

// Reads the parameters of an operation and sets its results through a
// CORBA::ServerRequest.
class Serve {
 public:
  explicit Serve(CORBA::ServerRequest& r)
      : r_(r), params_(CORBA::make_reference<CORBA::NVList>()) {}

  template <typename T> Serve& in() { return add<T>(CORBA::ARG_IN); }
  template <typename T> Serve& inout() { return add<T>(CORBA::ARG_INOUT); }
  template <typename T> Serve& out() { return add<T>(CORBA::ARG_OUT); }
  Serve& read() {
    r_.arguments(params_);
    return *this;
  }

  template <typename T> T arg(uint32_t i) { return get<T>(params_->item(i)->value()); }
  template <typename T> void set(uint32_t i, const T& v) { params_->item(i)->value() <<= v; }
  template <typename T> void result(const T& v) { r_.set_result(any_of(v)); }

  // Runs f, replying with the user exceptions E... it raises.
  template <typename... E, typename F> void run(F f) { run_catching<E...>(f); }

 private:
  template <typename T> Serve& add(CORBA::Flags flags) {
    // The value only gives the type (rustorb's any without a value: a
    // TypeCode, say, has no value to insert).
    params_->add_value("", rustorb::detail::typed_any(tc<T>()), flags);
    return *this;
  }
  template <typename F> void run_catching(F f) { f(); }
  template <typename E, typename... Es, typename F> void run_catching(F f) {
    try {
      run_catching<Es...>(f);
    } catch (const E& e) {
      r_.set_exception(any_of(e));
    }
  }

  CORBA::ServerRequest& r_;
  IDL::traits<CORBA::NVList>::ref_type params_;
};

#define DSI_ECHO(NAME, T)                     \
  if (op == #NAME) {                          \
    Serve s(r);                               \
    s.in<T>().read();                         \
    s.result<T>(impl.NAME(s.arg<T>(0)));      \
    return true;                              \
  }

template <class Impl>
std::enable_if_t<std::is_base_of<POA_InteropTest::Echo, Impl>::value, bool> dispatch(
    Impl& impl, CORBA::ServerRequest& r) {
  using namespace InteropTest;
  using Ref = IDL::traits<Echo>::ref_type;
  using CbRef = IDL::traits<Callback>::ref_type;
  const std::string op = r.operation();
  DSI_ECHO(echo_boolean, bool)
  DSI_ECHO(echo_octet, uint8_t)
  DSI_ECHO(echo_char, char)
  DSI_ECHO(echo_wchar, wchar_t)
  DSI_ECHO(echo_short, int16_t)
  DSI_ECHO(echo_ushort, uint16_t)
  DSI_ECHO(echo_long, int32_t)
  DSI_ECHO(echo_ulong, uint32_t)
  DSI_ECHO(echo_longlong, int64_t)
  DSI_ECHO(echo_ulonglong, uint64_t)
  DSI_ECHO(echo_float, float)
  DSI_ECHO(echo_double, double)
  DSI_ECHO(echo_string, std::string)
  DSI_ECHO(echo_wstring, std::wstring)
  DSI_ECHO(echo_color, Color)
  DSI_ECHO(echo_point, Point)
  DSI_ECHO(echo_record, Record)
  DSI_ECHO(echo_records, RecordSeq)
  DSI_ECHO(echo_value, Value)
  DSI_ECHO(echo_shade, Shade)
  DSI_ECHO(echo_long_array, LongArray)
  DSI_ECHO(echo_matrix, Matrix)
  DSI_ECHO(echo_long_seq, LongSeq)
  DSI_ECHO(echo_string_seq, StringSeq)
  DSI_ECHO(echo_octets, OctetSeq)
  DSI_ECHO(echo_ref, Ref)
  Serve s(r);
  if (op == "swap") {
    s.inout<int32_t>().inout<int32_t>().read();
    int32_t a = s.arg<int32_t>(0), b = s.arg<int32_t>(1);
    int32_t sum = impl.swap(a, b);
    s.set(0, a);
    s.set(1, b);
    s.result(sum);
  } else if (op == "divide") {
    s.in<int32_t>().in<int32_t>().out<int32_t>().read();
    s.run<DivideByZero>([&] {
      int32_t rem = 0;
      int32_t q = impl.divide(s.arg<int32_t>(0), s.arg<int32_t>(1), rem);
      s.set(2, rem);
      s.result(q);
    });
  } else if (op == "make") {
    s.in<int32_t>().out<Point>().out<Record>().read();
    Point p;
    Record rec;
    impl.make(s.arg<int32_t>(0), p, rec);
    s.set(1, p);
    s.set(2, rec);
  } else if (op == "fail") {
    s.in<int32_t>().in<std::string>().read();
    s.run<Oops>([&] { impl.fail(s.arg<int32_t>(0), s.arg<std::string>(1)); });
  } else if (op == "fail_system") {
    s.read();
    impl.fail_system();
  } else if (op == "_get_counter") {
    s.read();
    s.result(impl.counter());
  } else if (op == "_set_counter") {
    s.in<int32_t>().read();
    impl.counter(s.arg<int32_t>(0));
  } else if (op == "_get_name") {
    s.read();
    s.result(impl.name());
  } else if (op == "post") {
    s.in<std::string>().read();
    impl.post(s.arg<std::string>(0));
  } else if (op == "posted") {
    s.read();
    s.result(impl.posted());
  } else if (op == "self_ref") {
    s.read();
    s.result(impl.self_ref());
  } else if (op == "is_nil") {
    s.in<Ref>().read();
    s.result(impl.is_nil(s.arg<Ref>(0)));
  } else if (op == "call_back") {
    s.in<CbRef>().in<std::string>().read();
    s.result(impl.call_back(s.arg<CbRef>(0), s.arg<std::string>(1)));
  } else if (op == "register_callback") {
    s.in<CbRef>().read();
    impl.register_callback(s.arg<CbRef>(0));
  } else if (op == "trigger") {
    s.in<std::string>().read();
    s.run<Oops>([&] { s.result(impl.trigger(s.arg<std::string>(0))); });
  } else if (op == "shutdown") {
    s.read();
    impl.shutdown();
  } else {
    return false;
  }
  return true;
}

template <class Impl>
std::enable_if_t<std::is_base_of<POA_AnyTest::AnyEcho, Impl>::value, bool> dispatch(
    Impl& impl, CORBA::ServerRequest& r) {
  using TcRef = IDL::traits<CORBA::TypeCode>::ref_type;
  const std::string op = r.operation();
  DSI_ECHO(echo, CORBA::Any)
  DSI_ECHO(echo_tc, TcRef)
  DSI_ECHO(echo_nvlist, AnyTest::NVList)
  Serve s(r);
  if (op == "describe") {
    s.in<CORBA::Any>().read();
    s.result(impl.describe(s.arg<CORBA::Any>(0)));
  } else if (op == "make") {
    s.in<std::string>().read();
    s.result(impl.make(s.arg<std::string>(0)));
  } else if (op == "check") {
    s.in<std::string>().in<CORBA::Any>().read();
    s.result(impl.check(s.arg<std::string>(0), s.arg<CORBA::Any>(1)));
  } else if (op == "out_inout") {
    s.in<std::string>().out<CORBA::Any>().inout<CORBA::Any>().read();
    CORBA::Any b;
    CORBA::Any c = s.arg<CORBA::Any>(2);
    impl.out_inout(s.arg<std::string>(0), b, c);
    s.set(1, b);
    s.set(2, c);
  } else if (op == "make_tc") {
    s.in<std::string>().read();
    s.result(impl.make_tc(s.arg<std::string>(0)));
  } else if (op == "check_tc") {
    s.in<std::string>().in<TcRef>().read();
    s.result(impl.check_tc(s.arg<std::string>(0), s.arg<TcRef>(1)));
  } else {
    return false;
  }
  return true;
}

template <class Impl>
std::enable_if_t<std::is_base_of<POA_ValueTest::ValueEcho, Impl>::value, bool> dispatch(
    Impl& impl, CORBA::ServerRequest& r) {
  using namespace ValueTest;
  const std::string op = r.operation();
  DSI_ECHO(echo_point, IDL::traits<Point>::ref_type)
  DSI_ECHO(echo_pair, Pair)
  DSI_ECHO(echo_node, IDL::traits<Node>::ref_type)
  DSI_ECHO(echo_shape, IDL::traits<Shape>::ref_type)
  DSI_ECHO(echo_points, PointSeq)
  DSI_ECHO(echo_long_box, IDL::traits<LongBox>::ref_type)
  DSI_ECHO(echo_string_box, IDL::traits<StringBox>::ref_type)
  DSI_ECHO(echo_pair_box, IDL::traits<PairBox>::ref_type)
  DSI_ECHO(echo_any, CORBA::Any)
  DSI_ECHO(echo_tally, IDL::traits<Tally>::ref_type)
  Serve s(r);
  if (op == "same") {
    s.in<IDL::traits<Point>::ref_type>().in<IDL::traits<Point>::ref_type>().read();
    s.result(impl.same(s.arg<IDL::traits<Point>::ref_type>(0),
                       s.arg<IDL::traits<Point>::ref_type>(1)));
  } else if (op == "count_nodes") {
    s.in<IDL::traits<Node>::ref_type>().read();
    s.result(impl.count_nodes(s.arg<IDL::traits<Node>::ref_type>(0)));
  } else if (op == "call_count") {
    s.in<IDL::traits<Counter>::ref_type>().read();
    s.result(impl.call_count(s.arg<IDL::traits<Counter>::ref_type>(0)));
  } else if (op == "account_doubled") {
    s.in<IDL::traits<Account>::ref_type>().read();
    s.result(impl.account_doubled(s.arg<IDL::traits<Account>::ref_type>(0)));
  } else if (op == "make_account") {
    s.in<int32_t>().read();
    s.result(impl.make_account(s.arg<int32_t>(0)));
  } else if (op == "shutdown") {
    s.read();
    impl.shutdown();
  } else {
    return false;
  }
  return true;
}

#undef DSI_ECHO

// A static servant Impl run through the DSI: its operations are found by
// name in dispatch(Impl&, ...) above.
template <class Impl>
class Dsi : public Impl, public virtual PortableServer::DynamicImplementation {
 public:
  template <typename... A> explicit Dsi(A&&... a) : Impl(std::forward<A>(a)...) {}

  void invoke(CORBA::object_reference<CORBA::ServerRequest> request) override {
    if (!dispatch(static_cast<Impl&>(*this), *request)) {
      throw CORBA::BAD_OPERATION(0x4f4d0002, CORBA::CompletionStatus::COMPLETED_NO);
    }
  }
  std::string _primary_interface(const PortableServer::ObjectId&,
                                 CORBA::object_reference<PortableServer::POA>) override {
    return Impl::_rorb_repository_id();
  }
  // The DSI's, over the skeleton's.
  const char* _rorb_repository_id() const override {
    return PortableServer::DynamicImplementation::_rorb_repository_id();
  }
  rorb_status _rorb_dispatch(const char* op, size_t len, rorb_in* in,
                             rorb_server_reply* reply) override {
    return PortableServer::DynamicImplementation::_rorb_dispatch(op, len, in, reply);
  }
  // The skeleton's, which knows the base interfaces.
  bool _is_a(const std::string& repository_id) override { return Impl::_is_a(repository_id); }
};

// The servant to activate: Impl as itself, or through the DSI.
template <class Impl, typename... A> PortableServer::Servant servant(A&&... a) {
  if (enabled()) return CORBA::make_reference<Dsi<Impl>>(std::forward<A>(a)...);
  return CORBA::make_reference<Impl>(std::forward<A>(a)...);
}

// The reference to test: ref as it is, or behind the DII proxy P.
template <typename P, typename R> R proxy(const R& ref) {
  return enabled() ? wrap<P>(ref) : ref;
}

}  // namespace dii

#endif

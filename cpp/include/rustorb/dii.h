// The Dynamic Invocation Interface and the Dynamic Skeleton Interface
// (IDL to C++11): calls and servants described at run time, by TypeCodes,
// on the DII and DSI of the rustorb core (the rorb_request and
// rorb_server_request functions of the C ABI). See docs/dii-dsi.md.
//
//   auto req = obj->_request("divide");
//   req->add_in_arg("a") <<= int32_t(7);
//   req->add_in_arg("b") <<= int32_t(2);
//   req->add_out_arg("remainder") <<= int32_t();   // the value gives the type
//   req->set_return_type(CORBA::_tc_long);
//   req->exceptions()->add(InteropTest::_tc_DivideByZero);
//   req->invoke();    // a user exception: CORBA::UnknownUserException
//   int32_t q = 0;
//   req->return_value() >>= q;
//
// A servant derived from PortableServer::DynamicImplementation gets each
// request as a CORBA::ServerRequest in invoke(): it reads the arguments
// with arguments(nvlist), whose anys give the types and the modes of the
// parameters, sets the out and inout values in the list, and calls
// set_result or set_exception.
//
// Contexts (CORBA::Context) are not supported.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

#ifndef RUSTORB_DII_H
#define RUSTORB_DII_H

#include <rustorb/cdr.h>

#include <chrono>
#include <mutex>
#include <string>
#include <thread>
#include <vector>

#define RUSTORB_DII_EXCEPTION_BODY(NAME, ID)                 \
  const char* _name() const override { return #NAME; }       \
  const char* _rep_id() const override { return ID; }        \
  [[noreturn]] void _raise() const override { throw *this; }

namespace CORBA {

// The modes of an argument (CORBA::Flags).
constexpr Flags ARG_IN = 1;
constexpr Flags ARG_OUT = 2;
constexpr Flags ARG_INOUT = 3;

// Raised by NVList and ExceptionList for an index out of range.
class Bounds : public UserException {
 public:
  RUSTORB_DII_EXCEPTION_BODY(Bounds, "IDL:omg.org/CORBA/Bounds:1.0")
};

// A user exception the DII got back: Request::invoke and get_response
// raise it with the exception in an any, of the TypeCode given in the
// request's exception list. A DSI servant may raise it too, to reply with
// that exception.
class UnknownUserException : public UserException {
 public:
  UnknownUserException() = default;
  explicit UnknownUserException(Any exception) : exception_(std::move(exception)) {}
  const Any& exception() const { return exception_; }
  Any& exception() { return exception_; }
  RUSTORB_DII_EXCEPTION_BODY(UnknownUserException,
                               "IDL:omg.org/CORBA/UnknownUserException:1.0")
 private:
  Any exception_;
};

// Contexts are not supported: there is no way to make one.
class Context : public LocalObject {
 protected:
  Context() = default;
};
class ContextList : public LocalObject {
 protected:
  ContextList() = default;
};

// An argument: a name, a value and a mode (ARG_IN, ARG_OUT or ARG_INOUT).
// The value of an out argument gives only its type until the call.
class NamedValue : public LocalObject {
 public:
  NamedValue(std::string name, Any value, Flags flags)
      : name_(std::move(name)), value_(std::move(value)), flags_(flags) {}

  std::string name() const { return name_; }
  Any& value() { return value_; }
  const Any& value() const { return value_; }
  Flags flags() const { return flags_; }

 private:
  std::string name_;
  Any value_;
  Flags flags_;
};

// A list of arguments (or of parameters, for a DSI servant).
class NVList : public LocalObject {
 public:
  NVList() = default;

  uint32_t count() const {
    std::lock_guard<std::mutex> lock(mutex_);
    return static_cast<uint32_t>(items_.size());
  }
  object_reference<NamedValue> add(Flags flags) { return add_value("", Any(), flags); }
  object_reference<NamedValue> add_item(const std::string& item_name, Flags flags) {
    return add_value(item_name, Any(), flags);
  }
  object_reference<NamedValue> add_value(const std::string& item_name, const Any& value,
                                         Flags flags) {
    auto nv = make_reference<NamedValue>(item_name, value, flags);
    std::lock_guard<std::mutex> lock(mutex_);
    items_.push_back(nv);
    return nv;
  }
  object_reference<NamedValue> item(uint32_t index) const {
    std::lock_guard<std::mutex> lock(mutex_);
    if (index >= items_.size()) throw Bounds();
    return items_[index];
  }
  void remove(uint32_t index) {
    std::lock_guard<std::mutex> lock(mutex_);
    if (index >= items_.size()) throw Bounds();
    items_.erase(items_.begin() + index);
  }

  // The items, in order.
  std::vector<object_reference<NamedValue>> _rorb_items() const {
    std::lock_guard<std::mutex> lock(mutex_);
    return items_;
  }

 private:
  mutable std::mutex mutex_;
  std::vector<object_reference<NamedValue>> items_;
};

// The user exceptions an operation may raise, by TypeCode.
class ExceptionList : public LocalObject {
 public:
  ExceptionList() = default;

  uint32_t count() const {
    std::lock_guard<std::mutex> lock(mutex_);
    return static_cast<uint32_t>(items_.size());
  }
  void add(const object_reference<TypeCode>& exc) {
    if (!exc) throw BAD_PARAM(0, CompletionStatus::COMPLETED_NO);
    std::lock_guard<std::mutex> lock(mutex_);
    items_.push_back(exc);
  }
  object_reference<TypeCode> item(uint32_t index) const {
    std::lock_guard<std::mutex> lock(mutex_);
    if (index >= items_.size()) throw Bounds();
    return items_[index];
  }
  void remove(uint32_t index) {
    std::lock_guard<std::mutex> lock(mutex_);
    if (index >= items_.size()) throw Bounds();
    items_.erase(items_.begin() + index);
  }

  std::vector<object_reference<TypeCode>> _rorb_items() const {
    std::lock_guard<std::mutex> lock(mutex_);
    return items_;
  }

 private:
  mutable std::mutex mutex_;
  std::vector<object_reference<TypeCode>> items_;
};

}  // namespace CORBA

namespace rustorb {
namespace detail {

// An any of type tc without a value (the result and the out arguments of
// a request before the call).
inline CORBA::Any typed_any(const CORBA::object_reference<CORBA::TypeCode>& tc) {
  return CORBA::Any(adopt, rorb_any_typed(tc_handle(tc)));
}

struct RequestDeleter {
  void operator()(rorb_request* r) const noexcept { rorb_request_release(r); }
};
using RequestHandle = std::unique_ptr<rorb_request, RequestDeleter>;

struct ServerRequestDeleter {
  void operator()(rorb_server_request* r) const noexcept { rorb_server_request_release(r); }
};

}  // namespace detail
}  // namespace rustorb

namespace CORBA {

// A call whose operation, arguments, result type and user exceptions are
// given at run time, made by Object::_request or _create_request.
//
// After invoke (or send_deferred and get_response), the out and inout
// arguments hold the values the server sent back and return_value() the
// result. A system exception is raised as such; a user exception of the
// exception list as UnknownUserException; one that is not in the list is
// UNKNOWN (minor 0x4f4d0001). A request may be sent again once its call
// has ended.
class Request : public LocalObject {
 public:
  Request(object_reference<Object> target, std::string operation,
          object_reference<NVList> arguments, object_reference<NamedValue> result,
          object_reference<ExceptionList> exceptions)
      : target_(std::move(target)),
        operation_(std::move(operation)),
        arguments_(arguments ? std::move(arguments) : make_reference<NVList>()),
        result_(result ? std::move(result) : make_reference<NamedValue>("", Any(), 0)),
        exceptions_(exceptions ? std::move(exceptions) : make_reference<ExceptionList>()) {
    if (result_->value().type()->kind() == TCKind::tk_null) {
      result_->value() = rustorb::detail::typed_any(_tc_void);
    }
  }

  object_reference<Object> target() const { return target_; }
  std::string operation() const { return operation_; }
  object_reference<NVList> arguments() const { return arguments_; }
  object_reference<NamedValue> result() const { return result_; }
  object_reference<ExceptionList> exceptions() const { return exceptions_; }
  object_reference<ContextList> contexts() const {
    throw NO_IMPLEMENT(0, CompletionStatus::COMPLETED_NO);
  }
  object_reference<Context> ctx() const { throw NO_IMPLEMENT(0, CompletionStatus::COMPLETED_NO); }
  void ctx(const object_reference<Context>&) {
    throw NO_IMPLEMENT(0, CompletionStatus::COMPLETED_NO);
  }

  // Adds an argument and returns its value, to set with <<=. The value of
  // an out argument gives only its type (insert a value of the type).
  Any& add_in_arg(const std::string& name = "") { return add(name, ARG_IN); }
  Any& add_inout_arg(const std::string& name = "") { return add(name, ARG_INOUT); }
  Any& add_out_arg(const std::string& name = "") { return add(name, ARG_OUT); }

  // The type of the return value (void until set).
  void set_return_type(const object_reference<TypeCode>& tc) {
    result_->value() = rustorb::detail::typed_any(tc);
  }
  Any& return_value() { return result_->value(); }

  void invoke() {
    rustorb::detail::RequestHandle h = make();
    rustorb::detail::Exc exc;
    rorb_status status = rorb_request_invoke(h.get(), &exc);
    take(h.get(), status, exc);
  }

  void send_oneway() {
    rustorb::detail::RequestHandle h = make();
    rustorb::detail::Exc exc;
    rustorb::detail::check(rorb_request_send_oneway(h.get(), &exc), exc);
  }

  // Sends the request without waiting: poll_response and get_response
  // then take the response. BAD_INV_ORDER while one is outstanding.
  void send_deferred() {
    std::lock_guard<std::mutex> lock(mutex_);
    if (pending_) throw BAD_INV_ORDER(0, CompletionStatus::COMPLETED_NO);
    rustorb::detail::RequestHandle h = make();
    rustorb::detail::Exc exc;
    rustorb::detail::check(rorb_request_send_deferred(h.get(), &exc), exc);
    pending_ = std::move(h);
  }

  // Whether the response has arrived (BAD_INV_ORDER when nothing was sent
  // deferred).
  bool poll_response() {
    std::lock_guard<std::mutex> lock(mutex_);
    if (!pending_) throw BAD_INV_ORDER(0, CompletionStatus::COMPLETED_NO);
    int32_t ready = 0;
    rustorb::detail::Exc exc;
    rustorb::detail::check(rorb_request_poll_response(pending_.get(), &ready, &exc), exc);
    return ready != 0;
  }

  // Waits for the response and takes it, as invoke does.
  void get_response() {
    rustorb::detail::RequestHandle h;
    {
      std::lock_guard<std::mutex> lock(mutex_);
      if (!pending_) throw BAD_INV_ORDER(0, CompletionStatus::COMPLETED_NO);
      h = std::move(pending_);
    }
    rustorb::detail::Exc exc;
    rorb_status status = rorb_request_get_response(h.get(), &exc);
    take(h.get(), status, exc);
  }

 private:
  Any& add(const std::string& name, Flags flags) {
    return arguments_->add_value(name, Any(), flags)->value();
  }

  // A request of the C ABI with the arguments, result type and
  // exceptions as they are now.
  rustorb::detail::RequestHandle make() const {
    rorb_object* target = target_ ? target_->_rorb_handle() : nullptr;
    if (!target) throw NO_IMPLEMENT(0, CompletionStatus::COMPLETED_NO);
    rorb_request* raw = nullptr;
    rustorb::detail::Exc exc;
    rustorb::detail::check(
        rorb_request_new(target, operation_.data(), operation_.size(), &raw, &exc), exc);
    rustorb::detail::RequestHandle h(raw);
    for (const auto& nv : arguments_->_rorb_items()) {
      std::string name = nv->name();
      rustorb::detail::check(rorb_request_add_arg(h.get(), name.data(), name.size(),
                                                  nv->flags(), nv->value()._rorb_handle(),
                                                  &exc),
                             exc);
    }
    {
      rorb_typecode* tc = rorb_any_type(result_->value()._rorb_handle());
      rorb_request_set_return_type(h.get(), tc);
      rorb_tc_release(tc);
    }
    for (const auto& tc : exceptions_->_rorb_items()) {
      rorb_request_add_exception(h.get(), rustorb::detail::tc_handle(tc));
    }
    return h;
  }

  // Takes the outcome of a call: the values received, or the exception.
  void take(rorb_request* h, rorb_status status, rustorb::detail::Exc& exc) {
    if (status == RORB_USER_EXCEPTION) {
      throw UnknownUserException(Any(rustorb::detail::adopt, rorb_request_exception(h)));
    }
    rustorb::detail::check(status, exc);
    std::vector<object_reference<NamedValue>> items = arguments_->_rorb_items();
    for (size_t i = 0; i < items.size(); ++i) {
      if ((items[i]->flags() & 3) == ARG_IN) continue;
      rorb_any* a = rorb_request_argument(h, i);
      if (a) items[i]->value() = Any(rustorb::detail::adopt, a);
    }
    result_->value() = Any(rustorb::detail::adopt, rorb_request_result(h));
  }

  object_reference<Object> target_;
  std::string operation_;
  object_reference<NVList> arguments_;
  object_reference<NamedValue> result_;
  object_reference<ExceptionList> exceptions_;
  std::mutex mutex_;
  // The request sent deferred, until get_response.
  rustorb::detail::RequestHandle pending_;
};

// A request to a PortableServer::DynamicImplementation, given to its
// invoke. Its calls are made in this order: arguments once (BAD_INV_ORDER
// again), then set_result or set_exception (set_result before arguments
// is BAD_INV_ORDER). A system exception is raised from invoke instead.
class ServerRequest : public LocalObject {
 public:
  ServerRequest(rustorb::detail::adopt_t, rorb_server_request* handle, std::string operation)
      : handle_(handle), operation_(std::move(operation)) {}

  std::string operation() const { return operation_; }

  // Reads the in and inout arguments into parameters, whose anys give the
  // types (their values are ignored) and modes of the operation's
  // parameters, in order. The servant then sets the values of the out and
  // inout items (of their types, else the reply is BAD_PARAM). Arguments
  // that do not fit the types are MARSHAL.
  void arguments(object_reference<NVList>& parameters) {
    if (!parameters) throw BAD_PARAM(0, CompletionStatus::COMPLETED_NO);
    std::lock_guard<std::mutex> lock(mutex_);
    if (params_) throw BAD_INV_ORDER(0, CompletionStatus::COMPLETED_NO);
    std::vector<object_reference<NamedValue>> items = parameters->_rorb_items();
    rustorb::detail::Exc exc;
    for (const auto& nv : items) {
      std::string name = nv->name();
      rorb_typecode* tc = rorb_any_type(nv->value()._rorb_handle());
      rorb_status status = rorb_server_request_add_param(handle_.get(), name.data(), name.size(),
                                                         nv->flags(), tc, &exc);
      rorb_tc_release(tc);
      rustorb::detail::check(status, exc);
    }
    rustorb::detail::check(rorb_server_request_arguments(handle_.get(), &exc), exc);
    for (size_t i = 0; i < items.size(); ++i) {
      if ((items[i]->flags() & 3) == ARG_OUT) continue;
      rorb_any* a = rorb_server_request_argument(handle_.get(), i);
      if (a) items[i]->value() = Any(rustorb::detail::adopt, a);
    }
    params_ = parameters;
  }

  object_reference<Context> ctx() const { throw NO_IMPLEMENT(0, CompletionStatus::COMPLETED_NO); }

  void set_result(const Any& value) {
    rustorb::detail::Exc exc;
    rustorb::detail::check(rorb_server_request_set_result(handle_.get(), value._rorb_handle(), &exc),
                           exc);
  }

  // value: an any of a user exception (else BAD_PARAM).
  void set_exception(const Any& value) {
    rustorb::detail::Exc exc;
    rustorb::detail::check(
        rorb_server_request_set_exception(handle_.get(), value._rorb_handle(), &exc), exc);
    std::lock_guard<std::mutex> lock(mutex_);
    raised_ = true;
  }

  // Writes the reply (DynamicImplementation, after invoke): the out and
  // inout values of the parameters first.
  void _rorb_reply(rorb_server_reply* reply) {
    rustorb::detail::Exc exc;
    object_reference<NVList> params;
    bool raised = false;
    {
      std::lock_guard<std::mutex> lock(mutex_);
      params = params_;
      raised = raised_;
    }
    if (params && !raised) {
      std::vector<object_reference<NamedValue>> items = params->_rorb_items();
      for (size_t i = 0; i < items.size(); ++i) {
        if ((items[i]->flags() & 3) == ARG_IN) continue;
        rustorb::detail::check(rorb_server_request_set_argument(
                                   handle_.get(), i, items[i]->value()._rorb_handle(), &exc),
                               exc);
      }
    }
    rustorb::detail::check(rorb_server_request_reply(handle_.get(), reply, &exc), exc);
  }

 private:
  std::unique_ptr<rorb_server_request, rustorb::detail::ServerRequestDeleter> handle_;
  std::string operation_;
  std::mutex mutex_;
  object_reference<NVList> params_;
  bool raised_ = false;
};

// --- Object and ORB -------------------------------------------------------------

inline object_reference<Request> Object::_request(const std::string& operation) {
  if (!_rorb_handle()) throw NO_IMPLEMENT(0, CompletionStatus::COMPLETED_NO);
  return make_reference<Request>(rustorb::detail::object(rorb_object_clone(_rorb_handle())),
                                 operation, nullptr, nullptr, nullptr);
}

inline void Object::_create_request(const object_reference<Context>& ctx,
                                    const std::string& operation,
                                    const object_reference<NVList>& arg_list,
                                    const object_reference<NamedValue>& result,
                                    object_reference<Request>& request, Flags req_flags) {
  _create_request(ctx, operation, arg_list, result, nullptr, nullptr, request, req_flags);
}

inline void Object::_create_request(const object_reference<Context>& ctx,
                                    const std::string& operation,
                                    const object_reference<NVList>& arg_list,
                                    const object_reference<NamedValue>& result,
                                    const object_reference<ExceptionList>& exclist,
                                    const object_reference<ContextList>& ctxlist,
                                    object_reference<Request>& request, Flags req_flags) {
  (void)req_flags;
  if (ctx || ctxlist) throw NO_IMPLEMENT(0, CompletionStatus::COMPLETED_NO);
  if (!_rorb_handle()) throw NO_IMPLEMENT(0, CompletionStatus::COMPLETED_NO);
  request = make_reference<Request>(rustorb::detail::object(rorb_object_clone(_rorb_handle())),
                                    operation, arg_list, result, exclist);
}

inline object_reference<NVList> ORB::create_list(int32_t count) {
  if (count < 0) throw BAD_PARAM(0, CompletionStatus::COMPLETED_NO);
  return make_reference<NVList>();
}

inline object_reference<NamedValue> ORB::create_named_value() {
  return make_reference<NamedValue>("", Any(), 0);
}

inline object_reference<ExceptionList> ORB::create_exception_list() {
  return make_reference<ExceptionList>();
}

inline void ORB::send_multiple_requests_oneway(const std::vector<object_reference<Request>>& req) {
  for (const auto& r : req) r->send_oneway();
}

inline void ORB::send_multiple_requests_deferred(
    const std::vector<object_reference<Request>>& req) {
  std::lock_guard<std::mutex> lock(deferred_mutex_);
  for (const auto& r : req) {
    r->send_deferred();
    deferred_.push_back(r);
  }
}

inline bool ORB::poll_next_response() {
  std::lock_guard<std::mutex> lock(deferred_mutex_);
  for (const auto& r : deferred_) {
    if (r->poll_response()) return true;
  }
  return false;
}

// Waits for a response of the requests of send_multiple_requests_deferred
// (looking every millisecond) and gives its request, whose get_response
// then takes it at once. BAD_INV_ORDER when none is outstanding.
inline void ORB::get_next_response(object_reference<Request>& req) {
  for (;;) {
    {
      std::lock_guard<std::mutex> lock(deferred_mutex_);
      if (deferred_.empty()) throw BAD_INV_ORDER(0, CompletionStatus::COMPLETED_NO);
      for (auto it = deferred_.begin(); it != deferred_.end(); ++it) {
        if ((*it)->poll_response()) {
          req = *it;
          deferred_.erase(it);
          return;
        }
      }
    }
    std::this_thread::sleep_for(std::chrono::milliseconds(1));
  }
}

}  // namespace CORBA

namespace PortableServer {

// A servant that handles every operation itself, from a
// CORBA::ServerRequest (the DSI). Its references have the repository id
// _primary_interface gives (called with an empty id and a nil POA). A
// system exception raised from invoke is the reply; so is a user
// exception raised as CORBA::UnknownUserException. Other C++ exceptions
// are UNKNOWN.
class DynamicImplementation : public virtual ServantBase {
 public:
  virtual void invoke(CORBA::object_reference<CORBA::ServerRequest> request) = 0;
  virtual std::string _primary_interface(const ObjectId& oid,
                                         CORBA::object_reference<POA> poa) = 0;

  // The object of the running request for this servant, or the servant
  // activated implicitly in _default_POA().
  CORBA::object_reference<CORBA::Object> _this() { return _rorb_this(); }

  bool _is_a(const std::string& repository_id) override {
    return repository_id == _rorb_repository_id() ||
           repository_id == CORBA::Object::_rorb_repository_id;
  }

  const char* _rorb_repository_id() const override {
    std::call_once(primary_once_, [this] {
      primary_ = const_cast<DynamicImplementation*>(this)->_primary_interface(ObjectId(), nullptr);
    });
    return primary_.c_str();
  }

  rorb_status _rorb_dispatch(const char* op, size_t len, rorb_in* in,
                             rorb_server_reply* reply) override {
    return rustorb::detail::upcall(reply, [&]() -> rorb_status {
      rorb_server_request* h = nullptr;
      rustorb::detail::Exc exc;
      rustorb::detail::check(rorb_server_request_new(in, op, len, &h, &exc), exc);
      auto request = std::make_shared<CORBA::ServerRequest>(rustorb::detail::adopt, h,
                                                            std::string(op, len));
      try {
        invoke(CORBA::object_reference<CORBA::ServerRequest>(request));
      } catch (const CORBA::UnknownUserException& e) {
        request->set_exception(e.exception());
      }
      request->_rorb_reply(reply);
      return RORB_OK;
    });
  }

 protected:
  DynamicImplementation() = default;

 private:
  mutable std::once_flag primary_once_;
  mutable std::string primary_;
};

}  // namespace PortableServer

#undef RUSTORB_DII_EXCEPTION_BODY

#endif

// Callback AMI (docs/ami.md): the Messaging module as far as AMI needs it,
// Messaging::ReplyHandler and Messaging::ExceptionHolder, and what the
// sendc_ operations of code generated with rustorb-idl -bc++11 -Wbami
// call. The generated headers include it.
//
// A sendc_<op>(ami_handler, in and inout arguments) of a stub copies the
// arguments and returns at once. The reply comes to the handler's <op>
// (the result, then the inout and out arguments), an exception to its
// <op>_excep with an ExceptionHolder, whose raise_exception() throws it.
// Handlers are servants of POA_<Module>::AMI_<I>Handler, called like any
// servant, from a thread of the ORB's reply pool. What a handler throws is
// dropped. A nil handler drops the reply.

#ifndef RUSTORB_MESSAGING_H
#define RUSTORB_MESSAGING_H

#include <rustorb/cdr.h>

#include <cstring>
#include <exception>
#include <functional>
#include <map>
#include <memory>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

namespace rustorb {
namespace detail {

// --- user exceptions by repository id -----------------------------------------

// Reads the members of a user exception and makes it.
using UserExceptionReader = std::exception_ptr (*)(rorb_in* in);

inline std::map<std::string, UserExceptionReader>& user_exception_readers() {
  static std::map<std::string, UserExceptionReader> readers;
  return readers;
}

template <typename E> std::exception_ptr read_user_exception(rorb_in* in) {
  E e;
  cdr::codec<E>::read(in, e);
  return std::make_exception_ptr(e);
}

// Makes the user exception E known by its repository id, for the
// ExceptionHolders that come from another process (code generated with
// -Wbami registers each exception of the IDL file while static objects are
// made).
template <typename E> struct UserExceptionRegistration {
  UserExceptionRegistration() {
    user_exception_readers()[E::_rorb_repository_id] = &read_user_exception<E>;
  }
};

// --- the CDR of an ExceptionHolder ---------------------------------------------

inline bool host_little_endian() {
  const uint16_t one = 1;
  uint8_t first = 0;
  std::memcpy(&first, &one, 1);
  return first == 1;
}

// The bytes write(rorb_out*) writes, as marshaled_exception holds them:
// plain CDR from offset 0 in the host byte order.
template <typename W> std::vector<uint8_t> holder_cdr(const W& write) {
  struct Ctx {
    const W* write;
    std::exception_ptr error;
  } ctx{&write, nullptr};
  rorb_bytes bytes{};
  Exc exc;
  rorb_status status = rorb_cdr_encode(
      [](void* c, rorb_out* out) -> rorb_status {
        auto* x = static_cast<Ctx*>(c);
        try {
          (*x->write)(out);
          return RORB_OK;
        } catch (...) {
          x->error = std::current_exception();
          return RORB_CALLBACK_FAILED;
        }
      },
      &ctx, host_little_endian() ? 1 : 0, &bytes, &exc);
  if (status == RORB_CALLBACK_FAILED && ctx.error) std::rethrow_exception(ctx.error);
  check(status, exc);
  std::vector<uint8_t> v(bytes.data, bytes.data + bytes.len);
  rorb_bytes_free(bytes);
  return v;
}

inline std::vector<uint8_t> system_exception_cdr(const CORBA::SystemException& e) {
  return holder_cdr([&](rorb_out* out) {
    const char* id = e._rep_id();
    rorb_out_write_string(out, id, std::strlen(id));
    rorb_out_write_ulong(out, e.minor());
    rorb_out_write_ulong(out, static_cast<uint32_t>(e.completed()));
  });
}

template <typename E> std::vector<uint8_t> user_exception_cdr(const E& e) {
  return holder_cdr([&](rorb_out* out) {
    const char* id = E::_rorb_repository_id;
    rorb_out_write_string(out, id, std::strlen(id));
    cdr::codec<E>::write(out, e);
  });
}

// An owned stream over marshaled_exception.
struct HolderInput {
  rorb_in* in = nullptr;
  HolderInput(const std::vector<uint8_t>& data, bool little) {
    Exc exc;
    check(rorb_in_from_bytes(data.data(), data.size(), little ? 1 : 0, nullptr, &in, &exc), exc);
  }
  ~HolderInput() { rorb_in_release(in); }
  HolderInput(const HolderInput&) = delete;
  HolderInput& operator=(const HolderInput&) = delete;
  bool failed() const { return rorb_in_failed(in) != 0; }
};

}  // namespace detail
}  // namespace rustorb

namespace Messaging {

class ReplyHandler : public virtual ::CORBA::Object {
 public:
  static constexpr const char* _rorb_repository_id = "IDL:omg.org/Messaging/ReplyHandler:1.0";
  ReplyHandler(::rustorb::detail::adopt_t, rorb_object* _h) noexcept
      : ::CORBA::Object(::rustorb::detail::adopt, _h) {}

 protected:
  ReplyHandler() = default;
};

inline ::IDL::traits<::CORBA::TypeCode>::ref_type _rorb_tc_ReplyHandler() {
  static const ::IDL::traits<::CORBA::TypeCode>::ref_type _tc = ::rustorb::detail::tc_interface(
      ::CORBA::TCKind::tk_objref, ReplyHandler::_rorb_repository_id, "ReplyHandler");
  return _tc;
}

inline const ::IDL::traits<::CORBA::TypeCode>::ref_type _tc_ReplyHandler = _rorb_tc_ReplyHandler();

// The exception of an AMI reply. raise_exception() throws it;
// raise_exception_with_list(exc_list) throws it if it is a system
// exception or its TypeCode is in exc_list, else CORBA::UNKNOWN.
//
// The state is that of the IDL: is_system_exception, byte_order (true for
// little endian) and marshaled_exception, the exception as a reply carries
// it (the repository id, then the members, or the minor code and the
// completion status), plain CDR from offset 0 with the code sets UTF-8 and
// UTF-16. A holder from another process has only that: its user
// exceptions are read with the readers of the code generated with -Wbami
// (UserExceptionRegistration); an unknown one raises CORBA::UNKNOWN. A
// holder made in this process also keeps the exception itself.
class ExceptionHolder : public virtual ::CORBA::ValueBase {
 public:
  static constexpr const char* _rorb_repository_id = "IDL:omg.org/Messaging/ExceptionHolder:1.0";

  virtual void raise_exception() { _rorb_raise(nullptr); }

  virtual void raise_exception_with_list(
      const std::vector<::IDL::traits<::CORBA::TypeCode>::ref_type>& exc_list) {
    _rorb_raise(&exc_list);
  }

  const std::vector<const char*>& _rorb_ids() const override {
    static const std::vector<const char*> _ids{_rorb_repository_id};
    return _ids;
  }
  void _rorb_write_state(rorb_out* out) const override {
    ::rustorb::cdr::codec<bool>::write(out, is_system_exception());
    ::rustorb::cdr::codec<bool>::write(out, byte_order());
    ::rustorb::cdr::codec<std::vector<uint8_t>>::write(out, marshaled_exception());
  }
  void _rorb_read_state(rorb_in* in) override {
    ::rustorb::cdr::codec<bool>::read(in, is_system_exception());
    ::rustorb::cdr::codec<bool>::read(in, byte_order());
    ::rustorb::cdr::codec<std::vector<uint8_t>>::read(in, marshaled_exception());
  }

  // The exception itself and its repository id, for a holder made in this
  // process.
  void _rorb_exception(std::exception_ptr e, std::string id) {
    exception_ = std::move(e);
    exception_id_ = std::move(id);
  }

 protected:
  ExceptionHolder() = default;
  virtual bool is_system_exception() const = 0;
  virtual void is_system_exception(bool _v) = 0;
  virtual bool& is_system_exception() = 0;
  virtual bool byte_order() const = 0;
  virtual void byte_order(bool _v) = 0;
  virtual bool& byte_order() = 0;
  virtual const std::vector<uint8_t>& marshaled_exception() const = 0;
  virtual void marshaled_exception(const std::vector<uint8_t>& _v) = 0;
  virtual void marshaled_exception(std::vector<uint8_t>&& _v) = 0;
  virtual std::vector<uint8_t>& marshaled_exception() = 0;

 private:
  [[noreturn]] void _rorb_raise(
      const std::vector<::IDL::traits<::CORBA::TypeCode>::ref_type>* exc_list) {
    using ::CORBA::CompletionStatus;
    const auto unlisted = [] {
      return ::CORBA::UNKNOWN(0x4f4d0001, CompletionStatus::COMPLETED_YES);
    };
    const auto listed = [&](const std::string& id) {
      if (!exc_list) return true;
      for (const auto& tc : *exc_list) {
        if (tc && tc->id() == id) return true;
      }
      return false;
    };
    if (exception_) {
      if (is_system_exception() || listed(exception_id_)) std::rethrow_exception(exception_);
      throw unlisted();
    }
    ::rustorb::detail::HolderInput input(marshaled_exception(), byte_order());
    std::string id;
    ::rustorb::cdr::codec<std::string>::read(input.in, id);
    if (input.failed()) throw ::CORBA::MARSHAL(0, CompletionStatus::COMPLETED_YES);
    if (is_system_exception()) {
      uint32_t minor = rorb_in_read_ulong(input.in);
      uint32_t completed = rorb_in_read_ulong(input.in);
      if (input.failed()) throw ::CORBA::MARSHAL(0, CompletionStatus::COMPLETED_YES);
      ::rustorb::detail::throw_system(std::move(id), minor,
                                      static_cast<CompletionStatus>(completed % 3));
    }
    if (!listed(id)) throw unlisted();
    auto& readers = ::rustorb::detail::user_exception_readers();
    auto it = readers.find(id);
    if (it == readers.end()) throw unlisted();
    std::exception_ptr e = it->second(input.in);
    if (input.failed()) throw ::CORBA::MARSHAL(0, CompletionStatus::COMPLETED_YES);
    std::rethrow_exception(e);
  }

  std::exception_ptr exception_;
  std::string exception_id_;
};

inline ::IDL::traits<::CORBA::TypeCode>::ref_type _rorb_tc_ExceptionHolder() {
  static const ::IDL::traits<::CORBA::TypeCode>::ref_type _tc = ::rustorb::detail::tc_value(
      ExceptionHolder::_rorb_repository_id, "ExceptionHolder", ::CORBA::VM_NONE, nullptr,
      {{"is_system_exception", ::rustorb::cdr::type_code<bool>::get(), ::CORBA::PRIVATE_MEMBER},
       {"byte_order", ::rustorb::cdr::type_code<bool>::get(), ::CORBA::PRIVATE_MEMBER},
       {"marshaled_exception",
        ::rustorb::detail::tc_sequence(::rustorb::cdr::type_code<uint8_t>::get(), 0),
        ::CORBA::PRIVATE_MEMBER}});
  return _tc;
}

inline const ::IDL::traits<::CORBA::TypeCode>::ref_type _tc_ExceptionHolder =
    _rorb_tc_ExceptionHolder();

}  // namespace Messaging

namespace OBV_Messaging {

class ExceptionHolder : public virtual ::Messaging::ExceptionHolder {
 public:
  ExceptionHolder() = default;
  ExceptionHolder(bool is_system_exception, bool byte_order,
                  const std::vector<uint8_t>& marshaled_exception)
      : _m_is_system_exception(is_system_exception),
        _m_byte_order(byte_order),
        _m_marshaled_exception(marshaled_exception) {}

  ::IDL::traits<::CORBA::ValueBase>::ref_type _copy_value() const override {
    return ::IDL::traits<::CORBA::ValueBase>::ref_type(std::make_shared<ExceptionHolder>(*this));
  }

 protected:
  bool is_system_exception() const override { return _m_is_system_exception; }
  void is_system_exception(bool _v) override { _m_is_system_exception = _v; }
  bool& is_system_exception() override { return _m_is_system_exception; }
  bool byte_order() const override { return _m_byte_order; }
  void byte_order(bool _v) override { _m_byte_order = _v; }
  bool& byte_order() override { return _m_byte_order; }
  const std::vector<uint8_t>& marshaled_exception() const override {
    return _m_marshaled_exception;
  }
  void marshaled_exception(const std::vector<uint8_t>& _v) override {
    _m_marshaled_exception = _v;
  }
  void marshaled_exception(std::vector<uint8_t>&& _v) override {
    _m_marshaled_exception = std::move(_v);
  }
  std::vector<uint8_t>& marshaled_exception() override { return _m_marshaled_exception; }

 private:
  bool _m_is_system_exception{};
  bool _m_byte_order{};
  std::vector<uint8_t> _m_marshaled_exception{};
};

}  // namespace OBV_Messaging

namespace POA_Messaging {

class ReplyHandler : public virtual ::PortableServer::ServantBase {
 public:
  ::IDL::traits<::Messaging::ReplyHandler>::ref_type _this() {
    return ::IDL::traits<::Messaging::ReplyHandler>::narrow(_rorb_this());
  }
  bool _is_a(const std::string& repository_id) override {
    return repository_id == ::Messaging::ReplyHandler::_rorb_repository_id ||
           repository_id == "IDL:omg.org/CORBA/Object:1.0";
  }
  const char* _rorb_repository_id() const override {
    return ::Messaging::ReplyHandler::_rorb_repository_id;
  }
  rorb_status _rorb_dispatch(const char* op, size_t len, rorb_in* in,
                             rorb_server_reply* reply) override {
    std::string_view name(op, len);
    return ::rustorb::detail::upcall(reply, [&]() -> rorb_status {
      if (_rorb_ops_Messaging_ReplyHandler(name, in, reply)) return RORB_OK;
      return ::rustorb::detail::bad_operation(reply);
    });
  }

 protected:
  ReplyHandler() = default;
  bool _rorb_ops_Messaging_ReplyHandler(std::string_view op, rorb_in* in,
                                        rorb_server_reply* reply) {
    (void)op;
    (void)in;
    (void)reply;
    return false;
  }
};

}  // namespace POA_Messaging

namespace CORBA {

template <> struct servant_traits<::Messaging::ReplyHandler> {
  using base_type = ::POA_Messaging::ReplyHandler;
  using ref_type = ::CORBA::servant_reference<base_type>;
};

}  // namespace CORBA

namespace rustorb {
namespace cdr {

template <> struct type_code<::Messaging::ReplyHandler> {
  static ::IDL::traits<::CORBA::TypeCode>::ref_type get() {
    return ::Messaging::_rorb_tc_ReplyHandler();
  }
};
template <> struct type_code<::Messaging::ExceptionHolder> {
  static ::IDL::traits<::CORBA::TypeCode>::ref_type get() {
    return ::Messaging::_rorb_tc_ExceptionHolder();
  }
};

}  // namespace cdr

namespace detail {

inline const InterfaceRegistration reply_handler_registration(
    ::Messaging::ReplyHandler::_rorb_repository_id,
    {::Messaging::ReplyHandler::_rorb_repository_id});

inline const ValueRegistration exception_holder_registration(
    ::Messaging::ExceptionHolder::_rorb_repository_id,
    ::CORBA::ValueFactory(
        std::make_shared<DefaultValueFactory<::OBV_Messaging::ExceptionHolder>>()),
    ::Messaging::_rorb_tc_ExceptionHolder());

// --- sendc ------------------------------------------------------------------------

template <typename E> bool take_user_exception_for_holder(std::exception_ptr& error,
                                                          std::vector<uint8_t>& data,
                                                          std::string& taken,
                                                          std::string_view id, rorb_in* in) {
  if (id != E::_rorb_repository_id) return false;
  E e;
  cdr::codec<E>::read(in, e);
  data = user_exception_cdr(e);
  error = std::make_exception_ptr(e);
  taken = E::_rorb_repository_id;
  return true;
}

// A holder of the exception e (with its CDR) made in this process.
inline ::IDL::traits<::Messaging::ExceptionHolder>::ref_type make_holder(
    bool system, std::vector<uint8_t> data, std::exception_ptr e, std::string id) {
  auto h = ::CORBA::make_reference<::OBV_Messaging::ExceptionHolder>(
      system, host_little_endian(), std::move(data));
  h->_rorb_exception(std::move(e), std::move(id));
  return h;
}

inline ::IDL::traits<::Messaging::ExceptionHolder>::ref_type system_holder(
    const CORBA::SystemException& e) {
  std::exception_ptr p;
  try {
    throw_system(e._rep_id(), e.minor(), e.completed());
  } catch (...) {
    p = std::current_exception();
  }
  return make_holder(true, system_exception_cdr(e), p, e._rep_id());
}

// Sends op on the stub self without waiting (callback AMI): write writes
// the in and inout arguments (copies of them: it may run more than once,
// after sendc returns); read(rorb_in*) reads the reply and returns the
// function that calls the handler with it; excep(handler, holder) calls
// the handler's <op>_excep. Excs are the user exceptions of op. The
// handler is called from a thread of the reply pool; what it throws is
// dropped.
template <typename... Excs, typename H, typename W, typename R, typename X>
void sendc(const CORBA::Object* self, const char* op, H handler, W write, R read, X excep) {
  struct Ctx {
    H handler;
    W write;
    R read;
    X excep;
    std::function<void(const H&)> deliver;
    std::exception_ptr error;
    std::vector<uint8_t> user_cdr;
    std::string user_id;
  };
  rorb_object* handle = self->_rorb_handle();
  if (!handle) throw CORBA::INV_OBJREF(0, CORBA::CompletionStatus::COMPLETED_NO);
  auto* ctx = new Ctx{std::move(handler), std::move(write), std::move(read), std::move(excep),
                      nullptr, nullptr, {}, {}};
  rorb_invocation inv{};
  inv.operation = op;
  inv.operation_len = std::strlen(op);
  inv.oneway = 0;
  inv.timeout_us = -1;
  inv.context = ctx;
  inv.write_args = [](void* c, rorb_out* out) -> rorb_status {
    auto* x = static_cast<Ctx*>(c);
    try {
      x->write(out);
      return RORB_OK;
    } catch (...) {
      x->error = std::current_exception();
      return RORB_CALLBACK_FAILED;
    }
  };
  inv.read_reply = [](void* c, rorb_in* in) -> rorb_status {
    auto* x = static_cast<Ctx*>(c);
    try {
      if (x->handler) x->deliver = x->read(in);
      return RORB_OK;
    } catch (...) {
      x->error = std::current_exception();
      return RORB_CALLBACK_FAILED;
    }
  };
  inv.read_user_exception = [](void* c, const char* id, size_t len, rorb_in* in) -> rorb_status {
    auto* x = static_cast<Ctx*>(c);
    try {
      std::string_view name(id, len);
      bool taken = (take_user_exception_for_holder<Excs>(x->error, x->user_cdr, x->user_id,
                                                          name, in) ||
                    ... || false);
      (void)name;
      (void)in;
      return taken ? RORB_USER_EXCEPTION : RORB_SYSTEM_EXCEPTION;
    } catch (...) {
      x->error = std::current_exception();
      return RORB_CALLBACK_FAILED;
    }
  };
  auto done = [](void* c, rorb_status status, const rorb_exception* exc) {
    std::unique_ptr<Ctx> x(static_cast<Ctx*>(c));
    if (!x->handler) return;
    try {
      ::IDL::traits<::Messaging::ExceptionHolder>::ref_type holder;
      switch (status) {
        case RORB_OK:
          if (x->deliver) x->deliver(x->handler);
          return;
        case RORB_USER_EXCEPTION:
          holder = make_holder(false, std::move(x->user_cdr), x->error, x->user_id);
          break;
        case RORB_SYSTEM_EXCEPTION: {
          std::string id(exc->repository_id ? exc->repository_id : "", exc->repository_id_len);
          auto completed = static_cast<CORBA::CompletionStatus>(exc->completed % 3);
          try {
            throw_system(id, exc->minor, completed);
          } catch (const CORBA::SystemException& e) {
            holder = system_holder(e);
          }
          break;
        }
        default:
          // What write or read threw (a bound exceeded, say), else an
          // error of the ORB.
          try {
            if (x->error) std::rethrow_exception(x->error);
            throw CORBA::INTERNAL(0, CORBA::CompletionStatus::COMPLETED_MAYBE);
          } catch (const CORBA::SystemException& e) {
            holder = system_holder(e);
          } catch (...) {
            holder = system_holder(CORBA::UNKNOWN(0, CORBA::CompletionStatus::COMPLETED_MAYBE));
          }
          break;
      }
      x->excep(x->handler, std::move(holder));
    } catch (...) {
      // The handler's exception has no one to go to.
    }
  };
  Exc exc;
  rorb_status status = rorb_object_sendc(handle, &inv, done, &exc);
  if (status != RORB_OK) {
    delete ctx;
    check(status, exc);
  }
}

}  // namespace detail
}  // namespace rustorb

#endif

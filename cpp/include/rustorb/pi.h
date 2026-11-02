// Portable Interceptors and IOP::Codec (IDL to C++11), on the rorb_pi,
// rorb_cri, rorb_sri, rorb_ior_info, rorb_init_info and rorb_codec
// functions of the C ABI. See docs/portable-interceptors.md and
// docs/cpp-binding.md.
//
//   class Init : public PortableInterceptor::ORBInitializer {
//    public:
//     void pre_init(IDL::traits<PortableInterceptor::ORBInitInfo>::ref_type info) override {
//       info->add_client_request_interceptor(CORBA::make_reference<MyInterceptor>());
//     }
//   };
//   PortableInterceptor::register_orb_initializer(CORBA::make_reference<Init>());
//   auto orb = CORBA::ORB_init(argc, argv, "my-orb");
//
// The info objects a point gets are only usable during the point: kept
// past it they raise BAD_INV_ORDER (minor 14), and an ORBInitInfo kept
// past ORB_init raises OBJECT_NOT_EXIST (minor 10). The operations of the
// interceptors and initializers do nothing unless overridden.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

#ifndef RUSTORB_PI_H
#define RUSTORB_PI_H

#include <rustorb/cdr.h>

#include <memory>
#include <mutex>
#include <string>
#include <vector>

#define RUSTORB_PI_EXCEPTION_BODY(NAME, ID)            \
  const char* _name() const override { return #NAME; } \
  const char* _rep_id() const override { return ID; }  \
  [[noreturn]] void _raise() const override { throw *this; }

// A member of a struct of the mapping: the getter, a setter and a field.
#define RUSTORB_PI_MEMBER(TYPE, NAME)          \
 public:                                       \
  const TYPE& NAME() const { return NAME##_; } \
  TYPE& NAME() { return NAME##_; }             \
  void NAME(const TYPE& v) { NAME##_ = v; }    \
                                               \
 private:                                      \
  TYPE NAME##_{};

// ---------------------------------------------------------------------------
// CORBA::ParameterMode, Dynamic

namespace CORBA {
enum class ParameterMode : uint32_t { PARAM_IN, PARAM_OUT, PARAM_INOUT };
using StringSeq = std::vector<std::string>;
}  // namespace CORBA

namespace Messaging {
using SyncScope = int16_t;
}  // namespace Messaging

namespace Dynamic {
class Parameter {
 public:
  Parameter() = default;
  Parameter(CORBA::Any argument, CORBA::ParameterMode mode)
      : argument_(std::move(argument)), mode_(mode) {}
  RUSTORB_PI_MEMBER(CORBA::Any, argument)
  RUSTORB_PI_MEMBER(CORBA::ParameterMode, mode)
};
using ParameterList = std::vector<Parameter>;
using ContextList = std::vector<std::string>;
using ExceptionList = std::vector<CORBA::object_reference<CORBA::TypeCode>>;
using RequestContext = std::vector<std::string>;
}  // namespace Dynamic

// ---------------------------------------------------------------------------
// IOP

namespace IOP {

using ProfileId = uint32_t;
using ComponentId = uint32_t;
using ServiceId = uint32_t;
using EncodingFormat = int16_t;

// Profile tags.
constexpr ProfileId TAG_INTERNET_IOP = 0;
constexpr ProfileId TAG_MULTIPLE_COMPONENTS = 1;

// Component tags (those the core knows).
constexpr ComponentId TAG_ORB_TYPE = 0;
constexpr ComponentId TAG_CODE_SETS = 1;
constexpr ComponentId TAG_POLICIES = 2;
constexpr ComponentId TAG_ALTERNATE_IIOP_ADDRESS = 3;
constexpr ComponentId TAG_COMPLETE_OBJECT_KEY = 5;
constexpr ComponentId TAG_ENDPOINT_ID_POSITION = 6;
constexpr ComponentId TAG_LOCATION_POLICY = 12;
constexpr ComponentId TAG_ASSOCIATION_OPTIONS = 13;
constexpr ComponentId TAG_SEC_NAME = 14;
constexpr ComponentId TAG_SSL_SEC_TRANS = 20;
constexpr ComponentId TAG_JAVA_CODEBASE = 25;
constexpr ComponentId TAG_CSI_SEC_MECH_LIST = 33;
constexpr ComponentId TAG_NULL_TAG = 34;
constexpr ComponentId TAG_TLS_SEC_TRANS = 36;

// Service context ids.
constexpr ServiceId TransactionService = 0;
constexpr ServiceId CodeSets = 1;
constexpr ServiceId ChainBypassCheck = 2;
constexpr ServiceId ChainBypassInfo = 3;
constexpr ServiceId LogicalThreadId = 4;
constexpr ServiceId BI_DIR_IIOP = 5;
constexpr ServiceId SendingContextRunTime = 6;
constexpr ServiceId INVOCATION_POLICIES = 7;
constexpr ServiceId FORWARDED_IDENTITY = 8;
constexpr ServiceId UnknownExceptionInfo = 9;
constexpr ServiceId RTCorbaPriority = 10;
constexpr ServiceId RTCorbaPriorityRange = 11;
constexpr ServiceId FT_GROUP_VERSION = 12;
constexpr ServiceId FT_REQUEST = 13;
constexpr ServiceId ExceptionDetailMessage = 14;

constexpr EncodingFormat ENCODING_CDR_ENCAPS = RORB_ENCODING_CDR_ENCAPS;

class TaggedProfile {
 public:
  TaggedProfile() = default;
  TaggedProfile(ProfileId tag, std::vector<uint8_t> profile_data)
      : tag_(tag), profile_data_(std::move(profile_data)) {}
  RUSTORB_PI_MEMBER(ProfileId, tag)
  RUSTORB_PI_MEMBER(std::vector<uint8_t>, profile_data)
};

class TaggedComponent {
 public:
  TaggedComponent() = default;
  TaggedComponent(ComponentId tag, std::vector<uint8_t> component_data)
      : tag_(tag), component_data_(std::move(component_data)) {}
  RUSTORB_PI_MEMBER(ComponentId, tag)
  RUSTORB_PI_MEMBER(std::vector<uint8_t>, component_data)
};
using TaggedComponentSeq = std::vector<TaggedComponent>;

class ServiceContext {
 public:
  ServiceContext() = default;
  ServiceContext(ServiceId context_id, std::vector<uint8_t> context_data)
      : context_id_(context_id), context_data_(std::move(context_data)) {}
  RUSTORB_PI_MEMBER(ServiceId, context_id)
  RUSTORB_PI_MEMBER(std::vector<uint8_t>, context_data)
};
using ServiceContextList = std::vector<ServiceContext>;

class Encoding {
 public:
  Encoding() = default;
  Encoding(EncodingFormat format, uint8_t major_version, uint8_t minor_version)
      : format_(format), major_version_(major_version), minor_version_(minor_version) {}
  RUSTORB_PI_MEMBER(EncodingFormat, format)
  RUSTORB_PI_MEMBER(uint8_t, major_version)
  RUSTORB_PI_MEMBER(uint8_t, minor_version)
};

// CDR encapsulations of anys (encode / decode) and of values alone
// (encode_value / decode_value).
class Codec : public CORBA::LocalObject {
 public:
  class InvalidTypeForEncoding : public CORBA::UserException {
   public:
    RUSTORB_PI_EXCEPTION_BODY(InvalidTypeForEncoding,
                              "IDL:omg.org/IOP/Codec/InvalidTypeForEncoding:1.0")
  };
  class FormatMismatch : public CORBA::UserException {
   public:
    RUSTORB_PI_EXCEPTION_BODY(FormatMismatch, "IDL:omg.org/IOP/Codec/FormatMismatch:1.0")
  };
  class TypeMismatch : public CORBA::UserException {
   public:
    RUSTORB_PI_EXCEPTION_BODY(TypeMismatch, "IDL:omg.org/IOP/Codec/TypeMismatch:1.0")
  };

  Codec(rustorb::detail::adopt_t, rorb_codec* handle) noexcept : h_(handle) {}
  ~Codec() override { rorb_codec_release(h_); }
  Codec(const Codec&) = delete;
  Codec& operator=(const Codec&) = delete;

  std::vector<uint8_t> encode(const CORBA::Any& data) { return encode(data, 0); }
  CORBA::Any decode(const std::vector<uint8_t>& data) { return decode(data, nullptr); }
  std::vector<uint8_t> encode_value(const CORBA::Any& data) { return encode(data, 1); }
  CORBA::Any decode_value(const std::vector<uint8_t>& data,
                          const CORBA::object_reference<CORBA::TypeCode>& tc) {
    if (!tc) throw CORBA::BAD_PARAM(0, CORBA::CompletionStatus::COMPLETED_NO);
    return decode(data, tc->_rorb_tc());
  }

 private:
  static void check(rorb_status status, rorb_exception& exc) {
    if (status == RORB_LOCAL_EXCEPTION) {
      switch (exc.local) {
        case RORB_CODEC_INVALID_TYPE_FOR_ENCODING:
          throw InvalidTypeForEncoding();
        case RORB_CODEC_FORMAT_MISMATCH:
          throw FormatMismatch();
        case RORB_CODEC_TYPE_MISMATCH:
          throw TypeMismatch();
        default:
          break;
      }
    }
    rustorb::detail::check(status, exc);
  }
  std::vector<uint8_t> encode(const CORBA::Any& data, int32_t value_only) {
    rorb_bytes out{};
    rustorb::detail::Exc exc;
    check(rorb_codec_encode(h_, data._rorb_handle(), value_only, &out, &exc), exc);
    return rustorb::detail::take_bytes(out);
  }
  CORBA::Any decode(const std::vector<uint8_t>& data, const rorb_typecode* tc) {
    rorb_any* any = nullptr;
    rustorb::detail::Exc exc;
    check(rorb_codec_decode(h_, data.data(), data.size(), tc, &any, &exc), exc);
    return CORBA::Any(rustorb::detail::adopt, any);
  }

  rorb_codec* h_;
};

// The initial reference "CodecFactory" (and ORBInitInfo::codec_factory).
class CodecFactory : public CORBA::LocalObject {
 public:
  class UnknownEncoding : public CORBA::UserException {
   public:
    RUSTORB_PI_EXCEPTION_BODY(UnknownEncoding, "IDL:omg.org/IOP/CodecFactory/UnknownEncoding:1.0")
  };

  // Takes over orb.
  CodecFactory(rustorb::detail::adopt_t, rorb_orb* orb) noexcept : orb_(orb) {}
  ~CodecFactory() override { rorb_orb_release(orb_); }
  CodecFactory(const CodecFactory&) = delete;
  CodecFactory& operator=(const CodecFactory&) = delete;

  CORBA::object_reference<Codec> create_codec(const Encoding& enc) {
    rorb_codec* codec = nullptr;
    rustorb::detail::Exc exc;
    rorb_status status = rorb_orb_create_codec(orb_, enc.format(), enc.major_version(),
                                               enc.minor_version(), &codec, &exc);
    if (status == RORB_LOCAL_EXCEPTION && exc.local == RORB_UNKNOWN_ENCODING) {
      throw UnknownEncoding();
    }
    rustorb::detail::check(status, exc);
    return CORBA::object_reference<Codec>(std::make_shared<Codec>(rustorb::detail::adopt, codec));
  }

 private:
  rorb_orb* orb_;
};

}  // namespace IOP

// ---------------------------------------------------------------------------
// PortableInterceptor

namespace PortableInterceptor {

using SlotId = uint32_t;
using ReplyStatus = int16_t;
using AdapterName = std::vector<std::string>;
using ORBId = std::string;
using ServerId = std::string;
using ObjectId = std::vector<uint8_t>;

constexpr ReplyStatus SUCCESSFUL = RORB_PI_SUCCESSFUL;
constexpr ReplyStatus SYSTEM_EXCEPTION = RORB_PI_SYSTEM_EXCEPTION;
constexpr ReplyStatus USER_EXCEPTION = RORB_PI_USER_EXCEPTION;
constexpr ReplyStatus LOCATION_FORWARD = RORB_PI_LOCATION_FORWARD;
constexpr ReplyStatus TRANSPORT_RETRY = RORB_PI_TRANSPORT_RETRY;
constexpr ReplyStatus UNKNOWN = RORB_PI_UNKNOWN;

class ForwardRequest : public CORBA::UserException {
 public:
  ForwardRequest() = default;
  explicit ForwardRequest(CORBA::object_reference<CORBA::Object> forward)
      : forward_(std::move(forward)) {}
  const CORBA::object_reference<CORBA::Object>& forward() const { return forward_; }
  void forward(CORBA::object_reference<CORBA::Object> f) { forward_ = std::move(f); }
  RUSTORB_PI_EXCEPTION_BODY(ForwardRequest, "IDL:omg.org/PortableInterceptor/ForwardRequest:1.0")
 private:
  CORBA::object_reference<CORBA::Object> forward_;
};

class InvalidSlot : public CORBA::UserException {
 public:
  RUSTORB_PI_EXCEPTION_BODY(InvalidSlot, "IDL:omg.org/PortableInterceptor/InvalidSlot:1.0")
};

class Interceptor : public virtual CORBA::LocalObject {
 public:
  // The name, unique among the interceptors of a kind in an ORB; any
  // number may be anonymous ("").
  virtual std::string name() { return std::string(); }
  // When the ORB is destroyed.
  virtual void destroy() {}
};

namespace rustorb_detail {

// An info handle the binding lends to the application for one point:
// close() ends the loan, after which uses raise what gone() throws. Uses
// from other threads during the point wait for each other.
template <typename H>
class Loan {
 public:
  Loan(H* h, void (*gone)()) noexcept : h_(h), gone_(gone) {}
  template <typename F>
  auto operator()(const F& f) const {
    std::lock_guard<std::mutex> lock(m_);
    if (!h_) gone_();
    return f(h_);
  }
  void close() noexcept {
    std::lock_guard<std::mutex> lock(m_);
    h_ = nullptr;
  }

 private:
  mutable std::mutex m_;
  H* h_;
  void (*gone_)();
};

[[noreturn]] inline void invalid_point() {
  throw CORBA::BAD_INV_ORDER(0x4f4d0000 | 14, CORBA::CompletionStatus::COMPLETED_NO);
}

[[noreturn]] inline void init_info_gone() {
  throw CORBA::OBJECT_NOT_EXIST(0x4f4d0000 | 10, CORBA::CompletionStatus::COMPLETED_NO);
}

// check(), with InvalidSlot.
inline void check_slot(rorb_status status, rorb_exception& exc) {
  if (status == RORB_LOCAL_EXCEPTION && exc.local == RORB_INVALID_SLOT) throw InvalidSlot();
  rustorb::detail::check(status, exc);
}

inline void add_string(void* ctx, const char* s, size_t len) {
  static_cast<std::vector<std::string>*>(ctx)->emplace_back(s, len);
}

inline std::string borrowed_string(const char* s, size_t len) {
  return s ? std::string(s, len) : std::string();
}

inline CORBA::object_reference<CORBA::Policy> policy(uint32_t type, rorb_status status,
                                                     uint64_t raw, rorb_exception& exc) {
  rustorb::detail::check(status, exc);
  return rustorb::detail::make_policy(type, raw);
}

inline std::vector<uint8_t> bytes(const uint8_t* data, size_t len) {
  return std::vector<uint8_t>(data, data + len);
}

}  // namespace rustorb_detail

// What ClientRequestInfo and ServerRequestInfo share. Arguments,
// exceptions and the result are NO_RESOURCES (minor 1) but for requests
// of the DII or the DSI (of other bindings); contexts and
// operation_context always are.
class RequestInfo : public virtual CORBA::LocalObject {
 public:
  virtual uint32_t request_id() = 0;
  virtual std::string operation() = 0;
  virtual Dynamic::ParameterList arguments() = 0;
  virtual Dynamic::ExceptionList exceptions() = 0;
  virtual Dynamic::ContextList contexts() {
    throw CORBA::NO_RESOURCES(0x4f4d0000 | 1, CORBA::CompletionStatus::COMPLETED_NO);
  }
  virtual Dynamic::RequestContext operation_context() {
    throw CORBA::NO_RESOURCES(0x4f4d0000 | 1, CORBA::CompletionStatus::COMPLETED_NO);
  }
  virtual CORBA::Any result() = 0;
  virtual bool response_expected() = 0;
  virtual Messaging::SyncScope sync_scope() = 0;
  virtual ReplyStatus reply_status() = 0;
  virtual CORBA::object_reference<CORBA::Object> forward_reference() = 0;
  virtual CORBA::Any get_slot(SlotId id) = 0;
  virtual IOP::ServiceContext get_request_service_context(IOP::ServiceId id) = 0;
  virtual IOP::ServiceContext get_reply_service_context(IOP::ServiceId id) = 0;
};

// The RequestInfo operations of one prefix of the C ABI (cri or sri).
#define RUSTORB_PI_REQUEST_INFO(P)                                                             \
  uint32_t request_id() override {                                                             \
    return loan_([](const auto* h) { return rorb_##P##_request_id(h); });                      \
  }                                                                                            \
  std::string operation() override {                                                           \
    return loan_([](const auto* h) {                                                           \
      size_t len = 0;                                                                          \
      const char* s = rorb_##P##_operation(h, &len);                                           \
      return rustorb_detail::borrowed_string(s, len);                                          \
    });                                                                                        \
  }                                                                                            \
  Dynamic::ParameterList arguments() override {                                                \
    return loan_([](const auto* h) {                                                           \
      Dynamic::ParameterList list;                                                             \
      rustorb::detail::Exc exc;                                                                \
      rustorb::detail::check(rorb_##P##_arguments(                                             \
                                 h, &list,                                                     \
                                 [](void* ctx, rorb_any* a, uint32_t mode) {                   \
                                   static_cast<Dynamic::ParameterList*>(ctx)->emplace_back(    \
                                       CORBA::Any(rustorb::detail::adopt, a),                  \
                                       static_cast<CORBA::ParameterMode>(mode));               \
                                 },                                                            \
                                 &exc),                                                        \
                             exc);                                                             \
      return list;                                                                             \
    });                                                                                        \
  }                                                                                            \
  Dynamic::ExceptionList exceptions() override {                                               \
    return loan_([](const auto* h) {                                                           \
      Dynamic::ExceptionList list;                                                             \
      rustorb::detail::Exc exc;                                                                \
      rustorb::detail::check(                                                                  \
          rorb_##P##_exceptions(                                                               \
              h, &list,                                                                        \
              [](void* ctx, rorb_typecode* tc) {                                               \
                static_cast<Dynamic::ExceptionList*>(ctx)->emplace_back(                       \
                    std::make_shared<CORBA::TypeCode>(rustorb::detail::adopt, tc));            \
              },                                                                               \
              &exc),                                                                           \
          exc);                                                                                \
      return list;                                                                             \
    });                                                                                        \
  }                                                                                            \
  CORBA::Any result() override {                                                               \
    return loan_([](const auto* h) {                                                           \
      rorb_any* a = nullptr;                                                                   \
      rustorb::detail::Exc exc;                                                                \
      rustorb::detail::check(rorb_##P##_result(h, &a, &exc), exc);                             \
      return CORBA::Any(rustorb::detail::adopt, a);                                            \
    });                                                                                        \
  }                                                                                            \
  bool response_expected() override {                                                          \
    return loan_([](const auto* h) { return rorb_##P##_response_expected(h) != 0; });          \
  }                                                                                            \
  Messaging::SyncScope sync_scope() override {                                                 \
    return loan_([](const auto* h) { return rorb_##P##_sync_scope(h); });                      \
  }                                                                                            \
  ReplyStatus reply_status() override {                                                        \
    return loan_([](const auto* h) {                                                           \
      int16_t s = 0;                                                                           \
      rustorb::detail::Exc exc;                                                                \
      rustorb::detail::check(rorb_##P##_reply_status(h, &s, &exc), exc);                       \
      return s;                                                                                \
    });                                                                                        \
  }                                                                                            \
  CORBA::object_reference<CORBA::Object> forward_reference() override {                        \
    return loan_([](const auto* h) {                                                           \
      rorb_object* obj = nullptr;                                                              \
      rustorb::detail::Exc exc;                                                                \
      rustorb::detail::check(rorb_##P##_forward_reference(h, &obj, &exc), exc);                \
      return rustorb::detail::object(obj);                                                     \
    });                                                                                        \
  }                                                                                            \
  CORBA::Any get_slot(SlotId id) override {                                                    \
    return loan_([id](const auto* h) {                                                         \
      rorb_any* a = nullptr;                                                                   \
      rustorb::detail::Exc exc;                                                                \
      rustorb_detail::check_slot(rorb_##P##_get_slot(h, id, &a, &exc), exc);                   \
      return CORBA::Any(rustorb::detail::adopt, a);                                            \
    });                                                                                        \
  }                                                                                            \
  IOP::ServiceContext get_request_service_context(IOP::ServiceId id) override {                \
    return loan_([id](const auto* h) {                                                         \
      rorb_bytes data{};                                                                       \
      rustorb::detail::Exc exc;                                                                \
      rustorb::detail::check(rorb_##P##_get_request_service_context(h, id, &data, &exc), exc); \
      return IOP::ServiceContext(id, rustorb::detail::take_bytes(data));                       \
    });                                                                                        \
  }                                                                                            \
  IOP::ServiceContext get_reply_service_context(IOP::ServiceId id) override {                  \
    return loan_([id](const auto* h) {                                                         \
      rorb_bytes data{};                                                                       \
      rustorb::detail::Exc exc;                                                                \
      rustorb::detail::check(rorb_##P##_get_reply_service_context(h, id, &data, &exc), exc);   \
      return IOP::ServiceContext(id, rustorb::detail::take_bytes(data));                       \
    });                                                                                        \
  }                                                                                            \
  void _rorb_close() noexcept { loan_.close(); }                                               \
                                                                                               \
 private:                                                                                      \
  rustorb_detail::Loan<const rorb_##P##_info_t> loan_;                                         \
                                                                                               \
 public:

using rorb_cri_info_t = rorb_client_request_info;
using rorb_sri_info_t = rorb_server_request_info;

class ClientRequestInfo : public virtual RequestInfo {
 public:
  ClientRequestInfo(rustorb::detail::adopt_t, const rorb_client_request_info* h)
      : loan_(h, &rustorb_detail::invalid_point) {}

  RUSTORB_PI_REQUEST_INFO(cri)

  CORBA::object_reference<CORBA::Object> target() {
    return loan_([](const auto* h) { return rustorb::detail::object(rorb_cri_target(h)); });
  }
  CORBA::object_reference<CORBA::Object> effective_target() {
    return loan_(
        [](const auto* h) { return rustorb::detail::object(rorb_cri_effective_target(h)); });
  }
  IOP::TaggedProfile effective_profile() {
    return loan_([](const auto* h) {
      rorb_bytes data{};
      uint32_t tag = rorb_cri_effective_profile(h, &data);
      return IOP::TaggedProfile(tag, rustorb::detail::take_bytes(data));
    });
  }
  CORBA::Any received_exception() {
    return loan_([](const auto* h) {
      rorb_any* a = nullptr;
      rustorb::detail::Exc exc;
      rustorb::detail::check(rorb_cri_received_exception(h, &a, &exc), exc);
      return CORBA::Any(rustorb::detail::adopt, a);
    });
  }
  std::string received_exception_id() {
    return loan_([](const auto* h) {
      rorb_bytes id{};
      rustorb::detail::Exc exc;
      rustorb::detail::check(rorb_cri_received_exception_id(h, &id, &exc), exc);
      return rustorb::detail::take_string(id);
    });
  }
  // BAD_PARAM (minor 28) when the effective profile has no such component.
  IOP::TaggedComponent get_effective_component(IOP::ComponentId id) {
    return loan_([id](const auto* h) {
      rorb_bytes data{};
      rustorb::detail::Exc exc;
      rustorb::detail::check(rorb_cri_get_effective_component(h, id, &data, &exc), exc);
      return IOP::TaggedComponent(id, rustorb::detail::take_bytes(data));
    });
  }
  IOP::TaggedComponentSeq get_effective_components(IOP::ComponentId id) {
    return loan_([id](const auto* h) {
      IOP::TaggedComponentSeq list;
      rustorb::detail::Exc exc;
      rustorb::detail::check(rorb_cri_get_effective_components(
                                 h, id, &list,
                                 [](void* ctx, uint32_t tag, const uint8_t* data, size_t len) {
                                   static_cast<IOP::TaggedComponentSeq*>(ctx)->emplace_back(
                                       tag, rustorb_detail::bytes(data, len));
                                 },
                                 &exc),
                             exc);
      return list;
    });
  }
  // The policy of the type in effect for the request, or nil; INV_POLICY
  // (minor 2) for a type that is not a client side policy.
  CORBA::object_reference<CORBA::Policy> get_request_policy(CORBA::PolicyType type) {
    return loan_([type](const auto* h) -> CORBA::object_reference<CORBA::Policy> {
      int32_t found = 0;
      uint64_t raw = 0;
      rustorb::detail::Exc exc;
      rustorb::detail::check(rorb_cri_get_request_policy(h, type, &found, &raw, &exc), exc);
      if (!found) return nullptr;
      return rustorb::detail::make_policy(type, raw);
    });
  }
  // Only in send_request; BAD_INV_ORDER (minor 15) for an id there already
  // without replace, or one of the ORB's own (CodeSets, BI_DIR_IIOP).
  void add_request_service_context(const IOP::ServiceContext& service_context, bool replace) {
    loan_([&](const auto* h) {
      rustorb::detail::Exc exc;
      const auto& data = service_context.context_data();
      rustorb::detail::check(
          rorb_cri_add_request_service_context(h, service_context.context_id(), data.data(),
                                               data.size(), replace ? 1 : 0, &exc),
          exc);
      return 0;
    });
  }
};

class ServerRequestInfo : public virtual RequestInfo {
 public:
  ServerRequestInfo(rustorb::detail::adopt_t, const rorb_server_request_info* h)
      : loan_(h, &rustorb_detail::invalid_point) {}

  RUSTORB_PI_REQUEST_INFO(sri)

  CORBA::Any sending_exception() {
    return loan_([](const auto* h) {
      rorb_any* a = nullptr;
      rustorb::detail::Exc exc;
      rustorb::detail::check(rorb_sri_sending_exception(h, &a, &exc), exc);
      return CORBA::Any(rustorb::detail::adopt, a);
    });
  }
  // The repository id of sending_exception (not in the IDL).
  std::string sending_exception_id() {
    return loan_([](const auto* h) {
      rorb_bytes id{};
      rustorb::detail::Exc exc;
      rustorb::detail::check(rorb_sri_sending_exception_id(h, &id, &exc), exc);
      return rustorb::detail::take_string(id);
    });
  }
  ObjectId object_id() {
    return loan_([](const auto* h) {
      rorb_bytes id{};
      rustorb::detail::Exc exc;
      rustorb::detail::check(rorb_sri_object_id(h, &id, &exc), exc);
      return rustorb::detail::take_bytes(id);
    });
  }
  std::vector<uint8_t> adapter_id() {
    return loan_([](const auto* h) {
      rorb_bytes id{};
      rustorb::detail::Exc exc;
      rustorb::detail::check(rorb_sri_adapter_id(h, &id, &exc), exc);
      return rustorb::detail::take_bytes(id);
    });
  }
  ServerId server_id() {
    return loan_([](const auto* h) {
      size_t len = 0;
      const char* s = rorb_sri_server_id(h, &len);
      return rustorb_detail::borrowed_string(s, len);
    });
  }
  ORBId orb_id() {
    return loan_([](const auto* h) {
      size_t len = 0;
      const char* s = rorb_sri_orb_id(h, &len);
      return rustorb_detail::borrowed_string(s, len);
    });
  }
  AdapterName adapter_name() {
    return loan_([](const auto* h) {
      AdapterName names;
      rustorb::detail::Exc exc;
      rustorb::detail::check(rorb_sri_adapter_name(h, &names, rustorb_detail::add_string, &exc),
                             exc);
      return names;
    });
  }
  std::string target_most_derived_interface() {
    return loan_([](const auto* h) {
      rorb_bytes id{};
      rustorb::detail::Exc exc;
      rustorb::detail::check(rorb_sri_target_most_derived_interface(h, &id, &exc), exc);
      return rustorb::detail::take_string(id);
    });
  }
  CORBA::object_reference<CORBA::Policy> get_server_policy(CORBA::PolicyType type) {
    return loan_([type](const auto* h) {
      uint64_t raw = 0;
      rustorb::detail::Exc exc;
      rorb_status status = rorb_sri_get_server_policy(h, type, &raw, &exc);
      return rustorb_detail::policy(type, status, raw, exc);
    });
  }
  // The request scope slot, which the servant's thread then sees.
  void set_slot(SlotId id, const CORBA::Any& data) {
    loan_([&](const auto* h) {
      rustorb::detail::Exc exc;
      rustorb_detail::check_slot(rorb_sri_set_slot(h, id, data._rorb_handle(), &exc), exc);
      return 0;
    });
  }
  bool target_is_a(const std::string& id) {
    return loan_([&](const auto* h) {
      int32_t is_a = 0;
      rustorb::detail::Exc exc;
      rustorb::detail::check(rorb_sri_target_is_a(h, id.data(), id.size(), &is_a, &exc), exc);
      return is_a != 0;
    });
  }
  void add_reply_service_context(const IOP::ServiceContext& service_context, bool replace) {
    loan_([&](const auto* h) {
      rustorb::detail::Exc exc;
      const auto& data = service_context.context_data();
      rustorb::detail::check(
          rorb_sri_add_reply_service_context(h, service_context.context_id(), data.data(),
                                             data.size(), replace ? 1 : 0, &exc),
          exc);
      return 0;
    });
  }
};

#undef RUSTORB_PI_REQUEST_INFO

class IORInfo : public CORBA::LocalObject {
 public:
  IORInfo(rustorb::detail::adopt_t, const rorb_ior_info* h)
      : loan_(h, &rustorb_detail::invalid_point) {}

  CORBA::object_reference<CORBA::Policy> get_effective_policy(CORBA::PolicyType type) {
    return loan_([type](const auto* h) {
      uint64_t raw = 0;
      rustorb::detail::Exc exc;
      rorb_status status = rorb_ior_info_get_effective_policy(h, type, &raw, &exc);
      return rustorb_detail::policy(type, status, raw, exc);
    });
  }
  void add_ior_component(const IOP::TaggedComponent& a_component) {
    loan_([&](const auto* h) {
      const auto& data = a_component.component_data();
      rorb_ior_info_add_ior_component(h, a_component.tag(), data.data(), data.size());
      return 0;
    });
  }
  // BAD_PARAM (minor 29) but for TAG_INTERNET_IOP.
  void add_ior_component_to_profile(const IOP::TaggedComponent& a_component,
                                    IOP::ProfileId profile_id) {
    loan_([&](const auto* h) {
      const auto& data = a_component.component_data();
      rustorb::detail::Exc exc;
      rustorb::detail::check(rorb_ior_info_add_ior_component_to_profile(
                                 h, a_component.tag(), data.data(), data.size(), profile_id, &exc),
                             exc);
      return 0;
    });
  }
  AdapterName adapter_name() {
    return loan_([](const auto* h) {
      AdapterName names;
      rorb_ior_info_adapter_name(h, &names, rustorb_detail::add_string);
      return names;
    });
  }
  void _rorb_close() noexcept { loan_.close(); }

 private:
  rustorb_detail::Loan<const rorb_ior_info> loan_;
};

class ClientRequestInterceptor : public virtual Interceptor {
 public:
  virtual void send_request(CORBA::object_reference<ClientRequestInfo> ri) { (void)ri; }
  // Never called: rustorb has no AMI polling.
  virtual void send_poll(CORBA::object_reference<ClientRequestInfo> ri) { (void)ri; }
  virtual void receive_reply(CORBA::object_reference<ClientRequestInfo> ri) { (void)ri; }
  virtual void receive_exception(CORBA::object_reference<ClientRequestInfo> ri) { (void)ri; }
  virtual void receive_other(CORBA::object_reference<ClientRequestInfo> ri) { (void)ri; }
};

class ServerRequestInterceptor : public virtual Interceptor {
 public:
  virtual void receive_request_service_contexts(CORBA::object_reference<ServerRequestInfo> ri) {
    (void)ri;
  }
  virtual void receive_request(CORBA::object_reference<ServerRequestInfo> ri) { (void)ri; }
  virtual void send_reply(CORBA::object_reference<ServerRequestInfo> ri) { (void)ri; }
  virtual void send_exception(CORBA::object_reference<ServerRequestInfo> ri) { (void)ri; }
  virtual void send_other(CORBA::object_reference<ServerRequestInfo> ri) { (void)ri; }
};

class IORInterceptor : public virtual Interceptor {
 public:
  virtual void establish_components(CORBA::object_reference<IORInfo> info) { (void)info; }
};

// The initial reference "PICurrent": the slots as the calling thread sees
// them.
class Current : public virtual CORBA::Current {
 public:
  // Takes over h.
  Current(rustorb::detail::adopt_t, rorb_pi_current* h) noexcept : h_(h) {}
  ~Current() override { rorb_pi_current_release(h_); }
  Current(const Current&) = delete;
  Current& operator=(const Current&) = delete;

  CORBA::Any get_slot(SlotId id) {
    rorb_any* a = nullptr;
    rustorb::detail::Exc exc;
    rustorb_detail::check_slot(rorb_pi_current_get_slot(h_, id, &a, &exc), exc);
    return CORBA::Any(rustorb::detail::adopt, a);
  }
  void set_slot(SlotId id, const CORBA::Any& data) {
    rustorb::detail::Exc exc;
    rustorb_detail::check_slot(rorb_pi_current_set_slot(h_, id, data._rorb_handle(), &exc), exc);
  }

 private:
  rorb_pi_current* h_;
};

class PolicyFactory : public virtual CORBA::LocalObject {
 public:
  virtual CORBA::object_reference<CORBA::Policy> create_policy(CORBA::PolicyType type,
                                                               const CORBA::Any& value) = 0;
};

class ORBInitInfo;

namespace rustorb_detail {
template <typename T>
void add_interceptor(ORBInitInfo& info, const CORBA::object_reference<T>&);
}

// What an ORBInitializer gets: usable only during pre_init or post_init.
class ORBInitInfo : public CORBA::LocalObject {
 public:
  class DuplicateName : public CORBA::UserException {
   public:
    DuplicateName() = default;
    explicit DuplicateName(std::string name) : name_(std::move(name)) {}
    const std::string& name() const { return name_; }
    void name(const std::string& n) { name_ = n; }
    RUSTORB_PI_EXCEPTION_BODY(DuplicateName,
                              "IDL:omg.org/PortableInterceptor/ORBInitInfo/DuplicateName:1.0")
   private:
    std::string name_;
  };
  class InvalidName : public CORBA::UserException {
   public:
    RUSTORB_PI_EXCEPTION_BODY(InvalidName,
                              "IDL:omg.org/PortableInterceptor/ORBInitInfo/InvalidName:1.0")
  };
  using ObjectId = std::string;

  ORBInitInfo(rustorb::detail::adopt_t, rorb_orb_init_info* h)
      : loan_(h, &rustorb_detail::init_info_gone) {}

  CORBA::StringSeq arguments() {
    return loan_([](rorb_orb_init_info* h) {
      CORBA::StringSeq args;
      rorb_init_info_arguments(h, &args, rustorb_detail::add_string);
      return args;
    });
  }
  std::string orb_id() {
    return loan_([](rorb_orb_init_info* h) {
      size_t len = 0;
      const char* s = rorb_init_info_orb_id(h, &len);
      return rustorb_detail::borrowed_string(s, len);
    });
  }
  CORBA::object_reference<IOP::CodecFactory> codec_factory() {
    return loan_([](rorb_orb_init_info* h) {
      return CORBA::object_reference<IOP::CodecFactory>(
          std::make_shared<IOP::CodecFactory>(rustorb::detail::adopt, rorb_init_info_orb(h)));
    });
  }
  // InvalidName for "" or a name already registered; local objects too.
  void register_initial_reference(const ObjectId& id,
                                  const CORBA::object_reference<CORBA::Object>& obj) {
    loan_([&](rorb_orb_init_info* h) {
      if (obj && !obj->_rorb_handle()) {
        CORBA::ORB orb(rustorb::detail::adopt, rorb_init_info_orb(h));
        try {
          orb.register_initial_reference(id, obj);
        } catch (const CORBA::ORB::InvalidName&) {
          throw InvalidName();
        }
        return 0;
      }
      rustorb::detail::Exc exc;
      rorb_status status = rorb_init_info_register_initial_reference(
          h, id.data(), id.size(), rustorb::detail::handle_of(obj), &exc);
      if (status == RORB_LOCAL_EXCEPTION && exc.local == RORB_INVALID_NAME) throw InvalidName();
      rustorb::detail::check(status, exc);
      return 0;
    });
  }
  // Only in post_init (BAD_INV_ORDER in pre_init).
  CORBA::object_reference<CORBA::Object> resolve_initial_references(const ObjectId& id) {
    return loan_([&](rorb_orb_init_info* h) -> CORBA::object_reference<CORBA::Object> {
      CORBA::ORB orb(rustorb::detail::adopt, rorb_init_info_orb(h));
      bool own = false;
      for (const char* local : rustorb::detail::local_initial_references) {
        own = own || id == local;
      }
      own = own || rorb_orb_resolve_local_initial_reference(
                       orb._rorb_handle(), id.data(), id.size(), nullptr, [](void*, void*) {}) != 0;
      rorb_object* obj = nullptr;
      rustorb::detail::Exc exc;
      // For the ORB's own (RootPOA, PICurrent, local objects, ...), the
      // core only checks the stage: BAD_INV_ORDER in pre_init for every
      // name, InvalidName for "" in post_init.
      const std::string& asked = own ? std::string() : id;
      rorb_status status =
          rorb_init_info_resolve_initial_references(h, asked.data(), asked.size(), &obj, &exc);
      if (status == RORB_OK) return rustorb::detail::object(obj);
      if (status != RORB_LOCAL_EXCEPTION || exc.local != RORB_INVALID_NAME) {
        rustorb::detail::check(status, exc);
      }
      if (!own) throw InvalidName();
      try {
        return orb.resolve_initial_references(id);
      } catch (const CORBA::ORB::InvalidName&) {
        throw InvalidName();
      }
    });
  }
  // DuplicateName for a name (but "") already added.
  void add_client_request_interceptor(
      const CORBA::object_reference<ClientRequestInterceptor>& interceptor) {
    rustorb_detail::add_interceptor(*this, interceptor);
  }
  void add_server_request_interceptor(
      const CORBA::object_reference<ServerRequestInterceptor>& interceptor) {
    rustorb_detail::add_interceptor(*this, interceptor);
  }
  void add_ior_interceptor(const CORBA::object_reference<IORInterceptor>& interceptor) {
    rustorb_detail::add_interceptor(*this, interceptor);
  }
  SlotId allocate_slot_id() {
    return loan_([](rorb_orb_init_info* h) { return rorb_init_info_allocate_slot_id(h); });
  }
  // Not supported: NO_IMPLEMENT.
  void register_policy_factory(CORBA::PolicyType type,
                               const CORBA::object_reference<PolicyFactory>& policy_factory) {
    (void)type;
    (void)policy_factory;
    loan_([](rorb_orb_init_info*) { return 0; });
    throw CORBA::NO_IMPLEMENT(0, CORBA::CompletionStatus::COMPLETED_NO);
  }

  void _rorb_close() noexcept { loan_.close(); }
  template <typename F>
  auto _rorb_with(const F& f) {
    return loan_(f);
  }

 private:
  rustorb_detail::Loan<rorb_orb_init_info> loan_;
};

class ORBInitializer : public virtual CORBA::LocalObject {
 public:
  virtual void pre_init(CORBA::object_reference<ORBInitInfo> info) { (void)info; }
  virtual void post_init(CORBA::object_reference<ORBInitInfo> info) { (void)info; }
};

namespace rustorb_detail {

using PortableServer::detail::Held;

// Runs a point: lends the info for the call, and reports what it raised.
template <typename Info, typename H, typename F>
rorb_status point(H* handle, rorb_pi_error* error, const F& body) noexcept {
  std::shared_ptr<Info> info;
  try {
    info = std::make_shared<Info>(rustorb::detail::adopt, handle);
    body(CORBA::object_reference<Info>(info));
    info->_rorb_close();
    return RORB_OK;
  } catch (const ForwardRequest& f) {
    if (info) info->_rorb_close();
    try {
      if (!f.forward()) throw CORBA::BAD_PARAM(0, CORBA::CompletionStatus::COMPLETED_NO);
      rorb_pi_error_forward(error, rustorb::detail::handle_of(f.forward()));
    } catch (const CORBA::SystemException& e) {
      rorb_pi_error_system(error, e._rep_id(), std::strlen(e._rep_id()), e.minor(),
                           static_cast<uint32_t>(e.completed()));
    }
    return RORB_SYSTEM_EXCEPTION;
  } catch (const CORBA::SystemException& e) {
    if (info) info->_rorb_close();
    rorb_pi_error_system(error, e._rep_id(), std::strlen(e._rep_id()), e.minor(),
                         static_cast<uint32_t>(e.completed()));
    return RORB_SYSTEM_EXCEPTION;
  } catch (const CORBA::UserException& e) {
    // Only ForwardRequest may be raised: the others are UNKNOWN.
    if (info) info->_rorb_close();
    static const char unknown[] = "IDL:omg.org/CORBA/UNKNOWN:1.0";
    rorb_pi_error_system(error, unknown, sizeof unknown - 1, 0,
                         static_cast<uint32_t>(CORBA::CompletionStatus::COMPLETED_MAYBE));
    return RORB_SYSTEM_EXCEPTION;
  } catch (const std::exception& e) {
    if (info) info->_rorb_close();
    std::cerr << "rustorb: exception in an interceptor: " << e.what() << std::endl;
  } catch (...) {
    if (info) info->_rorb_close();
    std::cerr << "rustorb: unknown exception in an interceptor" << std::endl;
  }
  return RORB_CALLBACK_FAILED;
}

template <typename T>
void destroy(void* self) {
  PortableServer::detail::notify("Interceptor::destroy",
                                 [&] { static_cast<Held<T>*>(self)->p->destroy(); });
}

#define RUSTORB_PI_POINT(T, INFO, H, OP)                                    \
  [](void* self, const H* ri, rorb_pi_error* error) -> rorb_status {        \
    return point<INFO>(ri, error, [&](CORBA::object_reference<INFO> info) { \
      static_cast<Held<T>*>(self)->p->OP(std::move(info));                  \
    });                                                                     \
  }

inline const rorb_client_interceptor_vtable* client_vtable() {
  using T = ClientRequestInterceptor;
  using H = rorb_client_request_info;
  static const rorb_client_interceptor_vtable vtable = {
      RUSTORB_PI_POINT(T, ClientRequestInfo, H, send_request),
      RUSTORB_PI_POINT(T, ClientRequestInfo, H, receive_reply),
      RUSTORB_PI_POINT(T, ClientRequestInfo, H, receive_exception),
      RUSTORB_PI_POINT(T, ClientRequestInfo, H, receive_other),
      &destroy<T>,
      &Held<T>::add_ref,
      &Held<T>::remove_ref,
  };
  return &vtable;
}

inline const rorb_server_interceptor_vtable* server_vtable() {
  using T = ServerRequestInterceptor;
  using H = rorb_server_request_info;
  static const rorb_server_interceptor_vtable vtable = {
      RUSTORB_PI_POINT(T, ServerRequestInfo, H, receive_request_service_contexts),
      RUSTORB_PI_POINT(T, ServerRequestInfo, H, receive_request),
      RUSTORB_PI_POINT(T, ServerRequestInfo, H, send_reply),
      RUSTORB_PI_POINT(T, ServerRequestInfo, H, send_exception),
      RUSTORB_PI_POINT(T, ServerRequestInfo, H, send_other),
      &destroy<T>,
      &Held<T>::add_ref,
      &Held<T>::remove_ref,
  };
  return &vtable;
}

inline const rorb_ior_interceptor_vtable* ior_vtable() {
  using T = IORInterceptor;
  static const rorb_ior_interceptor_vtable vtable = {
      RUSTORB_PI_POINT(T, IORInfo, rorb_ior_info, establish_components),
      &destroy<T>,
      &Held<T>::add_ref,
      &Held<T>::remove_ref,
  };
  return &vtable;
}

#undef RUSTORB_PI_POINT

template <typename T>
struct Adder;
template <>
struct Adder<ClientRequestInterceptor> {
  static rorb_status add(rorb_orb_init_info* h, const std::string& n, void* self,
                         rorb_exception* exc) {
    return rorb_init_info_add_client_request_interceptor(h, n.data(), n.size(), self,
                                                         client_vtable(), exc);
  }
};
template <>
struct Adder<ServerRequestInterceptor> {
  static rorb_status add(rorb_orb_init_info* h, const std::string& n, void* self,
                         rorb_exception* exc) {
    return rorb_init_info_add_server_request_interceptor(h, n.data(), n.size(), self,
                                                         server_vtable(), exc);
  }
};
template <>
struct Adder<IORInterceptor> {
  static rorb_status add(rorb_orb_init_info* h, const std::string& n, void* self,
                         rorb_exception* exc) {
    return rorb_init_info_add_ior_interceptor(h, n.data(), n.size(), self, ior_vtable(), exc);
  }
};

template <typename T>
void add_interceptor(ORBInitInfo& info, const CORBA::object_reference<T>& interceptor) {
  if (!interceptor) throw CORBA::BAD_PARAM(0, CORBA::CompletionStatus::COMPLETED_NO);
  std::string name = interceptor->name();
  info._rorb_with([&](rorb_orb_init_info* h) {
    auto* held = new Held<T>(interceptor._rorb_shared());
    rustorb::detail::Exc exc;
    rorb_status status = Adder<T>::add(h, name, held, &exc);
    Held<T>::remove_ref(held);
    if (status == RORB_LOCAL_EXCEPTION && exc.local == RORB_DUPLICATE_NAME) {
      throw ORBInitInfo::DuplicateName(name);
    }
    rustorb::detail::check(status, exc);
    return 0;
  });
}

#define RUSTORB_PI_INIT(OP)                                                              \
  [](void* self, rorb_orb_init_info* info, rorb_pi_error* error) -> rorb_status {        \
    return point<ORBInitInfo>(info, error, [&](CORBA::object_reference<ORBInitInfo> i) { \
      static_cast<Held<ORBInitializer>*>(self)->p->OP(std::move(i));                     \
    });                                                                                  \
  }

inline const rorb_orb_initializer_vtable* initializer_vtable() {
  static const rorb_orb_initializer_vtable vtable = {
      RUSTORB_PI_INIT(pre_init),
      RUSTORB_PI_INIT(post_init),
      &Held<ORBInitializer>::add_ref,
      &Held<ORBInitializer>::remove_ref,
  };
  return &vtable;
}

#undef RUSTORB_PI_INIT

inline CORBA::object_reference<CORBA::Object> initial_reference(rorb_orb* orb,
                                                                const std::string& name) {
  if (name == "PICurrent") {
    return CORBA::object_reference<CORBA::Object>(
        std::make_shared<Current>(rustorb::detail::adopt, rorb_orb_pi_current(orb)));
  }
  return CORBA::object_reference<CORBA::Object>(
      std::make_shared<IOP::CodecFactory>(rustorb::detail::adopt, rorb_orb_clone(orb)));
}

inline const bool initial_references_registered =
    (rustorb::detail::pi_references_hook() = &initial_reference, true);

}  // namespace rustorb_detail

// For every ORB ORB_init creates after it in the process.
inline void register_orb_initializer(const CORBA::object_reference<ORBInitializer>& init) {
  if (!init) throw CORBA::BAD_PARAM(0, CORBA::CompletionStatus::COMPLETED_NO);
  auto* held = new rustorb_detail::Held<ORBInitializer>(init._rorb_shared());
  rorb_register_orb_initializer(held, rustorb_detail::initializer_vtable());
  rustorb_detail::Held<ORBInitializer>::remove_ref(held);
}

}  // namespace PortableInterceptor

#undef RUSTORB_PI_MEMBER
#undef RUSTORB_PI_EXCEPTION_BODY

#endif

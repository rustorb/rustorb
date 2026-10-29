// DynamicAny (IDL to C++11): building and taking apart values of types
// known only by their TypeCode, on the DynAny of the rustorb core (the
// rorb_dynany functions of the C ABI). See docs/dynany.md.
//
//   auto factory = IDL::traits<DynamicAny::DynAnyFactory>::narrow(
//       orb->resolve_initial_references("DynAnyFactory"));
//   auto d = factory->create_dyn_any_from_type_code(AnyTest::_tc_Point);
//   d->insert_long(1);
//   d->next();
//   d->insert_double(2.5);
//   CORBA::Any a = d->to_any();
//
// The factory gives the DynAny of the class its type calls for, which
// IDL::traits<DynamicAny::DynStruct>::narrow and the like find.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

#ifndef RUSTORB_DYNANY_H
#define RUSTORB_DYNANY_H

#include <rustorb/cdr.h>

#include <string>
#include <vector>

#define RUSTORB_DYN_EXCEPTION_BODY(NAME, ID)                 \
  const char* _name() const override { return #NAME; }       \
  const char* _rep_id() const override { return ID; }        \
  [[noreturn]] void _raise() const override { throw *this; }

namespace CORBA {
using BooleanSeq = std::vector<bool>;
using OctetSeq = std::vector<uint8_t>;
using CharSeq = std::vector<char>;
using WCharSeq = std::vector<wchar_t>;
using ShortSeq = std::vector<int16_t>;
using UShortSeq = std::vector<uint16_t>;
using LongSeq = std::vector<int32_t>;
using ULongSeq = std::vector<uint32_t>;
using LongLongSeq = std::vector<int64_t>;
using ULongLongSeq = std::vector<uint64_t>;
using FloatSeq = std::vector<float>;
using DoubleSeq = std::vector<double>;
}  // namespace CORBA

namespace DynamicAny {

class DynAny;
using DynAny_ref = IDL::traits<DynAny>::ref_type;

using FieldName = std::string;
using AnySeq = std::vector<CORBA::Any>;
using DynAnySeq = std::vector<DynAny_ref>;

class NameValuePair {
 public:
  NameValuePair() = default;
  NameValuePair(FieldName id, CORBA::Any value) : id_(std::move(id)), value_(std::move(value)) {}
  const FieldName& id() const { return id_; }
  void id(const FieldName& v) { id_ = v; }
  const CORBA::Any& value() const { return value_; }
  CORBA::Any& value() { return value_; }
  void value(const CORBA::Any& v) { value_ = v; }

 private:
  FieldName id_;
  CORBA::Any value_;
};
using NameValuePairSeq = std::vector<NameValuePair>;

class NameDynAnyPair {
 public:
  NameDynAnyPair() = default;
  NameDynAnyPair(FieldName id, DynAny_ref value) : id_(std::move(id)), value_(std::move(value)) {}
  const FieldName& id() const { return id_; }
  void id(const FieldName& v) { id_ = v; }
  const DynAny_ref& value() const { return value_; }
  void value(const DynAny_ref& v) { value_ = v; }

 private:
  FieldName id_;
  DynAny_ref value_;
};
using NameDynAnyPairSeq = std::vector<NameDynAnyPair>;

class MustTruncate : public CORBA::UserException {
 public:
  RUSTORB_DYN_EXCEPTION_BODY(MustTruncate, "IDL:omg.org/DynamicAny/MustTruncate:1.0")
};

}  // namespace DynamicAny

namespace rustorb {
namespace detail {

[[noreturn]] void throw_dyn(uint32_t local);

// check() with the DynamicAny exceptions.
inline void dyn_check(rorb_status status, rorb_exception& exc) {
  if (status == RORB_LOCAL_EXCEPTION && exc.local >= RORB_DYN_INVALID_VALUE &&
      exc.local <= RORB_DYN_MUST_TRUNCATE) {
    uint32_t local = exc.local;
    rorb_exception_clear(&exc);
    throw_dyn(local);
  }
  check(status, exc);
}

// Runs f(T*, rorb_exception*) and returns the T it stores.
template <typename T, typename F> T dyn_call(const F& f) {
  T out{};
  Exc exc;
  dyn_check(f(&out, &exc), exc);
  return out;
}

template <typename F> void dyn_run(const F& f) {
  Exc exc;
  dyn_check(f(&exc), exc);
}

DynamicAny::DynAny_ref dyn_any(rorb_dynany* handle);
CORBA::object_reference<CORBA::Object> dyn_any_factory();

}  // namespace detail
}  // namespace rustorb

namespace DynamicAny {

// DynamicAny::DynAny. A reference holds one handle of the core's DynAny;
// components share their state with the DynAny they belong to.
class DynAny : public virtual CORBA::LocalObject {
 public:
  class InvalidValue : public CORBA::UserException {
   public:
    RUSTORB_DYN_EXCEPTION_BODY(InvalidValue, "IDL:omg.org/DynamicAny/DynAny/InvalidValue:1.0")
  };
  class TypeMismatch : public CORBA::UserException {
   public:
    RUSTORB_DYN_EXCEPTION_BODY(TypeMismatch, "IDL:omg.org/DynamicAny/DynAny/TypeMismatch:1.0")
  };

  static constexpr const char* _rorb_repository_id = "IDL:omg.org/DynamicAny/DynAny:1.0";

  DynAny(rustorb::detail::adopt_t, rorb_dynany* handle) noexcept : h_(handle) {}
  ~DynAny() override { rorb_dynany_release(h_); }

  bool _is_a(const std::string& repository_id) override {
    return repository_id == _rorb_repository_id || CORBA::LocalObject::_is_a(repository_id);
  }
  bool _is_equivalent(const CORBA::object_reference<CORBA::Object>& other) override {
    auto d = rustorb::detail::narrow<DynAny>(other);
    return d && rorb_dynany_same(h_, d->h_) != 0;
  }

  IDL::traits<CORBA::TypeCode>::ref_type type() const {
    return rustorb::detail::typecode(rustorb::detail::dyn_call<rorb_typecode*>(
        [&](rorb_typecode** o, rorb_exception* e) { return rorb_dynany_type(h_, o, e); }));
  }
  void assign(const DynAny_ref& dyn_any) {
    rustorb::detail::dyn_run(
        [&](rorb_exception* e) { return rorb_dynany_assign(h_, handle(dyn_any), e); });
  }
  void from_any(const CORBA::Any& value) {
    rustorb::detail::dyn_run(
        [&](rorb_exception* e) { return rorb_dynany_from_any(h_, value._rorb_handle(), e); });
  }
  CORBA::Any to_any() const {
    return CORBA::Any(rustorb::detail::adopt, rustorb::detail::dyn_call<rorb_any*>(
        [&](rorb_any** o, rorb_exception* e) { return rorb_dynany_to_any(h_, o, e); }));
  }
  bool equal(const DynAny_ref& dyn_any) const {
    return rustorb::detail::dyn_call<int32_t>([&](int32_t* o, rorb_exception* e) {
      return rorb_dynany_equal(h_, handle(dyn_any), o, e);
    }) != 0;
  }
  void destroy() {
    rustorb::detail::dyn_run([&](rorb_exception* e) { return rorb_dynany_destroy(h_, e); });
  }
  DynAny_ref copy() const {
    return rustorb::detail::dyn_any(rustorb::detail::dyn_call<rorb_dynany*>(
        [&](rorb_dynany** o, rorb_exception* e) { return rorb_dynany_copy(h_, o, e); }));
  }

#define RUSTORB_DYN_BASIC(NAME, TYPE, KIND)                       \
  void insert_##NAME(TYPE value) { insert_(KIND, value); }        \
  TYPE get_##NAME() const { return get_<TYPE>(KIND); }
  RUSTORB_DYN_BASIC(boolean, bool, RORB_TK_BOOLEAN)
  RUSTORB_DYN_BASIC(octet, uint8_t, RORB_TK_OCTET)
  RUSTORB_DYN_BASIC(char, char, RORB_TK_CHAR)
  RUSTORB_DYN_BASIC(short, int16_t, RORB_TK_SHORT)
  RUSTORB_DYN_BASIC(ushort, uint16_t, RORB_TK_USHORT)
  RUSTORB_DYN_BASIC(long, int32_t, RORB_TK_LONG)
  RUSTORB_DYN_BASIC(ulong, uint32_t, RORB_TK_ULONG)
  RUSTORB_DYN_BASIC(float, float, RORB_TK_FLOAT)
  RUSTORB_DYN_BASIC(double, double, RORB_TK_DOUBLE)
  RUSTORB_DYN_BASIC(longlong, int64_t, RORB_TK_LONGLONG)
  RUSTORB_DYN_BASIC(ulonglong, uint64_t, RORB_TK_ULONGLONG)
  RUSTORB_DYN_BASIC(wchar, wchar_t, RORB_TK_WCHAR)
  RUSTORB_DYN_BASIC(string, std::string, RORB_TK_STRING)
  RUSTORB_DYN_BASIC(wstring, std::wstring, RORB_TK_WSTRING)
  RUSTORB_DYN_BASIC(any, CORBA::Any, RORB_TK_ANY)
  RUSTORB_DYN_BASIC(typecode, IDL::traits<CORBA::TypeCode>::ref_type, RORB_TK_TYPECODE)
  RUSTORB_DYN_BASIC(reference, IDL::traits<CORBA::Object>::ref_type, RORB_TK_OBJREF)
#undef RUSTORB_DYN_BASIC

  void insert_dyn_any(const DynAny_ref& value) {
    rustorb::detail::dyn_run(
        [&](rorb_exception* e) { return rorb_dynany_insert_dyn_any(h_, handle(value), e); });
  }
  DynAny_ref get_dyn_any() const {
    return rustorb::detail::dyn_any(rustorb::detail::dyn_call<rorb_dynany*>(
        [&](rorb_dynany** o, rorb_exception* e) { return rorb_dynany_get_dyn_any(h_, o, e); }));
  }

  // The value is written with the type of the current component (or of
  // this DynAny), and read with the factory of the instance's type.
  void insert_val(const IDL::traits<CORBA::ValueBase>::ref_type& value) {
    auto tc = rustorb::detail::typecode(rustorb::detail::dyn_call<rorb_typecode*>(
        [&](rorb_typecode** o, rorb_exception* e) { return rorb_dynany_target_type(h_, o, e); }));
    CORBA::Any a;
    a._rorb_insert(tc, [&](rorb_out* out) {
      rustorb::cdr::write_value(out, rustorb::detail::get_pointer(value));
    });
    rustorb::detail::dyn_run([&](rorb_exception* e) {
      return rorb_dynany_insert(h_, RORB_TK_VALUE, a._rorb_handle(), e);
    });
  }
  IDL::traits<CORBA::ValueBase>::ref_type get_val() const {
    CORBA::Any a(rustorb::detail::adopt, rustorb::detail::dyn_call<rorb_any*>(
        [&](rorb_any** o, rorb_exception* e) { return rorb_dynany_get(h_, RORB_TK_VALUE, o, e); }));
    IDL::traits<CORBA::ValueBase>::ref_type v;
    a._rorb_extract(a.type(), [&](rorb_in* in) { v = rustorb::cdr::read_value(in, ""); });
    return v;
  }

  // long double and abstract interfaces are not supported: NO_IMPLEMENT.
  void insert_abstract(const IDL::traits<CORBA::Object>::ref_type&) { no_implement(); }
  IDL::traits<CORBA::Object>::ref_type get_abstract() const {
    no_implement();
    return nullptr;
  }

#define RUSTORB_DYN_SEQ(NAME, SEQ, KIND)                                 \
  void insert_##NAME##_seq(const CORBA::SEQ& value) { insert_seq_(KIND, value); } \
  CORBA::SEQ get_##NAME##_seq() const { return get_seq_<CORBA::SEQ>(KIND); }
  RUSTORB_DYN_SEQ(boolean, BooleanSeq, RORB_TK_BOOLEAN)
  RUSTORB_DYN_SEQ(octet, OctetSeq, RORB_TK_OCTET)
  RUSTORB_DYN_SEQ(char, CharSeq, RORB_TK_CHAR)
  RUSTORB_DYN_SEQ(short, ShortSeq, RORB_TK_SHORT)
  RUSTORB_DYN_SEQ(ushort, UShortSeq, RORB_TK_USHORT)
  RUSTORB_DYN_SEQ(long, LongSeq, RORB_TK_LONG)
  RUSTORB_DYN_SEQ(ulong, ULongSeq, RORB_TK_ULONG)
  RUSTORB_DYN_SEQ(float, FloatSeq, RORB_TK_FLOAT)
  RUSTORB_DYN_SEQ(double, DoubleSeq, RORB_TK_DOUBLE)
  RUSTORB_DYN_SEQ(longlong, LongLongSeq, RORB_TK_LONGLONG)
  RUSTORB_DYN_SEQ(ulonglong, ULongLongSeq, RORB_TK_ULONGLONG)
  RUSTORB_DYN_SEQ(wchar, WCharSeq, RORB_TK_WCHAR)
#undef RUSTORB_DYN_SEQ

  bool seek(int32_t index) {
    return rustorb::detail::dyn_call<int32_t>(
               [&](int32_t* o, rorb_exception* e) { return rorb_dynany_seek(h_, index, o, e); }) !=
           0;
  }
  void rewind() {
    rustorb::detail::dyn_run([&](rorb_exception* e) { return rorb_dynany_rewind(h_, e); });
  }
  bool next() {
    return rustorb::detail::dyn_call<int32_t>(
               [&](int32_t* o, rorb_exception* e) { return rorb_dynany_next(h_, o, e); }) != 0;
  }
  uint32_t component_count() const {
    return rustorb::detail::dyn_call<uint32_t>(
        [&](uint32_t* o, rorb_exception* e) { return rorb_dynany_component_count(h_, o, e); });
  }
  // nullptr at position -1.
  DynAny_ref current_component() const {
    return rustorb::detail::dyn_any(rustorb::detail::dyn_call<rorb_dynany*>(
        [&](rorb_dynany** o, rorb_exception* e) {
          return rorb_dynany_current_component(h_, o, e);
        }));
  }

  rorb_dynany* _rorb_handle() const noexcept { return h_; }

 protected:
  static const rorb_dynany* handle(const DynAny_ref& d) {
    if (!d) throw CORBA::BAD_PARAM(0, CORBA::CompletionStatus::COMPLETED_NO);
    return d->h_;
  }
  void no_implement() const {
    rustorb::detail::dyn_run([&](rorb_exception* e) { return rorb_dynany_no_implement(h_, e); });
  }
  template <typename T> void insert_(uint32_t kind, const T& value) {
    CORBA::Any a;
    a <<= value;
    rustorb::detail::dyn_run(
        [&](rorb_exception* e) { return rorb_dynany_insert(h_, kind, a._rorb_handle(), e); });
  }
  template <typename T> T get_(uint32_t kind) const {
    CORBA::Any a(rustorb::detail::adopt, rustorb::detail::dyn_call<rorb_any*>(
        [&](rorb_any** o, rorb_exception* e) { return rorb_dynany_get(h_, kind, o, e); }));
    T v{};
    if (!(a >>= v)) throw TypeMismatch();
    return v;
  }
  template <typename S> void insert_seq_(uint32_t kind, const S& value) {
    CORBA::Any a;
    a <<= value;
    rustorb::detail::dyn_run(
        [&](rorb_exception* e) { return rorb_dynany_insert_seq(h_, kind, a._rorb_handle(), e); });
  }
  template <typename S> S get_seq_(uint32_t kind) const {
    CORBA::Any a(rustorb::detail::adopt, rustorb::detail::dyn_call<rorb_any*>(
        [&](rorb_any** o, rorb_exception* e) { return rorb_dynany_get_seq(h_, kind, o, e); }));
    S v{};
    if (!(a >>= v)) throw TypeMismatch();
    return v;
  }
  // The members (DynStruct, DynValue) and elements (DynSequence, DynArray).
  NameDynAnyPairSeq members_() const {
    uint32_t n = rustorb::detail::dyn_call<uint32_t>(
        [&](uint32_t* o, rorb_exception* e) { return rorb_dynany_member_count(h_, o, e); });
    NameDynAnyPairSeq out;
    out.reserve(n);
    for (uint32_t i = 0; i < n; ++i) {
      rorb_bytes name{};
      rorb_dynany* m = rustorb::detail::dyn_call<rorb_dynany*>(
          [&](rorb_dynany** o, rorb_exception* e) {
            return rorb_dynany_member_at(h_, i, &name, o, e);
          });
      out.emplace_back(rustorb::detail::take_string(name), rustorb::detail::dyn_any(m));
    }
    return out;
  }
  template <typename T, typename F> void set_members_(const std::vector<T>& value, const F& f) {
    std::vector<const char*> names;
    std::vector<size_t> lens;
    for (const auto& p : value) {
      names.push_back(p.id().data());
      lens.push_back(p.id().size());
    }
    rustorb::detail::dyn_run(
        [&](rorb_exception* e) { return f(names.data(), lens.data(), e); });
  }
  DynAnySeq elements_() const {
    uint32_t n = rustorb::detail::dyn_call<uint32_t>(
        [&](uint32_t* o, rorb_exception* e) { return rorb_dynany_element_count(h_, o, e); });
    DynAnySeq out;
    out.reserve(n);
    for (uint32_t i = 0; i < n; ++i) {
      out.push_back(rustorb::detail::dyn_any(rustorb::detail::dyn_call<rorb_dynany*>(
          [&](rorb_dynany** o, rorb_exception* e) {
            return rorb_dynany_element_at(h_, i, o, e);
          })));
    }
    return out;
  }
  static std::vector<CORBA::Any> anys(const DynAnySeq& d) {
    std::vector<CORBA::Any> out;
    for (const auto& x : d) out.push_back(x->to_any());
    return out;
  }

  rorb_dynany* h_;
};

class DynFixed : public DynAny {
 public:
  static constexpr const char* _rorb_repository_id = "IDL:omg.org/DynamicAny/DynFixed:1.0";
  using DynAny::DynAny;
  // The value as a fixed-point literal, such as "-12.5".
  std::string get_value() const {
    return rustorb::detail::take_string(rustorb::detail::dyn_call<rorb_bytes>(
        [&](rorb_bytes* o, rorb_exception* e) { return rorb_dynany_get_fixed_value(h_, o, e); }));
  }
  // False if digits after the scale were cut off.
  bool set_value(const std::string& val) {
    return rustorb::detail::dyn_call<int32_t>([&](int32_t* o, rorb_exception* e) {
             return rorb_dynany_set_fixed_value(h_, val.data(), val.size(), o, e);
           }) != 0;
  }
};

class DynEnum : public DynAny {
 public:
  static constexpr const char* _rorb_repository_id = "IDL:omg.org/DynamicAny/DynEnum:1.0";
  using DynAny::DynAny;
  std::string get_as_string() const {
    return rustorb::detail::take_string(rustorb::detail::dyn_call<rorb_bytes>(
        [&](rorb_bytes* o, rorb_exception* e) { return rorb_dynany_get_as_string(h_, o, e); }));
  }
  void set_as_string(const std::string& value) {
    rustorb::detail::dyn_run([&](rorb_exception* e) {
      return rorb_dynany_set_as_string(h_, value.data(), value.size(), e);
    });
  }
  uint32_t get_as_ulong() const {
    return rustorb::detail::dyn_call<uint32_t>(
        [&](uint32_t* o, rorb_exception* e) { return rorb_dynany_get_as_ulong(h_, o, e); });
  }
  void set_as_ulong(uint32_t value) {
    rustorb::detail::dyn_run(
        [&](rorb_exception* e) { return rorb_dynany_set_as_ulong(h_, value, e); });
  }
};

// The member operations of DynStruct and DynValue.
#define RUSTORB_DYN_MEMBERS                                                                   \
  FieldName current_member_name() const {                                                    \
    return rustorb::detail::take_string(rustorb::detail::dyn_call<rorb_bytes>(                \
        [&](rorb_bytes* o, rorb_exception* e) {                                               \
          return rorb_dynany_current_member_name(h_, o, e);                                   \
        }));                                                                                  \
  }                                                                                           \
  CORBA::TCKind current_member_kind() const {                                                \
    return static_cast<CORBA::TCKind>(rustorb::detail::dyn_call<uint32_t>(                   \
        [&](uint32_t* o, rorb_exception* e) {                                                 \
          return rorb_dynany_current_member_kind(h_, o, e);                                   \
        }));                                                                                  \
  }                                                                                           \
  NameValuePairSeq get_members() const {                                                     \
    NameValuePairSeq out;                                                                     \
    for (auto& p : members_()) out.emplace_back(p.id(), p.value()->to_any());               \
    return out;                                                                               \
  }                                                                                           \
  void set_members(const NameValuePairSeq& value) {                                          \
    std::vector<const rorb_any*> v;                                                           \
    for (const auto& p : value) v.push_back(p.value()._rorb_handle());                       \
    set_members_(value, [&](const char* const* n, const size_t* l, rorb_exception* e) {     \
      return rorb_dynany_set_members(h_, v.size(), n, l, v.data(), e);                       \
    });                                                                                       \
  }                                                                                           \
  NameDynAnyPairSeq get_members_as_dyn_any() const { return members_(); }                   \
  void set_members_as_dyn_any(const NameDynAnyPairSeq& value) {                              \
    std::vector<const rorb_dynany*> v;                                                        \
    for (const auto& p : value) v.push_back(handle(p.value()));                              \
    set_members_(value, [&](const char* const* n, const size_t* l, rorb_exception* e) {     \
      return rorb_dynany_set_members_as_dyn_any(h_, v.size(), n, l, v.data(), e);            \
    });                                                                                       \
  }

// Structs and exceptions.
class DynStruct : public DynAny {
 public:
  static constexpr const char* _rorb_repository_id = "IDL:omg.org/DynamicAny/DynStruct:1.0";
  using DynAny::DynAny;
  RUSTORB_DYN_MEMBERS
};

class DynUnion : public DynAny {
 public:
  static constexpr const char* _rorb_repository_id = "IDL:omg.org/DynamicAny/DynUnion:1.0";
  using DynAny::DynAny;
  DynAny_ref get_discriminator() const {
    return rustorb::detail::dyn_any(rustorb::detail::dyn_call<rorb_dynany*>(
        [&](rorb_dynany** o, rorb_exception* e) {
          return rorb_dynany_get_discriminator(h_, o, e);
        }));
  }
  void set_discriminator(const DynAny_ref& d) {
    rustorb::detail::dyn_run(
        [&](rorb_exception* e) { return rorb_dynany_set_discriminator(h_, handle(d), e); });
  }
  void set_to_default_member() {
    rustorb::detail::dyn_run(
        [&](rorb_exception* e) { return rorb_dynany_set_to_default_member(h_, e); });
  }
  void set_to_no_active_member() {
    rustorb::detail::dyn_run(
        [&](rorb_exception* e) { return rorb_dynany_set_to_no_active_member(h_, e); });
  }
  bool has_no_active_member() const {
    return rustorb::detail::dyn_call<int32_t>([&](int32_t* o, rorb_exception* e) {
             return rorb_dynany_has_no_active_member(h_, o, e);
           }) != 0;
  }
  CORBA::TCKind discriminator_kind() const {
    return static_cast<CORBA::TCKind>(rustorb::detail::dyn_call<uint32_t>(
        [&](uint32_t* o, rorb_exception* e) { return rorb_dynany_discriminator_kind(h_, o, e); }));
  }
  DynAny_ref member() const {
    return rustorb::detail::dyn_any(rustorb::detail::dyn_call<rorb_dynany*>(
        [&](rorb_dynany** o, rorb_exception* e) { return rorb_dynany_member(h_, o, e); }));
  }
  FieldName member_name() const {
    return rustorb::detail::take_string(rustorb::detail::dyn_call<rorb_bytes>(
        [&](rorb_bytes* o, rorb_exception* e) { return rorb_dynany_member_name(h_, o, e); }));
  }
  CORBA::TCKind member_kind() const {
    return static_cast<CORBA::TCKind>(rustorb::detail::dyn_call<uint32_t>(
        [&](uint32_t* o, rorb_exception* e) { return rorb_dynany_member_kind(h_, o, e); }));
  }
};

// The element operations of DynSequence and DynArray.
#define RUSTORB_DYN_ELEMENTS                                                               \
  AnySeq get_elements() const { return anys(elements_()); }                               \
  void set_elements(const AnySeq& value) {                                                \
    std::vector<const rorb_any*> v;                                                        \
    for (const auto& a : value) v.push_back(a._rorb_handle());                            \
    rustorb::detail::dyn_run([&](rorb_exception* e) {                                     \
      return rorb_dynany_set_elements(h_, v.size(), v.data(), e);                         \
    });                                                                                    \
  }                                                                                        \
  DynAnySeq get_elements_as_dyn_any() const { return elements_(); }                       \
  void set_elements_as_dyn_any(const DynAnySeq& value) {                                  \
    std::vector<const rorb_dynany*> v;                                                     \
    for (const auto& d : value) v.push_back(handle(d));                                   \
    rustorb::detail::dyn_run([&](rorb_exception* e) {                                     \
      return rorb_dynany_set_elements_as_dyn_any(h_, v.size(), v.data(), e);              \
    });                                                                                    \
  }

class DynSequence : public DynAny {
 public:
  static constexpr const char* _rorb_repository_id = "IDL:omg.org/DynamicAny/DynSequence:1.0";
  using DynAny::DynAny;
  uint32_t get_length() const {
    return rustorb::detail::dyn_call<uint32_t>(
        [&](uint32_t* o, rorb_exception* e) { return rorb_dynany_get_length(h_, o, e); });
  }
  void set_length(uint32_t len) {
    rustorb::detail::dyn_run([&](rorb_exception* e) { return rorb_dynany_set_length(h_, len, e); });
  }
  RUSTORB_DYN_ELEMENTS
};

class DynArray : public DynAny {
 public:
  static constexpr const char* _rorb_repository_id = "IDL:omg.org/DynamicAny/DynArray:1.0";
  using DynAny::DynAny;
  RUSTORB_DYN_ELEMENTS
};

class DynValueCommon : public DynAny {
 public:
  static constexpr const char* _rorb_repository_id = "IDL:omg.org/DynamicAny/DynValueCommon:1.0";
  using DynAny::DynAny;
  bool is_null() const {
    return rustorb::detail::dyn_call<int32_t>(
               [&](int32_t* o, rorb_exception* e) { return rorb_dynany_is_null(h_, o, e); }) != 0;
  }
  void set_to_null() {
    rustorb::detail::dyn_run([&](rorb_exception* e) { return rorb_dynany_set_to_null(h_, e); });
  }
  void set_to_value() {
    rustorb::detail::dyn_run([&](rorb_exception* e) { return rorb_dynany_set_to_value(h_, e); });
  }
};

// Valuetypes and eventtypes.
class DynValue : public DynValueCommon {
 public:
  static constexpr const char* _rorb_repository_id = "IDL:omg.org/DynamicAny/DynValue:1.0";
  using DynValueCommon::DynValueCommon;
  RUSTORB_DYN_MEMBERS
};

class DynValueBox : public DynValueCommon {
 public:
  static constexpr const char* _rorb_repository_id = "IDL:omg.org/DynamicAny/DynValueBox:1.0";
  using DynValueCommon::DynValueCommon;
  CORBA::Any get_boxed_value() const {
    return CORBA::Any(rustorb::detail::adopt, rustorb::detail::dyn_call<rorb_any*>(
        [&](rorb_any** o, rorb_exception* e) { return rorb_dynany_get_boxed_value(h_, o, e); }));
  }
  void set_boxed_value(const CORBA::Any& boxed) {
    rustorb::detail::dyn_run([&](rorb_exception* e) {
      return rorb_dynany_set_boxed_value(h_, boxed._rorb_handle(), e);
    });
  }
  DynAny_ref get_boxed_value_as_dyn_any() const {
    return rustorb::detail::dyn_any(rustorb::detail::dyn_call<rorb_dynany*>(
        [&](rorb_dynany** o, rorb_exception* e) {
          return rorb_dynany_get_boxed_value_as_dyn_any(h_, o, e);
        }));
  }
  void set_boxed_value_as_dyn_any(const DynAny_ref& boxed) {
    rustorb::detail::dyn_run([&](rorb_exception* e) {
      return rorb_dynany_set_boxed_value_as_dyn_any(h_, handle(boxed), e);
    });
  }
};

#undef RUSTORB_DYN_MEMBERS
#undef RUSTORB_DYN_ELEMENTS

// DynamicAny::DynAnyFactory, the initial reference "DynAnyFactory".
class DynAnyFactory : public virtual CORBA::LocalObject {
 public:
  class InconsistentTypeCode : public CORBA::UserException {
   public:
    RUSTORB_DYN_EXCEPTION_BODY(InconsistentTypeCode,
                                 "IDL:omg.org/DynamicAny/DynAnyFactory/InconsistentTypeCode:1.0")
  };

  static constexpr const char* _rorb_repository_id = "IDL:omg.org/DynamicAny/DynAnyFactory:1.0";

  bool _is_a(const std::string& repository_id) override {
    return repository_id == _rorb_repository_id || CORBA::LocalObject::_is_a(repository_id);
  }

  DynAny_ref create_dyn_any(const CORBA::Any& value) { return create(value, true); }
  DynAny_ref create_dyn_any_from_type_code(const IDL::traits<CORBA::TypeCode>::ref_type& type) {
    return rustorb::detail::dyn_any(rustorb::detail::dyn_call<rorb_dynany*>(
        [&](rorb_dynany** o, rorb_exception* e) {
          return rorb_dynany_from_typecode(rustorb::detail::tc_handle(type), o, e);
        }));
  }
  DynAny_ref create_dyn_any_without_truncation(const CORBA::Any& value) {
    return create(value, false);
  }
  DynAnySeq create_multiple_dyn_anys(const AnySeq& values, bool allow_truncate) {
    DynAnySeq out;
    for (const auto& v : values) out.push_back(create(v, allow_truncate));
    return out;
  }
  AnySeq create_multiple_anys(const DynAnySeq& values) {
    AnySeq out;
    for (const auto& d : values) out.push_back(d->to_any());
    return out;
  }

 private:
  DynAny_ref create(const CORBA::Any& value, bool truncate) {
    return rustorb::detail::dyn_any(rustorb::detail::dyn_call<rorb_dynany*>(
        [&](rorb_dynany** o, rorb_exception* e) {
          return rorb_dynany_create(value._rorb_handle(), truncate ? 1 : 0, o, e);
        }));
  }
};

}  // namespace DynamicAny

namespace rustorb {
namespace detail {

[[noreturn]] inline void throw_dyn(uint32_t local) {
  switch (local) {
    case RORB_DYN_INVALID_VALUE: throw DynamicAny::DynAny::InvalidValue();
    case RORB_DYN_TYPE_MISMATCH: throw DynamicAny::DynAny::TypeMismatch();
    case RORB_DYN_INCONSISTENT_TYPECODE: throw DynamicAny::DynAnyFactory::InconsistentTypeCode();
    default: throw DynamicAny::MustTruncate();
  }
}

// The DynAny of a handle (taken over), of the class its kind calls for;
// nullptr for NULL.
inline DynamicAny::DynAny_ref dyn_any(rorb_dynany* h) {
  using namespace DynamicAny;
  if (!h) return nullptr;
  auto make = [h](auto* tag) -> DynAny_ref {
    using T = std::remove_pointer_t<decltype(tag)>;
    return DynAny_ref(std::shared_ptr<DynAny>(std::make_shared<T>(adopt, h)));
  };
  switch (static_cast<CORBA::TCKind>(rorb_dynany_kind(h))) {
    case CORBA::TCKind::tk_fixed: return make(static_cast<DynFixed*>(nullptr));
    case CORBA::TCKind::tk_enum: return make(static_cast<DynEnum*>(nullptr));
    case CORBA::TCKind::tk_struct:
    case CORBA::TCKind::tk_except: return make(static_cast<DynStruct*>(nullptr));
    case CORBA::TCKind::tk_union: return make(static_cast<DynUnion*>(nullptr));
    case CORBA::TCKind::tk_sequence: return make(static_cast<DynSequence*>(nullptr));
    case CORBA::TCKind::tk_array: return make(static_cast<DynArray*>(nullptr));
    case CORBA::TCKind::tk_value:
    case CORBA::TCKind::tk_event: return make(static_cast<DynValue*>(nullptr));
    case CORBA::TCKind::tk_value_box: return make(static_cast<DynValueBox*>(nullptr));
    default: return make(static_cast<DynAny*>(nullptr));
  }
}

inline CORBA::object_reference<CORBA::Object> dyn_any_factory() {
  return CORBA::object_reference<CORBA::Object>(std::make_shared<DynamicAny::DynAnyFactory>());
}

// Makes ORB::resolve_initial_references("DynAnyFactory") answer.
inline const bool dyn_any_factory_registered = (dyn_any_factory_hook() = &dyn_any_factory, true);

}  // namespace detail
}  // namespace rustorb

#undef RUSTORB_DYN_EXCEPTION_BODY

#endif

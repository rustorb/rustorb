// The custom valuetype tests (idl/CustomTest.idl) on the rustorb C++
// binding: the marshaling of Blob, the CustomEcho servant and the client
// cases, those of impls/rustorbpy/customtest.py.

#ifndef INTEROP_RUSTORBCPP_CUSTOMTEST_H
#define INTEROP_RUSTORBCPP_CUSTOMTEST_H

#include <functional>
#include <string>

#include "CustomTest.hh"
#include "valuetest.h"

namespace customtest {

using namespace CustomTest;

template <typename T> using ref = typename IDL::traits<T>::ref_type;

// Blob, marshaled as CustomTest.idl says.
class BlobImpl : public OBV_CustomTest::Blob {
 public:
  BlobImpl() = default;
  BlobImpl(int32_t n, const std::string& label, ref<ValueTest::Point> pt, const LongSeq& values)
      : OBV_CustomTest::Blob(n, label, std::move(pt), values) {}

  void marshal(CORBA::DataOutputStream& os) const override {
    os.write_long(n());
    os.write_string(label());
    os.write_Value(pt());
    auto len = static_cast<uint32_t>(values().size());
    os.write_ulong(len);
    os.write_long_array(values(), 0, len);
  }

  void unmarshal(CORBA::DataInputStream& is) override {
    n(is.read_long());
    label(is.read_string());
    pt(IDL::traits<ValueTest::Point>::narrow(is.read_Value()));
    uint32_t len = is.read_ulong();
    LongSeq v;
    is.read_long_array(v, 0, len);
    values(std::move(v));
  }

  ::IDL::traits<::CORBA::ValueBase>::ref_type _copy_value() const override {
    return CORBA::make_reference<BlobImpl>(*this);
  }
};

class BlobFactory : public CORBA::ValueFactoryBase {
 public:
  ref<CORBA::ValueBase> create_for_unmarshal() override {
    return CORBA::make_reference<BlobImpl>();
  }
};

inline void register_factories(ref<CORBA::ORB> orb) {
  valuetest::register_factories(orb);
  orb->register_value_factory(Blob::_rorb_repository_id, CORBA::make_reference<BlobFactory>());
}

class CustomEchoImpl : public CORBA::servant_traits<CustomEcho>::base_type {
 public:
  explicit CustomEchoImpl(ref<CORBA::ORB> orb) : orb_(std::move(orb)) {}

  ref<Blob> echo_blob(ref<Blob> b) override { return b; }
  BlobPair echo_blob_pair(const BlobPair& p) override { return p; }
  BlobSeq echo_blobs(const BlobSeq& s) override { return s; }
  void shutdown() override { orb_->shutdown(false); }

 private:
  ref<CORBA::ORB> orb_;
};

inline ref<Blob> blob(int32_t n, const std::string& label, ref<ValueTest::Point> pt,
                      const LongSeq& values) {
  return CORBA::make_reference<BlobImpl>(n, label, std::move(pt), values);
}

inline bool is_blob(const ref<Blob>& b, int32_t n, const std::string& label,
                    const LongSeq& values) {
  return b && std::dynamic_pointer_cast<BlobImpl>(b._rorb_shared()) && b->n() == n &&
         b->label() == label && b->values() == values;
}

// Runs the custom valuetype cases; `run(name, fn)` reports a case,
// `check(cond, what)` fails one.
template <typename Runner>
void run_cases(Runner& r, ref<CustomEcho> echo,
               const std::function<void(bool, const std::string&)>& check) {
  using valuetest::is_point;
  using valuetest::point;
  r.run("echo_blob[plain]", [&]() {
    auto got = echo->echo_blob(blob(7, "seven", point(1, -2), {1, 2, 3}));
    check(is_blob(got, 7, "seven", {1, 2, 3}), "wrong blob");
    check(is_point(got->pt(), 1, -2), "wrong point");
  });
  r.run("echo_blob[null]", [&]() { check(echo->echo_blob(nullptr) == nullptr, "not null"); });
  r.run("echo_blob[empty]", [&]() {
    auto got = echo->echo_blob(blob(0, "", nullptr, {}));
    check(is_blob(got, 0, "", {}), "wrong blob");
    check(got->pt() == nullptr, "not null");
  });
  r.run("echo_blob[derived]", [&]() {
    auto got = echo->echo_blob(blob(3, "three", valuetest::point3(), {-1}));
    check(is_blob(got, 3, "three", {-1}), "wrong blob");
    check(valuetest::is_point3(got->pt()), "wrong point3");
  });
  r.run("echo_blob_pair[shared]", [&]() {
    auto p = point(5, 6);
    BlobPair got = echo->echo_blob_pair(BlobPair(blob(1, "one", p, {4, 5}), p));
    check(is_blob(got.data(), 1, "one", {4, 5}), "wrong blob");
    check(got.data()->pt() == got.pt(), "not one instance");
    check(is_point(got.pt(), 5, 6), "wrong point");
  });
  r.run("echo_blob_pair[distinct]", [&]() {
    BlobPair got = echo->echo_blob_pair(BlobPair(blob(1, "one", point(5, 6), {}), point(5, 6)));
    check(is_blob(got.data(), 1, "one", {}), "wrong blob");
    check(got.data()->pt() != got.pt(), "one instance");
  });
  r.run("echo_blobs", [&]() {
    LongSeq many;
    for (int32_t i = 0; i < 100; ++i) many.push_back(i);
    auto b = blob(2, "two", nullptr, many);
    auto c = blob(9, "nine", point(9, 9), {});
    BlobSeq got = echo->echo_blobs(BlobSeq{b, c, b, nullptr});
    check(got.size() == 4, "wrong length");
    check(is_blob(got[0], 2, "two", many) && is_blob(got[1], 9, "nine", {}), "wrong blobs");
    check(got[0] == got[2] && got[0] != got[1], "sharing lost");
    check(got[3] == nullptr, "not null");
  });
}

}  // namespace customtest

#endif

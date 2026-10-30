// omniORB C++ implementation of the InteropTest::Echo server.

#include <mutex>

#include "common.h"
#include "anytest.h"
#include "abstracttest.h"
#include "valuetest.h"

using namespace InteropTest;

class EchoImpl : public POA_InteropTest::Echo {
 public:
  explicit EchoImpl(CORBA::ORB_ptr orb) : orb_(CORBA::ORB::_duplicate(orb)) {}

  // --- basic types ---
  CORBA::Boolean echo_boolean(CORBA::Boolean v) override { return v; }
  CORBA::Octet echo_octet(CORBA::Octet v) override { return v; }
  CORBA::Char echo_char(CORBA::Char v) override { return v; }
  CORBA::WChar echo_wchar(CORBA::WChar v) override { return v; }
  CORBA::Short echo_short(CORBA::Short v) override { return v; }
  CORBA::UShort echo_ushort(CORBA::UShort v) override { return v; }
  CORBA::Long echo_long(CORBA::Long v) override { return v; }
  CORBA::ULong echo_ulong(CORBA::ULong v) override { return v; }
  CORBA::LongLong echo_longlong(CORBA::LongLong v) override { return v; }
  CORBA::ULongLong echo_ulonglong(CORBA::ULongLong v) override { return v; }
  CORBA::Float echo_float(CORBA::Float v) override { return v; }
  CORBA::Double echo_double(CORBA::Double v) override { return v; }
  char* echo_string(const char* v) override { return CORBA::string_dup(v); }
  CORBA::WChar* echo_wstring(const CORBA::WChar* v) override { return CORBA::wstring_dup(v); }

  // --- constructed types ---
  Color echo_color(Color v) override { return v; }
  Point echo_point(const Point& v) override { return v; }
  Record* echo_record(const Record& v) override { return new Record(v); }
  RecordSeq* echo_records(const RecordSeq& v) override { return new RecordSeq(v); }
  Value* echo_value(const Value& v) override { return new Value(v); }
  Shade* echo_shade(const Shade& v) override { return new Shade(v); }
  LongArray_slice* echo_long_array(const LongArray v) override { return LongArray_dup(v); }
  Matrix_slice* echo_matrix(const Matrix v) override { return Matrix_dup(v); }
  LongSeq* echo_long_seq(const LongSeq& v) override { return new LongSeq(v); }
  StringSeq* echo_string_seq(const StringSeq& v) override { return new StringSeq(v); }
  OctetSeq* echo_octets(const OctetSeq& v) override { return new OctetSeq(v); }

  // --- parameter modes ---
  CORBA::Long swap(CORBA::Long& a, CORBA::Long& b) override {
    CORBA::Long sum = a + b;
    std::swap(a, b);
    return sum;
  }

  CORBA::Long divide(CORBA::Long a, CORBA::Long b, CORBA::Long& remainder) override {
    if (b == 0) throw DivideByZero();
    remainder = a % b;
    return a / b;
  }

  void make(CORBA::Long x, Point& p, Record_out r) override {
    p.x = x;
    p.y = -x;
    Record_var rec = new Record;
    rec->id = x;
    rec->name = CORBA::string_dup(("r" + std::to_string(x)).c_str());
    rec->hue = BLUE;
    rec->pos = p;
    rec->values.length(2);
    rec->values[0] = x;
    rec->values[1] = x + 1;
    rec->active = true;
    r = rec._retn();
  }

  // --- exceptions ---
  void fail(CORBA::Long code, const char* reason) override { throw Oops(code, reason); }
  void fail_system() override { throw CORBA::BAD_PARAM(42, CORBA::COMPLETED_NO); }

  // --- attributes ---
  CORBA::Long counter() override {
    std::lock_guard<std::mutex> lock(mutex_);
    return counter_;
  }
  void counter(CORBA::Long v) override {
    std::lock_guard<std::mutex> lock(mutex_);
    counter_ = v;
  }
  char* name() override { return CORBA::string_dup("rustorb-interop-echo"); }

  // --- oneway ---
  void post(const char* message) override {
    std::lock_guard<std::mutex> lock(mutex_);
    posted_.push_back(message);
  }

  StringSeq* posted() override {
    std::lock_guard<std::mutex> lock(mutex_);
    StringSeq_var result = new StringSeq;
    result->length(static_cast<CORBA::ULong>(posted_.size()));
    for (CORBA::ULong i = 0; i < posted_.size(); ++i)
      result[i] = CORBA::string_dup(posted_[i].c_str());
    posted_.clear();
    return result._retn();
  }

  // --- object references ---
  InteropTest::Echo_ptr self_ref() override { return _this(); }

  InteropTest::Echo_ptr echo_ref(InteropTest::Echo_ptr other) override {
    if (CORBA::is_nil(other)) return _this();
    return InteropTest::Echo::_duplicate(other);
  }

  CORBA::Boolean is_nil(InteropTest::Echo_ptr other) override { return CORBA::is_nil(other); }

  // --- callbacks ---
  char* call_back(Callback_ptr cb, const char* message) override { return cb->notify(message); }

  void register_callback(Callback_ptr cb) override {
    std::lock_guard<std::mutex> lock(mutex_);
    callback_ = Callback::_duplicate(cb);
  }

  char* trigger(const char* message) override {
    Callback_var cb;
    {
      std::lock_guard<std::mutex> lock(mutex_);
      cb = Callback::_duplicate(callback_);
    }
    if (CORBA::is_nil(cb)) throw Oops(1, "no callback");
    return cb->notify(message);
  }

  // --- lifecycle ---
  void shutdown() override { orb_->shutdown(false); }

 private:
  CORBA::ORB_var orb_;
  std::mutex mutex_;
  CORBA::Long counter_ = 0;
  std::vector<std::string> posted_;
  Callback_var callback_;
};

// Binds obj at path, creating intermediate contexts as needed.
static void bind_name(CosNaming::NamingContext_ptr root, const std::string& path, CORBA::Object_ptr obj) {
  CosNaming::Name name = to_name(path);
  CosNaming::NamingContext_var ctx = CosNaming::NamingContext::_duplicate(root);
  for (CORBA::ULong i = 0; i + 1 < name.length(); ++i) {
    CosNaming::Name component;
    component.length(1);
    component[0] = name[i];
    try {
      ctx = ctx->bind_new_context(component);
    } catch (CosNaming::NamingContext::AlreadyBound&) {
      CORBA::Object_var o = ctx->resolve(component);
      ctx = CosNaming::NamingContext::_narrow(o);
    }
  }
  CosNaming::Name last;
  last.length(1);
  last[0] = name[name.length() - 1];
  ctx->rebind(last, obj);
}

int main(int argc, char** argv) {
  std::vector<std::string> orb_args;
  Options opt = parse_args(argc, argv, orb_args);
  try {
    CORBA::ORB_var orb = orb_init(orb_args);
    if (opt.suite == "value") {
      // The valuetype suite: ValueTest::ValueEcho, which also stops the
      // server.
      valuetest::register_factories(orb);
      PortableServer::POA_var poa = persistent_poa(orb, "ValueTest");
      valuetest::ValueEchoImpl* servant = new valuetest::ValueEchoImpl(orb);
      PortableServer::ObjectId_var oid = PortableServer::string_to_ObjectId("ValueEcho");
      poa->activate_object_with_id(oid, servant);
      servant->_remove_ref();
      CORBA::Object_var obj = poa->id_to_reference(oid);
      CosNaming::NamingContext_var root = root_context(orb, opt.timeout);
      bind_name(root, opt.name, obj);
      std::cout << "server ready: " << opt.name << std::endl;
      orb->run();
      orb->destroy();
      std::cout << "server stopped" << std::endl;
      return 0;
    }
    if (opt.suite == "abstract") {
      // The abstract interface suite: AbstractTest::AbstractEcho, which
      // also stops the server.
      abstracttest::register_factories(orb);
      PortableServer::POA_var poa = persistent_poa(orb, "AbstractTest");
      abstracttest::AbstractEchoImpl* servant = new abstracttest::AbstractEchoImpl(orb);
      PortableServer::ObjectId_var oid = PortableServer::string_to_ObjectId("AbstractEcho");
      poa->activate_object_with_id(oid, servant);
      servant->_remove_ref();
      CORBA::Object_var obj = poa->id_to_reference(oid);
      CosNaming::NamingContext_var root = root_context(orb, opt.timeout);
      bind_name(root, opt.name, obj);
      std::cout << "server ready: " << opt.name << std::endl;
      orb->run();
      orb->destroy();
      std::cout << "server stopped" << std::endl;
      return 0;
    }
    if (opt.suite == "any") {
      // The stage 2 suite: AnyTest::AnyEcho, with an Echo object next to it
      // for the client to shut the server down.
      PortableServer::POA_var poa = persistent_poa(orb, "AnyTest");
      anytest::AnyEchoImpl* any_servant = new anytest::AnyEchoImpl;
      PortableServer::ObjectId_var any_oid = PortableServer::string_to_ObjectId("AnyEcho");
      poa->activate_object_with_id(any_oid, any_servant);
      any_servant->_remove_ref();
      EchoImpl* servant = new EchoImpl(orb);
      PortableServer::ObjectId_var oid = PortableServer::string_to_ObjectId("Echo");
      poa->activate_object_with_id(oid, servant);
      servant->_remove_ref();
      CORBA::Object_var any_obj = poa->id_to_reference(any_oid);
      CORBA::Object_var obj = poa->id_to_reference(oid);
      CosNaming::NamingContext_var root = root_context(orb, opt.timeout);
      bind_name(root, opt.name, any_obj);
      bind_name(root, opt.name + "-Echo", obj);
      std::cout << "server ready: " << opt.name << std::endl;
      orb->run();
      orb->destroy();
      std::cout << "server stopped" << std::endl;
      return 0;
    }
    PortableServer::POA_var poa = persistent_poa(orb, "InteropTest");

    EchoImpl* servant = new EchoImpl(orb);
    PortableServer::ObjectId_var oid = PortableServer::string_to_ObjectId("Echo");
    poa->activate_object_with_id(oid, servant);
    servant->_remove_ref();

    CORBA::Object_var obj = poa->id_to_reference(oid);
    CosNaming::NamingContext_var root = root_context(orb, opt.timeout);
    bind_name(root, opt.name, obj);
    std::cout << "server ready: " << opt.name << std::endl;

    orb->run();
    orb->destroy();
    std::cout << "server stopped" << std::endl;
  } catch (CORBA::Exception& e) {
    std::cerr << "CORBA exception: " << e._name() << std::endl;
    return 1;
  }
  return 0;
}

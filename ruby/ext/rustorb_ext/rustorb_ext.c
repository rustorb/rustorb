/*
 * rustorb_ext: the native part of the rustorb gem, on the C ABI of
 * librustorb_ffi (rustorb.h). It wraps the handles (ORB, object references,
 * POAs, TypeCodes, anys, servants) and the CDR streams for lib/rustorb, which
 * does the mapping (docs/ruby-binding.md).
 *
 * Threads. Ruby code runs only on Ruby threads holding the GVL. Calls that
 * block (an invocation, ORB#run) release the GVL; their callbacks take it
 * back with rb_thread_call_with_gvl. Upcalls (a servant's dispatch) come on
 * rustorb's own threads, which cannot take the GVL: they hand the work to a
 * pool of Ruby dispatcher threads (Rustorb::Native.dispatcher_loop) and wait.
 * The pool grows by one whenever a dispatcher takes the last idle slot, so a
 * servant that calls out and is called back in still finds one.
 *
 * Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.
 */

#include <ruby.h>
#include <ruby/encoding.h>
#include <ruby/thread.h>

#include <pthread.h>
#include <stdlib.h>
#include <string.h>

#include <rustorb/rustorb.h>

static VALUE mRustorb, mNative;
static VALUE cORB, cRef, cOut, cIn, cTC, cAny, cPOA, cPOAManager, cServantRef, cReply, cPeer;
static ID id_call, id_system_exception, id_local_exception, id_dispatch, id_spawn_dispatcher,
    id_peer_ivar;

/* --- exceptions ------------------------------------------------------------- */

/* Raises the exception of status/exc (freeing exc), through
   Rustorb::Native.system_exception / local_exception, which make the
   CORBA exception objects. */
static void raise_status(rorb_status st, rorb_exception* exc) {
  VALUE e;
  if (st == RORB_SYSTEM_EXCEPTION) {
    VALUE id = rb_utf8_str_new(exc->repository_id ? exc->repository_id : "",
                               (long)exc->repository_id_len);
    VALUE minor = UINT2NUM(exc->minor), completed = UINT2NUM(exc->completed);
    rorb_exception_clear(exc);
    e = rb_funcall(mNative, id_system_exception, 3, id, minor, completed);
  } else if (st == RORB_LOCAL_EXCEPTION) {
    VALUE local = UINT2NUM(exc->local), index = UINT2NUM(exc->index);
    rorb_exception_clear(exc);
    e = rb_funcall(mNative, id_local_exception, 2, local, index);
  } else {
    rorb_exception_clear(exc);
    e = rb_funcall(mNative, id_system_exception, 3,
                   rb_str_new_cstr("IDL:omg.org/CORBA/INTERNAL:1.0"), INT2FIX(0), INT2FIX(2));
  }
  rb_exc_raise(e);
}

#define CHECK(call)                                  \
  do {                                               \
    rorb_exception exc_;                             \
    rorb_status st_;                                 \
    memset(&exc_, 0, sizeof exc_);                   \
    st_ = (call);                                    \
    if (st_ != RORB_OK) raise_status(st_, &exc_);    \
  } while (0)
#define EXC (&exc_)

static void raise_bad_inv_order(const char* why) {
  VALUE e = rb_funcall(mNative, id_system_exception, 3,
                       rb_str_new_cstr("IDL:omg.org/CORBA/BAD_INV_ORDER:1.0"), INT2FIX(0),
                       INT2FIX(RORB_COMPLETED_NO));
  (void)why;
  rb_exc_raise(e);
}

/* --- the GVL ---------------------------------------------------------------- */

/* Whether this thread is a Ruby thread that released the GVL with
   without_gvl below (then callbacks use rb_thread_call_with_gvl). */
static __thread int tls_released;

typedef void* (*gvl_fn)(void*);

struct nogvl_call {
  gvl_fn fn;
  void* arg;
};

static void* nogvl_trampoline(void* p) {
  struct nogvl_call* c = p;
  void* r;
  tls_released = 1;
  r = c->fn(c->arg);
  tls_released = 0;
  return r;
}

/* Runs fn(arg) without the GVL. */
static void* without_gvl(gvl_fn fn, void* arg) {
  struct nogvl_call c;
  c.fn = fn;
  c.arg = arg;
  return rb_thread_call_without_gvl(nogvl_trampoline, &c, NULL, NULL);
}

/* The hand-off to the dispatcher threads. */
typedef struct job {
  gvl_fn fn;
  void* arg;
  void* result;
  int done;
  /* A call the dispatcher running this job asks the waiting thread to make
     (at_home below): 1 asked, 2 running, 0 answered. */
  gvl_fn home_fn;
  void* home_arg;
  void* home_result;
  int home_pending;
  pthread_cond_t cond;
  struct job* next;
} job;

/* The job a dispatcher thread is running. */
static __thread job* tls_job;

static pthread_mutex_t queue_lock = PTHREAD_MUTEX_INITIALIZER;
static pthread_cond_t queue_cond = PTHREAD_COND_INITIALIZER;
static job *queue_head, *queue_tail;
static int idle_dispatchers, dispatchers, interrupt_dispatchers, shutting_down;

static void* with_gvl_trampoline(void* p) {
  struct nogvl_call* c = p;
  void* r;
  tls_released = 0;
  r = c->fn(c->arg);
  tls_released = 1;
  return r;
}

/* Runs fn(arg) with the GVL, from any thread: on a Ruby thread that released
   it, by taking it back; on another thread, on a dispatcher thread. Returns
   NULL without running fn when that cannot be done (shutting down). */
static void* with_gvl(gvl_fn fn, void* arg) {
  job j;
  if (tls_released) {
    struct nogvl_call c;
    c.fn = fn;
    c.arg = arg;
    return rb_thread_call_with_gvl(with_gvl_trampoline, &c);
  }
  if (ruby_native_thread_p()) return fn(arg); /* holds the GVL */
  memset(&j, 0, sizeof j);
  j.fn = fn;
  j.arg = arg;
  pthread_cond_init(&j.cond, NULL);
  pthread_mutex_lock(&queue_lock);
  if (shutting_down || dispatchers == 0) {
    pthread_mutex_unlock(&queue_lock);
    pthread_cond_destroy(&j.cond);
    return NULL;
  }
  if (queue_tail)
    queue_tail->next = &j;
  else
    queue_head = &j;
  queue_tail = &j;
  pthread_cond_signal(&queue_cond);
  while (!j.done) {
    pthread_cond_wait(&j.cond, &queue_lock);
    if (j.home_pending == 1) {
      gvl_fn f = j.home_fn;
      void* a = j.home_arg;
      void* r;
      j.home_pending = 2;
      pthread_mutex_unlock(&queue_lock);
      r = f(a);
      pthread_mutex_lock(&queue_lock);
      j.home_result = r;
      j.home_pending = 0;
      pthread_cond_signal(&j.cond);
    }
  }
  pthread_mutex_unlock(&queue_lock);
  pthread_cond_destroy(&j.cond);
  return j.result;
}

static void* home_nogvl(void* p) {
  job* j = tls_job;
  void* r;
  struct nogvl_call* c = p;
  pthread_mutex_lock(&queue_lock);
  j->home_fn = c->fn;
  j->home_arg = c->arg;
  j->home_pending = 1;
  pthread_cond_signal(&j->cond);
  while (j->home_pending) pthread_cond_wait(&j->cond, &queue_lock);
  r = j->home_result;
  pthread_mutex_unlock(&queue_lock);
  return r;
}

/* Runs fn(arg) (which must not call Ruby) on the rustorb thread whose upcall
   this dispatcher is running, or here if it runs none: what rustorb keeps
   per thread about the request being served (PortableServer::Current, the
   servant's current reference) is on that thread. */
static void* at_home(gvl_fn fn, void* arg) {
  struct nogvl_call c;
  if (!tls_job) return fn(arg);
  c.fn = fn;
  c.arg = arg;
  return without_gvl(home_nogvl, &c);
}

/* Runs fn(arg) (a rustorb call, which must not call Ruby) without the GVL:
   on the rustorb thread whose upcall this dispatcher runs, when it runs
   one, so that the core sees the call made inside that upcall (a nested
   call to a SINGLE_THREAD POA, ORB::shutdown(true) in an upcall...). */
static void* blocking(gvl_fn fn, void* arg) {
  return tls_job ? at_home(fn, arg) : without_gvl(fn, arg);
}

static void* wait_job(void* unused) {
  job* j = NULL;
  (void)unused;
  pthread_mutex_lock(&queue_lock);
  ++idle_dispatchers;
  while (!queue_head && !interrupt_dispatchers && !shutting_down)
    pthread_cond_wait(&queue_cond, &queue_lock);
  if (queue_head) {
    j = queue_head;
    queue_head = j->next;
    if (!queue_head) queue_tail = NULL;
  }
  interrupt_dispatchers = 0;
  --idle_dispatchers;
  pthread_mutex_unlock(&queue_lock);
  return j;
}

static void wake_dispatchers(void* unused) {
  (void)unused;
  pthread_mutex_lock(&queue_lock);
  interrupt_dispatchers = 1;
  pthread_cond_broadcast(&queue_cond);
  pthread_mutex_unlock(&queue_lock);
}

/* Rustorb::Native.dispatcher_loop: runs upcalls until the process ends. */
static VALUE native_dispatcher_loop(VALUE self) {
  (void)self;
  pthread_mutex_lock(&queue_lock);
  ++dispatchers;
  pthread_mutex_unlock(&queue_lock);
  for (;;) {
    job* j = rb_thread_call_without_gvl(wait_job, NULL, wake_dispatchers, NULL);
    int spawn;
    if (!j) {
      rb_thread_check_ints();
      pthread_mutex_lock(&queue_lock);
      if (shutting_down) {
        --dispatchers;
        pthread_mutex_unlock(&queue_lock);
        return Qnil;
      }
      pthread_mutex_unlock(&queue_lock);
      continue;
    }
    pthread_mutex_lock(&queue_lock);
    spawn = idle_dispatchers == 0;
    pthread_mutex_unlock(&queue_lock);
    if (spawn) rb_funcall(mNative, id_spawn_dispatcher, 0);
    tls_job = j;
    j->result = j->fn(j->arg);
    tls_job = NULL;
    pthread_mutex_lock(&queue_lock);
    j->done = 1;
    pthread_cond_signal(&j->cond);
    pthread_mutex_unlock(&queue_lock);
  }
  return Qnil;
}

static void at_exit_dispatchers(VALUE unused) {
  job* j;
  (void)unused;
  pthread_mutex_lock(&queue_lock);
  shutting_down = 1;
  /* Jobs nobody will run: they return NULL. */
  for (j = queue_head; j; j = j->next) {
    j->result = NULL;
    j->done = 1;
    pthread_cond_signal(&j->cond);
  }
  queue_head = queue_tail = NULL;
  pthread_cond_broadcast(&queue_cond);
  pthread_mutex_unlock(&queue_lock);
}

/* --- handles ---------------------------------------------------------------- */

typedef struct handle {
  void* p;
  void (*release)(void*);
} handle;

static void handle_free(void* ptr) {
  handle* h = ptr;
  if (h->p && h->release) h->release(h->p);
  xfree(h);
}

static size_t handle_size(const void* ptr) {
  (void)ptr;
  return sizeof(handle);
}

static const rb_data_type_t handle_type = {
    "rustorb_handle", {NULL, handle_free, handle_size, NULL, {0}}, NULL, NULL,
    RUBY_TYPED_FREE_IMMEDIATELY};

static void rel_orb(void* p) { rorb_orb_release(p); }
static void rel_obj(void* p) { rorb_object_release(p); }
static void rel_tc(void* p) { rorb_tc_release(p); }
static void rel_any(void* p) { rorb_any_release(p); }
static void rel_poa(void* p) { rorb_poa_release(p); }
static void rel_mgr(void* p) { rorb_poa_manager_release(p); }
static void rel_servant(void* p) { rorb_servant_release(p); }

/* Wraps p (taking its reference); nil for NULL. */
static VALUE wrap(VALUE klass, void* p, void (*release)(void*)) {
  handle* h;
  VALUE v;
  if (!p) return Qnil;
  v = TypedData_Make_Struct(klass, handle, &handle_type, h);
  h->p = p;
  h->release = release;
  return v;
}

static void* unwrap(VALUE v, VALUE klass) {
  handle* h;
  if (!rb_obj_is_kind_of(v, klass))
    rb_raise(rb_eTypeError, "expected %s", rb_class2name(klass));
  TypedData_Get_Struct(v, handle, &handle_type, h);
  if (!h->p) raise_bad_inv_order("stale handle");
  return h->p;
}

/* nil or a handle of klass. */
static void* unwrap_opt(VALUE v, VALUE klass) { return NIL_P(v) ? NULL : unwrap(v, klass); }

#define W_ORB(p) wrap(cORB, (p), rel_orb)
#define W_OBJ(p) wrap(cRef, (p), rel_obj)
#define W_TC(p) wrap(cTC, (p), rel_tc)
#define W_ANY(p) wrap(cAny, (p), rel_any)
#define W_POA(p) wrap(cPOA, (p), rel_poa)
#define W_MGR(p) wrap(cPOAManager, (p), rel_mgr)
#define ORB(v) ((rorb_orb*)unwrap((v), cORB))
#define OBJ(v) ((rorb_object*)unwrap((v), cRef))
#define TC(v) ((rorb_typecode*)unwrap((v), cTC))
#define ANY(v) ((rorb_any*)unwrap((v), cAny))
#define POA(v) ((rorb_poa*)unwrap((v), cPOA))
#define MGR(v) ((rorb_poa_manager*)unwrap((v), cPOAManager))

/* Streams and replies: valid only during the callback that has them, then
   invalidated (using them raises BAD_INV_ORDER). */
static VALUE wrap_borrowed(VALUE klass, void* p) {
  handle* h;
  VALUE v = TypedData_Make_Struct(klass, handle, &handle_type, h);
  h->p = p;
  h->release = NULL;
  return v;
}

static void invalidate(VALUE v) {
  handle* h;
  if (NIL_P(v)) return;
  TypedData_Get_Struct(v, handle, &handle_type, h);
  h->p = NULL;
}

/* An owned input stream (released with the Ruby object). */
static void rel_in(void* p) { rorb_in_release(p); }

static VALUE bytes_str(rorb_bytes b) {
  VALUE s = rb_str_new((const char*)b.data, (long)b.len);
  rorb_bytes_free(b);
  return s;
}

static VALUE utf8(const char* s, size_t len) { return rb_utf8_str_new(s ? s : "", (long)len); }

/* A String argument as UTF-8 bytes. */
static VALUE utf8_arg(VALUE s) {
  StringValue(s);
  if (rb_enc_get_index(s) != rb_utf8_encindex() && rb_enc_get_index(s) != rb_usascii_encindex())
    s = rb_str_export_to_enc(s, rb_utf8_encoding());
  return s;
}

/* --- ORB ---------------------------------------------------------------- */

/* Rustorb::Native.orb_init(args): [orb, args left]. args[0] is not an
   option (the program name). */
static VALUE native_orb_init(VALUE self, VALUE args) {
  long n, i;
  int argc;
  char** argv;
  rorb_orb* orb = NULL;
  VALUE rest, strs;
  rorb_exception exc;
  rorb_status st;
  (void)self;
  Check_Type(args, T_ARRAY);
  n = RARRAY_LEN(args);
  strs = rb_ary_new_capa(n);
  argv = ALLOCA_N(char*, n + 1);
  for (i = 0; i < n; ++i) {
    VALUE s = rb_str_dup(utf8_arg(rb_ary_entry(args, i)));
    rb_ary_push(strs, s);
    argv[i] = StringValueCStr(s);
  }
  argv[n] = NULL;
  argc = (int)n;
  memset(&exc, 0, sizeof exc);
  if (rorb_abi_version() != RORB_ABI_VERSION) {
    VALUE e = rb_funcall(mNative, id_system_exception, 3,
                         rb_str_new_cstr("IDL:omg.org/CORBA/INITIALIZE:1.0"), INT2FIX(0),
                         INT2FIX(RORB_COMPLETED_NO));
    rb_exc_raise(e);
  }
  st = rorb_orb_init(&argc, argv, &orb, &exc);
  if (st != RORB_OK) raise_status(st, &exc);
  rest = rb_ary_new_capa(argc);
  for (i = 0; i < argc; ++i) rb_ary_push(rest, rb_utf8_str_new_cstr(argv[i]));
  RB_GC_GUARD(strs);
  return rb_assoc_new(W_ORB(orb), rest);
}

static VALUE native_default_orb(VALUE self) {
  (void)self;
  return W_ORB(rorb_default_orb());
}

static VALUE native_default_root_poa(VALUE self) {
  rorb_poa* poa = NULL;
  (void)self;
  CHECK(rorb_default_root_poa(&poa, EXC));
  return W_POA(poa);
}

static VALUE orb_string_to_object(VALUE self, VALUE s) {
  rorb_object* obj = NULL;
  s = utf8_arg(s);
  CHECK(rorb_orb_string_to_object(ORB(self), RSTRING_PTR(s), (size_t)RSTRING_LEN(s), &obj, EXC));
  return W_OBJ(obj);
}

static VALUE orb_object_to_string(VALUE self, VALUE obj) {
  rorb_bytes b;
  CHECK(rorb_orb_object_to_string(ORB(self), unwrap_opt(obj, cRef), &b, EXC));
  return rb_enc_associate(bytes_str(b), rb_utf8_encoding());
}

static VALUE orb_resolve(VALUE self, VALUE name) {
  rorb_object* obj = NULL;
  name = utf8_arg(name);
  CHECK(rorb_orb_resolve_initial_references(ORB(self), RSTRING_PTR(name),
                                            (size_t)RSTRING_LEN(name), &obj, EXC));
  return W_OBJ(obj);
}

static void each_service(void* ctx, const char* name, size_t len) {
  rb_ary_push((VALUE)ctx, utf8(name, len));
}

static VALUE orb_list_initial_services(VALUE self) {
  VALUE a = rb_ary_new();
  rorb_orb_list_initial_services(ORB(self), (void*)a, each_service);
  return a;
}

static VALUE orb_root_poa(VALUE self) {
  rorb_poa* poa = NULL;
  CHECK(rorb_orb_root_poa(ORB(self), &poa, EXC));
  return W_POA(poa);
}

static void* run_nogvl(void* orb) {
  rorb_orb_run(orb);
  return NULL;
}

static VALUE orb_run(VALUE self) {
  rorb_orb* orb = ORB(self);
  without_gvl(run_nogvl, orb);
  return Qnil;
}

struct orb_op {
  rorb_orb* orb;
  int32_t wait;
  rorb_status st;
  rorb_exception exc;
};

static void* shutdown_nogvl(void* p) {
  struct orb_op* o = p;
  o->st = rorb_orb_shutdown(o->orb, o->wait, &o->exc);
  return NULL;
}

static void* destroy_nogvl(void* p) {
  struct orb_op* o = p;
  o->st = rorb_orb_destroy(o->orb, &o->exc);
  return NULL;
}

static void* perform_work_nogvl(void* p) {
  struct orb_op* o = p;
  o->st = rorb_orb_perform_work(o->orb, &o->exc);
  return NULL;
}

static VALUE orb_shutdown(VALUE self, VALUE wait) {
  struct orb_op o;
  memset(&o, 0, sizeof o);
  o.orb = ORB(self);
  o.wait = RTEST(wait);
  blocking(shutdown_nogvl, &o);
  if (o.st != RORB_OK) raise_status(o.st, &o.exc);
  return Qnil;
}

static VALUE orb_destroy(VALUE self) {
  struct orb_op o;
  memset(&o, 0, sizeof o);
  o.orb = ORB(self);
  blocking(destroy_nogvl, &o);
  if (o.st != RORB_OK) raise_status(o.st, &o.exc);
  return Qnil;
}

static VALUE orb_work_pending(VALUE self) {
  int32_t pending = 0;
  CHECK(rorb_orb_work_pending(ORB(self), &pending, EXC));
  return pending ? Qtrue : Qfalse;
}

static VALUE orb_perform_work(VALUE self) {
  struct orb_op o;
  memset(&o, 0, sizeof o);
  o.orb = ORB(self);
  blocking(perform_work_nogvl, &o);
  if (o.st != RORB_OK) raise_status(o.st, &o.exc);
  return Qnil;
}

static VALUE orb_set_call_timeout(VALUE self, VALUE us) {
  rorb_orb_set_call_timeout(ORB(self), NUM2LL(us));
  return Qnil;
}

static VALUE orb_call_timeout(VALUE self) { return LL2NUM(rorb_orb_call_timeout(ORB(self))); }

static VALUE orb_set_connect_timeout(VALUE self, VALUE us) {
  rorb_orb_set_connect_timeout(ORB(self), NUM2LL(us));
  return Qnil;
}

static VALUE orb_register_initial_reference(VALUE self, VALUE name, VALUE obj) {
  name = utf8_arg(name);
  CHECK(rorb_orb_register_initial_reference(ORB(self), RSTRING_PTR(name),
                                            (size_t)RSTRING_LEN(name), unwrap_opt(obj, cRef),
                                            EXC));
  return Qnil;
}

static VALUE native_set_thread_call_timeout(VALUE self, VALUE us) {
  (void)self;
  rorb_set_thread_call_timeout(NUM2LL(us));
  return Qnil;
}

static VALUE native_thread_call_timeout(VALUE self) {
  (void)self;
  return LL2NUM(rorb_thread_call_timeout());
}

/* --- object references ------------------------------------------------------- */

struct ref_op {
  rorb_object* obj;
  const char* s;
  size_t len;
  int32_t result;
  rorb_status st;
  rorb_exception exc;
};

static void* is_a_nogvl(void* p) {
  struct ref_op* o = p;
  o->st = rorb_object_is_a(o->obj, o->s, o->len, &o->result, &o->exc);
  return NULL;
}

static void* non_existent_nogvl(void* p) {
  struct ref_op* o = p;
  o->st = rorb_object_non_existent(o->obj, &o->result, &o->exc);
  return NULL;
}

static void* validate_nogvl(void* p) {
  struct ref_op* o = p;
  o->st = rorb_object_validate_connection(o->obj, &o->result, &o->exc);
  return NULL;
}

static VALUE ref_call(VALUE self, gvl_fn fn, VALUE s) {
  struct ref_op o;
  memset(&o, 0, sizeof o);
  o.obj = OBJ(self);
  if (!NIL_P(s)) {
    s = utf8_arg(s);
    o.s = RSTRING_PTR(s);
    o.len = (size_t)RSTRING_LEN(s);
  }
  blocking(fn, &o);
  RB_GC_GUARD(s);
  if (o.st != RORB_OK) raise_status(o.st, &o.exc);
  return o.result ? Qtrue : Qfalse;
}

static VALUE ref_is_a(VALUE self, VALUE id) { return ref_call(self, is_a_nogvl, id); }
static VALUE ref_non_existent(VALUE self) { return ref_call(self, non_existent_nogvl, Qnil); }
static VALUE ref_validate_connection(VALUE self) { return ref_call(self, validate_nogvl, Qnil); }

static VALUE ref_equivalent(VALUE self, VALUE other) {
  return rorb_object_is_equivalent(OBJ(self), OBJ(other)) ? Qtrue : Qfalse;
}

static VALUE ref_hash(VALUE self, VALUE max) {
  return UINT2NUM(rorb_object_hash(OBJ(self), NUM2UINT(max)));
}

static VALUE ref_type_id(VALUE self) {
  size_t len = 0;
  const char* s = rorb_object_type_id(OBJ(self), &len);
  return utf8(s, len);
}

static VALUE ref_orb(VALUE self) { return W_ORB(rorb_object_orb(OBJ(self))); }

static VALUE ref_dup(VALUE self) { return W_OBJ(rorb_object_clone(OBJ(self))); }

static VALUE ref_set_call_timeout(VALUE self, VALUE us) {
  rorb_object_set_call_timeout(OBJ(self), NUM2LL(us));
  return Qnil;
}

static VALUE ref_call_timeout(VALUE self) { return LL2NUM(rorb_object_call_timeout(OBJ(self))); }

/* The context of an invocation: the Ruby procs and the error a callback
   raised. On the stack of the invoking Ruby thread (so its VALUEs are
   marked). */
struct invoke_ctx {
  VALUE writer, reader, user_exc;
  VALUE error, user_error;
  rorb_out* out;
  rorb_in* in;
  const char* exc_id;
  size_t exc_id_len;
  rorb_object* obj;
  rorb_invocation inv;
  rorb_status st;
  rorb_exception exc;
};

struct protect_call {
  VALUE proc;
  int argc;
  VALUE argv[2];
};

static VALUE protect_body(VALUE p) {
  struct protect_call* c = (struct protect_call*)p;
  return rb_funcallv(c->proc, id_call, c->argc, c->argv);
}

/* Calls proc with the arguments; on an exception keeps it in *error and
   returns Qundef. */
static VALUE call_protected(VALUE proc, int argc, VALUE* argv, VALUE* error) {
  struct protect_call c;
  int state = 0;
  VALUE r;
  c.proc = proc;
  c.argc = argc;
  c.argv[0] = argc > 0 ? argv[0] : Qnil;
  c.argv[1] = argc > 1 ? argv[1] : Qnil;
  r = rb_protect(protect_body, (VALUE)&c, &state);
  if (state) {
    *error = rb_errinfo();
    rb_set_errinfo(Qnil);
    return Qundef;
  }
  return r;
}

static void* cb_write(void* p) {
  struct invoke_ctx* c = p;
  VALUE out = wrap_borrowed(cOut, c->out);
  VALUE r = call_protected(c->writer, 1, &out, &c->error);
  invalidate(out);
  return r == Qundef ? (void*)1 : NULL;
}

static void* cb_read(void* p) {
  struct invoke_ctx* c = p;
  VALUE in = wrap_borrowed(cIn, c->in);
  VALUE r = call_protected(c->reader, 1, &in, &c->error);
  invalidate(in);
  return r == Qundef ? (void*)1 : NULL;
}

/* Returns 1 (failed), 2 (taken) or 3 (not declared). */
static void* cb_user_exc(void* p) {
  struct invoke_ctx* c = p;
  VALUE argv[2];
  VALUE r;
  argv[0] = utf8(c->exc_id, c->exc_id_len);
  argv[1] = wrap_borrowed(cIn, c->in);
  r = call_protected(c->user_exc, 2, argv, &c->error);
  invalidate(argv[1]);
  if (r == Qundef) return (void*)1;
  if (NIL_P(r)) return (void*)3;
  c->user_error = r;
  return (void*)2;
}

static rorb_status write_args(void* ctx, rorb_out* out) {
  struct invoke_ctx* c = ctx;
  if (NIL_P(c->writer)) return RORB_OK;
  c->out = out;
  return with_gvl(cb_write, c) ? RORB_CALLBACK_FAILED : RORB_OK;
}

static rorb_status read_reply(void* ctx, rorb_in* in) {
  struct invoke_ctx* c = ctx;
  if (NIL_P(c->reader)) return RORB_OK;
  c->in = in;
  return with_gvl(cb_read, c) ? RORB_CALLBACK_FAILED : RORB_OK;
}

static rorb_status read_user_exception(void* ctx, const char* id, size_t len, rorb_in* in) {
  struct invoke_ctx* c = ctx;
  void* r;
  if (NIL_P(c->user_exc)) return RORB_SYSTEM_EXCEPTION;
  c->in = in;
  c->exc_id = id;
  c->exc_id_len = len;
  r = with_gvl(cb_user_exc, c);
  if (r == (void*)2) return RORB_USER_EXCEPTION;
  if (r == (void*)3) return RORB_SYSTEM_EXCEPTION;
  return RORB_CALLBACK_FAILED;
}

static void* invoke_nogvl(void* p) {
  struct invoke_ctx* c = p;
  c->st = rorb_object_invoke(c->obj, &c->inv, &c->exc);
  return NULL;
}

/* Ref#invoke(op, oneway, timeout_us, writer, reader, user_exc): calls the
   operation. writer.call(out) writes the arguments, reader.call(in) reads
   the reply, user_exc.call(id, in) reads a user exception and returns it,
   or nil when the operation does not declare it (the call then raises
   UNKNOWN). The user exception, or an error a proc raised, is raised
   here. Returns nil. */
static VALUE ref_invoke(VALUE self, VALUE op, VALUE oneway, VALUE timeout, VALUE writer,
                        VALUE reader, VALUE user_exc) {
  struct invoke_ctx c;
  memset(&c, 0, sizeof c);
  op = utf8_arg(op);
  c.writer = writer;
  c.reader = reader;
  c.user_exc = user_exc;
  c.error = Qnil;
  c.user_error = Qnil;
  c.obj = OBJ(self);
  c.inv.operation = RSTRING_PTR(op);
  c.inv.operation_len = (size_t)RSTRING_LEN(op);
  c.inv.oneway = RTEST(oneway);
  c.inv.timeout_us = NIL_P(timeout) ? -1 : NUM2LL(timeout);
  c.inv.context = &c;
  c.inv.write_args = write_args;
  c.inv.read_reply = read_reply;
  c.inv.read_user_exception = read_user_exception;
  blocking(invoke_nogvl, &c);
  RB_GC_GUARD(op);
  RB_GC_GUARD(writer);
  RB_GC_GUARD(reader);
  RB_GC_GUARD(user_exc);
  if (!NIL_P(c.error)) {
    rorb_exception_clear(&c.exc);
    rb_exc_raise(c.error);
  }
  if (c.st == RORB_USER_EXCEPTION && !NIL_P(c.user_error)) rb_exc_raise(c.user_error);
  if (c.st == RORB_CALLBACK_FAILED || c.st == RORB_USER_EXCEPTION) {
    rorb_exception_clear(&c.exc);
    raise_bad_inv_order("callback failed without an error");
  }
  if (c.st != RORB_OK) raise_status(c.st, &c.exc);
  return Qnil;
}

/* --- output streams ------------------------------------------------------------ */

#define OUT(v) ((rorb_out*)unwrap((v), cOut))
#define IN(v) ((rorb_in*)unwrap((v), cIn))

static VALUE out_write_boolean(VALUE self, VALUE v) {
  rorb_out_write_boolean(OUT(self), RTEST(v));
  return self;
}
static VALUE out_write_octet(VALUE self, VALUE v) {
  rorb_out_write_octet(OUT(self), (uint8_t)NUM2UINT(v));
  return self;
}
static VALUE out_write_char(VALUE self, VALUE v) {
  rorb_out_write_char(OUT(self), (char)NUM2INT(v));
  return self;
}
static VALUE out_write_wchar(VALUE self, VALUE v) {
  rorb_out_write_wchar(OUT(self), (uint16_t)NUM2UINT(v));
  return self;
}
static VALUE out_write_short(VALUE self, VALUE v) {
  rorb_out_write_short(OUT(self), (int16_t)NUM2INT(v));
  return self;
}
static VALUE out_write_ushort(VALUE self, VALUE v) {
  rorb_out_write_ushort(OUT(self), (uint16_t)NUM2UINT(v));
  return self;
}
static VALUE out_write_long(VALUE self, VALUE v) {
  rorb_out_write_long(OUT(self), (int32_t)NUM2LONG(v));
  return self;
}
static VALUE out_write_ulong(VALUE self, VALUE v) {
  rorb_out_write_ulong(OUT(self), (uint32_t)NUM2ULONG(v));
  return self;
}
static VALUE out_write_longlong(VALUE self, VALUE v) {
  rorb_out_write_longlong(OUT(self), (int64_t)NUM2LL(v));
  return self;
}
static VALUE out_write_ulonglong(VALUE self, VALUE v) {
  rorb_out_write_ulonglong(OUT(self), (uint64_t)NUM2ULL(v));
  return self;
}
static VALUE out_write_float(VALUE self, VALUE v) {
  rorb_out_write_float(OUT(self), (float)NUM2DBL(v));
  return self;
}
static VALUE out_write_double(VALUE self, VALUE v) {
  rorb_out_write_double(OUT(self), NUM2DBL(v));
  return self;
}
static VALUE out_write_string(VALUE self, VALUE v) {
  v = utf8_arg(v);
  rorb_out_write_string(OUT(self), RSTRING_PTR(v), (size_t)RSTRING_LEN(v));
  return self;
}
/* A wstring as a String of UTF-16LE code units. */
static VALUE out_write_wstring(VALUE self, VALUE units) {
  StringValue(units);
  rorb_out_write_wstring(OUT(self), (const uint16_t*)RSTRING_PTR(units),
                         (size_t)RSTRING_LEN(units) / 2);
  return self;
}
static VALUE out_write_object(VALUE self, VALUE ref) {
  rorb_out_write_object(OUT(self), unwrap_opt(ref, cRef));
  return self;
}
static VALUE out_write_typecode(VALUE self, VALUE tc) {
  rorb_out_write_typecode(OUT(self), TC(tc));
  return self;
}
static VALUE out_write_any(VALUE self, VALUE any) {
  rorb_out_write_any(OUT(self), ANY(any));
  return self;
}
static VALUE out_write_any_value(VALUE self, VALUE any) {
  rorb_out_write_any_value(OUT(self), ANY(any));
  return self;
}
static VALUE out_write_longdouble(VALUE self, VALUE hi, VALUE lo) {
  rorb_out_write_longdouble(OUT(self), NUM2ULL(hi), NUM2ULL(lo));
  return self;
}
/* digits: a String of digit values 0 to 9, most significant first. */
static VALUE out_write_fixed(VALUE self, VALUE digits, VALUE negative) {
  StringValue(digits);
  rorb_out_write_fixed(OUT(self), (const uint8_t*)RSTRING_PTR(digits),
                       (size_t)RSTRING_LEN(digits), RTEST(negative));
  return self;
}
static const size_t prim_size[] = {1, 2, 2, 4, 4, 8, 8, 4, 8};
/* data: the elements packed in the host's representation. */
static VALUE out_write_array(VALUE self, VALUE kind, VALUE data) {
  int k = NUM2INT(kind);
  if (k < 0 || k > RORB_PRIM_DOUBLE) rb_raise(rb_eArgError, "bad element kind");
  StringValue(data);
  rorb_out_write_array(OUT(self), k, RSTRING_PTR(data), (size_t)RSTRING_LEN(data) / prim_size[k]);
  return self;
}
static VALUE out_failed(VALUE self) { return rorb_out_failed(OUT(self)) ? Qtrue : Qfalse; }
static VALUE out_keeps_hosts(VALUE self) {
  return rorb_out_keeps_hosts(OUT(self)) ? Qtrue : Qfalse;
}

/* --- input streams --------------------------------------------------------------- */

static VALUE in_read_boolean(VALUE self) { return rorb_in_read_boolean(IN(self)) ? Qtrue : Qfalse; }
static VALUE in_read_octet(VALUE self) { return UINT2NUM(rorb_in_read_octet(IN(self))); }
static VALUE in_read_char(VALUE self) { return INT2FIX((unsigned char)rorb_in_read_char(IN(self))); }
static VALUE in_read_wchar(VALUE self) { return UINT2NUM(rorb_in_read_wchar(IN(self))); }
static VALUE in_read_short(VALUE self) { return INT2FIX(rorb_in_read_short(IN(self))); }
static VALUE in_read_ushort(VALUE self) { return UINT2NUM(rorb_in_read_ushort(IN(self))); }
static VALUE in_read_long(VALUE self) { return LONG2NUM(rorb_in_read_long(IN(self))); }
static VALUE in_read_ulong(VALUE self) { return ULONG2NUM(rorb_in_read_ulong(IN(self))); }
static VALUE in_read_longlong(VALUE self) { return LL2NUM(rorb_in_read_longlong(IN(self))); }
static VALUE in_read_ulonglong(VALUE self) { return ULL2NUM(rorb_in_read_ulonglong(IN(self))); }
static VALUE in_read_float(VALUE self) { return DBL2NUM(rorb_in_read_float(IN(self))); }
static VALUE in_read_double(VALUE self) { return DBL2NUM(rorb_in_read_double(IN(self))); }
static VALUE in_read_longdouble(VALUE self) {
  uint64_t hi = 0, lo = 0;
  rorb_in_read_longdouble(IN(self), &hi, &lo);
  return rb_assoc_new(ULL2NUM(hi), ULL2NUM(lo));
}
static VALUE in_read_string(VALUE self) {
  size_t len = 0;
  const char* s = rorb_in_read_string(IN(self), &len);
  return utf8(s, len);
}
/* A wstring as a String of UTF-16LE code units. */
static VALUE in_read_wstring(VALUE self) {
  size_t len = 0;
  const uint16_t* s = rorb_in_read_wstring(IN(self), &len);
  return rb_str_new((const char*)s, (long)(len * 2));
}
static VALUE in_read_object(VALUE self) { return W_OBJ(rorb_in_read_object(IN(self))); }
static VALUE in_read_typecode(VALUE self) { return W_TC(rorb_in_read_typecode(IN(self))); }
static VALUE in_read_any(VALUE self) { return W_ANY(rorb_in_read_any(IN(self))); }
static VALUE in_read_any_value(VALUE self, VALUE tc) {
  return W_ANY(rorb_in_read_any_value(IN(self), TC(tc)));
}
static VALUE in_read_array(VALUE self, VALUE kind, VALUE count) {
  int k = NUM2INT(kind);
  size_t n = NUM2SIZET(count);
  VALUE s;
  if (k < 0 || k > RORB_PRIM_DOUBLE) rb_raise(rb_eArgError, "bad element kind");
  s = rb_str_new(NULL, (long)(n * prim_size[k]));
  rorb_in_read_array(IN(self), k, RSTRING_PTR(s), n);
  return s;
}
/* [digits, negative]: count digit values. */
static VALUE in_read_fixed(VALUE self, VALUE count) {
  size_t n = NUM2SIZET(count);
  VALUE s = rb_str_new(NULL, (long)n);
  int32_t neg = rorb_in_read_fixed(IN(self), (uint8_t*)RSTRING_PTR(s), n);
  return rb_assoc_new(s, neg ? Qtrue : Qfalse);
}
static VALUE in_read_length(VALUE self, VALUE min) {
  return UINT2NUM(rorb_in_read_length(IN(self), NUM2SIZET(min)));
}
static VALUE in_fail(VALUE self) {
  rorb_in_fail(IN(self));
  return self;
}
static VALUE in_failed(VALUE self) { return rorb_in_failed(IN(self)) ? Qtrue : Qfalse; }

/* --- valuetypes ----------------------------------------------------------------- */

/* The Ruby instances rustorb keeps (in the streams of one message, in the
   core instances of anys): one host per instance, counted, released from
   any thread. The GC marks (and pins) the instances while they are kept,
   and the same instance always has the same host, whose address is the key
   the core knows it by. */
typedef struct host {
  VALUE obj;
  long count;
  struct host* next;
} host;

#define HOST_BUCKETS 4096
static host* host_buckets[HOST_BUCKETS];
static pthread_mutex_t host_lock = PTHREAD_MUTEX_INITIALIZER;

static size_t host_bucket(VALUE obj) { return (size_t)(((uintptr_t)obj >> 3) % HOST_BUCKETS); }

/* With the GVL. */
static host* host_retain(VALUE obj) {
  host* h;
  size_t b = host_bucket(obj);
  pthread_mutex_lock(&host_lock);
  for (h = host_buckets[b]; h; h = h->next)
    if (h->obj == obj) break;
  if (!h) {
    h = calloc(1, sizeof *h);
    if (!h) {
      pthread_mutex_unlock(&host_lock);
      rb_raise(rb_eNoMemError, "rustorb: no memory for a value");
    }
    h->obj = obj;
    h->next = host_buckets[b];
    host_buckets[b] = h;
  }
  ++h->count;
  pthread_mutex_unlock(&host_lock);
  return h;
}

/* From any thread. */
static void host_release(void* p) {
  host* h = p;
  host** link;
  pthread_mutex_lock(&host_lock);
  if (--h->count == 0) {
    for (link = &host_buckets[host_bucket(h->obj)]; *link; link = &(*link)->next)
      if (*link == h) {
        *link = h->next;
        break;
      }
    free(h);
  }
  pthread_mutex_unlock(&host_lock);
}

static void hosts_mark(void* unused) {
  size_t b;
  host* h;
  (void)unused;
  pthread_mutex_lock(&host_lock);
  for (b = 0; b < HOST_BUCKETS; ++b)
    for (h = host_buckets[b]; h; h = h->next) rb_gc_mark(h->obj);
  pthread_mutex_unlock(&host_lock);
}

static const rb_data_type_t hosts_type = {
    "rustorb_hosts", {hosts_mark, NULL, NULL, NULL, {0}}, NULL, NULL, 0};

static VALUE native_kept_values(VALUE self) {
  size_t b, n = 0;
  host* h;
  (void)self;
  pthread_mutex_lock(&host_lock);
  for (b = 0; b < HOST_BUCKETS; ++b)
    for (h = host_buckets[b]; h; h = h->next) ++n;
  pthread_mutex_unlock(&host_lock);
  return SIZET2NUM(n);
}

static VALUE out_write_null_value(VALUE self) {
  rorb_out_write_null_value(OUT(self));
  return self;
}

/* begin_value(obj, ids, chunked): true when the caller now writes the
   state (then end_value), false when an indirection was written. */
static VALUE out_begin_value(VALUE self, VALUE obj, VALUE ids, VALUE chunked) {
  rorb_out* out = OUT(self);
  size_t n, i;
  const char** cp;
  size_t* cl;
  int32_t r;
  Check_Type(ids, T_ARRAY);
  n = (size_t)RARRAY_LEN(ids);
  cp = ALLOCA_N(const char*, n + 1);
  cl = ALLOCA_N(size_t, n + 1);
  ids = rb_ary_dup(ids);
  for (i = 0; i < n; ++i) {
    VALUE s = utf8_arg(rb_ary_entry(ids, (long)i));
    rb_ary_store(ids, (long)i, s);
    cp[i] = RSTRING_PTR(s);
    cl[i] = (size_t)RSTRING_LEN(s);
  }
  if (rorb_out_keeps_hosts(out)) {
    host* h = host_retain(obj);
    r = rorb_out_begin_host_value(out, (uint64_t)(uintptr_t)h, h, host_release, n, cp, cl,
                                  RTEST(chunked));
  } else {
    uint64_t key = ((uint64_t)1 << 62) | (NUM2ULL(rb_obj_id(obj)) & (((uint64_t)1 << 62) - 1));
    r = rorb_out_begin_value(out, key, n, cp, cl, RTEST(chunked));
  }
  RB_GC_GUARD(ids);
  return r ? Qtrue : Qfalse;
}

static VALUE out_end_value(VALUE self) {
  rorb_out_end_value(OUT(self));
  return self;
}

/* [kind, slot, ids]: kind 0 null, 1 an indirection (value_at(slot)), 2 a
   new value (keep_value(slot, obj), read its state, end_value). */
static VALUE in_read_value_header(VALUE self) {
  rorb_in* in = IN(self);
  size_t slot = 0, count = 0, i;
  int32_t kind = rorb_in_read_value_header(in, &slot, &count);
  VALUE ids = rb_ary_new();
  for (i = 0; kind == RORB_VALUE_NEW && i < count; ++i) {
    size_t len = 0;
    const char* id = rorb_in_value_id(in, i, &len);
    rb_ary_push(ids, utf8(id, len));
  }
  return rb_ary_new_from_args(3, INT2NUM(kind), SIZET2NUM(slot), ids);
}

static VALUE in_value_at(VALUE self, VALUE slot) {
  host* h = rorb_in_get_value(IN(self), NUM2SIZET(slot));
  return h ? h->obj : Qnil;
}

static VALUE in_keep_value(VALUE self, VALUE slot, VALUE obj) {
  rorb_in* in = IN(self);
  host* h = host_retain(obj);
  if (rorb_in_keeps_hosts(in))
    rorb_in_set_host_value(in, NUM2SIZET(slot), (uint64_t)(uintptr_t)h, h, host_release);
  else
    rorb_in_set_value(in, NUM2SIZET(slot), h, host_release);
  return obj;
}

static VALUE in_end_value(VALUE self) {
  rorb_in_end_value(IN(self));
  return self;
}

/* The valuetypes the core reads in anys and does not know:
   Rustorb.resolve_value_tc(id) gives their TypeCode. */
struct resolve_call {
  const char* id;
  size_t len;
  rorb_typecode* tc;
};

static VALUE resolve_body(VALUE p) {
  struct resolve_call* c = (struct resolve_call*)p;
  VALUE tc = rb_funcall(mRustorb, rb_intern("resolve_value_tc"), 1, utf8(c->id, c->len));
  if (!NIL_P(tc)) c->tc = rorb_tc_clone(TC(tc));
  return Qnil;
}

static void* resolve_gvl(void* p) {
  int state = 0;
  rb_protect(resolve_body, (VALUE)p, &state);
  if (state) rb_set_errinfo(Qnil);
  return NULL;
}

static rorb_typecode* resolve_value_type(void* context, const char* id, size_t len) {
  struct resolve_call c;
  (void)context;
  c.id = id;
  c.len = len;
  c.tc = NULL;
  with_gvl(resolve_gvl, &c);
  return c.tc;
}

/* --- callback AMI ----------------------------------------------------------------- */

/* A request rorb_object_sendc sent: the arguments, written into anys when
   sendc_ was called (rorb_out_write_any_value writes them, without the
   GVL, as often as the core asks); the procs, kept alive by the host of
   keep until done. */
struct sendc_ctx {
  struct invoke_ctx c;
  host* keep;
  rorb_any** anys;
  size_t count;
  VALUE done;
};

static rorb_status sendc_write(void* ctx, rorb_out* out) {
  struct sendc_ctx* s = ctx;
  size_t i;
  for (i = 0; i < s->count; ++i) rorb_out_write_any_value(out, s->anys[i]);
  return RORB_OK;
}

struct sendc_done_call {
  struct sendc_ctx* s;
  rorb_status st;
  const rorb_exception* exc;
};

static void* sendc_done_gvl(void* p) {
  struct sendc_done_call* d = p;
  VALUE argv[2], error = Qnil;
  argv[0] = INT2NUM(d->st);
  argv[1] = Qnil;
  if (d->st == RORB_SYSTEM_EXCEPTION && d->exc)
    argv[1] = rb_ary_new_from_args(
        3, utf8(d->exc->repository_id, d->exc->repository_id_len), UINT2NUM(d->exc->minor),
        UINT2NUM(d->exc->completed));
  call_protected(d->s->done, 2, argv, &error);
  return NULL;
}

static void sendc_done(void* ctx, rorb_status st, const rorb_exception* exc) {
  struct sendc_done_call d;
  struct sendc_ctx* s = ctx;
  d.s = s;
  d.st = st;
  d.exc = exc;
  with_gvl(sendc_done_gvl, &d);
  host_release(s->keep);
  free(s->anys);
  free(s);
}

struct sendc_call {
  rorb_object* obj;
  struct sendc_ctx* s;
  rorb_status st;
  rorb_exception exc;
};

static void* sendc_nogvl(void* p) {
  struct sendc_call* c = p;
  c->st = rorb_object_sendc(c->obj, &c->s->c.inv, sendc_done, &c->exc);
  return NULL;
}

/* Ref#sendc(op, anys, timeout_us, reader, user_exc, done): sends the
   request without waiting. anys are the in and inout arguments (Native::Any
   values of their types); reader.call(in) reads the reply and
   user_exc.call(id, in) a user exception (as for invoke, on a thread of the
   reply pool); done.call(status, [id, minor, completed] or nil) follows
   (RORB_OK, RORB_USER_EXCEPTION, RORB_SYSTEM_EXCEPTION or
   RORB_CALLBACK_FAILED). Errors the procs raise go nowhere: they keep
   them. */
static VALUE ref_sendc(VALUE self, VALUE op, VALUE anys, VALUE timeout, VALUE reader,
                       VALUE user_exc, VALUE done) {
  struct sendc_call c;
  struct sendc_ctx* s;
  size_t i, n;
  VALUE keep;
  Check_Type(anys, T_ARRAY);
  op = utf8_arg(op);
  n = (size_t)RARRAY_LEN(anys);
  for (i = 0; i < n; ++i) ANY(rb_ary_entry(anys, (long)i)); /* type checks */
  s = calloc(1, sizeof *s);
  if (!s) rb_raise(rb_eNoMemError, "rustorb: no memory for a request");
  s->anys = calloc(n + 1, sizeof *s->anys);
  if (!s->anys) {
    free(s);
    rb_raise(rb_eNoMemError, "rustorb: no memory for a request");
  }
  for (i = 0; i < n; ++i) s->anys[i] = ANY(rb_ary_entry(anys, (long)i));
  s->count = n;
  keep = rb_ary_new_from_args(4, rb_ary_dup(anys), reader, user_exc, done);
  s->keep = host_retain(keep);
  s->done = done;
  s->c.writer = Qnil;
  s->c.reader = reader;
  s->c.user_exc = user_exc;
  s->c.error = Qnil;
  s->c.user_error = Qnil;
  s->c.inv.operation = RSTRING_PTR(op);
  s->c.inv.operation_len = (size_t)RSTRING_LEN(op);
  s->c.inv.timeout_us = NIL_P(timeout) ? -1 : NUM2LL(timeout);
  s->c.inv.context = s;
  s->c.inv.write_args = sendc_write;
  s->c.inv.read_reply = read_reply;
  s->c.inv.read_user_exception = read_user_exception;
  memset(&c, 0, sizeof c);
  c.obj = OBJ(self);
  c.s = s;
  blocking(sendc_nogvl, &c);
  RB_GC_GUARD(op);
  RB_GC_GUARD(keep);
  if (c.st != RORB_OK) {
    host_release(s->keep);
    free(s->anys);
    free(s);
    raise_status(c.st, &c.exc);
  }
  return Qnil;
}

/* Native.cdr_encode(little_endian, writer): the bytes writer.call(out)
   writes on a standalone stream (rorb_cdr_encode). */
struct cdr_ctx {
  VALUE proc;
  VALUE error;
};

static rorb_status cdr_write(void* ctx, rorb_out* out) {
  struct cdr_ctx* c = ctx;
  VALUE o = wrap_borrowed(cOut, out);
  VALUE r = call_protected(c->proc, 1, &o, &c->error);
  invalidate(o);
  return r == Qundef ? RORB_CALLBACK_FAILED : RORB_OK;
}

static VALUE native_cdr_encode(VALUE self, VALUE little, VALUE writer) {
  struct cdr_ctx c;
  rorb_bytes bytes;
  rorb_exception exc;
  rorb_status st;
  VALUE r;
  (void)self;
  c.proc = writer;
  c.error = Qnil;
  memset(&bytes, 0, sizeof bytes);
  memset(&exc, 0, sizeof exc);
  st = rorb_cdr_encode(cdr_write, &c, RTEST(little), &bytes, &exc);
  if (!NIL_P(c.error)) {
    rorb_exception_clear(&exc);
    rb_exc_raise(c.error);
  }
  if (st != RORB_OK) raise_status(st, &exc);
  r = rb_str_new((const char*)bytes.data, (long)bytes.len);
  rorb_bytes_free(bytes);
  return r;
}

/* Native.cdr_decode(bytes, little_endian, reader): reader.call(in) on a
   stream over bytes (rorb_in_from_bytes); returns what it returns. */
static VALUE native_cdr_decode(VALUE self, VALUE bytes, VALUE little, VALUE reader) {
  rorb_in* in = NULL;
  VALUE i, r, error = Qnil;
  (void)self;
  StringValue(bytes);
  CHECK(rorb_in_from_bytes((const uint8_t*)RSTRING_PTR(bytes), (size_t)RSTRING_LEN(bytes),
                           RTEST(little), NULL, &in, EXC));
  i = wrap_borrowed(cIn, in);
  r = call_protected(reader, 1, &i, &error);
  invalidate(i);
  rorb_in_release(in);
  RB_GC_GUARD(bytes);
  if (r == Qundef) rb_exc_raise(error);
  return r;
}

/* --- TypeCodes ------------------------------------------------------------------- */

static VALUE tc_simple(VALUE klass, VALUE kind) {
  rorb_typecode* tc = NULL;
  (void)klass;
  CHECK(rorb_tc_simple(NUM2UINT(kind), &tc, EXC));
  return W_TC(tc);
}
static VALUE tc_string(VALUE klass, VALUE wide, VALUE bound) {
  rorb_typecode* tc = NULL;
  (void)klass;
  CHECK(rorb_tc_string(RTEST(wide), NUM2UINT(bound), &tc, EXC));
  return W_TC(tc);
}
static VALUE tc_fixed(VALUE klass, VALUE digits, VALUE scale) {
  rorb_typecode* tc = NULL;
  (void)klass;
  CHECK(rorb_tc_fixed((uint16_t)NUM2UINT(digits), (int16_t)NUM2INT(scale), &tc, EXC));
  return W_TC(tc);
}
static VALUE tc_sequence(VALUE klass, VALUE content, VALUE bound) {
  rorb_typecode* tc = NULL;
  (void)klass;
  CHECK(rorb_tc_sequence(TC(content), NUM2UINT(bound), &tc, EXC));
  return W_TC(tc);
}
static VALUE tc_array(VALUE klass, VALUE content, VALUE length) {
  rorb_typecode* tc = NULL;
  (void)klass;
  CHECK(rorb_tc_array(TC(content), NUM2UINT(length), &tc, EXC));
  return W_TC(tc);
}
static VALUE tc_alias(VALUE klass, VALUE kind, VALUE id, VALUE name, VALUE content) {
  rorb_typecode* tc = NULL;
  (void)klass;
  id = utf8_arg(id);
  name = utf8_arg(name);
  CHECK(rorb_tc_alias(NUM2UINT(kind), RSTRING_PTR(id), (size_t)RSTRING_LEN(id),
                      RSTRING_PTR(name), (size_t)RSTRING_LEN(name), TC(content), &tc, EXC));
  return W_TC(tc);
}
static VALUE tc_interface(VALUE klass, VALUE kind, VALUE id, VALUE name) {
  rorb_typecode* tc = NULL;
  (void)klass;
  id = utf8_arg(id);
  name = utf8_arg(name);
  CHECK(rorb_tc_interface(NUM2UINT(kind), RSTRING_PTR(id), (size_t)RSTRING_LEN(id),
                          RSTRING_PTR(name), (size_t)RSTRING_LEN(name), &tc, EXC));
  return W_TC(tc);
}
static VALUE tc_recursive(VALUE klass, VALUE id) {
  rorb_typecode* tc = NULL;
  (void)klass;
  id = utf8_arg(id);
  CHECK(rorb_tc_recursive(RSTRING_PTR(id), (size_t)RSTRING_LEN(id), &tc, EXC));
  return W_TC(tc);
}

/* Names (Strings) and TypeCodes as C arrays; the Strings stay in keep. */
static void member_arrays(VALUE names, VALUE types, size_t n, const char** cn, size_t* cl,
                          const rorb_typecode** ct, VALUE keep) {
  size_t i;
  for (i = 0; i < n; ++i) {
    VALUE s = utf8_arg(rb_ary_entry(names, (long)i));
    rb_ary_push(keep, s);
    cn[i] = RSTRING_PTR(s);
    cl[i] = (size_t)RSTRING_LEN(s);
    if (ct) ct[i] = TC(rb_ary_entry(types, (long)i));
  }
}

static VALUE tc_struct(VALUE klass, VALUE kind, VALUE id, VALUE name, VALUE names, VALUE types) {
  rorb_typecode* tc = NULL;
  size_t n;
  const char** cn;
  size_t* cl;
  const rorb_typecode** ct;
  VALUE keep = rb_ary_new();
  (void)klass;
  Check_Type(names, T_ARRAY);
  Check_Type(types, T_ARRAY);
  n = (size_t)RARRAY_LEN(names);
  cn = ALLOCA_N(const char*, n + 1);
  cl = ALLOCA_N(size_t, n + 1);
  ct = ALLOCA_N(const rorb_typecode*, n + 1);
  member_arrays(names, types, n, cn, cl, ct, keep);
  id = utf8_arg(id);
  name = utf8_arg(name);
  CHECK(rorb_tc_struct(NUM2UINT(kind), RSTRING_PTR(id), (size_t)RSTRING_LEN(id),
                       RSTRING_PTR(name), (size_t)RSTRING_LEN(name), n, cn, cl, ct, &tc, EXC));
  RB_GC_GUARD(keep);
  return W_TC(tc);
}

/* TC.value(kind, id, name, modifier, base, names, types, visibilities):
   a valuetype (TK_VALUE) or eventtype (TK_EVENT); base nil for none. */
static VALUE tc_value(int argc, VALUE* argv, VALUE klass) {
  VALUE kind, id, name, modifier, base, names, types, vis;
  rorb_typecode* tc = NULL;
  size_t n, i;
  const char** cn;
  size_t* cl;
  const rorb_typecode** ct;
  int16_t* cv;
  VALUE keep = rb_ary_new();
  (void)klass;
  rb_scan_args(argc, argv, "8", &kind, &id, &name, &modifier, &base, &names, &types, &vis);
  Check_Type(names, T_ARRAY);
  Check_Type(types, T_ARRAY);
  Check_Type(vis, T_ARRAY);
  n = (size_t)RARRAY_LEN(names);
  if ((size_t)RARRAY_LEN(types) != n || (size_t)RARRAY_LEN(vis) != n)
    rb_raise(rb_eArgError, "names, types and visibilities differ in length");
  cn = ALLOCA_N(const char*, n + 1);
  cl = ALLOCA_N(size_t, n + 1);
  ct = ALLOCA_N(const rorb_typecode*, n + 1);
  cv = ALLOCA_N(int16_t, n + 1);
  member_arrays(names, types, n, cn, cl, ct, keep);
  for (i = 0; i < n; i++) cv[i] = (int16_t)NUM2INT(rb_ary_entry(vis, (long)i));
  id = utf8_arg(id);
  name = utf8_arg(name);
  CHECK(rorb_tc_value(NUM2UINT(kind), RSTRING_PTR(id), (size_t)RSTRING_LEN(id), RSTRING_PTR(name),
                      (size_t)RSTRING_LEN(name), (int16_t)NUM2INT(modifier),
                      NIL_P(base) ? NULL : TC(base), n, cn, cl, ct, cv, &tc, EXC));
  RB_GC_GUARD(keep);
  return W_TC(tc);
}

static VALUE tc_union(VALUE klass, VALUE id, VALUE name, VALUE disc, VALUE labels,
                      VALUE default_index, VALUE names, VALUE types) {
  rorb_typecode* tc = NULL;
  size_t n, i;
  const char** cn;
  size_t* cl;
  const rorb_typecode** ct;
  uint64_t* lb;
  VALUE keep = rb_ary_new();
  (void)klass;
  Check_Type(names, T_ARRAY);
  Check_Type(types, T_ARRAY);
  Check_Type(labels, T_ARRAY);
  n = (size_t)RARRAY_LEN(names);
  cn = ALLOCA_N(const char*, n + 1);
  cl = ALLOCA_N(size_t, n + 1);
  ct = ALLOCA_N(const rorb_typecode*, n + 1);
  lb = ALLOCA_N(uint64_t, n + 1);
  member_arrays(names, types, n, cn, cl, ct, keep);
  for (i = 0; i < n; ++i) lb[i] = NUM2ULL(rb_ary_entry(labels, (long)i));
  id = utf8_arg(id);
  name = utf8_arg(name);
  CHECK(rorb_tc_union(RSTRING_PTR(id), (size_t)RSTRING_LEN(id), RSTRING_PTR(name),
                      (size_t)RSTRING_LEN(name), TC(disc), n, lb, NUM2INT(default_index), cn, cl,
                      ct, &tc, EXC));
  RB_GC_GUARD(keep);
  return W_TC(tc);
}

static VALUE tc_enum(VALUE klass, VALUE id, VALUE name, VALUE names) {
  rorb_typecode* tc = NULL;
  size_t n;
  const char** cn;
  size_t* cl;
  VALUE keep = rb_ary_new();
  (void)klass;
  Check_Type(names, T_ARRAY);
  n = (size_t)RARRAY_LEN(names);
  cn = ALLOCA_N(const char*, n + 1);
  cl = ALLOCA_N(size_t, n + 1);
  member_arrays(names, Qnil, n, cn, cl, NULL, keep);
  id = utf8_arg(id);
  name = utf8_arg(name);
  CHECK(rorb_tc_enum(RSTRING_PTR(id), (size_t)RSTRING_LEN(id), RSTRING_PTR(name),
                     (size_t)RSTRING_LEN(name), n, cn, cl, &tc, EXC));
  RB_GC_GUARD(keep);
  return W_TC(tc);
}

static VALUE tc_kind(VALUE self) { return UINT2NUM(rorb_tc_kind(TC(self))); }
static VALUE tc_id(VALUE self) {
  const char* s = NULL;
  size_t len = 0;
  CHECK(rorb_tc_id(TC(self), &s, &len, EXC));
  return utf8(s, len);
}
static VALUE tc_name(VALUE self) {
  const char* s = NULL;
  size_t len = 0;
  CHECK(rorb_tc_name(TC(self), &s, &len, EXC));
  return utf8(s, len);
}
static VALUE tc_member_count(VALUE self) {
  uint32_t n = 0;
  CHECK(rorb_tc_member_count(TC(self), &n, EXC));
  return UINT2NUM(n);
}
static VALUE tc_member_name(VALUE self, VALUE i) {
  const char* s = NULL;
  size_t len = 0;
  CHECK(rorb_tc_member_name(TC(self), NUM2UINT(i), &s, &len, EXC));
  return utf8(s, len);
}
static VALUE tc_member_type(VALUE self, VALUE i) {
  rorb_typecode* tc = NULL;
  CHECK(rorb_tc_member_type(TC(self), NUM2UINT(i), &tc, EXC));
  return W_TC(tc);
}
static VALUE tc_member_label(VALUE self, VALUE i) {
  rorb_any* a = NULL;
  CHECK(rorb_tc_member_label(TC(self), NUM2UINT(i), &a, EXC));
  return W_ANY(a);
}
static VALUE tc_discriminator_type(VALUE self) {
  rorb_typecode* tc = NULL;
  CHECK(rorb_tc_discriminator_type(TC(self), &tc, EXC));
  return W_TC(tc);
}
static VALUE tc_default_index(VALUE self) {
  int32_t i = -1;
  CHECK(rorb_tc_default_index(TC(self), &i, EXC));
  return INT2NUM(i);
}
static VALUE tc_length(VALUE self) {
  uint32_t n = 0;
  CHECK(rorb_tc_length(TC(self), &n, EXC));
  return UINT2NUM(n);
}
static VALUE tc_content_type(VALUE self) {
  rorb_typecode* tc = NULL;
  CHECK(rorb_tc_content_type(TC(self), &tc, EXC));
  return W_TC(tc);
}
static VALUE tc_type_modifier(VALUE self) {
  int16_t v = 0;
  CHECK(rorb_tc_type_modifier(TC(self), &v, EXC));
  return INT2NUM(v);
}
static VALUE tc_member_visibility(VALUE self, VALUE index) {
  int16_t v = 0;
  CHECK(rorb_tc_member_visibility(TC(self), NUM2UINT(index), &v, EXC));
  return INT2NUM(v);
}
static VALUE tc_concrete_base_type(VALUE self) {
  rorb_typecode* tc = NULL;
  CHECK(rorb_tc_concrete_base_type(TC(self), &tc, EXC));
  return W_TC(tc);
}
static VALUE tc_fixed_digits(VALUE self) {
  uint16_t d = 0;
  CHECK(rorb_tc_fixed_digits(TC(self), &d, EXC));
  return UINT2NUM(d);
}
static VALUE tc_fixed_scale(VALUE self) {
  int16_t s = 0;
  CHECK(rorb_tc_fixed_scale(TC(self), &s, EXC));
  return INT2NUM(s);
}
static VALUE tc_equal(VALUE self, VALUE other) {
  return rorb_tc_equal(TC(self), TC(other)) ? Qtrue : Qfalse;
}
static VALUE tc_equivalent(VALUE self, VALUE other) {
  return rorb_tc_equivalent(TC(self), TC(other)) ? Qtrue : Qfalse;
}
static VALUE tc_unaliased(VALUE self) { return W_TC(rorb_tc_unaliased(TC(self))); }
static VALUE tc_compact(VALUE self) { return W_TC(rorb_tc_compact(TC(self))); }

/* --- anys -------------------------------------------------------------------------- */

struct any_ctx {
  VALUE proc;
  VALUE error;
  rorb_out* out;
  rorb_in* in;
};

static void* cb_any_write(void* p) {
  struct any_ctx* c = p;
  VALUE out = wrap_borrowed(cOut, c->out);
  VALUE r = call_protected(c->proc, 1, &out, &c->error);
  invalidate(out);
  return r == Qundef ? (void*)1 : NULL;
}

static void* cb_any_read(void* p) {
  struct any_ctx* c = p;
  VALUE in = wrap_borrowed(cIn, c->in);
  VALUE r = call_protected(c->proc, 1, &in, &c->error);
  invalidate(in);
  return r == Qundef ? (void*)1 : NULL;
}

static rorb_status any_write(void* ctx, rorb_out* out) {
  struct any_ctx* c = ctx;
  c->out = out;
  return with_gvl(cb_any_write, c) ? RORB_CALLBACK_FAILED : RORB_OK;
}

static rorb_status any_read(void* ctx, rorb_in* in) {
  struct any_ctx* c = ctx;
  c->in = in;
  return with_gvl(cb_any_read, c) ? RORB_CALLBACK_FAILED : RORB_OK;
}

static VALUE any_new(VALUE klass) {
  (void)klass;
  return W_ANY(rorb_any_new());
}

/* Any.insert(tc, writer): an any of type tc with what writer.call(out)
   writes. */
static VALUE any_insert(VALUE klass, VALUE tc, VALUE writer) {
  struct any_ctx c;
  rorb_any* a = NULL;
  rorb_exception exc;
  rorb_status st;
  (void)klass;
  memset(&c, 0, sizeof c);
  memset(&exc, 0, sizeof exc);
  c.proc = writer;
  c.error = Qnil;
  st = rorb_any_insert(TC(tc), any_write, &c, &a, &exc);
  RB_GC_GUARD(writer);
  if (!NIL_P(c.error)) {
    rorb_exception_clear(&exc);
    rb_exc_raise(c.error);
  }
  if (st != RORB_OK) raise_status(st, &exc);
  return W_ANY(a);
}

static VALUE any_typed(VALUE klass, VALUE tc) {
  (void)klass;
  return W_ANY(rorb_any_typed(TC(tc)));
}

/* any.extract(reader): reader.call(in) reads the value. */
static VALUE any_extract(VALUE self, VALUE reader) {
  struct any_ctx c;
  rorb_exception exc;
  rorb_status st;
  memset(&c, 0, sizeof c);
  memset(&exc, 0, sizeof exc);
  c.proc = reader;
  c.error = Qnil;
  st = rorb_any_extract(ANY(self), any_read, &c, &exc);
  RB_GC_GUARD(reader);
  if (!NIL_P(c.error)) {
    rorb_exception_clear(&exc);
    rb_exc_raise(c.error);
  }
  if (st != RORB_OK) raise_status(st, &exc);
  return Qnil;
}

static VALUE any_type(VALUE self) { return W_TC(rorb_any_type(ANY(self))); }
static VALUE any_set_type(VALUE self, VALUE tc) {
  CHECK(rorb_any_set_type(ANY(self), TC(tc), EXC));
  return self;
}
static VALUE any_equal(VALUE self, VALUE other) {
  return rorb_any_equal(ANY(self), ANY(other)) ? Qtrue : Qfalse;
}
static VALUE any_label(VALUE self) {
  uint64_t l = 0;
  return rorb_any_label(ANY(self), &l) ? ULL2NUM(l) : Qnil;
}
static VALUE any_dup(VALUE self) { return W_ANY(rorb_any_clone(ANY(self))); }

/* --- servants ------------------------------------------------------------------------ */

/* The C side of a Ruby servant: what rustorb holds as self. The Ruby object
   is kept alive while rustorb holds a reference (refs > 0): the registry's
   mark function marks it. The peer is freed with the Ruby object (the
   Rustorb::Native::Peer in its ivar), which then is not referenced from
   rustorb any more, except at exit, when Ruby frees every object: then the
   peer is left as an orphan (PEER_ORPHAN added to refs) and freed by the
   last servant_remove_ref. */
typedef struct peer {
  VALUE obj;
  int refs; /* atomic */
  char** ids;
  size_t* id_lens;
  size_t id_count;
  struct peer *prev, *next;
} peer;

static peer* peers;

static void registry_mark(void* unused) {
  peer* p;
  (void)unused;
  for (p = peers; p; p = p->next)
    if (__atomic_load_n(&p->refs, __ATOMIC_SEQ_CST) > 0) rb_gc_mark(p->obj);
}

#define PEER_ORPHAN (1 << 30)

static void peer_delete(peer* p) {
  size_t i;
  for (i = 0; i < p->id_count; ++i) free(p->ids[i]);
  free(p->ids);
  free(p->id_lens);
  free(p);
}

static void peer_free(void* ptr) {
  peer* p = ptr;
  if (p->prev)
    p->prev->next = p->next;
  else
    peers = p->next;
  if (p->next) p->next->prev = p->prev;
  if (__atomic_add_fetch(&p->refs, PEER_ORPHAN, __ATOMIC_SEQ_CST) == PEER_ORPHAN) peer_delete(p);
}

static size_t peer_size(const void* ptr) {
  (void)ptr;
  return sizeof(peer);
}

static const rb_data_type_t peer_type = {
    "rustorb_peer", {NULL, peer_free, peer_size, NULL, {0}}, NULL, NULL, 0};

static void servant_add_ref(void* self) {
  __atomic_add_fetch(&((peer*)self)->refs, 1, __ATOMIC_SEQ_CST);
}

static void servant_remove_ref(void* self) {
  if (__atomic_sub_fetch(&((peer*)self)->refs, 1, __ATOMIC_SEQ_CST) == PEER_ORPHAN)
    peer_delete(self);
}

static int32_t servant_is_a(void* self, const char* id, size_t len) {
  peer* p = self;
  size_t i;
  for (i = 0; i < p->id_count; ++i)
    if (p->id_lens[i] == len && memcmp(p->ids[i], id, len) == 0) return 1;
  return len == 28 && memcmp(id, "IDL:omg.org/CORBA/Object:1.0", 28) == 0;
}

struct dispatch_ctx {
  peer* p;
  const char* op;
  size_t op_len;
  rorb_in* in;
  rorb_server_reply* reply;
  rorb_status st;
};

static VALUE dispatch_body(VALUE arg) {
  struct dispatch_ctx* c = (struct dispatch_ctx*)arg;
  VALUE in = wrap_borrowed(cIn, c->in);
  VALUE reply = wrap_borrowed(cReply, c->reply);
  VALUE r;
  r = rb_funcall(c->p->obj, id_dispatch, 3, utf8(c->op, c->op_len), in, reply);
  invalidate(in);
  invalidate(reply);
  return r;
}

static void* do_dispatch(void* arg) {
  struct dispatch_ctx* c = arg;
  int state = 0;
  VALUE r = rb_protect(dispatch_body, (VALUE)c, &state);
  if (state) {
    /* The Ruby side answers every exception of the servant; this is one of
       its own. */
    VALUE e = rb_errinfo();
    rb_set_errinfo(Qnil);
    rb_funcall(rb_stderr, rb_intern("puts"), 1,
               rb_str_concat(rb_str_new_cstr("rustorb: upcall failed: "), rb_inspect(e)));
    rorb_server_reply_system_exception(c->reply, "IDL:omg.org/CORBA/UNKNOWN:1.0", 29, 0,
                                       RORB_COMPLETED_MAYBE);
    c->st = RORB_SYSTEM_EXCEPTION;
  } else {
    c->st = RTEST(r) ? RORB_OK : RORB_SYSTEM_EXCEPTION;
  }
  return c;
}

static rorb_status servant_dispatch(void* self, const char* op, size_t op_len, rorb_in* in,
                                    rorb_server_reply* reply) {
  struct dispatch_ctx c;
  c.p = self;
  c.op = op;
  c.op_len = op_len;
  c.in = in;
  c.reply = reply;
  c.st = RORB_OK;
  if (!with_gvl(do_dispatch, &c)) {
    rorb_server_reply_system_exception(reply, "IDL:omg.org/CORBA/TRANSIENT:1.0", 31, 0,
                                       RORB_COMPLETED_NO);
    return RORB_SYSTEM_EXCEPTION;
  }
  return c.st;
}

static const rorb_servant_vtable servant_vtable = {servant_dispatch, servant_is_a,
                                                   servant_add_ref, servant_remove_ref};

static peer* peer_of(VALUE servant, VALUE ids) {
  VALUE holder = rb_ivar_get(servant, id_peer_ivar);
  peer* p;
  long i;
  if (!NIL_P(holder)) {
    TypedData_Get_Struct(holder, peer, &peer_type, p);
    return p;
  }
  Check_Type(ids, T_ARRAY);
  p = calloc(1, sizeof *p);
  if (!p) rb_memerror();
  p->obj = servant;
  p->id_count = (size_t)RARRAY_LEN(ids);
  p->ids = calloc(p->id_count + 1, sizeof(char*));
  p->id_lens = calloc(p->id_count + 1, sizeof(size_t));
  for (i = 0; i < RARRAY_LEN(ids); ++i) {
    VALUE s = utf8_arg(rb_ary_entry(ids, i));
    p->id_lens[i] = (size_t)RSTRING_LEN(s);
    p->ids[i] = malloc(p->id_lens[i] + 1);
    memcpy(p->ids[i], RSTRING_PTR(s), p->id_lens[i]);
    p->ids[i][p->id_lens[i]] = 0;
  }
  p->next = peers;
  if (peers) peers->prev = p;
  peers = p;
  holder = TypedData_Wrap_Struct(cPeer, &peer_type, p);
  rb_ivar_set(servant, id_peer_ivar, holder);
  return p;
}

/* Rustorb::Native.servant_ref(servant, ids): the rustorb servant of a Ruby
   servant; ids are its repository ids, its own first. */
static VALUE native_servant_ref(VALUE self, VALUE servant, VALUE ids) {
  peer* p = peer_of(servant, ids);
  (void)self;
  return wrap(cServantRef,
              rorb_servant_get(p, p->ids[0] ? p->ids[0] : "", p->id_lens[0], &servant_vtable),
              rel_servant);
}

#define SERVANT(v) ((rorb_servant*)unwrap((v), cServantRef))

/* The PortableServer::Current queries, made by at_home. */
struct current_call {
  int what;
  rorb_servant* servant;
  rorb_poa* poa;
  rorb_bytes id;
  rorb_object* obj;
  void* self;
  int32_t found;
  rorb_status st;
  rorb_exception exc;
};

static void* current_nogvl(void* p) {
  struct current_call* c = p;
  switch (c->what) {
    case 0: c->found = rorb_servant_current_reference(c->servant, &c->obj); break;
    case 1: c->st = rorb_current_get_poa(&c->poa, &c->exc); break;
    case 2: c->st = rorb_current_get_object_id(&c->id, &c->exc); break;
    case 3: c->st = rorb_current_get_reference(&c->obj, &c->exc); break;
    default: c->st = rorb_current_get_servant(&c->self, &c->exc); break;
  }
  return NULL;
}

static void current_call(struct current_call* c, int what) {
  memset(c, 0, sizeof *c);
  c->what = what;
}

static VALUE servant_ref_current_reference(VALUE self) {
  struct current_call c;
  current_call(&c, 0);
  c.servant = SERVANT(self);
  at_home(current_nogvl, &c);
  if (!c.found) return Qfalse;
  return W_OBJ(c.obj);
}

/* The Ruby servant of a self rustorb gave with a reference added. */
static VALUE servant_of(void* self) {
  peer* p = self;
  VALUE obj;
  if (!p) return Qnil;
  obj = p->obj;
  servant_remove_ref(p);
  return obj;
}

static VALUE reply_output(VALUE self) {
  return wrap_borrowed(cOut, rorb_server_reply_output(unwrap(self, cReply)));
}
static VALUE reply_user_exception(VALUE self, VALUE id) {
  id = utf8_arg(id);
  return wrap_borrowed(cOut, rorb_server_reply_user_exception(unwrap(self, cReply), RSTRING_PTR(id),
                                                              (size_t)RSTRING_LEN(id)));
}
static VALUE reply_system_exception(VALUE self, VALUE id, VALUE minor, VALUE completed) {
  id = utf8_arg(id);
  rorb_server_reply_system_exception(unwrap(self, cReply), RSTRING_PTR(id),
                                     (size_t)RSTRING_LEN(id), NUM2UINT(minor), NUM2UINT(completed));
  return Qnil;
}

/* --- POA ------------------------------------------------------------------------------ */

static VALUE poa_name(VALUE self) {
  size_t len = 0;
  const char* s = rorb_poa_name(POA(self), &len);
  return utf8(s, len);
}
static VALUE poa_parent(VALUE self) { return W_POA(rorb_poa_parent(POA(self))); }
static void each_child(void* ctx, rorb_poa* child) {
  rb_ary_push((VALUE)ctx, W_POA(rorb_poa_clone(child)));
}
static VALUE poa_children(VALUE self) {
  VALUE a = rb_ary_new();
  rorb_poa_children(POA(self), (void*)a, each_child);
  return a;
}
static VALUE poa_manager(VALUE self) { return W_MGR(rorb_poa_the_manager(POA(self))); }
static VALUE poa_policies(VALUE self) {
  rorb_policy ps[7];
  VALUE a = rb_ary_new();
  int i;
  rorb_poa_policies(POA(self), ps);
  for (i = 0; i < 7; ++i) rb_ary_push(a, UINT2NUM(ps[i].value));
  return a;
}
/* policies: [[type, value], ...] */
static VALUE poa_create_poa(VALUE self, VALUE name, VALUE mgr, VALUE policies) {
  rorb_poa* child = NULL;
  rorb_policy* ps;
  long i, n;
  Check_Type(policies, T_ARRAY);
  n = RARRAY_LEN(policies);
  ps = ALLOCA_N(rorb_policy, n + 1);
  for (i = 0; i < n; ++i) {
    VALUE pv = rb_ary_entry(policies, i);
    ps[i].type = NUM2UINT(rb_ary_entry(pv, 0));
    ps[i].value = NUM2UINT(rb_ary_entry(pv, 1));
  }
  name = utf8_arg(name);
  CHECK(rorb_poa_create_poa(POA(self), RSTRING_PTR(name), (size_t)RSTRING_LEN(name),
                            unwrap_opt(mgr, cPOAManager), ps, (size_t)n, &child, EXC));
  return W_POA(child);
}
static VALUE poa_find_poa(VALUE self, VALUE name, VALUE activate) {
  rorb_poa* child = NULL;
  name = utf8_arg(name);
  CHECK(rorb_poa_find_poa(POA(self), RSTRING_PTR(name), (size_t)RSTRING_LEN(name),
                          RTEST(activate), &child, EXC));
  return W_POA(child);
}

struct poa_op {
  rorb_poa* poa;
  int32_t a, b;
  rorb_status st;
  rorb_exception exc;
};
static void* poa_destroy_nogvl(void* p) {
  struct poa_op* o = p;
  o->st = rorb_poa_destroy(o->poa, o->a, o->b, &o->exc);
  return NULL;
}
static VALUE poa_destroy(VALUE self, VALUE etherealize, VALUE wait) {
  struct poa_op o;
  memset(&o, 0, sizeof o);
  o.poa = POA(self);
  o.a = RTEST(etherealize);
  o.b = RTEST(wait);
  blocking(poa_destroy_nogvl, &o);
  if (o.st != RORB_OK) raise_status(o.st, &o.exc);
  return Qnil;
}
static VALUE poa_activate_object(VALUE self, VALUE servant) {
  rorb_bytes id;
  CHECK(rorb_poa_activate_object(POA(self), SERVANT(servant), &id, EXC));
  return bytes_str(id);
}
static VALUE poa_activate_object_with_id(VALUE self, VALUE id, VALUE servant) {
  StringValue(id);
  CHECK(rorb_poa_activate_object_with_id(POA(self), (const uint8_t*)RSTRING_PTR(id),
                                         (size_t)RSTRING_LEN(id), SERVANT(servant), EXC));
  return Qnil;
}
static VALUE poa_deactivate_object(VALUE self, VALUE id) {
  StringValue(id);
  CHECK(rorb_poa_deactivate_object(POA(self), (const uint8_t*)RSTRING_PTR(id),
                                   (size_t)RSTRING_LEN(id), EXC));
  return Qnil;
}
static VALUE poa_servant_to_id(VALUE self, VALUE servant) {
  rorb_bytes id;
  CHECK(rorb_poa_servant_to_id(POA(self), SERVANT(servant), &id, EXC));
  return bytes_str(id);
}
static VALUE poa_servant_to_reference(VALUE self, VALUE servant) {
  rorb_object* obj = NULL;
  CHECK(rorb_poa_servant_to_reference(POA(self), SERVANT(servant), &obj, EXC));
  return W_OBJ(obj);
}
static VALUE poa_id_to_servant(VALUE self, VALUE id) {
  void* s = NULL;
  StringValue(id);
  CHECK(rorb_poa_id_to_servant(POA(self), (const uint8_t*)RSTRING_PTR(id), (size_t)RSTRING_LEN(id),
                               &s, EXC));
  return servant_of(s);
}
static VALUE poa_id_to_reference(VALUE self, VALUE id) {
  rorb_object* obj = NULL;
  StringValue(id);
  CHECK(rorb_poa_id_to_reference(POA(self), (const uint8_t*)RSTRING_PTR(id),
                                 (size_t)RSTRING_LEN(id), &obj, EXC));
  return W_OBJ(obj);
}
static VALUE poa_reference_to_id(VALUE self, VALUE ref) {
  rorb_bytes id;
  CHECK(rorb_poa_reference_to_id(POA(self), OBJ(ref), &id, EXC));
  return bytes_str(id);
}
static VALUE poa_reference_to_servant(VALUE self, VALUE ref) {
  void* s = NULL;
  CHECK(rorb_poa_reference_to_servant(POA(self), OBJ(ref), &s, EXC));
  return servant_of(s);
}
static VALUE poa_create_reference(VALUE self, VALUE intf) {
  rorb_object* obj = NULL;
  intf = utf8_arg(intf);
  CHECK(rorb_poa_create_reference(POA(self), RSTRING_PTR(intf), (size_t)RSTRING_LEN(intf), &obj,
                                  EXC));
  return W_OBJ(obj);
}
static VALUE poa_create_reference_with_id(VALUE self, VALUE id, VALUE intf) {
  rorb_object* obj = NULL;
  StringValue(id);
  intf = utf8_arg(intf);
  CHECK(rorb_poa_create_reference_with_id(POA(self), (const uint8_t*)RSTRING_PTR(id),
                                          (size_t)RSTRING_LEN(id), RSTRING_PTR(intf),
                                          (size_t)RSTRING_LEN(intf), &obj, EXC));
  return W_OBJ(obj);
}
static VALUE poa_get_servant(VALUE self) {
  void* s = NULL;
  CHECK(rorb_poa_get_servant(POA(self), &s, EXC));
  return servant_of(s);
}
static VALUE poa_set_servant(VALUE self, VALUE servant) {
  CHECK(rorb_poa_set_servant(POA(self), SERVANT(servant), EXC));
  return Qnil;
}

/* --- POAManager ------------------------------------------------------------------------ */

struct mgr_op {
  rorb_poa_manager* m;
  int what;
  int32_t a, b;
  rorb_status st;
  rorb_exception exc;
};
static void* mgr_nogvl(void* p) {
  struct mgr_op* o = p;
  switch (o->what) {
    case 0: o->st = rorb_poa_manager_activate(o->m, &o->exc); break;
    case 1: o->st = rorb_poa_manager_hold_requests(o->m, o->a, &o->exc); break;
    case 2: o->st = rorb_poa_manager_discard_requests(o->m, o->a, &o->exc); break;
    default: o->st = rorb_poa_manager_deactivate(o->m, o->a, o->b, &o->exc); break;
  }
  return NULL;
}
static VALUE mgr_call(VALUE self, int what, VALUE a, VALUE b) {
  struct mgr_op o;
  memset(&o, 0, sizeof o);
  o.m = MGR(self);
  o.what = what;
  o.a = RTEST(a);
  o.b = RTEST(b);
  blocking(mgr_nogvl, &o);
  if (o.st != RORB_OK) raise_status(o.st, &o.exc);
  return Qnil;
}
static VALUE mgr_activate(VALUE self) { return mgr_call(self, 0, Qfalse, Qfalse); }
static VALUE mgr_hold(VALUE self, VALUE w) { return mgr_call(self, 1, w, Qfalse); }
static VALUE mgr_discard(VALUE self, VALUE w) { return mgr_call(self, 2, w, Qfalse); }
static VALUE mgr_deactivate(VALUE self, VALUE e, VALUE w) { return mgr_call(self, 3, e, w); }
static VALUE mgr_state(VALUE self) { return UINT2NUM(rorb_poa_manager_get_state(MGR(self))); }
static VALUE mgr_id(VALUE self) {
  size_t len = 0;
  const char* s = rorb_poa_manager_get_id(MGR(self), &len);
  return utf8(s, len);
}
static VALUE mgr_equal(VALUE self, VALUE other) {
  return rorb_poa_manager_equal(MGR(self), MGR(other)) ? Qtrue : Qfalse;
}

/* --- PortableServer::Current ------------------------------------------------------------ */

static VALUE native_current_poa(VALUE self) {
  struct current_call c;
  (void)self;
  current_call(&c, 1);
  at_home(current_nogvl, &c);
  if (c.st != RORB_OK) raise_status(c.st, &c.exc);
  return W_POA(c.poa);
}
static VALUE native_current_object_id(VALUE self) {
  struct current_call c;
  (void)self;
  current_call(&c, 2);
  at_home(current_nogvl, &c);
  if (c.st != RORB_OK) raise_status(c.st, &c.exc);
  return bytes_str(c.id);
}
static VALUE native_current_reference(VALUE self) {
  struct current_call c;
  (void)self;
  current_call(&c, 3);
  at_home(current_nogvl, &c);
  if (c.st != RORB_OK) raise_status(c.st, &c.exc);
  return W_OBJ(c.obj);
}
static VALUE native_current_servant(VALUE self) {
  struct current_call c;
  (void)self;
  current_call(&c, 4);
  at_home(current_nogvl, &c);
  if (c.st != RORB_OK) raise_status(c.st, &c.exc);
  return servant_of(c.self);
}

static VALUE native_longdouble_from_double(VALUE self, VALUE v) {
  uint64_t hi, lo;
  (void)self;
  rorb_longdouble_from_double(NUM2DBL(v), &hi, &lo);
  return rb_assoc_new(ULL2NUM(hi), ULL2NUM(lo));
}
static VALUE native_longdouble_to_double(VALUE self, VALUE hi, VALUE lo) {
  (void)self;
  return DBL2NUM(rorb_longdouble_to_double(NUM2ULL(hi), NUM2ULL(lo)));
}
static VALUE native_dispatchers(VALUE self) {
  int n;
  (void)self;
  pthread_mutex_lock(&queue_lock);
  n = dispatchers;
  pthread_mutex_unlock(&queue_lock);
  return INT2FIX(n);
}

/* --- Init ----------------------------------------------------------------------------------- */

static const rb_data_type_t registry_type = {
    "rustorb_registry", {registry_mark, NULL, NULL, NULL, {0}}, NULL, NULL, 0};

/* --- DynamicAny ------------------------------------------------------------------------- */

/* Rustorb::Native::DynAny: one rorb_dynany handle. Values go in and out as
   Rustorb::Native::Any; DynamicAny.rb makes the IDL interfaces. */
static VALUE cDynAny;
static void rel_dyn(void* p) { rorb_dynany_release(p); }
#define W_DYN(p) wrap(cDynAny, (p), rel_dyn)
#define DYN(v) ((rorb_dynany*)unwrap((v), cDynAny))

static VALUE dyn_create(VALUE klass, VALUE any, VALUE truncate) {
  rorb_dynany* d = NULL;
  (void)klass;
  CHECK(rorb_dynany_create(ANY(any), RTEST(truncate) ? 1 : 0, &d, EXC));
  return W_DYN(d);
}
static VALUE dyn_from_typecode(VALUE klass, VALUE tc) {
  rorb_dynany* d = NULL;
  (void)klass;
  CHECK(rorb_dynany_from_typecode(TC(tc), &d, EXC));
  return W_DYN(d);
}
static VALUE dyn_kind(VALUE self) { return UINT2NUM(rorb_dynany_kind(DYN(self))); }
static VALUE dyn_same(VALUE self, VALUE other) {
  return rorb_dynany_same(DYN(self), DYN(other)) ? Qtrue : Qfalse;
}

#define DYN_TC(name, fn)                       \
  static VALUE name(VALUE self) {              \
    rorb_typecode* t = NULL;                   \
    CHECK(fn(DYN(self), &t, EXC));             \
    return W_TC(t);                            \
  }
#define DYN_ANY(name, fn)                      \
  static VALUE name(VALUE self) {              \
    rorb_any* a = NULL;                        \
    CHECK(fn(DYN(self), &a, EXC));             \
    return W_ANY(a);                           \
  }
#define DYN_DYN(name, fn)                      \
  static VALUE name(VALUE self) {              \
    rorb_dynany* d = NULL;                     \
    CHECK(fn(DYN(self), &d, EXC));             \
    return W_DYN(d);                           \
  }
#define DYN_VOID(name, fn)                     \
  static VALUE name(VALUE self) {              \
    CHECK(fn(DYN(self), EXC));                 \
    return Qnil;                               \
  }
#define DYN_BOOL(name, fn)                     \
  static VALUE name(VALUE self) {              \
    int32_t b = 0;                             \
    CHECK(fn(DYN(self), &b, EXC));             \
    return b ? Qtrue : Qfalse;                 \
  }
#define DYN_U32(name, fn)                      \
  static VALUE name(VALUE self) {              \
    uint32_t u = 0;                            \
    CHECK(fn(DYN(self), &u, EXC));             \
    return UINT2NUM(u);                        \
  }
#define DYN_STR(name, fn)                      \
  static VALUE name(VALUE self) {              \
    rorb_bytes b;                              \
    VALUE s;                                   \
    memset(&b, 0, sizeof b);                   \
    CHECK(fn(DYN(self), &b, EXC));             \
    s = utf8((const char*)b.data, b.len);      \
    rorb_bytes_free(b);                        \
    return s;                                  \
  }
#define DYN_SET_ANY(name, fn)                  \
  static VALUE name(VALUE self, VALUE a) {     \
    CHECK(fn(DYN(self), ANY(a), EXC));         \
    return Qnil;                               \
  }
#define DYN_SET_DYN(name, fn)                  \
  static VALUE name(VALUE self, VALUE d) {     \
    CHECK(fn(DYN(self), DYN(d), EXC));         \
    return Qnil;                               \
  }

DYN_TC(dyn_type, rorb_dynany_type)
DYN_TC(dyn_target_type, rorb_dynany_target_type)
DYN_SET_DYN(dyn_assign, rorb_dynany_assign)
DYN_SET_ANY(dyn_from_any, rorb_dynany_from_any)
DYN_ANY(dyn_to_any, rorb_dynany_to_any)
DYN_VOID(dyn_destroy, rorb_dynany_destroy)
DYN_DYN(dyn_copy, rorb_dynany_copy)
DYN_SET_DYN(dyn_insert_dyn_any, rorb_dynany_insert_dyn_any)
DYN_DYN(dyn_get_dyn_any, rorb_dynany_get_dyn_any)
DYN_VOID(dyn_no_implement, rorb_dynany_no_implement)
DYN_VOID(dyn_rewind, rorb_dynany_rewind)
DYN_BOOL(dyn_next, rorb_dynany_next)
DYN_U32(dyn_component_count, rorb_dynany_component_count)
DYN_DYN(dyn_current_component, rorb_dynany_current_component)
DYN_STR(dyn_get_fixed_value, rorb_dynany_get_fixed_value)
DYN_STR(dyn_get_as_string, rorb_dynany_get_as_string)
DYN_U32(dyn_get_as_ulong, rorb_dynany_get_as_ulong)
DYN_STR(dyn_current_member_name, rorb_dynany_current_member_name)
DYN_U32(dyn_current_member_kind, rorb_dynany_current_member_kind)
DYN_U32(dyn_member_count, rorb_dynany_member_count)
DYN_DYN(dyn_get_discriminator, rorb_dynany_get_discriminator)
DYN_SET_DYN(dyn_set_discriminator, rorb_dynany_set_discriminator)
DYN_VOID(dyn_set_to_default_member, rorb_dynany_set_to_default_member)
DYN_VOID(dyn_set_to_no_active_member, rorb_dynany_set_to_no_active_member)
DYN_BOOL(dyn_has_no_active_member, rorb_dynany_has_no_active_member)
DYN_U32(dyn_discriminator_kind, rorb_dynany_discriminator_kind)
DYN_DYN(dyn_member, rorb_dynany_member)
DYN_STR(dyn_member_name, rorb_dynany_member_name)
DYN_U32(dyn_member_kind, rorb_dynany_member_kind)
DYN_U32(dyn_get_length, rorb_dynany_get_length)
DYN_U32(dyn_element_count, rorb_dynany_element_count)
DYN_BOOL(dyn_is_null, rorb_dynany_is_null)
DYN_VOID(dyn_set_to_null, rorb_dynany_set_to_null)
DYN_VOID(dyn_set_to_value, rorb_dynany_set_to_value)
DYN_ANY(dyn_get_boxed_value, rorb_dynany_get_boxed_value)
DYN_SET_ANY(dyn_set_boxed_value, rorb_dynany_set_boxed_value)
DYN_DYN(dyn_get_boxed_value_as_dyn_any, rorb_dynany_get_boxed_value_as_dyn_any)
DYN_SET_DYN(dyn_set_boxed_value_as_dyn_any, rorb_dynany_set_boxed_value_as_dyn_any)

static VALUE dyn_equal(VALUE self, VALUE other) {
  int32_t b = 0;
  CHECK(rorb_dynany_equal(DYN(self), DYN(other), &b, EXC));
  return b ? Qtrue : Qfalse;
}
static VALUE dyn_insert(VALUE self, VALUE kind, VALUE a) {
  CHECK(rorb_dynany_insert(DYN(self), NUM2UINT(kind), ANY(a), EXC));
  return Qnil;
}
static VALUE dyn_get(VALUE self, VALUE kind) {
  rorb_any* a = NULL;
  CHECK(rorb_dynany_get(DYN(self), NUM2UINT(kind), &a, EXC));
  return W_ANY(a);
}
static VALUE dyn_insert_seq(VALUE self, VALUE kind, VALUE a) {
  CHECK(rorb_dynany_insert_seq(DYN(self), NUM2UINT(kind), ANY(a), EXC));
  return Qnil;
}
static VALUE dyn_get_seq(VALUE self, VALUE kind) {
  rorb_any* a = NULL;
  CHECK(rorb_dynany_get_seq(DYN(self), NUM2UINT(kind), &a, EXC));
  return W_ANY(a);
}
static VALUE dyn_seek(VALUE self, VALUE index) {
  int32_t b = 0;
  CHECK(rorb_dynany_seek(DYN(self), NUM2INT(index), &b, EXC));
  return b ? Qtrue : Qfalse;
}
static VALUE dyn_set_fixed_value(VALUE self, VALUE s) {
  int32_t b = 0;
  s = utf8_arg(s);
  CHECK(rorb_dynany_set_fixed_value(DYN(self), RSTRING_PTR(s), (size_t)RSTRING_LEN(s), &b, EXC));
  RB_GC_GUARD(s);
  return b ? Qtrue : Qfalse;
}
static VALUE dyn_set_as_string(VALUE self, VALUE s) {
  s = utf8_arg(s);
  CHECK(rorb_dynany_set_as_string(DYN(self), RSTRING_PTR(s), (size_t)RSTRING_LEN(s), EXC));
  RB_GC_GUARD(s);
  return Qnil;
}
static VALUE dyn_set_as_ulong(VALUE self, VALUE u) {
  CHECK(rorb_dynany_set_as_ulong(DYN(self), NUM2UINT(u), EXC));
  return Qnil;
}
static VALUE dyn_set_length(VALUE self, VALUE n) {
  CHECK(rorb_dynany_set_length(DYN(self), NUM2UINT(n), EXC));
  return Qnil;
}

/* [name, DynAny] of member index. */
static VALUE dyn_member_at(VALUE self, VALUE index) {
  rorb_bytes b;
  rorb_dynany* d = NULL;
  VALUE name;
  memset(&b, 0, sizeof b);
  CHECK(rorb_dynany_member_at(DYN(self), NUM2UINT(index), &b, &d, EXC));
  name = utf8((const char*)b.data, b.len);
  rorb_bytes_free(b);
  return rb_assoc_new(name, W_DYN(d));
}
static VALUE dyn_element_at(VALUE self, VALUE index) {
  rorb_dynany* d = NULL;
  CHECK(rorb_dynany_element_at(DYN(self), NUM2UINT(index), &d, EXC));
  return W_DYN(d);
}

/* set_members / set_members_as_dyn_any: names (Strings) and values
   (Native::Any, or Native::DynAny when dyn). */
static VALUE dyn_set_members_common(VALUE self, VALUE names, VALUE values, int dyn) {
  long n, i;
  const char** cn;
  size_t* cl;
  const void** cv;
  rorb_exception exc;
  rorb_status st;
  Check_Type(names, T_ARRAY);
  Check_Type(values, T_ARRAY);
  n = RARRAY_LEN(names);
  if (RARRAY_LEN(values) != n) rb_raise(rb_eArgError, "names and values differ in length");
  names = rb_ary_dup(names);
  for (i = 0; i < n; i++) rb_ary_store(names, i, utf8_arg(rb_ary_entry(names, i)));
  cn = ALLOCA_N(const char*, n ? n : 1);
  cl = ALLOCA_N(size_t, n ? n : 1);
  cv = ALLOCA_N(const void*, n ? n : 1);
  for (i = 0; i < n; i++) {
    VALUE s = rb_ary_entry(names, i);
    cn[i] = RSTRING_PTR(s);
    cl[i] = (size_t)RSTRING_LEN(s);
    cv[i] = dyn ? (const void*)DYN(rb_ary_entry(values, i)) : (const void*)ANY(rb_ary_entry(values, i));
  }
  memset(&exc, 0, sizeof exc);
  st = dyn ? rorb_dynany_set_members_as_dyn_any(DYN(self), (size_t)n, cn, cl,
                                                (const rorb_dynany* const*)cv, &exc)
           : rorb_dynany_set_members(DYN(self), (size_t)n, cn, cl, (const rorb_any* const*)cv, &exc);
  RB_GC_GUARD(names);
  RB_GC_GUARD(values);
  if (st != RORB_OK) raise_status(st, &exc);
  return Qnil;
}
static VALUE dyn_set_members(VALUE self, VALUE names, VALUE values) {
  return dyn_set_members_common(self, names, values, 0);
}
static VALUE dyn_set_members_as_dyn_any(VALUE self, VALUE names, VALUE values) {
  return dyn_set_members_common(self, names, values, 1);
}

static VALUE dyn_set_elements_common(VALUE self, VALUE values, int dyn) {
  long n, i;
  const void** cv;
  rorb_exception exc;
  rorb_status st;
  Check_Type(values, T_ARRAY);
  n = RARRAY_LEN(values);
  cv = ALLOCA_N(const void*, n ? n : 1);
  for (i = 0; i < n; i++)
    cv[i] = dyn ? (const void*)DYN(rb_ary_entry(values, i)) : (const void*)ANY(rb_ary_entry(values, i));
  memset(&exc, 0, sizeof exc);
  st = dyn ? rorb_dynany_set_elements_as_dyn_any(DYN(self), (size_t)n, (const rorb_dynany* const*)cv, &exc)
           : rorb_dynany_set_elements(DYN(self), (size_t)n, (const rorb_any* const*)cv, &exc);
  RB_GC_GUARD(values);
  if (st != RORB_OK) raise_status(st, &exc);
  return Qnil;
}
static VALUE dyn_set_elements(VALUE self, VALUE values) {
  return dyn_set_elements_common(self, values, 0);
}
static VALUE dyn_set_elements_as_dyn_any(VALUE self, VALUE values) {
  return dyn_set_elements_common(self, values, 1);
}

static void init_dynany(void) {
  cDynAny = rb_define_class_under(mNative, "DynAny", rb_cObject);
  rb_undef_alloc_func(cDynAny);
  rb_define_singleton_method(cDynAny, "create", RUBY_METHOD_FUNC(dyn_create), 2);
  rb_define_singleton_method(cDynAny, "from_type_code", RUBY_METHOD_FUNC(dyn_from_typecode), 1);
#define D(name, fn, n) rb_define_method(cDynAny, name, RUBY_METHOD_FUNC(fn), n)
  D("kind", dyn_kind, 0);
  D("same", dyn_same, 1);
  D("type", dyn_type, 0);
  D("target_type", dyn_target_type, 0);
  D("assign", dyn_assign, 1);
  D("from_any", dyn_from_any, 1);
  D("to_any", dyn_to_any, 0);
  D("equal", dyn_equal, 1);
  D("destroy", dyn_destroy, 0);
  D("copy", dyn_copy, 0);
  D("insert", dyn_insert, 2);
  D("get", dyn_get, 1);
  D("insert_seq", dyn_insert_seq, 2);
  D("get_seq", dyn_get_seq, 1);
  D("insert_dyn_any", dyn_insert_dyn_any, 1);
  D("get_dyn_any", dyn_get_dyn_any, 0);
  D("no_implement", dyn_no_implement, 0);
  D("seek", dyn_seek, 1);
  D("rewind", dyn_rewind, 0);
  D("next", dyn_next, 0);
  D("component_count", dyn_component_count, 0);
  D("current_component", dyn_current_component, 0);
  D("get_fixed_value", dyn_get_fixed_value, 0);
  D("set_fixed_value", dyn_set_fixed_value, 1);
  D("get_as_string", dyn_get_as_string, 0);
  D("set_as_string", dyn_set_as_string, 1);
  D("get_as_ulong", dyn_get_as_ulong, 0);
  D("set_as_ulong", dyn_set_as_ulong, 1);
  D("current_member_name", dyn_current_member_name, 0);
  D("current_member_kind", dyn_current_member_kind, 0);
  D("member_count", dyn_member_count, 0);
  D("member_at", dyn_member_at, 1);
  D("set_members", dyn_set_members, 2);
  D("set_members_as_dyn_any", dyn_set_members_as_dyn_any, 2);
  D("get_discriminator", dyn_get_discriminator, 0);
  D("set_discriminator", dyn_set_discriminator, 1);
  D("set_to_default_member", dyn_set_to_default_member, 0);
  D("set_to_no_active_member", dyn_set_to_no_active_member, 0);
  D("has_no_active_member", dyn_has_no_active_member, 0);
  D("discriminator_kind", dyn_discriminator_kind, 0);
  D("member", dyn_member, 0);
  D("member_name", dyn_member_name, 0);
  D("member_kind", dyn_member_kind, 0);
  D("get_length", dyn_get_length, 0);
  D("set_length", dyn_set_length, 1);
  D("element_count", dyn_element_count, 0);
  D("element_at", dyn_element_at, 1);
  D("set_elements", dyn_set_elements, 1);
  D("set_elements_as_dyn_any", dyn_set_elements_as_dyn_any, 1);
  D("is_null", dyn_is_null, 0);
  D("set_to_null", dyn_set_to_null, 0);
  D("set_to_value", dyn_set_to_value, 0);
  D("get_boxed_value", dyn_get_boxed_value, 0);
  D("set_boxed_value", dyn_set_boxed_value, 1);
  D("get_boxed_value_as_dyn_any", dyn_get_boxed_value_as_dyn_any, 0);
  D("set_boxed_value_as_dyn_any", dyn_set_boxed_value_as_dyn_any, 1);
#undef D
}

#define DEF(klass, name, fn, n) rb_define_method(klass, name, RUBY_METHOD_FUNC(fn), n)
#define SDEF(klass, name, fn, n) rb_define_singleton_method(klass, name, RUBY_METHOD_FUNC(fn), n)

/* --- the DII and the DSI ------------------------------------------------------------- */

/* Rustorb::Native::Request (one rorb_request) and ServerRequest (one
   rorb_server_request); values are Rustorb::Native::Any. CORBA::Request
   and CORBA::ServerRequest (dii.rb) are built on them. */
static VALUE cRequest, cServerRequest;
static void rel_request(void* p) { rorb_request_release(p); }
static void rel_server_request(void* p) { rorb_server_request_release(p); }
#define REQ(v) ((rorb_request*)unwrap((v), cRequest))
#define SREQ(v) ((rorb_server_request*)unwrap((v), cServerRequest))

static VALUE request_new(VALUE klass, VALUE target, VALUE op) {
  rorb_request* r = NULL;
  (void)klass;
  op = utf8_arg(op);
  CHECK(rorb_request_new(OBJ(target), RSTRING_PTR(op), (size_t)RSTRING_LEN(op), &r, EXC));
  RB_GC_GUARD(op);
  return wrap(cRequest, r, rel_request);
}
static VALUE request_add_arg(VALUE self, VALUE name, VALUE flags, VALUE any) {
  name = utf8_arg(name);
  CHECK(rorb_request_add_arg(REQ(self), RSTRING_PTR(name), (size_t)RSTRING_LEN(name), NUM2UINT(flags),
                             ANY(any), EXC));
  RB_GC_GUARD(name);
  return Qnil;
}
static VALUE request_set_return_type(VALUE self, VALUE tc) {
  rorb_request_set_return_type(REQ(self), TC(tc));
  return Qnil;
}
static VALUE request_add_exception(VALUE self, VALUE tc) {
  rorb_request_add_exception(REQ(self), TC(tc));
  return Qnil;
}
static VALUE request_set_timeout(VALUE self, VALUE us) {
  rorb_request_set_timeout(REQ(self), NUM2LL(us));
  return Qnil;
}

struct request_call {
  rorb_request* r;
  int what;
  int32_t ready;
  rorb_status st;
  rorb_exception exc;
};

static void* request_nogvl(void* p) {
  struct request_call* c = p;
  switch (c->what) {
    case 0: c->st = rorb_request_invoke(c->r, &c->exc); break;
    case 1: c->st = rorb_request_send_oneway(c->r, &c->exc); break;
    case 2: c->st = rorb_request_send_deferred(c->r, &c->exc); break;
    default: c->st = rorb_request_get_response(c->r, &c->exc); break;
  }
  return NULL;
}

/* true, or false when the operation raised a user exception of the list
   (Request#exception has it). */
static VALUE request_call(VALUE self, int what) {
  struct request_call c;
  memset(&c, 0, sizeof c);
  c.r = REQ(self);
  c.what = what;
  blocking(request_nogvl, &c);
  if (c.st == RORB_USER_EXCEPTION) {
    rorb_exception_clear(&c.exc);
    return Qfalse;
  }
  if (c.st != RORB_OK) raise_status(c.st, &c.exc);
  return Qtrue;
}
static VALUE request_invoke(VALUE self) { return request_call(self, 0); }
static VALUE request_send_oneway(VALUE self) { return request_call(self, 1); }
static VALUE request_send_deferred(VALUE self) { return request_call(self, 2); }
static VALUE request_get_response(VALUE self) { return request_call(self, 3); }
static VALUE request_poll_response(VALUE self) {
  int32_t ready = 0;
  CHECK(rorb_request_poll_response(REQ(self), &ready, EXC));
  return ready ? Qtrue : Qfalse;
}
static VALUE request_result(VALUE self) { return W_ANY(rorb_request_result(REQ(self))); }
static VALUE request_argument(VALUE self, VALUE i) {
  return W_ANY(rorb_request_argument(REQ(self), NUM2SIZET(i)));
}
static VALUE request_exception(VALUE self) { return W_ANY(rorb_request_exception(REQ(self))); }

/* ServerRequest.new(in, op): the request of the dispatch whose input
   stream is in (nothing read from it yet). */
static VALUE server_request_new(VALUE klass, VALUE in, VALUE op) {
  rorb_server_request* r = NULL;
  (void)klass;
  op = utf8_arg(op);
  CHECK(rorb_server_request_new(IN(in), RSTRING_PTR(op), (size_t)RSTRING_LEN(op), &r, EXC));
  RB_GC_GUARD(op);
  return wrap(cServerRequest, r, rel_server_request);
}
static VALUE server_request_add_param(VALUE self, VALUE name, VALUE flags, VALUE tc) {
  name = utf8_arg(name);
  CHECK(rorb_server_request_add_param(SREQ(self), RSTRING_PTR(name), (size_t)RSTRING_LEN(name),
                                      NUM2UINT(flags), TC(tc), EXC));
  RB_GC_GUARD(name);
  return Qnil;
}
static VALUE server_request_arguments(VALUE self) {
  CHECK(rorb_server_request_arguments(SREQ(self), EXC));
  return Qnil;
}
static VALUE server_request_argument(VALUE self, VALUE i) {
  return W_ANY(rorb_server_request_argument(SREQ(self), NUM2SIZET(i)));
}
static VALUE server_request_set_argument(VALUE self, VALUE i, VALUE any) {
  CHECK(rorb_server_request_set_argument(SREQ(self), NUM2SIZET(i), ANY(any), EXC));
  return Qnil;
}
static VALUE server_request_set_result(VALUE self, VALUE any) {
  CHECK(rorb_server_request_set_result(SREQ(self), ANY(any), EXC));
  return Qnil;
}
static VALUE server_request_set_exception(VALUE self, VALUE any) {
  CHECK(rorb_server_request_set_exception(SREQ(self), ANY(any), EXC));
  return Qnil;
}
struct sreq_reply {
  rorb_server_request* r;
  rorb_server_reply* reply;
  rorb_status st;
  rorb_exception exc;
};
static void* sreq_reply_nogvl(void* p) {
  struct sreq_reply* c = p;
  c->st = rorb_server_request_reply(c->r, c->reply, &c->exc);
  return NULL;
}
/* On the thread rustorb runs the upcall on (at_home), where the server
   interceptors look for what the servant set. */
static VALUE server_request_reply(VALUE self, VALUE reply) {
  struct sreq_reply c;
  memset(&c, 0, sizeof c);
  c.r = SREQ(self);
  c.reply = unwrap(reply, cReply);
  at_home(sreq_reply_nogvl, &c);
  if (c.st != RORB_OK) raise_status(c.st, &c.exc);
  return Qnil;
}

static void init_dii(void) {
  cRequest = rb_define_class_under(mNative, "Request", rb_cObject);
  rb_undef_alloc_func(cRequest);
  rb_define_singleton_method(cRequest, "create", RUBY_METHOD_FUNC(request_new), 2);
  DEF(cRequest, "add_arg", request_add_arg, 3);
  DEF(cRequest, "set_return_type", request_set_return_type, 1);
  DEF(cRequest, "add_exception", request_add_exception, 1);
  DEF(cRequest, "set_timeout", request_set_timeout, 1);
  DEF(cRequest, "invoke", request_invoke, 0);
  DEF(cRequest, "send_oneway", request_send_oneway, 0);
  DEF(cRequest, "send_deferred", request_send_deferred, 0);
  DEF(cRequest, "get_response", request_get_response, 0);
  DEF(cRequest, "poll_response", request_poll_response, 0);
  DEF(cRequest, "result", request_result, 0);
  DEF(cRequest, "argument", request_argument, 1);
  DEF(cRequest, "exception", request_exception, 0);

  cServerRequest = rb_define_class_under(mNative, "ServerRequest", rb_cObject);
  rb_undef_alloc_func(cServerRequest);
  rb_define_singleton_method(cServerRequest, "create", RUBY_METHOD_FUNC(server_request_new), 2);
  DEF(cServerRequest, "add_param", server_request_add_param, 3);
  DEF(cServerRequest, "arguments", server_request_arguments, 0);
  DEF(cServerRequest, "argument", server_request_argument, 1);
  DEF(cServerRequest, "set_argument", server_request_set_argument, 2);
  DEF(cServerRequest, "set_result", server_request_set_result, 1);
  DEF(cServerRequest, "set_exception", server_request_set_exception, 1);
  DEF(cServerRequest, "reply", server_request_reply, 1);
}

/* --- servant managers and adapter activators ------------------------------------------ */

/* A Ruby ServantActivator, ServantLocator or AdapterActivator is a peer
   too (kept alive while the POA holds it). The callbacks run its
   _rustorb_* methods (portable_server.rb) with the GVL:

     _rustorb_incarnate(oid, poa)              -> outcome
     _rustorb_etherealize(oid, poa, servant, cleanup, remaining)
     _rustorb_preinvoke(oid, poa, op)          -> outcome (with a cookie key)
     _rustorb_postinvoke(oid, poa, op, key, servant)
     _rustorb_unknown_adapter(parent, name)    -> true / false

   An outcome is [:servant, servant, ids(, key)], [:forward, Native::Ref]
   or [:system, id, minor, completed]. */
static ID id_incarnate, id_etherealize, id_preinvoke, id_postinvoke, id_unknown_adapter;
static VALUE sym_servant, sym_forward;

struct mgr_cb {
  peer* p;
  const uint8_t* id;
  size_t id_len;
  rorb_poa* adapter;
  const char* op;
  size_t op_len;
  void* servant_self;
  int32_t a, b;
  void* cookie;
  rorb_servant** servant;
  void** cookie_out;
  rorb_manager_error* error;
  int what;
  rorb_status st;
  int32_t answer;
};

static VALUE mgr_poa(rorb_poa* poa) { return W_POA(rorb_poa_clone(poa)); }

/* Applies an outcome; returns its status. */
static rorb_status mgr_outcome(struct mgr_cb* c, VALUE r) {
  VALUE kind;
  if (!RB_TYPE_P(r, T_ARRAY) || RARRAY_LEN(r) < 2) return RORB_SYSTEM_EXCEPTION;
  kind = rb_ary_entry(r, 0);
  if (kind == sym_servant) {
    VALUE servant = rb_ary_entry(r, 1);
    peer* sp = peer_of(servant, rb_ary_entry(r, 2));
    *c->servant = rorb_servant_get(sp, sp->ids[0] ? sp->ids[0] : "", sp->id_lens[0], &servant_vtable);
    if (c->cookie_out) *c->cookie_out = (void*)(uintptr_t)NUM2ULL(rb_ary_entry(r, 3));
    return RORB_OK;
  }
  if (kind == sym_forward) {
    rorb_manager_error_forward(c->error, OBJ(rb_ary_entry(r, 1)));
    return RORB_SYSTEM_EXCEPTION;
  }
  {
    VALUE id = utf8_arg(rb_ary_entry(r, 1));
    rorb_manager_error_system(c->error, RSTRING_PTR(id), (size_t)RSTRING_LEN(id),
                              NUM2UINT(rb_ary_entry(r, 2)), NUM2UINT(rb_ary_entry(r, 3)));
    return RORB_SYSTEM_EXCEPTION;
  }
}

static VALUE mgr_body(VALUE arg) {
  struct mgr_cb* c = (struct mgr_cb*)arg;
  VALUE oid = c->id ? rb_str_new((const char*)c->id, (long)c->id_len) : Qnil;
  VALUE poa = mgr_poa(c->adapter);
  VALUE servant = c->servant_self ? ((peer*)c->servant_self)->obj : Qnil;
  VALUE r;
  switch (c->what) {
    case 0:
      r = rb_funcall(c->p->obj, id_incarnate, 2, oid, poa);
      c->st = mgr_outcome(c, r);
      break;
    case 1:
      rb_funcall(c->p->obj, id_etherealize, 5, oid, poa, servant, c->a ? Qtrue : Qfalse,
                 c->b ? Qtrue : Qfalse);
      break;
    case 2:
      r = rb_funcall(c->p->obj, id_preinvoke, 3, oid, poa, utf8(c->op, c->op_len));
      c->st = mgr_outcome(c, r);
      break;
    case 3:
      rb_funcall(c->p->obj, id_postinvoke, 5, oid, poa, utf8(c->op, c->op_len),
                 ULL2NUM((uintptr_t)c->cookie), servant);
      break;
    default:
      r = rb_funcall(c->p->obj, id_unknown_adapter, 2, poa, utf8(c->op, c->op_len));
      c->answer = RTEST(r) ? 1 : 0;
      break;
  }
  return Qnil;
}

static void* mgr_gvl(void* arg) {
  struct mgr_cb* c = arg;
  int state = 0;
  rb_protect(mgr_body, (VALUE)c, &state);
  if (state) {
    VALUE e = rb_errinfo();
    rb_set_errinfo(Qnil);
    rb_funcall(rb_stderr, rb_intern("puts"), 1,
               rb_str_concat(rb_str_new_cstr("rustorb: servant manager failed: "), rb_inspect(e)));
    if (c->error) {
      rorb_manager_error_system(c->error, "IDL:omg.org/CORBA/UNKNOWN:1.0", 29, 0, RORB_COMPLETED_NO);
      c->st = RORB_SYSTEM_EXCEPTION;
    }
    c->answer = 0;
  }
  return c;
}

static void mgr_run(struct mgr_cb* c) {
  if (!with_gvl(mgr_gvl, c) && c->error) {
    rorb_manager_error_system(c->error, "IDL:omg.org/CORBA/TRANSIENT:1.0", 31, 0, RORB_COMPLETED_NO);
    c->st = RORB_SYSTEM_EXCEPTION;
  }
}

static rorb_status act_incarnate(void* self, const uint8_t* id, size_t id_len, rorb_poa* adapter,
                                 rorb_servant** servant, rorb_manager_error* error) {
  struct mgr_cb c;
  memset(&c, 0, sizeof c);
  c.p = self, c.id = id, c.id_len = id_len, c.adapter = adapter, c.servant = servant;
  c.error = error, c.what = 0, c.st = RORB_SYSTEM_EXCEPTION;
  mgr_run(&c);
  return c.st;
}
static void act_etherealize(void* self, const uint8_t* id, size_t id_len, rorb_poa* adapter,
                            void* servant, int32_t cleanup, int32_t remaining) {
  struct mgr_cb c;
  memset(&c, 0, sizeof c);
  c.p = self, c.id = id, c.id_len = id_len, c.adapter = adapter, c.servant_self = servant;
  c.a = cleanup, c.b = remaining, c.what = 1;
  mgr_run(&c);
}
static rorb_status loc_preinvoke(void* self, const uint8_t* id, size_t id_len, rorb_poa* adapter,
                                 const char* op, size_t op_len, rorb_servant** servant,
                                 void** cookie, rorb_manager_error* error) {
  struct mgr_cb c;
  memset(&c, 0, sizeof c);
  c.p = self, c.id = id, c.id_len = id_len, c.adapter = adapter, c.op = op, c.op_len = op_len;
  c.servant = servant, c.cookie_out = cookie, c.error = error, c.what = 2;
  c.st = RORB_SYSTEM_EXCEPTION;
  mgr_run(&c);
  return c.st;
}
static void loc_postinvoke(void* self, const uint8_t* id, size_t id_len, rorb_poa* adapter,
                           const char* op, size_t op_len, void* cookie, void* servant) {
  struct mgr_cb c;
  memset(&c, 0, sizeof c);
  c.p = self, c.id = id, c.id_len = id_len, c.adapter = adapter, c.op = op, c.op_len = op_len;
  c.cookie = cookie, c.servant_self = servant, c.what = 3;
  mgr_run(&c);
}
static int32_t aa_unknown_adapter(void* self, rorb_poa* parent, const char* name, size_t len) {
  struct mgr_cb c;
  memset(&c, 0, sizeof c);
  c.p = self, c.adapter = parent, c.op = name, c.op_len = len, c.what = 4;
  mgr_run(&c);
  return c.answer;
}

static const rorb_servant_activator_vtable activator_vtable = {
    act_incarnate, act_etherealize, servant_add_ref, servant_remove_ref};
static const rorb_servant_locator_vtable locator_vtable = {loc_preinvoke, loc_postinvoke,
                                                           servant_add_ref, servant_remove_ref};
static const rorb_adapter_activator_vtable adapter_activator_vtable = {
    aa_unknown_adapter, servant_add_ref, servant_remove_ref};

static VALUE poa_set_servant_activator(VALUE self, VALUE obj) {
  peer* p = peer_of(obj, rb_ary_new());
  CHECK(rorb_poa_set_servant_activator(POA(self), p, &activator_vtable, EXC));
  return Qnil;
}
static VALUE poa_set_servant_locator(VALUE self, VALUE obj) {
  peer* p = peer_of(obj, rb_ary_new());
  CHECK(rorb_poa_set_servant_locator(POA(self), p, &locator_vtable, EXC));
  return Qnil;
}
static VALUE poa_get_servant_manager(VALUE self) {
  int32_t kind = 0;
  void* s = NULL;
  CHECK(rorb_poa_get_servant_manager(POA(self), &kind, &s, EXC));
  return servant_of(s);
}
static VALUE poa_set_the_activator(VALUE self, VALUE obj) {
  if (NIL_P(obj)) {
    CHECK(rorb_poa_set_the_activator(POA(self), NULL, &adapter_activator_vtable, EXC));
  } else {
    peer* p = peer_of(obj, rb_ary_new());
    CHECK(rorb_poa_set_the_activator(POA(self), p, &adapter_activator_vtable, EXC));
  }
  return Qnil;
}
static VALUE poa_the_activator(VALUE self) { return servant_of(rorb_poa_the_activator(POA(self))); }

static void init_managers(void) {
  id_incarnate = rb_intern("_rustorb_incarnate");
  id_etherealize = rb_intern("_rustorb_etherealize");
  id_preinvoke = rb_intern("_rustorb_preinvoke");
  id_postinvoke = rb_intern("_rustorb_postinvoke");
  id_unknown_adapter = rb_intern("_rustorb_unknown_adapter");
  sym_servant = ID2SYM(rb_intern("servant"));
  sym_forward = ID2SYM(rb_intern("forward"));
  DEF(cPOA, "set_servant_activator", poa_set_servant_activator, 1);
  DEF(cPOA, "set_servant_locator", poa_set_servant_locator, 1);
  DEF(cPOA, "get_servant_manager", poa_get_servant_manager, 0);
  DEF(cPOA, "set_the_activator", poa_set_the_activator, 1);
  DEF(cPOA, "the_activator", poa_the_activator, 0);
}

/* --- Portable Interceptors and IOP::Codec ------------------------------------------- */

/* An interceptor or ORB initializer is a host of its Ruby object (the ORB's
   add_ref / remove_ref count on it). A point runs, with the GVL,
   Rustorb::PI.point(obj, point, info): info is a Native::ClientRequestInfo,
   ServerRequestInfo, IORInfo or ORBInitInfo, valid only during the point;
   it returns nil, [:forward, Native::Ref] or [:system, id, minor,
   completed]. The info calls are made here, on the thread that runs the
   point, while the thread rustorb called the point on waits for it. */
static VALUE cCRInfo, cSRInfo, cIORInfo, cInitInfo, cPICurrent, cCodec, mPI;
static ID id_point, id_destroy_interceptor;
static VALUE sym_system;

static void rel_pi_current(void* p) { rorb_pi_current_release(p); }
static void rel_codec(void* p) { rorb_codec_release(p); }
#define CRI(v) ((const rorb_client_request_info*)unwrap((v), cCRInfo))
#define SRI(v) ((const rorb_server_request_info*)unwrap((v), cSRInfo))
#define IORI(v) ((const rorb_ior_info*)unwrap((v), cIORInfo))
#define INITI(v) ((rorb_orb_init_info*)unwrap((v), cInitInfo))
#define PICUR(v) ((rorb_pi_current*)unwrap((v), cPICurrent))
#define CODEC(v) ((rorb_codec*)unwrap((v), cCodec))

static void pi_add_ref(void* self) {
  host* h = self;
  pthread_mutex_lock(&host_lock);
  ++h->count;
  pthread_mutex_unlock(&host_lock);
}
static void pi_remove_ref(void* self) { host_release(self); }

struct pi_point {
  host* h;
  const char* point;
  VALUE* klass;
  void* info;
  rorb_pi_error* error;
  rorb_status st;
  VALUE info_v;
};

static VALUE pi_point_body(VALUE p) {
  struct pi_point* c = (struct pi_point*)p;
  return rb_funcall(mPI, id_point, 3, c->h->obj, ID2SYM(rb_intern(c->point)), c->info_v);
}

static void pi_unknown(struct pi_point* c) {
  rorb_pi_error_system(c->error, "IDL:omg.org/CORBA/UNKNOWN:1.0", 29, 0, RORB_COMPLETED_MAYBE);
  c->st = RORB_SYSTEM_EXCEPTION;
}

static void* pi_point_gvl(void* p) {
  struct pi_point* c = p;
  int state = 0;
  VALUE r, info = wrap_borrowed(*c->klass, c->info);
  c->info_v = info;
  r = rb_protect(pi_point_body, (VALUE)c, &state);
  invalidate(info);
  RB_GC_GUARD(info);
  if (state) {
    rb_set_errinfo(Qnil);
    pi_unknown(c);
    return c;
  }
  if (NIL_P(r)) return c;
  if (!RB_TYPE_P(r, T_ARRAY) || RARRAY_LEN(r) < 2) {
    pi_unknown(c);
  } else if (rb_ary_entry(r, 0) == sym_forward) {
    VALUE ref = rb_ary_entry(r, 1);
    if (!rb_obj_is_kind_of(ref, cRef)) {
      pi_unknown(c);
      return c;
    }
    rorb_pi_error_forward(c->error, OBJ(ref));
    c->st = RORB_SYSTEM_EXCEPTION;
  } else if (RARRAY_LEN(r) >= 4) {
    VALUE id = rb_ary_entry(r, 1);
    if (!RB_TYPE_P(id, T_STRING)) {
      pi_unknown(c);
      return c;
    }
    rorb_pi_error_system(c->error, RSTRING_PTR(id), (size_t)RSTRING_LEN(id),
                         NUM2UINT(rb_ary_entry(r, 2)), NUM2UINT(rb_ary_entry(r, 3)));
    c->st = RORB_SYSTEM_EXCEPTION;
  } else {
    pi_unknown(c);
  }
  return c;
}

static int ruby_ending(void) {
  int r;
  pthread_mutex_lock(&queue_lock);
  r = shutting_down;
  pthread_mutex_unlock(&queue_lock);
  return r;
}

static rorb_status pi_run(void* self, const char* point, VALUE* klass, const void* info,
                          rorb_pi_error* error) {
  struct pi_point c;
  memset(&c, 0, sizeof c);
  c.h = self;
  c.point = point;
  c.klass = klass;
  c.info = (void*)info;
  c.error = error;
  c.st = RORB_OK;
  c.info_v = Qnil;
  if (!ruby_ending()) with_gvl(pi_point_gvl, &c);
  return c.st;
}

static VALUE pi_destroy_body(VALUE obj) { return rb_funcall(mPI, id_destroy_interceptor, 1, obj); }
static void* pi_destroy_gvl(void* p) {
  int state = 0;
  rb_protect(pi_destroy_body, ((host*)p)->obj, &state);
  if (state) rb_set_errinfo(Qnil);
  return p;
}
/* At exit the ORBs are released while Ruby frees its objects, so an
   interceptor's object may be gone: no Ruby then. */
static void pi_destroy(void* self) {
  if (!ruby_ending()) with_gvl(pi_destroy_gvl, self);
}

#define PI_POINT(fn, name, T, klass)                                     \
  static rorb_status fn(void* self, const T* ri, rorb_pi_error* error) { \
    return pi_run(self, name, &klass, ri, error);                        \
  }
PI_POINT(pi_send_request, "send_request", rorb_client_request_info, cCRInfo)
PI_POINT(pi_receive_reply, "receive_reply", rorb_client_request_info, cCRInfo)
PI_POINT(pi_receive_exception, "receive_exception", rorb_client_request_info, cCRInfo)
PI_POINT(pi_receive_other, "receive_other", rorb_client_request_info, cCRInfo)
PI_POINT(pi_receive_request_service_contexts, "receive_request_service_contexts",
         rorb_server_request_info, cSRInfo)
PI_POINT(pi_receive_request, "receive_request", rorb_server_request_info, cSRInfo)
PI_POINT(pi_send_reply, "send_reply", rorb_server_request_info, cSRInfo)
PI_POINT(pi_send_exception, "send_exception", rorb_server_request_info, cSRInfo)
PI_POINT(pi_send_other, "send_other", rorb_server_request_info, cSRInfo)
PI_POINT(pi_establish_components, "establish_components", rorb_ior_info, cIORInfo)

static rorb_status pi_pre_init(void* self, rorb_orb_init_info* info, rorb_pi_error* error) {
  return pi_run(self, "pre_init", &cInitInfo, info, error);
}
static rorb_status pi_post_init(void* self, rorb_orb_init_info* info, rorb_pi_error* error) {
  return pi_run(self, "post_init", &cInitInfo, info, error);
}

static const rorb_client_interceptor_vtable client_interceptor_vtable = {
    pi_send_request, pi_receive_reply, pi_receive_exception, pi_receive_other,
    pi_destroy,      pi_add_ref,       pi_remove_ref};
static const rorb_server_interceptor_vtable server_interceptor_vtable = {
    pi_receive_request_service_contexts, pi_receive_request, pi_send_reply, pi_send_exception,
    pi_send_other, pi_destroy, pi_add_ref, pi_remove_ref};
static const rorb_ior_interceptor_vtable ior_interceptor_vtable = {
    pi_establish_components, pi_destroy, pi_add_ref, pi_remove_ref};
static const rorb_orb_initializer_vtable orb_initializer_vtable = {pi_pre_init, pi_post_init,
                                                                   pi_add_ref, pi_remove_ref};

/* Native.register_orb_initializer(obj). */
static VALUE native_register_orb_initializer(VALUE self, VALUE obj) {
  host* h = host_retain(obj);
  (void)self;
  rorb_register_orb_initializer(h, &orb_initializer_vtable);
  host_release(h);
  return Qnil;
}

/* Native.orb_init_pi(args, orb_id, initializers): [orb, args left], an ORB
   of its own with these initializers too (rorb_orb_init_pi). */
static VALUE native_orb_init_pi(VALUE self, VALUE args, VALUE orb_id, VALUE inits) {
  long n, i, k;
  int argc;
  char** argv;
  rorb_orb* orb = NULL;
  rorb_orb_initializer* entries;
  VALUE rest, strs;
  rorb_exception exc;
  rorb_status st;
  (void)self;
  Check_Type(args, T_ARRAY);
  Check_Type(inits, T_ARRAY);
  orb_id = utf8_arg(orb_id);
  n = RARRAY_LEN(args);
  strs = rb_ary_new_capa(n);
  argv = ALLOCA_N(char*, n + 1);
  for (i = 0; i < n; ++i) {
    VALUE s = rb_str_dup(utf8_arg(rb_ary_entry(args, i)));
    rb_ary_push(strs, s);
    argv[i] = StringValueCStr(s);
  }
  argv[n] = NULL;
  argc = (int)n;
  k = RARRAY_LEN(inits);
  entries = ALLOCA_N(rorb_orb_initializer, k + 1);
  for (i = 0; i < k; ++i) {
    entries[i].self = host_retain(rb_ary_entry(inits, i));
    entries[i].vtable = &orb_initializer_vtable;
  }
  memset(&exc, 0, sizeof exc);
  st = rorb_orb_init_pi(&argc, argv, RSTRING_PTR(orb_id), (size_t)RSTRING_LEN(orb_id), entries,
                        (size_t)k, &orb, &exc);
  for (i = 0; i < k; ++i) host_release(entries[i].self);
  if (st != RORB_OK) raise_status(st, &exc);
  rest = rb_ary_new_capa(argc);
  for (i = 0; i < argc; ++i) rb_ary_push(rest, rb_utf8_str_new_cstr(argv[i]));
  RB_GC_GUARD(strs);
  RB_GC_GUARD(orb_id);
  RB_GC_GUARD(inits);
  return rb_assoc_new(W_ORB(orb), rest);
}

/* --- the each callbacks (called on this thread, with the GVL) ------------------------- */

static void pi_each_string(void* ctx, const char* s, size_t len) {
  rb_ary_push(*(VALUE*)ctx, utf8(s, len));
}
static void pi_each_component(void* ctx, uint32_t tag, const uint8_t* data, size_t len) {
  rb_ary_push(*(VALUE*)ctx, rb_assoc_new(UINT2NUM(tag), rb_str_new((const char*)data, (long)len)));
}
static void pi_each_parameter(void* ctx, rorb_any* any, uint32_t mode) {
  rb_ary_push(*(VALUE*)ctx, rb_assoc_new(W_ANY(any), UINT2NUM(mode)));
}
static void pi_each_typecode(void* ctx, rorb_typecode* tc) { rb_ary_push(*(VALUE*)ctx, W_TC(tc)); }

static VALUE utf8_bytes(rorb_bytes b) { return rb_enc_associate(bytes_str(b), rb_utf8_encoding()); }

/* --- RequestInfo: what ClientRequestInfo (cri) and ServerRequestInfo (sri) share --- */

#define RI_COMMON(P, R)                                                                  \
  static VALUE P##_request_id(VALUE self) { return UINT2NUM(rorb_##P##_request_id(R(self))); } \
  static VALUE P##_operation(VALUE self) {                                               \
    size_t len = 0;                                                                      \
    const char* s = rorb_##P##_operation(R(self), &len);                                 \
    return utf8(s, len);                                                                 \
  }                                                                                      \
  static VALUE P##_response_expected(VALUE self) {                                       \
    return rorb_##P##_response_expected(R(self)) ? Qtrue : Qfalse;                       \
  }                                                                                      \
  static VALUE P##_sync_scope(VALUE self) { return INT2FIX(rorb_##P##_sync_scope(R(self))); } \
  static VALUE P##_reply_status(VALUE self) {                                            \
    int16_t s = 0;                                                                       \
    CHECK(rorb_##P##_reply_status(R(self), &s, EXC));                                    \
    return INT2FIX(s);                                                                   \
  }                                                                                      \
  static VALUE P##_forward_reference(VALUE self) {                                       \
    rorb_object* o = NULL;                                                               \
    CHECK(rorb_##P##_forward_reference(R(self), &o, EXC));                               \
    return W_OBJ(o);                                                                     \
  }                                                                                      \
  static VALUE P##_get_slot(VALUE self, VALUE id) {                                      \
    rorb_any* a = NULL;                                                                  \
    CHECK(rorb_##P##_get_slot(R(self), NUM2UINT(id), &a, EXC));                          \
    return W_ANY(a);                                                                     \
  }                                                                                      \
  static VALUE P##_get_request_service_context(VALUE self, VALUE id) {                   \
    rorb_bytes b;                                                                        \
    memset(&b, 0, sizeof b);                                                             \
    CHECK(rorb_##P##_get_request_service_context(R(self), NUM2UINT(id), &b, EXC));       \
    return bytes_str(b);                                                                 \
  }                                                                                      \
  static VALUE P##_get_reply_service_context(VALUE self, VALUE id) {                     \
    rorb_bytes b;                                                                        \
    memset(&b, 0, sizeof b);                                                             \
    CHECK(rorb_##P##_get_reply_service_context(R(self), NUM2UINT(id), &b, EXC));         \
    return bytes_str(b);                                                                 \
  }                                                                                      \
  static VALUE P##_arguments(VALUE self) {                                               \
    VALUE r = rb_ary_new();                                                              \
    CHECK(rorb_##P##_arguments(R(self), &r, pi_each_parameter, EXC));                    \
    return r;                                                                            \
  }                                                                                      \
  static VALUE P##_exceptions(VALUE self) {                                              \
    VALUE r = rb_ary_new();                                                              \
    CHECK(rorb_##P##_exceptions(R(self), &r, pi_each_typecode, EXC));                    \
    return r;                                                                            \
  }                                                                                      \
  static VALUE P##_result(VALUE self) {                                                  \
    rorb_any* a = NULL;                                                                  \
    CHECK(rorb_##P##_result(R(self), &a, EXC));                                          \
    return W_ANY(a);                                                                     \
  }

RI_COMMON(cri, CRI)
RI_COMMON(sri, SRI)

static VALUE cri_target(VALUE self) { return W_OBJ(rorb_cri_target(CRI(self))); }
static VALUE cri_effective_target(VALUE self) { return W_OBJ(rorb_cri_effective_target(CRI(self))); }
static VALUE cri_effective_profile(VALUE self) {
  rorb_bytes b;
  uint32_t tag;
  memset(&b, 0, sizeof b);
  tag = rorb_cri_effective_profile(CRI(self), &b);
  return rb_assoc_new(UINT2NUM(tag), bytes_str(b));
}
static VALUE cri_received_exception(VALUE self) {
  rorb_any* a = NULL;
  CHECK(rorb_cri_received_exception(CRI(self), &a, EXC));
  return W_ANY(a);
}
static VALUE cri_received_exception_id(VALUE self) {
  rorb_bytes b;
  memset(&b, 0, sizeof b);
  CHECK(rorb_cri_received_exception_id(CRI(self), &b, EXC));
  return utf8_bytes(b);
}
static VALUE cri_get_effective_component(VALUE self, VALUE tag) {
  rorb_bytes b;
  memset(&b, 0, sizeof b);
  CHECK(rorb_cri_get_effective_component(CRI(self), NUM2UINT(tag), &b, EXC));
  return bytes_str(b);
}
static VALUE cri_get_effective_components(VALUE self, VALUE tag) {
  VALUE r = rb_ary_new();
  CHECK(rorb_cri_get_effective_components(CRI(self), NUM2UINT(tag), &r, pi_each_component, EXC));
  return r;
}
/* The raw value of the policy, or nil. */
static VALUE cri_get_request_policy(VALUE self, VALUE type) {
  int32_t found = 0;
  uint64_t raw = 0;
  CHECK(rorb_cri_get_request_policy(CRI(self), NUM2UINT(type), &found, &raw, EXC));
  return found ? ULL2NUM(raw) : Qnil;
}
static VALUE cri_add_request_service_context(VALUE self, VALUE id, VALUE data, VALUE replace) {
  StringValue(data);
  CHECK(rorb_cri_add_request_service_context(CRI(self), NUM2UINT(id),
                                             (const uint8_t*)RSTRING_PTR(data),
                                             (size_t)RSTRING_LEN(data), RTEST(replace), EXC));
  RB_GC_GUARD(data);
  return Qnil;
}

static VALUE sri_set_slot(VALUE self, VALUE id, VALUE any) {
  CHECK(rorb_sri_set_slot(SRI(self), NUM2UINT(id), ANY(any), EXC));
  return Qnil;
}
static VALUE sri_add_reply_service_context(VALUE self, VALUE id, VALUE data, VALUE replace) {
  StringValue(data);
  CHECK(rorb_sri_add_reply_service_context(SRI(self), NUM2UINT(id),
                                           (const uint8_t*)RSTRING_PTR(data),
                                           (size_t)RSTRING_LEN(data), RTEST(replace), EXC));
  RB_GC_GUARD(data);
  return Qnil;
}
static VALUE sri_sending_exception(VALUE self) {
  rorb_any* a = NULL;
  CHECK(rorb_sri_sending_exception(SRI(self), &a, EXC));
  return W_ANY(a);
}
static VALUE sri_sending_exception_id(VALUE self) {
  rorb_bytes b;
  memset(&b, 0, sizeof b);
  CHECK(rorb_sri_sending_exception_id(SRI(self), &b, EXC));
  return utf8_bytes(b);
}
static VALUE sri_object_id(VALUE self) {
  rorb_bytes b;
  memset(&b, 0, sizeof b);
  CHECK(rorb_sri_object_id(SRI(self), &b, EXC));
  return bytes_str(b);
}
static VALUE sri_adapter_id(VALUE self) {
  rorb_bytes b;
  memset(&b, 0, sizeof b);
  CHECK(rorb_sri_adapter_id(SRI(self), &b, EXC));
  return bytes_str(b);
}
static VALUE sri_adapter_name(VALUE self) {
  VALUE r = rb_ary_new();
  CHECK(rorb_sri_adapter_name(SRI(self), &r, pi_each_string, EXC));
  return r;
}
static VALUE sri_orb_id(VALUE self) {
  size_t len = 0;
  const char* s = rorb_sri_orb_id(SRI(self), &len);
  return utf8(s, len);
}
static VALUE sri_server_id(VALUE self) {
  size_t len = 0;
  const char* s = rorb_sri_server_id(SRI(self), &len);
  return utf8(s, len);
}
static VALUE sri_target_most_derived_interface(VALUE self) {
  rorb_bytes b;
  memset(&b, 0, sizeof b);
  CHECK(rorb_sri_target_most_derived_interface(SRI(self), &b, EXC));
  return utf8_bytes(b);
}
static VALUE sri_target_is_a(VALUE self, VALUE id) {
  int32_t is_a = 0;
  id = utf8_arg(id);
  CHECK(rorb_sri_target_is_a(SRI(self), RSTRING_PTR(id), (size_t)RSTRING_LEN(id), &is_a, EXC));
  RB_GC_GUARD(id);
  return is_a ? Qtrue : Qfalse;
}
static VALUE sri_get_server_policy(VALUE self, VALUE type) {
  uint64_t raw = 0;
  CHECK(rorb_sri_get_server_policy(SRI(self), NUM2UINT(type), &raw, EXC));
  return ULL2NUM(raw);
}

/* --- IORInfo ------------------------------------------------------------------------ */

static VALUE iori_get_effective_policy(VALUE self, VALUE type) {
  uint64_t raw = 0;
  CHECK(rorb_ior_info_get_effective_policy(IORI(self), NUM2UINT(type), &raw, EXC));
  return ULL2NUM(raw);
}
static VALUE iori_adapter_name(VALUE self) {
  VALUE r = rb_ary_new();
  rorb_ior_info_adapter_name(IORI(self), &r, pi_each_string);
  return r;
}
static VALUE iori_add_ior_component(VALUE self, VALUE tag, VALUE data) {
  StringValue(data);
  rorb_ior_info_add_ior_component(IORI(self), NUM2UINT(tag), (const uint8_t*)RSTRING_PTR(data),
                                  (size_t)RSTRING_LEN(data));
  RB_GC_GUARD(data);
  return Qnil;
}
static VALUE iori_add_ior_component_to_profile(VALUE self, VALUE tag, VALUE data, VALUE profile) {
  StringValue(data);
  CHECK(rorb_ior_info_add_ior_component_to_profile(IORI(self), NUM2UINT(tag),
                                                   (const uint8_t*)RSTRING_PTR(data),
                                                   (size_t)RSTRING_LEN(data), NUM2UINT(profile),
                                                   EXC));
  RB_GC_GUARD(data);
  return Qnil;
}

/* --- ORBInitInfo ---------------------------------------------------------------------- */

static VALUE initi_arguments(VALUE self) {
  VALUE r = rb_ary_new();
  rorb_init_info_arguments(INITI(self), &r, pi_each_string);
  return r;
}
static VALUE initi_orb_id(VALUE self) {
  size_t len = 0;
  const char* s = rorb_init_info_orb_id(INITI(self), &len);
  return utf8(s, len);
}
static VALUE initi_orb(VALUE self) { return W_ORB(rorb_init_info_orb(INITI(self))); }
static VALUE initi_register_initial_reference(VALUE self, VALUE name, VALUE obj) {
  name = utf8_arg(name);
  CHECK(rorb_init_info_register_initial_reference(INITI(self), RSTRING_PTR(name),
                                                  (size_t)RSTRING_LEN(name), unwrap_opt(obj, cRef),
                                                  EXC));
  RB_GC_GUARD(name);
  return Qnil;
}
static VALUE initi_resolve_initial_references(VALUE self, VALUE name) {
  rorb_object* o = NULL;
  name = utf8_arg(name);
  CHECK(rorb_init_info_resolve_initial_references(INITI(self), RSTRING_PTR(name),
                                                  (size_t)RSTRING_LEN(name), &o, EXC));
  RB_GC_GUARD(name);
  return W_OBJ(o);
}

/* add_*_interceptor(name, obj): RORB_DUPLICATE_NAME raises the local
   exception 22. */
#define INIT_ADD(fn, cfn, vt)                                                                 \
  static VALUE fn(VALUE self, VALUE name, VALUE obj) {                                       \
    host* h;                                                                                 \
    rorb_exception exc;                                                                      \
    rorb_status st;                                                                          \
    rorb_orb_init_info* info = INITI(self);                                                  \
    name = utf8_arg(name);                                                                   \
    h = host_retain(obj);                                                                    \
    memset(&exc, 0, sizeof exc);                                                             \
    st = cfn(info, RSTRING_PTR(name), (size_t)RSTRING_LEN(name), h, &vt, &exc);              \
    host_release(h);                                                                         \
    RB_GC_GUARD(name);                                                                       \
    if (st != RORB_OK) raise_status(st, &exc);                                               \
    return Qnil;                                                                             \
  }
INIT_ADD(initi_add_client_request_interceptor, rorb_init_info_add_client_request_interceptor,
         client_interceptor_vtable)
INIT_ADD(initi_add_server_request_interceptor, rorb_init_info_add_server_request_interceptor,
         server_interceptor_vtable)
INIT_ADD(initi_add_ior_interceptor, rorb_init_info_add_ior_interceptor, ior_interceptor_vtable)

static VALUE initi_allocate_slot_id(VALUE self) {
  return UINT2NUM(rorb_init_info_allocate_slot_id(INITI(self)));
}
static VALUE initi_pi_current(VALUE self) {
  return wrap(cPICurrent, rorb_init_info_pi_current(INITI(self)), rel_pi_current);
}
static VALUE initi_create_codec(VALUE self, VALUE format, VALUE major, VALUE minor) {
  rorb_codec* c = NULL;
  CHECK(rorb_init_info_create_codec(INITI(self), (int16_t)NUM2INT(format), (uint8_t)NUM2UINT(major),
                                    (uint8_t)NUM2UINT(minor), &c, EXC));
  return wrap(cCodec, c, rel_codec);
}

/* --- PICurrent and the codecs --------------------------------------------------------- */

static VALUE orb_pi_current(VALUE self) {
  return wrap(cPICurrent, rorb_orb_pi_current(ORB(self)), rel_pi_current);
}

/* The slots are per thread: in an upcall, those of the thread rustorb runs
   it on (at_home). */
struct slot_call {
  rorb_pi_current* current;
  uint32_t id;
  rorb_any* any;
  int set;
  rorb_status st;
  rorb_exception exc;
};
static void* slot_nogvl(void* p) {
  struct slot_call* c = p;
  c->st = c->set ? rorb_pi_current_set_slot(c->current, c->id, c->any, &c->exc)
                 : rorb_pi_current_get_slot(c->current, c->id, &c->any, &c->exc);
  return NULL;
}
static VALUE picur_get_slot(VALUE self, VALUE id) {
  struct slot_call c;
  memset(&c, 0, sizeof c);
  c.current = PICUR(self);
  c.id = NUM2UINT(id);
  at_home(slot_nogvl, &c);
  if (c.st != RORB_OK) raise_status(c.st, &c.exc);
  return W_ANY(c.any);
}
static VALUE picur_set_slot(VALUE self, VALUE id, VALUE any) {
  struct slot_call c;
  memset(&c, 0, sizeof c);
  c.current = PICUR(self);
  c.id = NUM2UINT(id);
  c.any = ANY(any);
  c.set = 1;
  at_home(slot_nogvl, &c);
  RB_GC_GUARD(any);
  if (c.st != RORB_OK) raise_status(c.st, &c.exc);
  return Qnil;
}

static VALUE orb_create_codec(VALUE self, VALUE format, VALUE major, VALUE minor) {
  rorb_codec* c = NULL;
  CHECK(rorb_orb_create_codec(ORB(self), (int16_t)NUM2INT(format), (uint8_t)NUM2UINT(major),
                              (uint8_t)NUM2UINT(minor), &c, EXC));
  return wrap(cCodec, c, rel_codec);
}
static VALUE codec_encode(VALUE self, VALUE any, VALUE value_only) {
  rorb_bytes b;
  memset(&b, 0, sizeof b);
  CHECK(rorb_codec_encode(CODEC(self), ANY(any), RTEST(value_only), &b, EXC));
  return bytes_str(b);
}
static VALUE codec_decode(VALUE self, VALUE data, VALUE tc) {
  rorb_any* a = NULL;
  StringValue(data);
  CHECK(rorb_codec_decode(CODEC(self), (const uint8_t*)RSTRING_PTR(data), (size_t)RSTRING_LEN(data),
                          NIL_P(tc) ? NULL : TC(tc), &a, EXC));
  RB_GC_GUARD(data);
  return W_ANY(a);
}

static VALUE pi_class(const char* name) {
  VALUE k = rb_define_class_under(mNative, name, rb_cObject);
  rb_undef_alloc_func(k);
  return k;
}

static void init_pi(void) {
  id_point = rb_intern("point");
  id_destroy_interceptor = rb_intern("destroy_interceptor");
  sym_system = ID2SYM(rb_intern("system"));
  mPI = rb_define_module_under(mRustorb, "PI");
  SDEF(mNative, "register_orb_initializer", native_register_orb_initializer, 1);
  SDEF(mNative, "orb_init_pi", native_orb_init_pi, 3);
  DEF(cORB, "pi_current", orb_pi_current, 0);
  DEF(cORB, "create_codec", orb_create_codec, 3);

#define RI_DEFS(k, P)                                                                    \
  DEF(k, "request_id", P##_request_id, 0);                                               \
  DEF(k, "operation", P##_operation, 0);                                                 \
  DEF(k, "response_expected", P##_response_expected, 0);                                 \
  DEF(k, "sync_scope", P##_sync_scope, 0);                                               \
  DEF(k, "reply_status", P##_reply_status, 0);                                           \
  DEF(k, "forward_reference", P##_forward_reference, 0);                                 \
  DEF(k, "get_slot", P##_get_slot, 1);                                                   \
  DEF(k, "get_request_service_context", P##_get_request_service_context, 1);             \
  DEF(k, "get_reply_service_context", P##_get_reply_service_context, 1);                 \
  DEF(k, "arguments", P##_arguments, 0);                                                 \
  DEF(k, "exceptions", P##_exceptions, 0);                                               \
  DEF(k, "result", P##_result, 0)
  cCRInfo = pi_class("ClientRequestInfo");
  RI_DEFS(cCRInfo, cri);
  DEF(cCRInfo, "target", cri_target, 0);
  DEF(cCRInfo, "effective_target", cri_effective_target, 0);
  DEF(cCRInfo, "effective_profile", cri_effective_profile, 0);
  DEF(cCRInfo, "received_exception", cri_received_exception, 0);
  DEF(cCRInfo, "received_exception_id", cri_received_exception_id, 0);
  DEF(cCRInfo, "get_effective_component", cri_get_effective_component, 1);
  DEF(cCRInfo, "get_effective_components", cri_get_effective_components, 1);
  DEF(cCRInfo, "get_request_policy", cri_get_request_policy, 1);
  DEF(cCRInfo, "add_request_service_context", cri_add_request_service_context, 3);

  cSRInfo = pi_class("ServerRequestInfo");
  RI_DEFS(cSRInfo, sri);
  DEF(cSRInfo, "set_slot", sri_set_slot, 2);
  DEF(cSRInfo, "add_reply_service_context", sri_add_reply_service_context, 3);
  DEF(cSRInfo, "sending_exception", sri_sending_exception, 0);
  DEF(cSRInfo, "sending_exception_id", sri_sending_exception_id, 0);
  DEF(cSRInfo, "object_id", sri_object_id, 0);
  DEF(cSRInfo, "adapter_id", sri_adapter_id, 0);
  DEF(cSRInfo, "adapter_name", sri_adapter_name, 0);
  DEF(cSRInfo, "orb_id", sri_orb_id, 0);
  DEF(cSRInfo, "server_id", sri_server_id, 0);
  DEF(cSRInfo, "target_most_derived_interface", sri_target_most_derived_interface, 0);
  DEF(cSRInfo, "target_is_a", sri_target_is_a, 1);
  DEF(cSRInfo, "get_server_policy", sri_get_server_policy, 1);
#undef RI_DEFS

  cIORInfo = pi_class("IORInfo");
  DEF(cIORInfo, "get_effective_policy", iori_get_effective_policy, 1);
  DEF(cIORInfo, "adapter_name", iori_adapter_name, 0);
  DEF(cIORInfo, "add_ior_component", iori_add_ior_component, 2);
  DEF(cIORInfo, "add_ior_component_to_profile", iori_add_ior_component_to_profile, 3);

  cInitInfo = pi_class("ORBInitInfo");
  DEF(cInitInfo, "arguments", initi_arguments, 0);
  DEF(cInitInfo, "orb_id", initi_orb_id, 0);
  DEF(cInitInfo, "orb", initi_orb, 0);
  DEF(cInitInfo, "register_initial_reference", initi_register_initial_reference, 2);
  DEF(cInitInfo, "resolve_initial_references", initi_resolve_initial_references, 1);
  DEF(cInitInfo, "add_client_request_interceptor", initi_add_client_request_interceptor, 2);
  DEF(cInitInfo, "add_server_request_interceptor", initi_add_server_request_interceptor, 2);
  DEF(cInitInfo, "add_ior_interceptor", initi_add_ior_interceptor, 2);
  DEF(cInitInfo, "allocate_slot_id", initi_allocate_slot_id, 0);
  DEF(cInitInfo, "pi_current", initi_pi_current, 0);
  DEF(cInitInfo, "create_codec", initi_create_codec, 3);

  cPICurrent = pi_class("PICurrent");
  DEF(cPICurrent, "get_slot", picur_get_slot, 1);
  DEF(cPICurrent, "set_slot", picur_set_slot, 2);

  cCodec = pi_class("Codec");
  DEF(cCodec, "encode", codec_encode, 2);
  DEF(cCodec, "decode", codec_decode, 2);
}

/* Rustorb::Native.upcall_thread: an Integer that names the native thread
   rustorb runs the current upcall on (the thread waiting for this
   dispatcher), or this thread outside an upcall; main_thread: that of the
   thread that loaded the extension. For probes of the thread model. */
static pthread_t main_pthread;
static void* self_thread(void* out) {
  *(pthread_t*)out = pthread_self();
  return NULL;
}
static VALUE native_upcall_thread(VALUE self) {
  pthread_t t;
  (void)self;
  at_home(self_thread, &t);
  return ULL2NUM((unsigned long long)(uintptr_t)t);
}
static VALUE native_main_thread(VALUE self) {
  (void)self;
  return ULL2NUM((unsigned long long)(uintptr_t)main_pthread);
}

static VALUE handle_class(const char* name) {
  VALUE k = rb_define_class_under(mNative, name, rb_cObject);
  rb_undef_alloc_func(k);
  return k;
}

void Init_rustorb_ext(void) {
  VALUE registry;
  id_call = rb_intern("call");
  id_system_exception = rb_intern("system_exception");
  id_local_exception = rb_intern("local_exception");
  id_dispatch = rb_intern("_rustorb_dispatch");
  id_spawn_dispatcher = rb_intern("spawn_dispatcher");
  id_peer_ivar = rb_intern("@__rustorb_peer");

  mRustorb = rb_define_module("Rustorb");
  mNative = rb_define_module_under(mRustorb, "Native");
  rb_define_const(mNative, "ABI_VERSION", INT2FIX(RORB_ABI_VERSION));

  /* Not NULL: the GC calls the mark function of data only when it has a
     pointer. */
  registry = TypedData_Wrap_Struct(rb_cObject, &registry_type, &peers);
  rb_gc_register_mark_object(registry);
  rb_gc_register_mark_object(TypedData_Wrap_Struct(rb_cObject, &hosts_type, host_buckets));
  rorb_set_value_type_resolver(resolve_value_type, NULL);
  rb_define_module_function(mNative, "kept_values", native_kept_values, 0);
  rb_define_module_function(mNative, "cdr_encode", native_cdr_encode, 2);
  rb_define_module_function(mNative, "cdr_decode", native_cdr_decode, 3);
  rb_set_end_proc(at_exit_dispatchers, Qnil);

  SDEF(mNative, "orb_init", native_orb_init, 1);
  SDEF(mNative, "default_orb", native_default_orb, 0);
  SDEF(mNative, "default_root_poa", native_default_root_poa, 0);
  SDEF(mNative, "set_thread_call_timeout", native_set_thread_call_timeout, 1);
  SDEF(mNative, "thread_call_timeout", native_thread_call_timeout, 0);
  SDEF(mNative, "dispatcher_loop", native_dispatcher_loop, 0);
  SDEF(mNative, "dispatchers", native_dispatchers, 0);
  SDEF(mNative, "servant_ref", native_servant_ref, 2);
  SDEF(mNative, "current_poa", native_current_poa, 0);
  SDEF(mNative, "current_object_id", native_current_object_id, 0);
  SDEF(mNative, "current_reference", native_current_reference, 0);
  SDEF(mNative, "current_servant", native_current_servant, 0);
  SDEF(mNative, "longdouble_from_double", native_longdouble_from_double, 1);
  SDEF(mNative, "longdouble_to_double", native_longdouble_to_double, 2);

  cORB = handle_class("ORB");
  DEF(cORB, "string_to_object", orb_string_to_object, 1);
  DEF(cORB, "object_to_string", orb_object_to_string, 1);
  DEF(cORB, "resolve_initial_references", orb_resolve, 1);
  DEF(cORB, "list_initial_services", orb_list_initial_services, 0);
  DEF(cORB, "root_poa", orb_root_poa, 0);
  DEF(cORB, "run", orb_run, 0);
  DEF(cORB, "shutdown", orb_shutdown, 1);
  DEF(cORB, "destroy", orb_destroy, 0);
  DEF(cORB, "work_pending", orb_work_pending, 0);
  DEF(cORB, "perform_work", orb_perform_work, 0);
  DEF(cORB, "set_call_timeout", orb_set_call_timeout, 1);
  DEF(cORB, "call_timeout", orb_call_timeout, 0);
  DEF(cORB, "set_connect_timeout", orb_set_connect_timeout, 1);
  DEF(cORB, "register_initial_reference", orb_register_initial_reference, 2);

  cRef = handle_class("Ref");
  DEF(cRef, "is_a", ref_is_a, 1);
  DEF(cRef, "non_existent", ref_non_existent, 0);
  DEF(cRef, "validate_connection", ref_validate_connection, 0);
  DEF(cRef, "equivalent", ref_equivalent, 1);
  DEF(cRef, "hash_value", ref_hash, 1);
  DEF(cRef, "type_id", ref_type_id, 0);
  DEF(cRef, "orb", ref_orb, 0);
  DEF(cRef, "dup", ref_dup, 0);
  DEF(cRef, "set_call_timeout", ref_set_call_timeout, 1);
  DEF(cRef, "call_timeout", ref_call_timeout, 0);
  DEF(cRef, "invoke", ref_invoke, 6);
  DEF(cRef, "sendc", ref_sendc, 6);

  cOut = handle_class("Out");
  DEF(cOut, "write_boolean", out_write_boolean, 1);
  DEF(cOut, "write_octet", out_write_octet, 1);
  DEF(cOut, "write_char", out_write_char, 1);
  DEF(cOut, "write_wchar", out_write_wchar, 1);
  DEF(cOut, "write_short", out_write_short, 1);
  DEF(cOut, "write_ushort", out_write_ushort, 1);
  DEF(cOut, "write_long", out_write_long, 1);
  DEF(cOut, "write_ulong", out_write_ulong, 1);
  DEF(cOut, "write_longlong", out_write_longlong, 1);
  DEF(cOut, "write_ulonglong", out_write_ulonglong, 1);
  DEF(cOut, "write_float", out_write_float, 1);
  DEF(cOut, "write_double", out_write_double, 1);
  DEF(cOut, "write_string", out_write_string, 1);
  DEF(cOut, "write_wstring", out_write_wstring, 1);
  DEF(cOut, "write_object", out_write_object, 1);
  DEF(cOut, "write_typecode", out_write_typecode, 1);
  DEF(cOut, "write_any", out_write_any, 1);
  DEF(cOut, "write_any_value", out_write_any_value, 1);
  DEF(cOut, "write_longdouble", out_write_longdouble, 2);
  DEF(cOut, "write_fixed", out_write_fixed, 2);
  DEF(cOut, "write_array", out_write_array, 2);
  DEF(cOut, "failed?", out_failed, 0);
  DEF(cOut, "keeps_hosts?", out_keeps_hosts, 0);
  DEF(cOut, "write_null_value", out_write_null_value, 0);
  DEF(cOut, "begin_value", out_begin_value, 3);
  DEF(cOut, "end_value", out_end_value, 0);

  cIn = handle_class("In");
  DEF(cIn, "read_boolean", in_read_boolean, 0);
  DEF(cIn, "read_octet", in_read_octet, 0);
  DEF(cIn, "read_char", in_read_char, 0);
  DEF(cIn, "read_wchar", in_read_wchar, 0);
  DEF(cIn, "read_short", in_read_short, 0);
  DEF(cIn, "read_ushort", in_read_ushort, 0);
  DEF(cIn, "read_long", in_read_long, 0);
  DEF(cIn, "read_ulong", in_read_ulong, 0);
  DEF(cIn, "read_longlong", in_read_longlong, 0);
  DEF(cIn, "read_ulonglong", in_read_ulonglong, 0);
  DEF(cIn, "read_float", in_read_float, 0);
  DEF(cIn, "read_double", in_read_double, 0);
  DEF(cIn, "read_longdouble", in_read_longdouble, 0);
  DEF(cIn, "read_string", in_read_string, 0);
  DEF(cIn, "read_wstring", in_read_wstring, 0);
  DEF(cIn, "read_object", in_read_object, 0);
  DEF(cIn, "read_typecode", in_read_typecode, 0);
  DEF(cIn, "read_any", in_read_any, 0);
  DEF(cIn, "read_any_value", in_read_any_value, 1);
  DEF(cIn, "read_array", in_read_array, 2);
  DEF(cIn, "read_fixed", in_read_fixed, 1);
  DEF(cIn, "read_length", in_read_length, 1);
  DEF(cIn, "fail", in_fail, 0);
  DEF(cIn, "failed?", in_failed, 0);
  DEF(cIn, "read_value_header", in_read_value_header, 0);
  DEF(cIn, "value_at", in_value_at, 1);
  DEF(cIn, "keep_value", in_keep_value, 2);
  DEF(cIn, "end_value", in_end_value, 0);
  (void)rel_in;

  cTC = handle_class("TC");
  SDEF(cTC, "simple", tc_simple, 1);
  SDEF(cTC, "string", tc_string, 2);
  SDEF(cTC, "fixed", tc_fixed, 2);
  SDEF(cTC, "sequence", tc_sequence, 2);
  SDEF(cTC, "array", tc_array, 2);
  SDEF(cTC, "alias", tc_alias, 4);
  SDEF(cTC, "interface", tc_interface, 3);
  SDEF(cTC, "struct", tc_struct, 5);
  SDEF(cTC, "value", tc_value, -1);
  SDEF(cTC, "union", tc_union, 7);
  SDEF(cTC, "enum", tc_enum, 3);
  SDEF(cTC, "recursive", tc_recursive, 1);
  DEF(cTC, "kind", tc_kind, 0);
  DEF(cTC, "id", tc_id, 0);
  DEF(cTC, "name", tc_name, 0);
  DEF(cTC, "member_count", tc_member_count, 0);
  DEF(cTC, "member_name", tc_member_name, 1);
  DEF(cTC, "member_type", tc_member_type, 1);
  DEF(cTC, "member_label", tc_member_label, 1);
  DEF(cTC, "discriminator_type", tc_discriminator_type, 0);
  DEF(cTC, "default_index", tc_default_index, 0);
  DEF(cTC, "length", tc_length, 0);
  DEF(cTC, "content_type", tc_content_type, 0);
  DEF(cTC, "fixed_digits", tc_fixed_digits, 0);
  DEF(cTC, "type_modifier", tc_type_modifier, 0);
  DEF(cTC, "member_visibility", tc_member_visibility, 1);
  DEF(cTC, "concrete_base_type", tc_concrete_base_type, 0);
  DEF(cTC, "fixed_scale", tc_fixed_scale, 0);
  DEF(cTC, "equal", tc_equal, 1);
  DEF(cTC, "equivalent", tc_equivalent, 1);
  DEF(cTC, "unaliased", tc_unaliased, 0);
  DEF(cTC, "compact", tc_compact, 0);

  cAny = handle_class("Any");
  SDEF(cAny, "empty", any_new, 0);
  SDEF(cAny, "insert", any_insert, 2);
  SDEF(cAny, "typed", any_typed, 1);
  DEF(cAny, "extract", any_extract, 1);
  DEF(cAny, "type", any_type, 0);
  DEF(cAny, "set_type", any_set_type, 1);
  DEF(cAny, "equal", any_equal, 1);
  DEF(cAny, "label", any_label, 0);
  DEF(cAny, "dup", any_dup, 0);

  cPeer = handle_class("Peer");
  cServantRef = handle_class("ServantRef");
  DEF(cServantRef, "current_reference", servant_ref_current_reference, 0);
  cReply = handle_class("Reply");
  DEF(cReply, "output", reply_output, 0);
  DEF(cReply, "user_exception", reply_user_exception, 1);
  DEF(cReply, "system_exception", reply_system_exception, 3);

  cPOA = handle_class("POA");
  DEF(cPOA, "name", poa_name, 0);
  DEF(cPOA, "parent", poa_parent, 0);
  DEF(cPOA, "children", poa_children, 0);
  DEF(cPOA, "manager", poa_manager, 0);
  DEF(cPOA, "policies", poa_policies, 0);
  DEF(cPOA, "create_poa", poa_create_poa, 3);
  DEF(cPOA, "find_poa", poa_find_poa, 2);
  DEF(cPOA, "destroy", poa_destroy, 2);
  DEF(cPOA, "activate_object", poa_activate_object, 1);
  DEF(cPOA, "activate_object_with_id", poa_activate_object_with_id, 2);
  DEF(cPOA, "deactivate_object", poa_deactivate_object, 1);
  DEF(cPOA, "servant_to_id", poa_servant_to_id, 1);
  DEF(cPOA, "servant_to_reference", poa_servant_to_reference, 1);
  DEF(cPOA, "id_to_servant", poa_id_to_servant, 1);
  DEF(cPOA, "id_to_reference", poa_id_to_reference, 1);
  DEF(cPOA, "reference_to_id", poa_reference_to_id, 1);
  DEF(cPOA, "reference_to_servant", poa_reference_to_servant, 1);
  DEF(cPOA, "create_reference", poa_create_reference, 1);
  DEF(cPOA, "create_reference_with_id", poa_create_reference_with_id, 2);
  DEF(cPOA, "get_servant", poa_get_servant, 0);
  DEF(cPOA, "set_servant", poa_set_servant, 1);

  cPOAManager = handle_class("POAManager");
  DEF(cPOAManager, "activate", mgr_activate, 0);
  DEF(cPOAManager, "hold_requests", mgr_hold, 1);
  DEF(cPOAManager, "discard_requests", mgr_discard, 1);
  DEF(cPOAManager, "deactivate", mgr_deactivate, 2);
  DEF(cPOAManager, "state", mgr_state, 0);
  DEF(cPOAManager, "id", mgr_id, 0);
  DEF(cPOAManager, "equal", mgr_equal, 1);

  init_dynany();
  init_dii();
  init_managers();
  init_pi();
  main_pthread = pthread_self();
  rb_define_singleton_method(mNative, "upcall_thread", RUBY_METHOD_FUNC(native_upcall_thread), 0);
  rb_define_singleton_method(mNative, "main_thread", RUBY_METHOD_FUNC(native_main_thread), 0);
}

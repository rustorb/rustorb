/* The InteropTest::Echo servant of the C server (rustorbc).
 *
 * Licensed under MIT OR Apache-2.0, like the rest of the rustorb core. */

#include "echo_impl.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "posix_compat.h"

typedef struct EchoImpl {
  POA_InteropTest_Echo base;
  CORBA_ORB orb;
  pthread_mutex_t lock;
  CORBA_long counter;
  CORBA_char** posts;
  CORBA_unsigned_long post_count, post_cap;
  InteropTest_Callback callback;
} EchoImpl;

#define SELF ((EchoImpl*)servant)

/* --- basic types ---------------------------------------------------------- */

#define ECHO_BASIC(op, T)                                                        \
  static T op(PortableServer_Servant servant, T v, CORBA_Environment* ev) {      \
    (void)servant;                                                               \
    (void)ev;                                                                    \
    return v;                                                                    \
  }
ECHO_BASIC(echo_boolean, CORBA_boolean)
ECHO_BASIC(echo_octet, CORBA_octet)
ECHO_BASIC(echo_char, CORBA_char)
ECHO_BASIC(echo_wchar, CORBA_wchar)
ECHO_BASIC(echo_short, CORBA_short)
ECHO_BASIC(echo_ushort, CORBA_unsigned_short)
ECHO_BASIC(echo_long, CORBA_long)
ECHO_BASIC(echo_ulong, CORBA_unsigned_long)
ECHO_BASIC(echo_longlong, CORBA_long_long)
ECHO_BASIC(echo_ulonglong, CORBA_unsigned_long_long)
ECHO_BASIC(echo_float, CORBA_float)
ECHO_BASIC(echo_double, CORBA_double)
ECHO_BASIC(echo_color, InteropTest_Color)

static CORBA_char* echo_string(PortableServer_Servant servant, const CORBA_char* v,
                               CORBA_Environment* ev) {
  (void)servant;
  (void)ev;
  return CORBA_string_dup(v);
}

static CORBA_wchar* echo_wstring(PortableServer_Servant servant, const CORBA_wchar* v,
                                 CORBA_Environment* ev) {
  (void)servant;
  (void)ev;
  return CORBA_wstring_dup(v);
}

/* --- constructed types --------------------------------------------------------- */

static InteropTest_Point echo_point(PortableServer_Servant servant, const InteropTest_Point* v,
                                    CORBA_Environment* ev) {
  (void)servant;
  (void)ev;
  return *v;
}

/* The variable-length results are deep copies of the arguments, which the
   ORB frees after the call. */
#define ECHO_COPY(op, T, desc)                                                   \
  static T* op(PortableServer_Servant servant, const T* v, CORBA_Environment* ev) { \
    (void)servant;                                                               \
    (void)ev;                                                                    \
    return (T*)rustorb_c_copy(&desc, v);                                         \
  }
ECHO_COPY(echo_record, InteropTest_Record, InteropTest_Record__desc)
ECHO_COPY(echo_records, InteropTest_RecordSeq, InteropTest_RecordSeq__desc)
ECHO_COPY(echo_value, InteropTest_Value, InteropTest_Value__desc)
ECHO_COPY(echo_shade, InteropTest_Shade, InteropTest_Shade__desc)
ECHO_COPY(echo_long_seq, InteropTest_LongSeq, InteropTest_LongSeq__desc)
ECHO_COPY(echo_string_seq, InteropTest_StringSeq, InteropTest_StringSeq__desc)
ECHO_COPY(echo_octets, InteropTest_OctetSeq, InteropTest_OctetSeq__desc)

static InteropTest_LongArray_slice* echo_long_array(PortableServer_Servant servant,
                                                    const InteropTest_LongArray_slice* v,
                                                    CORBA_Environment* ev) {
  InteropTest_LongArray_slice* r = InteropTest_LongArray__alloc();
  (void)servant;
  (void)ev;
  if (r) memcpy(r, v, sizeof(InteropTest_LongArray));
  return r;
}

static InteropTest_Matrix_slice* echo_matrix(PortableServer_Servant servant,
                                             const InteropTest_Matrix_slice* v,
                                             CORBA_Environment* ev) {
  InteropTest_Matrix_slice* r = InteropTest_Matrix__alloc();
  (void)servant;
  (void)ev;
  if (r) memcpy(r, v, sizeof(InteropTest_Matrix));
  return r;
}

/* --- parameter modes ------------------------------------------------------------- */

static CORBA_long swap(PortableServer_Servant servant, CORBA_long* a, CORBA_long* b,
                       CORBA_Environment* ev) {
  CORBA_long sum = *a + *b, t = *a;
  (void)servant;
  (void)ev;
  *a = *b;
  *b = t;
  return sum;
}

static CORBA_long divide(PortableServer_Servant servant, CORBA_long a, CORBA_long b,
                         CORBA_long* remainder, CORBA_Environment* ev) {
  (void)servant;
  if (b == 0) {
    CORBA_exception_set(ev, CORBA_USER_EXCEPTION, ex_InteropTest_DivideByZero, NULL);
    return 0;
  }
  *remainder = a % b;
  return a / b;
}

static void make(PortableServer_Servant servant, CORBA_long x, InteropTest_Point* p,
                 InteropTest_Record** r, CORBA_Environment* ev) {
  char name[32];
  InteropTest_Record* rec = InteropTest_Record__alloc();
  (void)servant;
  (void)ev;
  p->x = x;
  p->y = -x;
  *r = rec;
  if (!rec) return;
  snprintf(name, sizeof name, "r%ld", (long)x);
  rec->id = x;
  rec->name = CORBA_string_dup(name);
  rec->hue = InteropTest_BLUE;
  rec->pos = *p;
  rec->values._buffer = CORBA_sequence_long_allocbuf(2);
  rec->values._buffer[0] = x;
  rec->values._buffer[1] = x + 1;
  rec->values._length = rec->values._maximum = 2;
  rec->values._release = CORBA_TRUE;
  rec->active = CORBA_TRUE;
}

/* --- exceptions -------------------------------------------------------------------- */

static void fail(PortableServer_Servant servant, CORBA_long code, const CORBA_char* reason,
                 CORBA_Environment* ev) {
  InteropTest_Oops* e = InteropTest_Oops__alloc();
  (void)servant;
  e->code = code;
  e->reason = CORBA_string_dup(reason);
  CORBA_exception_set(ev, CORBA_USER_EXCEPTION, ex_InteropTest_Oops, e);
}

static void fail_system(PortableServer_Servant servant, CORBA_Environment* ev) {
  (void)servant;
  CORBA_exception_set_system(ev, ex_CORBA_BAD_PARAM, 42, CORBA_COMPLETED_NO);
}

/* --- attributes ------------------------------------------------------------------------ */

static CORBA_long get_counter(PortableServer_Servant servant, CORBA_Environment* ev) {
  CORBA_long v;
  (void)ev;
  pthread_mutex_lock(&SELF->lock);
  v = SELF->counter;
  pthread_mutex_unlock(&SELF->lock);
  return v;
}

static void set_counter(PortableServer_Servant servant, CORBA_long value, CORBA_Environment* ev) {
  (void)ev;
  pthread_mutex_lock(&SELF->lock);
  SELF->counter = value;
  pthread_mutex_unlock(&SELF->lock);
}

static CORBA_char* get_name(PortableServer_Servant servant, CORBA_Environment* ev) {
  (void)servant;
  (void)ev;
  return CORBA_string_dup("rustorb-interop-echo");
}

/* --- oneway ------------------------------------------------------------------------------- */

static void post(PortableServer_Servant servant, const CORBA_char* message, CORBA_Environment* ev) {
  (void)ev;
  pthread_mutex_lock(&SELF->lock);
  if (SELF->post_count == SELF->post_cap) {
    CORBA_unsigned_long cap = SELF->post_cap ? SELF->post_cap * 2 : 16;
    CORBA_char** p = (CORBA_char**)realloc(SELF->posts, cap * sizeof *p);
    if (p) {
      SELF->posts = p;
      SELF->post_cap = cap;
    }
  }
  if (SELF->post_count < SELF->post_cap) SELF->posts[SELF->post_count++] = CORBA_string_dup(message);
  pthread_mutex_unlock(&SELF->lock);
}

static InteropTest_StringSeq* posted(PortableServer_Servant servant, CORBA_Environment* ev) {
  InteropTest_StringSeq* r = InteropTest_StringSeq__alloc();
  CORBA_unsigned_long i;
  (void)ev;
  pthread_mutex_lock(&SELF->lock);
  r->_buffer = CORBA_sequence_string_allocbuf(SELF->post_count);
  r->_length = r->_maximum = SELF->post_count;
  r->_release = CORBA_TRUE;
  for (i = 0; i < SELF->post_count; i++) r->_buffer[i] = SELF->posts[i];
  SELF->post_count = 0;
  pthread_mutex_unlock(&SELF->lock);
  return r;
}

/* --- object references ------------------------------------------------------------------------ */

static InteropTest_Echo self_ref(PortableServer_Servant servant, CORBA_Environment* ev) {
  EchoImpl* e = (EchoImpl*)servant;
  PortableServer_Current current;
  CORBA_Object r = PortableServer_ServantBase__this(servant, ev);
  if (r || ev->_major != CORBA_NO_EXCEPTION) return r;
  /* Run by a DSI servant (rustorbc-dii), the request is that servant's:
     the target from the POA Current. */
  current = CORBA_ORB_resolve_initial_references(e->orb, "POACurrent", ev);
  if (ev->_major != CORBA_NO_EXCEPTION) return CORBA_OBJECT_NIL;
  r = PortableServer_Current_get_reference(current, ev);
  CORBA_Object_release(current, ev);
  return r;
}

static InteropTest_Echo echo_ref(PortableServer_Servant servant, InteropTest_Echo other,
                                 CORBA_Environment* ev) {
  if (!CORBA_Object_is_nil(other, ev)) return CORBA_Object_duplicate(other, ev);
  return self_ref(servant, ev);
}

static CORBA_boolean is_nil(PortableServer_Servant servant, InteropTest_Echo other,
                            CORBA_Environment* ev) {
  (void)servant;
  return CORBA_Object_is_nil(other, ev);
}

/* --- callbacks ------------------------------------------------------------------------------------ */

static CORBA_char* call_back(PortableServer_Servant servant, InteropTest_Callback cb,
                             const CORBA_char* message, CORBA_Environment* ev) {
  (void)servant;
  return InteropTest_Callback_notify(cb, message, ev);
}

static void register_callback(PortableServer_Servant servant, InteropTest_Callback cb,
                              CORBA_Environment* ev) {
  InteropTest_Callback old;
  InteropTest_Callback dup = CORBA_Object_duplicate(cb, ev);
  pthread_mutex_lock(&SELF->lock);
  old = SELF->callback;
  SELF->callback = dup;
  pthread_mutex_unlock(&SELF->lock);
  CORBA_Object_release(old, ev);
}

static CORBA_char* trigger(PortableServer_Servant servant, const CORBA_char* message,
                           CORBA_Environment* ev) {
  InteropTest_Callback cb;
  CORBA_char* r;
  pthread_mutex_lock(&SELF->lock);
  cb = CORBA_Object_duplicate(SELF->callback, ev);
  pthread_mutex_unlock(&SELF->lock);
  if (CORBA_Object_is_nil(cb, ev)) {
    InteropTest_Oops* e = InteropTest_Oops__alloc();
    e->code = 1;
    e->reason = CORBA_string_dup("no callback");
    CORBA_exception_set(ev, CORBA_USER_EXCEPTION, ex_InteropTest_Oops, e);
    return NULL;
  }
  r = InteropTest_Callback_notify(cb, message, ev);
  {
    CORBA_Environment ev2 = {CORBA_NO_EXCEPTION, NULL, NULL};
    CORBA_Object_release(cb, &ev2);
  }
  return r;
}

/* --- lifecycle --------------------------------------------------------------------------------- */

static void shutdown_orb(PortableServer_Servant servant, CORBA_Environment* ev) {
  CORBA_ORB_shutdown(SELF->orb, CORBA_FALSE, ev);
}

static PortableServer_ServantBase__epv base_epv = {NULL, NULL, NULL};
static POA_InteropTest_Echo__epv echo_epv = {
    NULL,
    echo_boolean,
    echo_octet,
    echo_char,
    echo_wchar,
    echo_short,
    echo_ushort,
    echo_long,
    echo_ulong,
    echo_longlong,
    echo_ulonglong,
    echo_float,
    echo_double,
    echo_string,
    echo_wstring,
    echo_color,
    echo_point,
    echo_record,
    echo_records,
    echo_value,
    echo_shade,
    echo_long_array,
    echo_matrix,
    echo_long_seq,
    echo_string_seq,
    echo_octets,
    swap,
    divide,
    make,
    fail,
    fail_system,
    get_counter,
    set_counter,
    get_name,
    post,
    posted,
    self_ref,
    echo_ref,
    is_nil,
    call_back,
    register_callback,
    trigger,
    shutdown_orb,
};
static POA_InteropTest_Echo__vepv echo_vepv = {&base_epv, &echo_epv};

PortableServer_Servant echo_impl_new(CORBA_ORB orb, CORBA_Environment* ev) {
  EchoImpl* e = (EchoImpl*)calloc(1, sizeof *e);
  if (!e) return NULL;
  e->base.vepv = &echo_vepv;
  e->orb = orb;
  pthread_mutex_init(&e->lock, NULL);
  POA_InteropTest_Echo__init(e, ev);
  return e;
}

void echo_impl_free(PortableServer_Servant servant) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  CORBA_unsigned_long i;
  POA_InteropTest_Echo__fini(servant, &ev);
  CORBA_Object_release(SELF->callback, &ev);
  for (i = 0; i < SELF->post_count; i++) CORBA_free(SELF->posts[i]);
  free(SELF->posts);
  pthread_mutex_destroy(&SELF->lock);
  free(servant);
}

/* The test cases of the C client (rustorbc).
 *
 * Licensed under MIT OR Apache-2.0, like the rest of the rustorb core. */

#define _POSIX_C_SOURCE 200809L

#include "cases.h"

#include <float.h>
#include <pthread.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

typedef const char* (*case_fn)(InteropTest_Echo echo, size_t arg);

static char why[512];

static const char* unexpected(CORBA_Environment* ev) {
  if (ev->_major == CORBA_SYSTEM_EXCEPTION) {
    CORBA_SystemException* v = (CORBA_SystemException*)CORBA_exception_value(ev);
    snprintf(why, sizeof why, "unexpected %s (minor %lu, %d)", CORBA_exception_id(ev),
             v ? (unsigned long)v->minor : 0ul, v ? (int)v->completed : -1);
  } else {
    snprintf(why, sizeof why, "unexpected %s", CORBA_exception_id(ev));
  }
  CORBA_exception_free(ev);
  return why;
}

#define CHECK(cond, msg) \
  do {                   \
    if (!(cond)) return (msg); \
  } while (0)
#define NOEXC(ev)                                                   \
  do {                                                              \
    if ((ev)->_major != CORBA_NO_EXCEPTION) return unexpected(ev);  \
  } while (0)

static void run(runner* r, const char* name, case_fn fn, InteropTest_Echo echo, size_t arg) {
  const char* failure = fn(echo, arg);
  if (!failure) {
    r->passed++;
    printf("ok   %s\n", name);
  } else {
    size_t len = strlen(r->failed_names);
    r->failed++;
    printf("FAIL %s: %s\n", name, failure);
    snprintf(r->failed_names + len, sizeof r->failed_names - len, "%s%s", len ? ", " : "", name);
  }
  fflush(stdout);
}

static void run_each(runner* r, const char* op, case_fn fn, InteropTest_Echo echo, size_t n) {
  size_t i;
  char name[128];
  for (i = 0; i < n; i++) {
    snprintf(name, sizeof name, "%s[%u]", op, (unsigned)i);
    run(r, name, fn, echo, i);
  }
}

void cases_summary(const runner* r) {
  printf("\n%d passed, %d failed\n", r->passed, r->failed);
  if (r->failed) printf("failed: %s\n", r->failed_names);
  fflush(stdout);
}

/* --- basic types -------------------------------------------------------------- */

#define N(a) (sizeof(a) / sizeof((a)[0]))

#define ECHO_BASIC(op, T, ...)                                                   \
  static const T op##_values[] = {__VA_ARGS__};                                  \
  static const char* case_##op(InteropTest_Echo echo, size_t i) {                \
    CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};                     \
    T v = op##_values[i];                                                        \
    T got = InteropTest_Echo_##op(echo, v, &ev);                                 \
    NOEXC(&ev);                                                                  \
    CHECK(memcmp(&got, &v, sizeof v) == 0 || got == v, #op " returned another value"); \
    return NULL;                                                                 \
  }

ECHO_BASIC(echo_boolean, CORBA_boolean, CORBA_TRUE, CORBA_FALSE)
ECHO_BASIC(echo_octet, CORBA_octet, 0, 0x7f, 0xff)
ECHO_BASIC(echo_char, CORBA_char, 'a', '\0', '~')
ECHO_BASIC(echo_wchar, CORBA_wchar, 'a', 0x3042, 0xe9)
ECHO_BASIC(echo_short, CORBA_short, 0, -32767 - 1, 32767)
ECHO_BASIC(echo_ushort, CORBA_unsigned_short, 0, 65535)
ECHO_BASIC(echo_long, CORBA_long, 0, (-2147483647 - 1), 2147483647, InteropTest_MAGIC)
ECHO_BASIC(echo_ulong, CORBA_unsigned_long, 0, 4294967295u)
ECHO_BASIC(echo_longlong, CORBA_long_long, 0, (-9223372036854775807LL - 1), 9223372036854775807LL)
ECHO_BASIC(echo_ulonglong, CORBA_unsigned_long_long, 0, 18446744073709551615ULL)
ECHO_BASIC(echo_double, CORBA_double, 0.0, -1.5, 1e300, 4.9406564584124654e-324)
ECHO_BASIC(echo_float, CORBA_float, 0.0f, -1.5f, FLT_MAX, 1.17549435e-38f)

static char* long_string(void) {
  static char s[10001];
  if (!s[0]) memset(s, 'x', 10000);
  return s;
}

static const char* case_echo_string(InteropTest_Echo echo, size_t i) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  const char* values[4];
  CORBA_char* got;
  int same;
  values[0] = "";
  values[1] = "hello";
  values[2] = long_string();
  values[3] = "caf\xc3\xa9";
  got = InteropTest_Echo_echo_string(echo, values[i], &ev);
  NOEXC(&ev);
  same = got && strcmp(got, values[i]) == 0;
  CORBA_free(got);
  CHECK(same, "echo_string returned another string");
  return NULL;
}

static const CORBA_wchar ws_empty[] = {0};
static const CORBA_wchar ws_hello[] = {'h', 'e', 'l', 'l', 'o', 0};
static const CORBA_wchar ws_konnichiwa[] = {0x3053, 0x3093, 0x306b, 0x3061, 0x306f, 0};
static const CORBA_wchar ws_crab[] = {0xd83e, 0xdd80, 0};

static const char* case_echo_wstring(InteropTest_Echo echo, size_t i) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  const CORBA_wchar* values[] = {ws_empty, ws_hello, ws_konnichiwa, ws_crab};
  CORBA_wchar* got = InteropTest_Echo_echo_wstring(echo, values[i], &ev);
  int same;
  NOEXC(&ev);
  same = got && CORBA_wstring_len(got) == CORBA_wstring_len(values[i]) &&
         memcmp(got, values[i], CORBA_wstring_len(got) * sizeof(CORBA_wchar)) == 0;
  CORBA_free(got);
  CHECK(same, "echo_wstring returned another string");
  return NULL;
}

/* --- constructed types ---------------------------------------------------------- */

static const InteropTest_Point the_point = {1.25, -2.5};
static CORBA_long rec_values[] = {1, -2, 3};

static int point_eq(const InteropTest_Point* a, const InteropTest_Point* b) {
  return a->x == b->x && a->y == b->y;
}

static int long_seq_eq(const CORBA_sequence_long* a, const CORBA_sequence_long* b) {
  return a->_length == b->_length &&
         (a->_length == 0 || memcmp(a->_buffer, b->_buffer, a->_length * sizeof(CORBA_long)) == 0);
}

static int record_eq(const InteropTest_Record* a, const InteropTest_Record* b) {
  return a->id == b->id && a->name && b->name && strcmp(a->name, b->name) == 0 &&
         a->hue == b->hue && point_eq(&a->pos, &b->pos) && long_seq_eq(&a->values, &b->values) &&
         !a->active == !b->active;
}

static InteropTest_Record make_record(CORBA_long id, const char* name, InteropTest_Color hue,
                                      InteropTest_Point pos, CORBA_long* values,
                                      CORBA_unsigned_long n, CORBA_boolean active) {
  InteropTest_Record r;
  r.id = id;
  r.name = (CORBA_char*)name;
  r.hue = hue;
  r.pos = pos;
  r.values._buffer = values;
  r.values._length = r.values._maximum = n;
  r.values._release = CORBA_FALSE;
  r.active = active;
  return r;
}

static const char* case_echo_color(InteropTest_Echo echo, size_t i) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  InteropTest_Color colors[] = {InteropTest_RED, InteropTest_GREEN, InteropTest_BLUE};
  (void)i;
  for (i = 0; i < 3; i++) {
    InteropTest_Color got = InteropTest_Echo_echo_color(echo, colors[i], &ev);
    NOEXC(&ev);
    CHECK(got == colors[i], "echo_color");
  }
  return NULL;
}

static const char* case_echo_point(InteropTest_Echo echo, size_t i) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  InteropTest_Point got = InteropTest_Echo_echo_point(echo, &the_point, &ev);
  (void)i;
  NOEXC(&ev);
  CHECK(point_eq(&got, &the_point), "echo_point");
  return NULL;
}

static const char* case_echo_record(InteropTest_Echo echo, size_t i) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  InteropTest_Record rec = make_record(7, "seven", InteropTest_GREEN, the_point, rec_values, 3, 1);
  InteropTest_Record* got = InteropTest_Echo_echo_record(echo, &rec, &ev);
  int same;
  (void)i;
  NOEXC(&ev);
  same = got && record_eq(got, &rec);
  CORBA_free(got);
  CHECK(same, "echo_record");
  return NULL;
}

static int records_eq(const InteropTest_RecordSeq* a, const InteropTest_RecordSeq* b) {
  CORBA_unsigned_long i;
  if (a->_length != b->_length) return 0;
  for (i = 0; i < a->_length; i++)
    if (!record_eq(&a->_buffer[i], &b->_buffer[i])) return 0;
  return 1;
}

static const char* echo_records_check(InteropTest_Echo echo, const InteropTest_RecordSeq* seq,
                                      const char* what) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  InteropTest_RecordSeq* got = InteropTest_Echo_echo_records(echo, seq, &ev);
  int same;
  NOEXC(&ev);
  same = got && records_eq(got, seq);
  CORBA_free(got);
  CHECK(same, what);
  return NULL;
}

static const char* case_echo_records(InteropTest_Echo echo, size_t i) {
  static CORBA_long none[1];
  InteropTest_Record recs[3];
  InteropTest_Point origin = {0.0, 0.0};
  InteropTest_RecordSeq seq;
  (void)i;
  recs[0] = make_record(7, "seven", InteropTest_GREEN, the_point, rec_values, 3, 1);
  recs[1] = make_record(-1, "", InteropTest_RED, origin, none, 0, 0);
  recs[2] = recs[0];
  seq._buffer = recs;
  seq._length = seq._maximum = 3;
  seq._release = CORBA_FALSE;
  return echo_records_check(echo, &seq, "echo_records");
}

static const char* case_echo_records_empty(InteropTest_Echo echo, size_t i) {
  InteropTest_RecordSeq seq = {0, 0, NULL, CORBA_FALSE};
  (void)i;
  return echo_records_check(echo, &seq, "empty");
}

/* Large enough to be sent in several fragments, with doubles after each
   fragment boundary; record i has a name of varying length so that the
   doubles land at every alignment. */
static const char* case_echo_records_large(InteropTest_Echo echo, size_t n) {
  InteropTest_Color colors[] = {InteropTest_RED, InteropTest_GREEN, InteropTest_BLUE};
  InteropTest_RecordSeq seq;
  const char* r;
  CORBA_unsigned_long i;
  (void)n;
  seq._buffer = CORBA_sequence_InteropTest_Record_allocbuf(400);
  seq._length = seq._maximum = 400;
  seq._release = CORBA_TRUE;
  for (i = 0; i < 400; i++) {
    char name[16];
    InteropTest_Record* rec = &seq._buffer[i];
    snprintf(name, sizeof name, "r%u", (unsigned)i);
    rec->id = (CORBA_long)i;
    rec->name = CORBA_string_dup(name);
    rec->hue = colors[i % 3];
    rec->pos.x = i + 0.5;
    rec->pos.y = -0.25 * i;
    rec->values._buffer = CORBA_sequence_long_allocbuf(2);
    rec->values._buffer[0] = (CORBA_long)i;
    rec->values._buffer[1] = (CORBA_long)i + 1;
    rec->values._length = rec->values._maximum = 2;
    rec->values._release = CORBA_TRUE;
    rec->active = i % 2 == 0;
  }
  r = echo_records_check(echo, &seq, "echo_records[large]");
  CORBA_free(seq._buffer);
  return r;
}

static int value_eq(const InteropTest_Value* a, const InteropTest_Value* b) {
  if (a->_d != b->_d) return 0;
  switch (a->_d) {
    case 1: return a->_u.l == b->_u.l;
    case 2: return strcmp(a->_u.s, b->_u.s) == 0;
    case 3: return point_eq(&a->_u.p, &b->_u.p);
    case 4:
    case 5: return long_seq_eq(&a->_u.seq, &b->_u.seq);
    default: return !a->_u.flag == !b->_u.flag;
  }
}

static const char* case_echo_value(InteropTest_Echo echo, size_t i) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  static CORBA_long two[] = {1, 2};
  InteropTest_Value v, *got;
  int same;
  memset(&v, 0, sizeof v);
  switch (i) {
    case 0: v._d = 1; v._u.l = -5; break;
    case 1: v._d = 2; v._u.s = (CORBA_char*)"union"; break;
    case 2: v._d = 3; v._u.p = the_point; break;
    case 3: v._d = 4; v._u.seq._buffer = two; v._u.seq._length = v._u.seq._maximum = 2; break;
    case 4: v._d = 5; break;
    case 5: v._d = 99; v._u.flag = CORBA_TRUE; break;
    default: v._d = 0; v._u.flag = CORBA_FALSE; break;
  }
  got = InteropTest_Echo_echo_value(echo, &v, &ev);
  NOEXC(&ev);
  same = got && value_eq(got, &v);
  CORBA_free(got);
  CHECK(same, "echo_value");
  return NULL;
}

static const char* case_echo_shade(InteropTest_Echo echo, size_t i) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  InteropTest_Shade s, *got;
  int same;
  memset(&s, 0, sizeof s);
  if (i == 0) {
    s._d = InteropTest_RED;
    s._u.level = 200;
  } else if (i == 1) {
    s._d = InteropTest_GREEN;
    s._u.label = (CORBA_char*)"leaf";
  } else {
    /* The implicit default: no member. */
    s._d = InteropTest_BLUE;
  }
  got = InteropTest_Echo_echo_shade(echo, &s, &ev);
  NOEXC(&ev);
  same = got && got->_d == s._d &&
         (i == 0 ? got->_u.level == 200 : i == 1 ? strcmp(got->_u.label, "leaf") == 0 : 1);
  CORBA_free(got);
  CHECK(same, "echo_shade");
  return NULL;
}

static const char* case_echo_long_array(InteropTest_Echo echo, size_t i) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  InteropTest_LongArray a = {1, -1, 2147483647, 0};
  InteropTest_LongArray_slice* got = InteropTest_Echo_echo_long_array(echo, a, &ev);
  int same;
  (void)i;
  NOEXC(&ev);
  same = got && memcmp(got, a, sizeof a) == 0;
  CORBA_free(got);
  CHECK(same, "echo_long_array");
  return NULL;
}

static const char* case_echo_matrix(InteropTest_Echo echo, size_t i) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  InteropTest_Matrix m = {{1.0, 2.0, 3.0}, {-4.0, 0.5, 1e-9}};
  InteropTest_Matrix_slice* got = InteropTest_Echo_echo_matrix(echo, (const InteropTest_Matrix_slice*)m, &ev);
  int same;
  (void)i;
  NOEXC(&ev);
  same = got && memcmp(got, m, sizeof m) == 0;
  CORBA_free(got);
  CHECK(same, "echo_matrix");
  return NULL;
}

static const char* echo_long_seq_check(InteropTest_Echo echo, CORBA_unsigned_long n) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  InteropTest_LongSeq seq, *got;
  CORBA_unsigned_long i;
  int same;
  seq._buffer = n ? CORBA_sequence_long_allocbuf(n) : NULL;
  seq._length = seq._maximum = n;
  seq._release = CORBA_TRUE;
  for (i = 0; i < n; i++) seq._buffer[i] = (CORBA_long)i - 500;
  got = InteropTest_Echo_echo_long_seq(echo, &seq, &ev);
  if (ev._major != CORBA_NO_EXCEPTION) {
    CORBA_free(seq._buffer);
    return unexpected(&ev);
  }
  same = got && long_seq_eq(got, &seq);
  CORBA_free(got);
  CORBA_free(seq._buffer);
  CHECK(same, n ? "echo_long_seq" : "empty");
  return NULL;
}

static const char* case_echo_long_seq(InteropTest_Echo echo, size_t i) {
  (void)i;
  return echo_long_seq_check(echo, 1000);
}

static const char* case_echo_long_seq_empty(InteropTest_Echo echo, size_t i) {
  (void)i;
  return echo_long_seq_check(echo, 0);
}

static const char* case_echo_string_seq(InteropTest_Echo echo, size_t i) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  CORBA_char* strings[] = {(CORBA_char*)"a", (CORBA_char*)"", (CORBA_char*)"ccc"};
  InteropTest_StringSeq seq = {3, 3, strings, CORBA_FALSE};
  InteropTest_StringSeq* got = InteropTest_Echo_echo_string_seq(echo, &seq, &ev);
  int same;
  (void)i;
  NOEXC(&ev);
  same = got && got->_length == 3;
  for (i = 0; same && i < 3; i++) same = strcmp(got->_buffer[i], strings[i]) == 0;
  CORBA_free(got);
  CHECK(same, "echo_string_seq");
  return NULL;
}

static const char* echo_octets_check(InteropTest_Echo echo, CORBA_unsigned_long n, unsigned mod,
                                     const char* what) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  InteropTest_OctetSeq seq, *got;
  CORBA_unsigned_long i;
  int same;
  seq._buffer = CORBA_sequence_octet_allocbuf(n);
  seq._length = seq._maximum = n;
  seq._release = CORBA_TRUE;
  for (i = 0; i < n; i++) seq._buffer[i] = (CORBA_octet)(i % mod);
  got = InteropTest_Echo_echo_octets(echo, &seq, &ev);
  if (ev._major != CORBA_NO_EXCEPTION) {
    CORBA_free(seq._buffer);
    return unexpected(&ev);
  }
  same = got && got->_length == n && memcmp(got->_buffer, seq._buffer, n) == 0;
  CORBA_free(got);
  CORBA_free(seq._buffer);
  CHECK(same, what);
  return NULL;
}

static const char* case_echo_octets(InteropTest_Echo echo, size_t i) {
  (void)i;
  return echo_octets_check(echo, 256, 256, "echo_octets");
}

static const char* case_echo_octets_large(InteropTest_Echo echo, size_t i) {
  (void)i;
  return echo_octets_check(echo, 300000, 251, "echo_octets[large]");
}

/* --- parameter modes ---------------------------------------------------------------- */

static const char* case_swap(InteropTest_Echo echo, size_t i) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  CORBA_long a = 3, b = -10, sum;
  (void)i;
  sum = InteropTest_Echo_swap(echo, &a, &b, &ev);
  NOEXC(&ev);
  CHECK(sum == -7, "swap result");
  CHECK(a == -10 && b == 3, "swap inout");
  return NULL;
}

static const char* case_divide(InteropTest_Echo echo, size_t i) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  CORBA_long rem = 0, q;
  int raised;
  (void)i;
  q = InteropTest_Echo_divide(echo, 17, 5, &rem, &ev);
  NOEXC(&ev);
  CHECK(q == 3 && rem == 2, "17 / 5");
  q = InteropTest_Echo_divide(echo, -17, 5, &rem, &ev);
  NOEXC(&ev);
  CHECK(q == -3 && rem == -2, "-17 / 5");
  InteropTest_Echo_divide(echo, 1, 0, &rem, &ev);
  if (ev._major == CORBA_SYSTEM_EXCEPTION) return unexpected(&ev);
  raised = ev._major == CORBA_USER_EXCEPTION &&
           strcmp(CORBA_exception_id(&ev), ex_InteropTest_DivideByZero) == 0;
  CORBA_exception_free(&ev);
  CHECK(raised, "expected exception was not raised");
  return NULL;
}

static const char* case_make(InteropTest_Echo echo, size_t i) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  InteropTest_Point pt;
  InteropTest_Point want = {12.0, -12.0};
  InteropTest_Record* rc = NULL;
  CORBA_long values[] = {12, 13};
  InteropTest_Record expect = make_record(12, "r12", InteropTest_BLUE, want, values, 2, 1);
  int p_ok, r_ok;
  (void)i;
  InteropTest_Echo_make(echo, 12, &pt, &rc, &ev);
  NOEXC(&ev);
  p_ok = point_eq(&pt, &want);
  r_ok = rc && record_eq(rc, &expect);
  CORBA_free(rc);
  CHECK(p_ok, "make p");
  CHECK(r_ok, "make r");
  return NULL;
}

/* --- exceptions ---------------------------------------------------------------------- */

static const char* case_fail(InteropTest_Echo echo, size_t i) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  InteropTest_Oops* e;
  int ok;
  (void)i;
  InteropTest_Echo_fail(echo, 7, "boom", &ev);
  if (ev._major == CORBA_NO_EXCEPTION) return "expected exception was not raised";
  if (ev._major != CORBA_USER_EXCEPTION ||
      strcmp(CORBA_exception_id(&ev), ex_InteropTest_Oops) != 0)
    return unexpected(&ev);
  e = (InteropTest_Oops*)CORBA_exception_value(&ev);
  ok = e && e->code == 7 && strcmp(e->reason, "boom") == 0;
  CORBA_exception_free(&ev);
  CHECK(ok, "Oops members");
  return NULL;
}

static const char* case_fail_system(InteropTest_Echo echo, size_t i) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  CORBA_SystemException* e;
  (void)i;
  InteropTest_Echo_fail_system(echo, &ev);
  if (ev._major == CORBA_NO_EXCEPTION) return "expected exception was not raised";
  if (ev._major != CORBA_SYSTEM_EXCEPTION ||
      strcmp(CORBA_exception_id(&ev), ex_CORBA_BAD_PARAM) != 0)
    return unexpected(&ev);
  e = (CORBA_SystemException*)CORBA_exception_value(&ev);
  snprintf(why, sizeof why, "BAD_PARAM minor %lu", (unsigned long)e->minor);
  if (e->minor != 42 || e->completed != CORBA_COMPLETED_NO) {
    CORBA_exception_free(&ev);
    return why;
  }
  CORBA_exception_free(&ev);
  return NULL;
}

/* --- attributes ------------------------------------------------------------------------ */

static const char* case_counter(InteropTest_Echo echo, size_t i) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  CORBA_long v;
  (void)i;
  InteropTest_Echo__set_counter(echo, 41, &ev);
  NOEXC(&ev);
  v = InteropTest_Echo__get_counter(echo, &ev);
  NOEXC(&ev);
  CHECK(v == 41, "41");
  InteropTest_Echo__set_counter(echo, -1, &ev);
  NOEXC(&ev);
  v = InteropTest_Echo__get_counter(echo, &ev);
  NOEXC(&ev);
  CHECK(v == -1, "-1");
  return NULL;
}

static const char* case_name(InteropTest_Echo echo, size_t i) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  CORBA_char* name = InteropTest_Echo__get_name(echo, &ev);
  int ok;
  (void)i;
  NOEXC(&ev);
  ok = name && strcmp(name, "rustorb-interop-echo") == 0;
  CORBA_free(name);
  CHECK(ok, "name");
  return NULL;
}

/* --- object references --------------------------------------------------------------------- */

static const char* case_self_ref(InteropTest_Echo echo, size_t i) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  InteropTest_Echo self = InteropTest_Echo_self_ref(echo, &ev);
  int equivalent;
  CORBA_long five;
  (void)i;
  NOEXC(&ev);
  equivalent = CORBA_Object_is_equivalent(self, echo, &ev);
  five = InteropTest_Echo_echo_long(self, 5, &ev);
  CORBA_Object_release(self, NULL);
  NOEXC(&ev);
  CHECK(equivalent, "equivalent");
  CHECK(five == 5, "callable");
  return NULL;
}

static const char* case_echo_ref(InteropTest_Echo echo, size_t i) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  InteropTest_Echo a, b;
  int ok_a, ok_b;
  (void)i;
  a = InteropTest_Echo_echo_ref(echo, echo, &ev);
  NOEXC(&ev);
  b = InteropTest_Echo_echo_ref(echo, CORBA_OBJECT_NIL, &ev);
  if (ev._major != CORBA_NO_EXCEPTION) {
    CORBA_Object_release(a, NULL);
    return unexpected(&ev);
  }
  ok_a = CORBA_Object_is_equivalent(a, echo, &ev);
  ok_b = CORBA_Object_is_equivalent(b, echo, &ev);
  CORBA_Object_release(a, &ev);
  CORBA_Object_release(b, &ev);
  CHECK(ok_a, "echo_ref(echo)");
  CHECK(ok_b, "echo_ref(nil)");
  return NULL;
}

static const char* case_is_nil(InteropTest_Echo echo, size_t i) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  CORBA_boolean nil, non_nil;
  (void)i;
  nil = InteropTest_Echo_is_nil(echo, CORBA_OBJECT_NIL, &ev);
  NOEXC(&ev);
  non_nil = InteropTest_Echo_is_nil(echo, echo, &ev);
  NOEXC(&ev);
  CHECK(nil, "nil");
  CHECK(!non_nil, "non-nil");
  return NULL;
}

/* --- callbacks -------------------------------------------------------------------------------- */

typedef struct CallbackImpl {
  POA_InteropTest_Callback base;
  pthread_mutex_t lock;
  char last[64];
  int received;
} CallbackImpl;

static CORBA_char* notify_impl(PortableServer_Servant servant, const CORBA_char* message,
                               CORBA_Environment* ev) {
  CallbackImpl* cb = (CallbackImpl*)servant;
  size_t len = strlen(message);
  CORBA_char* r = CORBA_string_alloc((CORBA_unsigned_long)len + 4);
  (void)ev;
  pthread_mutex_lock(&cb->lock);
  snprintf(cb->last, sizeof cb->last, "%s", message);
  cb->received++;
  pthread_mutex_unlock(&cb->lock);
  memcpy(r, "ack:", 4);
  memcpy(r + 4, message, len + 1);
  return r;
}

static PortableServer_ServantBase__epv cb_base_epv = {NULL, NULL, NULL};
static POA_InteropTest_Callback__epv cb_epv = {NULL, notify_impl};
static POA_InteropTest_Callback__vepv cb_vepv = {&cb_base_epv, &cb_epv};
static CallbackImpl cb_servant;
static InteropTest_Callback the_cb;

static const char* case_call_back(InteropTest_Echo echo, size_t i) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  CORBA_char* r = InteropTest_Echo_call_back(echo, the_cb, "hi", &ev);
  int ok;
  (void)i;
  NOEXC(&ev);
  ok = r && strcmp(r, "ack:hi") == 0;
  CORBA_free(r);
  CHECK(ok, "call_back");
  return NULL;
}

static const char* case_trigger(InteropTest_Echo echo, size_t i) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  CORBA_char* r;
  int ok, received;
  (void)i;
  InteropTest_Echo_register_callback(echo, the_cb, &ev);
  NOEXC(&ev);
  r = InteropTest_Echo_trigger(echo, "later", &ev);
  NOEXC(&ev);
  ok = r && strcmp(r, "ack:later") == 0;
  CORBA_free(r);
  CHECK(ok, "trigger");
  pthread_mutex_lock(&cb_servant.lock);
  received = cb_servant.received > 0 && strcmp(cb_servant.last, "later") == 0;
  pthread_mutex_unlock(&cb_servant.lock);
  CHECK(received, "received");
  return NULL;
}

/* --- oneway -------------------------------------------------------------------------------------- */

static int cmp_str(const void* a, const void* b) {
  return strcmp(*(const char* const*)a, *(const char* const*)b);
}

static double now(void) {
  struct timespec ts;
  clock_gettime(CLOCK_MONOTONIC, &ts);
  return ts.tv_sec + ts.tv_nsec / 1e9;
}

static const char* case_post(InteropTest_Echo echo, size_t n) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  const char* sent[] = {"one", "three", "two"};
  char* got[16];
  size_t count = 0, i;
  double start;
  int ok;
  InteropTest_StringSeq* s;
  (void)n;
  s = InteropTest_Echo_posted(echo, &ev);
  NOEXC(&ev);
  CORBA_free(s);
  InteropTest_Echo_post(echo, "one", &ev);
  InteropTest_Echo_post(echo, "two", &ev);
  InteropTest_Echo_post(echo, "three", &ev);
  NOEXC(&ev);
  start = now();
  while (count < 3 && now() - start < 5.0) {
    struct timespec ts = {0, 50 * 1000 * 1000};
    s = InteropTest_Echo_posted(echo, &ev);
    NOEXC(&ev);
    for (i = 0; i < s->_length && count < 16; i++) got[count++] = CORBA_string_dup(s->_buffer[i]);
    CORBA_free(s);
    if (count < 3) nanosleep(&ts, NULL);
  }
  /* Oneway requests may be dispatched concurrently, so order is not
     checked. */
  qsort(got, count, sizeof got[0], cmp_str);
  ok = count == 3;
  for (i = 0; i < count; i++) {
    if (ok && strcmp(got[i], sent[i]) != 0) ok = 0;
    CORBA_free(got[i]);
  }
  snprintf(why, sizeof why, "posted %u messages", (unsigned)count);
  return ok ? NULL : why;
}

void cases_run(runner* r, InteropTest_Echo echo, PortableServer_POA poa) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  PortableServer_ObjectId* oid;

  /* --- basic types --- */
  run_each(r, "echo_boolean", case_echo_boolean, echo, N(echo_boolean_values));
  run_each(r, "echo_octet", case_echo_octet, echo, N(echo_octet_values));
  run_each(r, "echo_char", case_echo_char, echo, N(echo_char_values));
  run_each(r, "echo_wchar", case_echo_wchar, echo, N(echo_wchar_values));
  run_each(r, "echo_short", case_echo_short, echo, N(echo_short_values));
  run_each(r, "echo_ushort", case_echo_ushort, echo, N(echo_ushort_values));
  run_each(r, "echo_long", case_echo_long, echo, N(echo_long_values));
  run_each(r, "echo_ulong", case_echo_ulong, echo, N(echo_ulong_values));
  run_each(r, "echo_longlong", case_echo_longlong, echo, N(echo_longlong_values));
  run_each(r, "echo_ulonglong", case_echo_ulonglong, echo, N(echo_ulonglong_values));
  run_each(r, "echo_double", case_echo_double, echo, N(echo_double_values));
  run_each(r, "echo_string", case_echo_string, echo, 4);
  run_each(r, "echo_wstring", case_echo_wstring, echo, 4);
  run_each(r, "echo_float", case_echo_float, echo, N(echo_float_values));

  /* --- constructed types --- */
  run(r, "echo_color", case_echo_color, echo, 0);
  run(r, "echo_point", case_echo_point, echo, 0);
  run(r, "echo_record", case_echo_record, echo, 0);
  run(r, "echo_records", case_echo_records, echo, 0);
  run(r, "echo_records[empty]", case_echo_records_empty, echo, 0);
  run(r, "echo_records[large]", case_echo_records_large, echo, 0);
  run_each(r, "echo_value", case_echo_value, echo, 7);
  run_each(r, "echo_shade", case_echo_shade, echo, 3);
  run(r, "echo_long_array", case_echo_long_array, echo, 0);
  run(r, "echo_matrix", case_echo_matrix, echo, 0);
  run(r, "echo_long_seq", case_echo_long_seq, echo, 0);
  run(r, "echo_long_seq[empty]", case_echo_long_seq_empty, echo, 0);
  run(r, "echo_string_seq", case_echo_string_seq, echo, 0);
  run(r, "echo_octets", case_echo_octets, echo, 0);
  run(r, "echo_octets[large]", case_echo_octets_large, echo, 0);

  /* --- parameter modes --- */
  run(r, "swap", case_swap, echo, 0);
  run(r, "divide", case_divide, echo, 0);
  run(r, "make", case_make, echo, 0);

  /* --- exceptions --- */
  run(r, "fail", case_fail, echo, 0);
  run(r, "fail_system", case_fail_system, echo, 0);

  /* --- attributes --- */
  run(r, "attribute counter", case_counter, echo, 0);
  run(r, "attribute name", case_name, echo, 0);

  /* --- object references --- */
  run(r, "self_ref", case_self_ref, echo, 0);
  run(r, "echo_ref", case_echo_ref, echo, 0);
  run(r, "is_nil", case_is_nil, echo, 0);

  /* --- callbacks --- */
  cb_servant.base.vepv = &cb_vepv;
  pthread_mutex_init(&cb_servant.lock, NULL);
  POA_InteropTest_Callback__init(&cb_servant, &ev);
  oid = PortableServer_string_to_ObjectId("Callback");
  PortableServer_POA_activate_object_with_id(poa, oid, &cb_servant, &ev);
  the_cb = PortableServer_POA_id_to_reference(poa, oid, &ev);
  if (ev._major != CORBA_NO_EXCEPTION) {
    fprintf(stderr, "callback activation: %s\n", CORBA_exception_id(&ev));
    CORBA_exception_free(&ev);
  }
  run(r, "call_back", case_call_back, echo, 0);
  run(r, "register_callback/trigger", case_trigger, echo, 0);

  /* --- oneway --- */
  /* Last, as in the other clients (the number of posted() polls depends on
     timing). */
  run(r, "oneway post", case_post, echo, 0);

  CORBA_Object_release(the_cb, &ev);
  the_cb = CORBA_OBJECT_NIL;
  PortableServer_POA_deactivate_object(poa, oid, &ev);
  CORBA_exception_free(&ev);
  CORBA_free(oid);
}

/* After cases_run and once the POA no longer has the callback servant. */
void cases_fini(void) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  POA_InteropTest_Callback__fini(&cb_servant, &ev);
  pthread_mutex_destroy(&cb_servant.lock);
}

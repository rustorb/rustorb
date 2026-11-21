/* rustorb C: runs the CosNaming operation sequence of
 * impls/omniorbpy/naming_check.py with the CosNaming stubs of librustorb_c,
 * printing the same lines, so that the output can be compared with
 * expected/naming-check.txt (run-naming-check.sh).
 *
 *   naming_check --naming corbaloc::naming:2809/NameService [--timeout S]
 *
 * Licensed under MIT OR Apache-2.0, like the rest of the rustorb core. */

#define _POSIX_C_SOURCE 200809L

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "common.h"

#define NAMING_CONTEXT_ID "IDL:omg.org/CosNaming/NamingContext:1.0"
#define NAMING_CONTEXT_EXT_ID "IDL:omg.org/CosNaming/NamingContextExt:1.0"

static CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
static char out[4096];

static void append(char* buf, size_t size, const char* s) {
  size_t n = strlen(buf);
  if (n + 1 < size) snprintf(buf + n, size - n, "%s", s);
}

static void fmt_name(const CosNaming_Name* name, char* buf, size_t size) {
  CORBA_unsigned_long i;
  buf[0] = 0;
  if (name->_length == 0) {
    append(buf, size, "<empty>");
    return;
  }
  for (i = 0; i < name->_length; ++i) {
    if (i) append(buf, size, "/");
    append(buf, size, name->_buffer[i].id);
    if (name->_buffer[i].kind[0]) {
      append(buf, size, ".");
      append(buf, size, name->_buffer[i].kind);
    }
  }
}

/* The exception in ev as the omniORBpy check prints it. */
static void fmt_exc(char* buf, size_t size) {
  const char* id = CORBA_exception_id(&ev);
  char rest[1024];
  if (strcmp(id, ex_CosNaming_NamingContext_NotFound) == 0) {
    CosNaming_NamingContext_NotFound* nf = CORBA_exception_value(&ev);
    static const char* const why[] = {"missing_node", "not_context", "not_object"};
    fmt_name(&nf->rest_of_name, rest, sizeof rest);
    snprintf(buf, size, "NotFound(%s, rest=%s)", why[nf->why <= 2 ? nf->why : 2], rest);
  } else if (strcmp(id, ex_CosNaming_NamingContext_CannotProceed) == 0) {
    CosNaming_NamingContext_CannotProceed* cp = CORBA_exception_value(&ev);
    fmt_name(&cp->rest_of_name, rest, sizeof rest);
    snprintf(buf, size, "CannotProceed(rest=%s)", rest);
  } else {
    format_exception(&ev, buf, size);
  }
}

/* Prints the step's line: out, or the exception in ev. 0 if it raised. */
static int step(const char* label) {
  char exc[1200];
  int ok = ev._major == CORBA_NO_EXCEPTION;
  if (!ok) {
    fmt_exc(exc, sizeof exc);
    snprintf(out, sizeof out, "raises %s", exc);
    CORBA_exception_free(&ev);
  }
  printf("%-40s -> %s\n", label, out);
  out[0] = 0;
  return ok;
}

static void set_out(const char* s) {
  if (ev._major == CORBA_NO_EXCEPTION) snprintf(out, sizeof out, "%s", s);
}

static void equivalent(CORBA_Object o, CORBA_Object obj) {
  if (ev._major != CORBA_NO_EXCEPTION) return;
  set_out(o != CORBA_OBJECT_NIL && CORBA_Object_is_equivalent(o, obj, &ev) ? "equivalent"
                                                                             : "different object");
  CORBA_Object_release(o, NULL);
}

static void is_context(CORBA_Object o) {
  CORBA_boolean yes;
  if (ev._major != CORBA_NO_EXCEPTION) return;
  yes = o != CORBA_OBJECT_NIL && CORBA_Object_is_a(o, NAMING_CONTEXT_ID, &ev);
  set_out(yes ? "NamingContext" : "not a context");
  CORBA_Object_release(o, NULL);
}

/* Python's repr of a plain string. */
static void py_repr(const char* s, char* buf, size_t size) {
  char one[3] = {0, 0, 0};
  append(buf, size, "'");
  for (; *s; ++s) {
    one[0] = *s;
    one[1] = 0;
    if (*s == '\\' || *s == '\'') {
      one[0] = '\\';
      one[1] = *s;
    }
    append(buf, size, one);
  }
  append(buf, size, "'");
  (void)size;
}

static void fmt_components(CosNaming_Name* name) {
  CORBA_unsigned_long i;
  if (ev._major != CORBA_NO_EXCEPTION) return;
  out[0] = 0;
  append(out, sizeof out, "[");
  for (i = 0; i < name->_length; ++i) {
    if (i) append(out, sizeof out, ", ");
    append(out, sizeof out, "(");
    py_repr(name->_buffer[i].id, out, sizeof out);
    append(out, sizeof out, ", ");
    py_repr(name->_buffer[i].kind, out, sizeof out);
    append(out, sizeof out, ")");
  }
  append(out, sizeof out, "]");
  CORBA_free(name);
}

typedef struct bindings {
  char* items[64];
  int count;
} bindings;

static void add_bindings(bindings* b, const CosNaming_BindingList* bl) {
  CORBA_unsigned_long i;
  for (i = 0; bl && i < bl->_length && b->count < 64; ++i) {
    char name[1024], item[1100];
    fmt_name(&bl->_buffer[i].binding_name, name, sizeof name);
    /* \001 sorts as the (name, type) tuples of the omniORBpy check. */
    snprintf(item, sizeof item, "%s\001%s", name,
             bl->_buffer[i].binding_type == CosNaming_nobject ? "nobject" : "ncontext");
    b->items[b->count++] = strdup(item);
  }
}

static int compare(const void* a, const void* b) {
  return strcmp(*(char* const*)a, *(char* const*)b);
}

static void fmt_bindings(bindings* b, char* buf, size_t size) {
  int i;
  qsort(b->items, (size_t)b->count, sizeof b->items[0], compare);
  buf[0] = 0;
  append(buf, size, "[");
  for (i = 0; i < b->count; ++i) {
    char* sep = strchr(b->items[i], '\001');
    if (sep) *sep = ':';
    if (i) append(buf, size, ", ");
    append(buf, size, b->items[i]);
    free(b->items[i]);
  }
  append(buf, size, "]");
  b->count = 0;
}

#define N(path) to_name(path, 1)

/* Runs a void naming call on a name it frees, then sets "ok". */
#define CALL_OK(call, name)  \
  do {                       \
    CosNaming_Name* n_ = (name); \
    call;                    \
    CORBA_free(n_);          \
    set_out("ok");           \
  } while (0)

int main(int argc, char** argv) {
  options opt;
  CORBA_ORB orb;
  CosNaming_NamingContext root, test, sub = CORBA_OBJECT_NIL, loose = CORBA_OBJECT_NIL;
  CosNaming_NamingContextExt ext;
  CORBA_Object obj;
  CosNaming_Name* n;
  CosNaming_Name empty = {0, 0, NULL, 0};
  char base[1024], url[1200];
  int has_ext;

  parse_args("naming_check", argc, argv, &opt, 1);
  orb = orb_init(&opt);
  root = root_context(orb, opt.timeout);
  has_ext = CORBA_Object_is_a(root, NAMING_CONTEXT_EXT_ID, &ev);
  die_on(&ev, "is_a NamingContextExt");
  ext = root;

  n = N("test");
  test = CosNaming_NamingContext_bind_new_context(root, n, &ev);
  CORBA_free(n);
  set_out("ok");
  if (!step("bind_new_context test")) return 1;
  /* Any object reference will do. The test context is used rather than the
     root (as the omniORBpy check does) because its IOR comes from the naming
     service, while the root's was made from the corbaloc URL, and
     _is_equivalent compares IORs. */
  obj = test;

  CALL_OK(CosNaming_NamingContext_bind(root, n_, obj, &ev), N("test/a"));
  step("bind test/a");
  CALL_OK(CosNaming_NamingContext_bind(root, n_, obj, &ev), N("test/a"));
  step("bind test/a (again)");
  CALL_OK(CosNaming_NamingContext_rebind(root, n_, obj, &ev), N("test/a"));
  step("rebind test/a");
  n = N("test/a");
  equivalent(CosNaming_NamingContext_resolve(root, n, &ev), obj);
  CORBA_free(n);
  step("resolve test/a");
  CALL_OK(CosNaming_NamingContext_bind(test, n_, obj, &ev), N("b.kind"));
  step("bind test/b.kind");
  CALL_OK(CORBA_Object_release(CosNaming_NamingContext_resolve(root, n_, &ev), NULL), N("test/b"));
  step("resolve test/b (no kind)");
  n = N("test/b.kind");
  equivalent(CosNaming_NamingContext_resolve(root, n, &ev), obj);
  CORBA_free(n);
  step("resolve test/b.kind");
  CALL_OK(CORBA_Object_release(CosNaming_NamingContext_resolve(root, n_, &ev), NULL),
          N("test/missing"));
  step("resolve test/missing");
  CALL_OK(CORBA_Object_release(CosNaming_NamingContext_resolve(root, n_, &ev), NULL),
          N("test/missing/deeper"));
  step("resolve test/missing/deeper");
  CALL_OK(CORBA_Object_release(CosNaming_NamingContext_resolve(root, n_, &ev), NULL),
          N("test/a/x"));
  step("resolve test/a/x (not a context)");
  CORBA_Object_release(CosNaming_NamingContext_resolve(root, &empty, &ev), NULL);
  set_out("ok");
  step("resolve <empty>");
  CosNaming_NamingContext_bind(root, &empty, obj, &ev);
  set_out("ok");
  step("bind <empty>");

  n = N("test/sub");
  sub = CosNaming_NamingContext_bind_new_context(root, n, &ev);
  CORBA_free(n);
  set_out("ok");
  step("bind_new_context test/sub");
  CALL_OK(CORBA_Object_release(CosNaming_NamingContext_bind_new_context(root, n_, &ev), NULL),
          N("test/sub"));
  step("bind_new_context test/sub (again)");
  CALL_OK(CosNaming_NamingContext_bind(root, n_, obj, &ev), N("test/sub/c"));
  step("bind test/sub/c");
  n = N("test/sub");
  is_context(CosNaming_NamingContext_resolve(root, n, &ev));
  CORBA_free(n);
  step("resolve test/sub (is a context)");
  loose = CosNaming_NamingContext_new_context(root, &ev);
  set_out("ok");
  step("new_context");
  CALL_OK(CosNaming_NamingContext_bind_context(root, n_, loose, &ev), N("test/loose"));
  step("bind_context test/loose");
  CALL_OK(CosNaming_NamingContext_rebind_context(root, n_, loose, &ev), N("test/loose"));
  step("rebind_context test/loose");

  {
    CosNaming_BindingList* bl = NULL;
    CosNaming_BindingIterator it = CORBA_OBJECT_NIL;
    bindings b;
    b.count = 0;
    CosNaming_NamingContext_list(test, 10, &bl, &it, &ev);
    if (ev._major == CORBA_NO_EXCEPTION) {
      add_bindings(&b, bl);
      fmt_bindings(&b, out, sizeof out);
      if (it != CORBA_OBJECT_NIL) append(out, sizeof out, " + iterator");
      CORBA_free(bl);
      CORBA_Object_release(it, NULL);
    }
    step("list test (how_many=10)");
  }
  {
    CosNaming_BindingList* bl = NULL;
    CosNaming_BindingIterator it = CORBA_OBJECT_NIL;
    bindings b;
    char steps[512], line[2048];
    b.count = 0;
    CosNaming_NamingContext_list(test, 1, &bl, &it, &ev);
    if (ev._major == CORBA_NO_EXCEPTION) {
      add_bindings(&b, bl);
      snprintf(steps, sizeof steps, "first=%u", (unsigned)bl->_length);
      CORBA_free(bl);
      while (it != CORBA_OBJECT_NIL && ev._major == CORBA_NO_EXCEPTION) {
        CosNaming_BindingList* chunk = NULL;
        CORBA_boolean more = CosNaming_BindingIterator_next_n(it, 2, &chunk, &ev);
        char one[64];
        if (ev._major != CORBA_NO_EXCEPTION) break;
        snprintf(one, sizeof one, " next_n(2)=%s/%u", more ? "True" : "False",
                 (unsigned)chunk->_length);
        append(steps, sizeof steps, one);
        add_bindings(&b, chunk);
        CORBA_free(chunk);
        if (!more) {
          CosNaming_BindingIterator_destroy(it, &ev);
          break;
        }
      }
      CORBA_Object_release(it, NULL);
      if (ev._major == CORBA_NO_EXCEPTION) {
        fmt_bindings(&b, line, sizeof line);
        snprintf(out, sizeof out, "%s %s", line, steps);
      }
    }
    step("list test (how_many=1) + iterator");
  }

  CosNaming_NamingContext_destroy(sub, &ev);
  set_out("ok");
  step("destroy test/sub (not empty)");
  CALL_OK(CosNaming_NamingContext_unbind(root, n_, &ev), N("test/sub/c"));
  step("unbind test/sub/c");
  CosNaming_NamingContext_destroy(sub, &ev);
  set_out("ok");
  step("destroy test/sub");
  n = N("test/sub");
  CORBA_Object_release(CosNaming_NamingContext_resolve(root, n, &ev), NULL);
  CORBA_free(n);
  set_out("still bound");
  step("resolve test/sub (dangling)");
  CALL_OK(CosNaming_NamingContext_unbind(root, n_, &ev), N("test/sub"));
  step("unbind test/sub");
  CALL_OK(CosNaming_NamingContext_unbind(root, n_, &ev), N("test/a"));
  step("unbind test/a");
  CALL_OK(CosNaming_NamingContext_unbind(root, n_, &ev), N("test/a"));
  step("unbind test/a (again)");
  CALL_OK(CORBA_Object_release(CosNaming_NamingContext_resolve(root, n_, &ev), NULL), N("test/a"));
  step("resolve test/a");

  if (has_ext) {
    CosNaming_NameComponent parts[3];
    CosNaming_Name name3;
    CORBA_char* s;
    parts[0].id = "a";
    parts[0].kind = "b";
    parts[1].id = "c/d";
    parts[1].kind = "";
    parts[2].id = "";
    parts[2].kind = "e";
    name3._maximum = name3._length = 3;
    name3._buffer = parts;
    name3._release = 0;
    s = CosNaming_NamingContextExt_to_string(ext, &name3, &ev);
    set_out(s);
    CORBA_free(s);
    step("to_string [a.b, c/d, .e]");
    fmt_components(CosNaming_NamingContextExt_to_name(ext, "a.b/c\\/d/.e", &ev));
    step("to_name a.b/c\\/d/.e");
    s = CosNaming_NamingContextExt_to_url(ext, "host:2809", "a/b", &ev);
    set_out(s);
    CORBA_free(s);
    step("to_url host:2809 a/b");
    s = CosNaming_NamingContextExt_to_url(ext, ":host:2809", "a/b c", &ev);
    set_out(s);
    CORBA_free(s);
    step("to_url :host:2809 a/b c");
    equivalent(CosNaming_NamingContextExt_resolve_str(ext, "test/b.kind", &ev), obj);
    step("resolve_str test/b.kind");
    fmt_components(CosNaming_NamingContextExt_to_name(ext, "a//b", &ev));
    step("to_name <invalid>");
  } else {
    printf("root context is not a NamingContextExt\n");
  }

  /* corbaname: URLs, resolved by the ORB through the naming service. */
  if (strncmp(opt.naming, "corbaloc:", 9) == 0)
    snprintf(base, sizeof base, "corbaname:%s", opt.naming + 9);
  else
    snprintf(base, sizeof base, "%s", opt.naming);
  snprintf(url, sizeof url, "%s#test/b.kind", base);
  equivalent(CORBA_ORB_string_to_object(orb, url, &ev), obj);
  step("corbaname #test/b.kind");
  snprintf(url, sizeof url, "%s#test%%2Fb%%2ekind", base);
  equivalent(CORBA_ORB_string_to_object(orb, url, &ev), obj);
  step("corbaname #test%2Fb%2ekind");
  equivalent(CORBA_ORB_string_to_object(orb, "corbaname:rir:#test/b.kind", &ev), obj);
  step("corbaname rir: #test/b.kind");
  is_context(CORBA_ORB_string_to_object(orb, base, &ev));
  step("corbaname (no name)");
  snprintf(url, sizeof url, "%s#test/loose", base);
  is_context(CORBA_ORB_string_to_object(orb, url, &ev));
  step("corbaname #test/loose");
  snprintf(url, sizeof url, "%s#test/missing", base);
  CORBA_Object_release(CORBA_ORB_string_to_object(orb, url, &ev), NULL);
  set_out("ok");
  step("corbaname #test/missing");
  snprintf(url, sizeof url, "%s#test//b", base);
  CORBA_Object_release(CORBA_ORB_string_to_object(orb, url, &ev), NULL);
  set_out("ok");
  step("corbaname #test//b (invalid)");

  {
    CosNaming_BindingList* bl = NULL;
    CosNaming_BindingIterator it = CORBA_OBJECT_NIL;
    bindings b;
    b.count = 0;
    CosNaming_NamingContext_list(test, 100, &bl, &it, &ev);
    if (ev._major == CORBA_NO_EXCEPTION) {
      add_bindings(&b, bl);
      fmt_bindings(&b, out, sizeof out);
      CORBA_free(bl);
      CORBA_Object_release(it, NULL);
    }
    step("list test (final)");
  }
  fflush(stdout);
  CORBA_Object_release(loose, NULL);
  CORBA_Object_release(sub, NULL);
  CORBA_Object_release(test, NULL);
  CORBA_Object_release(root, NULL);
  CORBA_ORB_destroy(orb, &ev);
  CORBA_Object_release(orb, NULL);
  return 0;
}

/* Helpers shared by the C InteropTest server, client and naming check.
 *
 * Licensed under MIT OR Apache-2.0, like the rest of the rustorb core. */

#define _POSIX_C_SOURCE 200809L

#include "common.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

static const char* value(int argc, char** argv, int i, const char* option) {
  if (i >= argc) {
    fprintf(stderr, "missing value for %s\n", option);
    exit(2);
  }
  return argv[i];
}

static void orb_arg(options* opt, const char* a) {
  if (opt->orb_argc >= (int)(sizeof opt->orb_argv / sizeof opt->orb_argv[0]) - 1) {
    fprintf(stderr, "too many ORB options\n");
    exit(2);
  }
  opt->orb_argv[opt->orb_argc++] = (char*)a;
  opt->orb_argv[opt->orb_argc] = NULL;
}

void parse_args(const char* prog, int argc, char** argv, options* opt, int naming_only) {
  int i;
  memset(opt, 0, sizeof *opt);
  opt->host = "";
  opt->timeout = 30.0;
  orb_arg(opt, prog);
  for (i = 1; i < argc; ++i) {
    const char* a = argv[i];
    if (strcmp(a, "--naming") == 0) {
      opt->naming = value(argc, argv, ++i, a);
    } else if (strcmp(a, "--timeout") == 0) {
      opt->timeout = atof(value(argc, argv, ++i, a));
    } else if (!naming_only && strcmp(a, "--name") == 0) {
      opt->name = value(argc, argv, ++i, a);
    } else if (!naming_only && strcmp(a, "--host") == 0) {
      opt->host = value(argc, argv, ++i, a);
    } else if (!naming_only && strcmp(a, "--port") == 0) {
      opt->port = atoi(value(argc, argv, ++i, a));
    } else if (!naming_only && strcmp(a, "--shutdown-server") == 0) {
      opt->shutdown_server = 1;
    } else if (!naming_only && strcmp(a, "--suite") == 0) {
      const char* suite = value(argc, argv, ++i, a);
      if (strcmp(suite, "basic") != 0) {
        /* The any, value, custom and abstract suites need what rustorb#187
           adds (DynAny, valuetypes...). */
        fprintf(stderr, "suite %s is not supported by rustorbc\n", suite);
        exit(2);
      }
    } else {
      orb_arg(opt, a);
    }
  }
  if (!opt->naming || (!naming_only && !opt->name)) {
    if (naming_only)
      fprintf(stderr, "usage: %s --naming CORBALOC [--timeout S]\n", prog);
    else
      fprintf(stderr,
              "usage: %s --naming CORBALOC --name PATH [--host H] [--port N] [--timeout S]"
              " [--shutdown-server] [--suite basic] [ORB options]\n",
              prog);
    exit(2);
  }
}

void die_on(CORBA_Environment* ev, const char* what) {
  if (ev->_major == CORBA_NO_EXCEPTION) return;
  fprintf(stderr, "%s: CORBA exception %s\n", what, CORBA_exception_id(ev));
  exit(1);
}

CORBA_ORB orb_init(options* opt) {
  static char buf[8][512];
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  const char* ssl = getenv("INTEROP_SSL");
  const char* unix_dir = getenv("INTEROP_UNIX");
  const char* giop = getenv("ORBmaxGIOPVersion");
  const char* frag = getenv("INTEROP_FRAGMENT_SIZE");
  int tls = ssl && *ssl;
  int argc;
  CORBA_ORB orb;
  snprintf(buf[0], sizeof buf[0], "NameService=%s", opt->naming);
  orb_arg(opt, "-ORBInitRef");
  orb_arg(opt, buf[0]);
  /* INTEROP_SSL=DIR (run-matrix.sh --ssl): TLS only, certificates required
     both ways, as omniORB requires by default. */
  if (tls) {
    snprintf(buf[1], sizeof buf[1], "%s/ca.pem", ssl);
    snprintf(buf[2], sizeof buf[2], "%s/server-bundle.pem", ssl);
    orb_arg(opt, "-ORBsslCAFile");
    orb_arg(opt, buf[1]);
    orb_arg(opt, "-ORBsslKeyFile");
    orb_arg(opt, buf[2]);
    orb_arg(opt, "-ORBsslVerifyClient");
    orb_arg(opt, "required");
  }
  /* INTEROP_UNIX=DIR (run-matrix.sh --unix): a Unix socket in DIR. */
  if (opt->port != 0 && unix_dir && *unix_dir) {
    snprintf(buf[3], sizeof buf[3], "giop:unix:%s/%s-%d.sock", unix_dir, opt->host, opt->port);
    orb_arg(opt, "-ORBendPoint");
    orb_arg(opt, buf[3]);
  } else if (opt->port != 0) {
    snprintf(buf[3], sizeof buf[3], "%s%s:%d", tls ? "giop:ssl:" : "giop:tcp:", opt->host,
             opt->port);
    orb_arg(opt, "-ORBendPoint");
    orb_arg(opt, buf[3]);
  }
  if (giop && *giop) {
    orb_arg(opt, "-ORBmaxGIOPVersion");
    orb_arg(opt, giop);
  }
  /* run-matrix.sh --fragment N: rustorb splits GIOP 1.2 messages. */
  if (frag && *frag) {
    orb_arg(opt, "-ORBgiopFragmentSize");
    orb_arg(opt, frag);
  }
  argc = opt->orb_argc;
  orb = CORBA_ORB_init(&argc, opt->orb_argv, "", &ev);
  die_on(&ev, "ORB_init");
  return orb;
}

const char* exception_name(const char* id) {
  static char name[256];
  const char* start = strrchr(id, '/');
  const char* end;
  size_t n;
  start = start ? start + 1 : id;
  end = strrchr(start, ':');
  n = end ? (size_t)(end - start) : strlen(start);
  if (n >= sizeof name) n = sizeof name - 1;
  memcpy(name, start, n);
  name[n] = 0;
  return name;
}

void format_exception(CORBA_Environment* ev, char* buf, size_t size) {
  const char* id = CORBA_exception_id(ev);
  if (ev->_major == CORBA_SYSTEM_EXCEPTION) {
    CORBA_SystemException* s = (CORBA_SystemException*)CORBA_exception_value(ev);
    static const char* const completed[] = {"COMPLETED_YES", "COMPLETED_NO", "COMPLETED_MAYBE"};
    snprintf(buf, size, "%s(completed=%s)", exception_name(id),
             s && s->completed <= CORBA_COMPLETED_MAYBE ? completed[s->completed] : "?");
  } else {
    snprintf(buf, size, "%s", exception_name(id));
  }
}

CosNaming_Name* to_name(const char* path, int split_kind) {
  CosNaming_Name* name = CosNaming_Name__alloc();
  CORBA_unsigned_long count = 0, i = 0;
  const char* p;
  for (p = path; *p;) {
    const char* e = strchr(p, '/');
    size_t n = e ? (size_t)(e - p) : strlen(p);
    if (n > 0) ++count;
    p += n;
    if (*p == '/') ++p;
  }
  name->_buffer = CORBA_sequence_CosNaming_NameComponent_allocbuf(count);
  name->_maximum = count;
  name->_release = 1;
  for (p = path; *p;) {
    const char* e = strchr(p, '/');
    size_t n = e ? (size_t)(e - p) : strlen(p);
    if (n > 0) {
      const char* dot = split_kind ? memchr(p, '.', n) : NULL;
      size_t idn = dot ? (size_t)(dot - p) : n;
      CosNaming_NameComponent* c = &name->_buffer[i++];
      c->id = CORBA_string_alloc((CORBA_unsigned_long)idn);
      memcpy(c->id, p, idn);
      c->id[idn] = 0;
      if (dot) {
        size_t kn = n - idn - 1;
        c->kind = CORBA_string_alloc((CORBA_unsigned_long)kn);
        memcpy(c->kind, dot + 1, kn);
        c->kind[kn] = 0;
      } else {
        c->kind = CORBA_string_dup("");
      }
    }
    p += n;
    if (*p == '/') ++p;
  }
  name->_length = count;
  return name;
}

/* An exception that means "not there yet": retried. */
static int retryable(CORBA_Environment* ev) {
  const char* id = CORBA_exception_id(ev);
  return strcmp(id, ex_CORBA_TRANSIENT) == 0 || strcmp(id, ex_CORBA_COMM_FAILURE) == 0 ||
         strcmp(id, ex_CORBA_OBJECT_NOT_EXIST) == 0 ||
         strcmp(id, ex_CosNaming_NamingContext_NotFound) == 0;
}

static double now(void) {
  struct timespec ts;
  clock_gettime(CLOCK_MONOTONIC, &ts);
  return (double)ts.tv_sec + (double)ts.tv_nsec / 1e9;
}

static void wait_or_die(double start, double timeout, const char* what) {
  struct timespec d = {0, 200 * 1000 * 1000};
  if (now() - start >= timeout) {
    fprintf(stderr, "timed out waiting for %s\n", what);
    exit(1);
  }
  nanosleep(&d, NULL);
}

CosNaming_NamingContext root_context(CORBA_ORB orb, double timeout) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  double start = now();
  for (;;) {
    CORBA_Object obj = CORBA_ORB_resolve_initial_references(orb, "NameService", &ev);
    if (ev._major == CORBA_NO_EXCEPTION) {
      CORBA_boolean ok = CORBA_Object_is_a(obj, "IDL:omg.org/CosNaming/NamingContext:1.0", &ev);
      if (ev._major == CORBA_NO_EXCEPTION && ok) return obj;
      CORBA_Object_release(obj, NULL);
    }
    if (ev._major != CORBA_NO_EXCEPTION && !retryable(&ev)) die_on(&ev, "the naming service");
    CORBA_exception_free(&ev);
    wait_or_die(start, timeout, "the naming service");
  }
}

CORBA_Object resolve_retry(CosNaming_NamingContext root, const char* path, double timeout) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  CosNaming_Name* name = to_name(path, 0);
  double start = now();
  for (;;) {
    CORBA_Object obj = CosNaming_NamingContext_resolve(root, name, &ev);
    if (ev._major == CORBA_NO_EXCEPTION) {
      /* And it answers. */
      CORBA_Object_non_existent(obj, &ev);
      if (ev._major == CORBA_NO_EXCEPTION) {
        CORBA_free(name);
        return obj;
      }
      CORBA_Object_release(obj, NULL);
    }
    if (!retryable(&ev)) die_on(&ev, "the Echo server");
    CORBA_exception_free(&ev);
    wait_or_die(start, timeout, "the Echo server");
  }
}

void bind_name(CosNaming_NamingContext root, const char* path, CORBA_Object obj) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  CosNaming_Name* name = to_name(path, 0);
  CosNaming_NamingContext ctx = CORBA_Object_duplicate(root, &ev);
  CORBA_unsigned_long i;
  CosNaming_Name one;
  one._maximum = one._length = 1;
  one._release = 0;
  for (i = 0; i + 1 < name->_length; ++i) {
    CosNaming_NamingContext next;
    one._buffer = &name->_buffer[i];
    next = CosNaming_NamingContext_bind_new_context(ctx, &one, &ev);
    if (ev._major != CORBA_NO_EXCEPTION &&
        strcmp(CORBA_exception_id(&ev), ex_CosNaming_NamingContext_AlreadyBound) == 0) {
      CORBA_exception_free(&ev);
      next = CosNaming_NamingContext_resolve(ctx, &one, &ev);
    }
    die_on(&ev, "bind_new_context");
    CORBA_Object_release(ctx, &ev);
    ctx = next;
  }
  one._buffer = &name->_buffer[name->_length - 1];
  CosNaming_NamingContext_rebind(ctx, &one, obj, &ev);
  die_on(&ev, "rebind");
  CORBA_Object_release(ctx, &ev);
  CORBA_free(name);
}

PortableServer_POA persistent_poa(CORBA_ORB orb, const char* name) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  PortableServer_POA root, poa;
  PortableServer_POAManager mgr;
  CORBA_Policy policies[2];
  CORBA_PolicyList list;
  root = (PortableServer_POA)CORBA_ORB_resolve_initial_references(orb, "RootPOA", &ev);
  die_on(&ev, "RootPOA");
  mgr = PortableServer_POA__get_the_POAManager(root, &ev);
  die_on(&ev, "the_POAManager");
  policies[0] = PortableServer_POA_create_lifespan_policy(root, PortableServer_PERSISTENT, &ev);
  die_on(&ev, "create_lifespan_policy");
  policies[1] = PortableServer_POA_create_id_assignment_policy(root, PortableServer_USER_ID, &ev);
  die_on(&ev, "create_id_assignment_policy");
  list._maximum = list._length = 2;
  list._buffer = policies;
  list._release = 0;
  poa = PortableServer_POA_create_POA(root, name, mgr, &list, &ev);
  die_on(&ev, "create_POA");
  PortableServer_POAManager_activate(mgr, &ev);
  die_on(&ev, "activate");
  CORBA_Object_release(policies[0], &ev);
  CORBA_Object_release(policies[1], &ev);
  CORBA_Object_release(mgr, &ev);
  CORBA_Object_release(root, &ev);
  return poa;
}

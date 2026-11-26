/* C program of the rustorb / omniORB comparison benchmark (interop/bench,
   docs/omniorb-comparison.md) on the rustorb C binding: the same cases,
   options and output as the Rust, C++ and Python programs.

     bench serve  [ORB options]
     bench client --ior IOR [options] [ORB options]
     bench inproc [options] [ORB options] */

#define _POSIX_C_SOURCE 200809L

#include <pthread.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

#include "Bench.h"

static const char* DEFAULT_CASES =
    "ping,long,string,octets:64,octets:1024,octets:16384,octets:262144,octets:1048576,"
    "octets:8388608,put:1048576,get:1048576,samples:100,any,oneway,threads:4";
static double seconds = 2, warmup = 0.3;

static double now(void) {
  struct timespec ts;
  clock_gettime(CLOCK_MONOTONIC, &ts);
  return ts.tv_sec + ts.tv_nsec / 1e9;
}

/* --- the servant ------------------------------------------------------------ */

typedef struct Impl {
  POA_Bench_Target base;
  CORBA_ORB orb;
  pthread_mutex_t lock;
  CORBA_unsigned_long posted;
} Impl;
#define SELF ((Impl*)servant)

static void s_ping(PortableServer_Servant servant, CORBA_Environment* ev) {
  (void)servant, (void)ev;
}
static CORBA_long s_echo_long(PortableServer_Servant servant, CORBA_long v, CORBA_Environment* ev) {
  (void)servant, (void)ev;
  return v;
}
static CORBA_char* s_echo_string(PortableServer_Servant servant, const CORBA_char* v,
                                 CORBA_Environment* ev) {
  (void)servant, (void)ev;
  return CORBA_string_dup(v);
}
static Bench_Octets* s_echo_octets(PortableServer_Servant servant, const Bench_Octets* v,
                                   CORBA_Environment* ev) {
  (void)servant, (void)ev;
  return (Bench_Octets*)rustorb_c_copy(&Bench_Octets__desc, v);
}
static CORBA_unsigned_long s_put_octets(PortableServer_Servant servant, const Bench_Octets* v,
                                        CORBA_Environment* ev) {
  (void)servant, (void)ev;
  return v->_length;
}
static Bench_Octets* s_get_octets(PortableServer_Servant servant, CORBA_unsigned_long n,
                                  CORBA_Environment* ev) {
  Bench_Octets* r = Bench_Octets__alloc();
  (void)servant, (void)ev;
  r->_buffer = CORBA_sequence_octet_allocbuf(n);
  r->_maximum = r->_length = n;
  CORBA_sequence_set_release(r, 1);
  if (n) memset(r->_buffer, 7, n);
  return r;
}
static Bench_Samples* s_echo_samples(PortableServer_Servant servant, const Bench_Samples* v,
                                     CORBA_Environment* ev) {
  (void)servant, (void)ev;
  return (Bench_Samples*)rustorb_c_copy(&Bench_Samples__desc, v);
}
static CORBA_any* s_echo_any(PortableServer_Servant servant, const CORBA_any* v,
                             CORBA_Environment* ev) {
  (void)servant, (void)ev;
  return (CORBA_any*)rustorb_c_copy(&CORBA_any__desc, v);
}
static void s_post(PortableServer_Servant servant, CORBA_long v, CORBA_Environment* ev) {
  (void)v, (void)ev;
  pthread_mutex_lock(&SELF->lock);
  SELF->posted++;
  pthread_mutex_unlock(&SELF->lock);
}
static CORBA_unsigned_long s_take_posted(PortableServer_Servant servant, CORBA_Environment* ev) {
  CORBA_unsigned_long n;
  (void)ev;
  pthread_mutex_lock(&SELF->lock);
  n = SELF->posted;
  SELF->posted = 0;
  pthread_mutex_unlock(&SELF->lock);
  return n;
}
static void s_shutdown(PortableServer_Servant servant, CORBA_Environment* ev) {
  CORBA_ORB_shutdown(SELF->orb, 0, ev);
}

static PortableServer_ServantBase__epv base_epv = {NULL, NULL, NULL};
static POA_Bench_Target__epv target_epv = {
    NULL,         s_ping,         s_echo_long, s_echo_string, s_echo_octets, s_put_octets,
    s_get_octets, s_echo_samples, s_echo_any,  s_post,        s_take_posted, s_shutdown,
};
static POA_Bench_Target__vepv target_vepv = {&base_epv, &target_epv};

/* --- the cases ---------------------------------------------------------------- */

static int failed(CORBA_Environment* ev, const char* what) {
  if (ev->_major == CORBA_NO_EXCEPTION) return 0;
  fprintf(stderr, "bench: %s: %s\n", what, CORBA_exception_id(ev));
  return 1;
}

typedef struct Ctx {
  Bench_Target t;
  CORBA_unsigned_long n;
  Bench_Octets octets;
  Bench_Samples samples;
  CORBA_any any;
  const char* string;
} Ctx;

/* One call of the case: returns 0 when it went well. */
typedef int (*Call)(Ctx* c, CORBA_Environment* ev);

static int c_ping(Ctx* c, CORBA_Environment* ev) {
  Bench_Target_ping(c->t, ev);
  return failed(ev, "ping");
}
static int c_long(Ctx* c, CORBA_Environment* ev) {
  CORBA_long r = Bench_Target_echo_long(c->t, 12345, ev);
  return failed(ev, "echo_long") || r != 12345;
}
static int c_string(Ctx* c, CORBA_Environment* ev) {
  CORBA_char* r = Bench_Target_echo_string(c->t, c->string, ev);
  int bad = failed(ev, "echo_string") || strlen(r) != strlen(c->string);
  CORBA_free(r);
  return bad;
}
static int c_octets(Ctx* c, CORBA_Environment* ev) {
  Bench_Octets* r = Bench_Target_echo_octets(c->t, &c->octets, ev);
  int bad = failed(ev, "echo_octets") || r->_length != c->n;
  CORBA_free(r);
  return bad;
}
static int c_put(Ctx* c, CORBA_Environment* ev) {
  CORBA_unsigned_long r = Bench_Target_put_octets(c->t, &c->octets, ev);
  return failed(ev, "put_octets") || r != c->n;
}
static int c_get(Ctx* c, CORBA_Environment* ev) {
  Bench_Octets* r = Bench_Target_get_octets(c->t, c->n, ev);
  int bad = failed(ev, "get_octets") || r->_length != c->n;
  CORBA_free(r);
  return bad;
}
static int c_samples(Ctx* c, CORBA_Environment* ev) {
  Bench_Samples* r = Bench_Target_echo_samples(c->t, &c->samples, ev);
  int bad = failed(ev, "echo_samples") || r->_length != c->n;
  CORBA_free(r);
  return bad;
}
static int c_any(Ctx* c, CORBA_Environment* ev) {
  CORBA_any* r = Bench_Target_echo_any(c->t, &c->any, ev);
  int bad = failed(ev, "echo_any") || !r->_value || ((Bench_Sample*)r->_value)->id != 1;
  CORBA_free(r);
  return bad;
}

static int cmp(const void* a, const void* b) {
  double x = *(const double*)a, y = *(const double*)b;
  return x < y ? -1 : x > y;
}

static int timed(const char* name, double bytes, Call call, Ctx* c) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  double warm_end = now() + warmup, start, end, took, sum = 0, rate;
  size_t warm = 0, n = 0, cap = 1 << 16, i;
  double* lat = malloc(cap * sizeof *lat);
  for (; now() < warm_end; ++warm) {
    if (call(c, &ev)) return 1;
  }
  start = now();
  end = start + seconds;
  for (;;) {
    double t = now(), d;
    if (call(c, &ev)) return 1;
    d = now();
    if (n == cap) lat = realloc(lat, (cap *= 2) * sizeof *lat);
    lat[n++] = (d - t) * 1e6;
    if (d >= end) break;
  }
  took = now() - start;
  qsort(lat, n, sizeof *lat, cmp);
  for (i = 0; i < n; ++i) sum += lat[i];
  rate = n / took;
#define PCT(p) lat[(size_t)((n - 1) * (p))]
  printf("{\"case\":\"%s\",\"calls\":%zu,\"seconds\":%.3f,\"rate\":%.1f,\"mean_us\":%.2f,"
         "\"p50_us\":%.2f,\"p90_us\":%.2f,\"p99_us\":%.2f,\"max_us\":%.2f,\"mb_per_s\":%.2f,"
         "\"total_calls\":%zu}\n",
         name, n, took, rate, sum / n, PCT(0.5), PCT(0.9), PCT(0.99), PCT(1.0),
         rate * bytes / 1e6, n + warm);
  fflush(stdout);
  free(lat);
  return 0;
}

typedef struct Worker {
  Bench_Target t;
  volatile int* stop;
  pthread_mutex_t* lock;
  unsigned long* calls;
  int failed;
  CORBA_long base;
} Worker;

static void* worker(void* arg) {
  Worker* w = (Worker*)arg;
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  CORBA_long i = w->base;
  while (!*w->stop) {
    ++i;
    if (Bench_Target_echo_long(w->t, i, &ev) != i || ev._major != CORBA_NO_EXCEPTION) {
      w->failed = 1;
      break;
    }
    pthread_mutex_lock(w->lock);
    ++*w->calls;
    pthread_mutex_unlock(w->lock);
  }
  return NULL;
}

static int run_case(Bench_Target t, const char* c) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  char name[32];
  const char* colon = strchr(c, ':');
  Ctx ctx;
  int rc = 0;
  CORBA_unsigned_long i;
  memset(&ctx, 0, sizeof ctx);
  ctx.t = t;
  snprintf(name, sizeof name, "%.*s", colon ? (int)(colon - c) : (int)strlen(c), c);
  ctx.n = colon ? (CORBA_unsigned_long)strtoul(colon + 1, NULL, 10) : 0;
  if (!strcmp(name, "first")) {
    /* The first call on a fresh reference: the connection is made. */
    double t0 = now(), us;
    Bench_Target_ping(t, &ev);
    if (failed(&ev, "ping")) return 1;
    us = (now() - t0) * 1e6;
    printf("{\"case\":\"first\",\"calls\":1,\"p50_us\":%.2f,\"mean_us\":%.2f}\n", us, us);
    fflush(stdout);
    return 0;
  }
  if (!strcmp(name, "ping")) return timed(c, 0, c_ping, &ctx);
  if (!strcmp(name, "long")) return timed(c, 0, c_long, &ctx);
  if (!strcmp(name, "string")) {
    size_t len = ctx.n ? ctx.n : 100;
    char* s = malloc(len + 1);
    memset(s, 'x', len);
    s[len] = 0;
    ctx.string = s;
    rc = timed(c, 0, c_string, &ctx);
    free(s);
    return rc;
  }
  if (!strcmp(name, "octets") || !strcmp(name, "put")) {
    ctx.octets._buffer = malloc(ctx.n ? ctx.n : 1);
    ctx.octets._maximum = ctx.octets._length = ctx.n;
    memset(ctx.octets._buffer, 1, ctx.n);
    rc = !strcmp(name, "octets") ? timed(c, 2.0 * ctx.n, c_octets, &ctx)
                                 : timed(c, ctx.n, c_put, &ctx);
    free(ctx.octets._buffer);
    return rc;
  }
  if (!strcmp(name, "get")) return timed(c, ctx.n, c_get, &ctx);
  if (!strcmp(name, "samples")) {
    char (*names)[24] = malloc((ctx.n ? ctx.n : 1) * sizeof *names);
    ctx.samples._buffer = calloc(ctx.n ? ctx.n : 1, sizeof(Bench_Sample));
    ctx.samples._maximum = ctx.samples._length = ctx.n;
    for (i = 0; i < ctx.n; ++i) {
      Bench_Sample* s = &ctx.samples._buffer[i];
      s->id = (CORBA_long)i;
      s->x = 1.5;
      s->y = 2.5;
      s->z = 3.5;
      snprintf(names[i], sizeof names[i], "sample-%u", (unsigned)i);
      s->name = names[i];
    }
    rc = timed(c, 0, c_samples, &ctx);
    free(ctx.samples._buffer);
    free(names);
    return rc;
  }
  if (!strcmp(name, "any")) {
    Bench_Sample s = {1, 1.5, 2.5, 3.5, (CORBA_char*)"sample"};
    ctx.any._type = TC_Bench_Sample;
    ctx.any._value = &s;
    return timed(c, 0, c_any, &ctx);
  }
  if (!strcmp(name, "oneway")) {
    double start, end, send_took, took, deadline;
    unsigned long sent = 0, got;
    int k;
    Bench_Target_take_posted(t, &ev);
    if (failed(&ev, "take_posted")) return 1;
    start = now();
    end = start + seconds;
    while (now() < end) {
      for (k = 0; k < 64; ++k) {
        Bench_Target_post(t, (CORBA_long)sent++, &ev);
        if (failed(&ev, "post")) return 1;
      }
    }
    send_took = now() - start;
    got = Bench_Target_take_posted(t, &ev);
    deadline = now() + 10;
    while (got < sent && now() < deadline) {
      struct timespec ms = {0, 1000000};
      nanosleep(&ms, NULL);
      got += Bench_Target_take_posted(t, &ev);
    }
    took = now() - start;
    printf("{\"case\":\"%s\",\"calls\":%lu,\"seconds\":%.3f,\"rate\":%.1f,\"send_rate\":%.1f,"
           "\"received\":%lu,\"total_calls\":%lu}\n",
           c, sent, took, sent / took, sent / send_took, got, sent);
    fflush(stdout);
    return got != sent;
  }
  if (!strcmp(name, "threads")) {
    volatile int stop = 0;
    pthread_mutex_t lock = PTHREAD_MUTEX_INITIALIZER;
    unsigned long calls = 0, c0, c1;
    CORBA_unsigned_long n = ctx.n ? ctx.n : 1;
    Worker* ws = calloc(n, sizeof *ws);
    pthread_t* hs = calloc(n, sizeof *hs);
    double start, took;
    struct timespec w = {(time_t)warmup, (long)((warmup - (time_t)warmup) * 1e9)};
    struct timespec s = {(time_t)seconds, (long)((seconds - (time_t)seconds) * 1e9)};
    for (i = 0; i < n; ++i) {
      ws[i].t = t;
      ws[i].stop = &stop;
      ws[i].lock = &lock;
      ws[i].calls = &calls;
      ws[i].base = (CORBA_long)i * 1000000;
      pthread_create(&hs[i], NULL, worker, &ws[i]);
    }
    nanosleep(&w, NULL);
    pthread_mutex_lock(&lock);
    c0 = calls;
    pthread_mutex_unlock(&lock);
    start = now();
    nanosleep(&s, NULL);
    pthread_mutex_lock(&lock);
    c1 = calls;
    pthread_mutex_unlock(&lock);
    took = now() - start;
    stop = 1;
    for (i = 0; i < n; ++i) {
      pthread_join(hs[i], NULL);
      rc |= ws[i].failed;
    }
    printf("{\"case\":\"%s\",\"calls\":%lu,\"seconds\":%.3f,\"rate\":%.1f,\"total_calls\":%lu}\n",
           c, c1 - c0, took, (c1 - c0) / took, calls);
    fflush(stdout);
    free(ws);
    free(hs);
    return rc;
  }
  fprintf(stderr, "bench: unknown case %s\n", c);
  return 1;
}

int main(int argc, char** argv) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  const char* mode = argc > 1 ? argv[1] : "";
  const char* ior = "";
  char* cases = strdup(DEFAULT_CASES);
  char** orb_argv = calloc((size_t)argc + 1, sizeof *orb_argv);
  int orb_argc = 1, i, status = 0;
  CORBA_ORB orb;
  Bench_Target target;
  char* c;
  orb_argv[0] = argv[0];
  for (i = 2; i < argc; ++i) {
    if (i + 1 < argc && !strcmp(argv[i], "--ior")) ior = argv[++i];
    else if (i + 1 < argc && !strcmp(argv[i], "--cases")) {
      free(cases);
      cases = strdup(argv[++i]);
    } else if (i + 1 < argc && !strcmp(argv[i], "--seconds")) seconds = atof(argv[++i]);
    else if (i + 1 < argc && !strcmp(argv[i], "--warmup")) warmup = atof(argv[++i]);
    else orb_argv[orb_argc++] = argv[i];
  }
  orb = CORBA_ORB_init(&orb_argc, orb_argv, "", &ev);
  if (failed(&ev, "ORB_init")) return 1;
  if (!strcmp(mode, "serve") || !strcmp(mode, "inproc")) {
    PortableServer_POA poa =
        (PortableServer_POA)CORBA_ORB_resolve_initial_references(orb, "RootPOA", &ev);
    Impl* impl = calloc(1, sizeof *impl);
    PortableServer_ObjectId* oid;
    CORBA_Object ref;
    if (failed(&ev, "RootPOA")) return 1;
    impl->base.vepv = &target_vepv;
    impl->orb = orb;
    pthread_mutex_init(&impl->lock, NULL);
    POA_Bench_Target__init(impl, &ev);
    oid = PortableServer_POA_activate_object(poa, impl, &ev);
    if (failed(&ev, "activate_object")) return 1;
    ref = PortableServer_POA_id_to_reference(poa, oid, &ev);
    PortableServer_POAManager_activate(PortableServer_POA__get_the_POAManager(poa, &ev), &ev);
    if (failed(&ev, "activate")) return 1;
    target = ref;
    if (!strcmp(mode, "serve")) {
      CORBA_char* s = CORBA_ORB_object_to_string(orb, ref, &ev);
      printf("%s\n", s);
      fflush(stdout);
      CORBA_free(s);
      CORBA_ORB_run(orb, &ev);
      CORBA_ORB_destroy(orb, &ev);
      return 0;
    }
  } else if (!strcmp(mode, "client")) {
    target = CORBA_ORB_string_to_object(orb, ior, &ev);
    if (failed(&ev, "string_to_object")) return 1;
  } else {
    fprintf(stderr, "usage: bench serve|client|inproc [options]\n");
    return 2;
  }
  for (c = strtok(cases, ","); c; c = strtok(NULL, ",")) {
    if (run_case(target, c)) {
      fprintf(stderr, "bench: %s failed\n", c);
      status = 1;
      break;
    }
  }
  CORBA_ORB_destroy(orb, &ev);
  return status;
}

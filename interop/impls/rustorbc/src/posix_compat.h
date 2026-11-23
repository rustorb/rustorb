/* The few POSIX threads and clock functions the C cases use, on Windows
 * (where the C tests build with MSVC) over SRW locks, condition variables
 * and Sleep. Elsewhere it includes <pthread.h> and <time.h>. Include it
 * after the other headers: <windows.h> defines short macros.
 *
 * Licensed under MIT OR Apache-2.0, like the rest of the rustorb core. */

#ifndef RUSTORBC_POSIX_COMPAT_H
#define RUSTORBC_POSIX_COMPAT_H

#ifdef _WIN32

#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN
#endif
#ifndef NOMINMAX
#define NOMINMAX
#endif
#ifndef NOGDI
#define NOGDI
#endif
#include <windows.h>

#include <errno.h>
#include <time.h>

typedef SRWLOCK pthread_mutex_t;
typedef CONDITION_VARIABLE pthread_cond_t;
#define PTHREAD_MUTEX_INITIALIZER SRWLOCK_INIT
#define PTHREAD_COND_INITIALIZER CONDITION_VARIABLE_INIT

static __inline int pthread_mutex_init(pthread_mutex_t* m, const void* attr) {
  (void)attr;
  InitializeSRWLock(m);
  return 0;
}
static __inline int pthread_mutex_destroy(pthread_mutex_t* m) {
  (void)m;
  return 0;
}
static __inline int pthread_mutex_lock(pthread_mutex_t* m) {
  AcquireSRWLockExclusive(m);
  return 0;
}
static __inline int pthread_mutex_unlock(pthread_mutex_t* m) {
  ReleaseSRWLockExclusive(m);
  return 0;
}
static __inline int pthread_cond_signal(pthread_cond_t* c) {
  WakeConditionVariable(c);
  return 0;
}

#define CLOCK_REALTIME 0
#define CLOCK_MONOTONIC 1

static __inline int clock_gettime(int clock, struct timespec* ts) {
  if (clock == CLOCK_MONOTONIC) {
    LARGE_INTEGER freq, count;
    QueryPerformanceFrequency(&freq);
    QueryPerformanceCounter(&count);
    ts->tv_sec = (time_t)(count.QuadPart / freq.QuadPart);
    ts->tv_nsec = (long)((count.QuadPart % freq.QuadPart) * 1000000000LL / freq.QuadPart);
    return 0;
  }
  return timespec_get(ts, TIME_UTC) == TIME_UTC ? 0 : -1;
}

/* Waits until the CLOCK_REALTIME time `until`. */
static __inline int pthread_cond_timedwait(pthread_cond_t* c, pthread_mutex_t* m,
                                           const struct timespec* until) {
  struct timespec now;
  long long ms;
  timespec_get(&now, TIME_UTC);
  ms = ((long long)until->tv_sec - now.tv_sec) * 1000 + (until->tv_nsec - now.tv_nsec) / 1000000;
  if (ms < 0) ms = 0;
  if (SleepConditionVariableSRW(c, m, (DWORD)ms, 0)) return 0;
  return GetLastError() == ERROR_TIMEOUT ? ETIMEDOUT : EINVAL;
}

static __inline int nanosleep(const struct timespec* t, struct timespec* rem) {
  (void)rem;
  Sleep((DWORD)(t->tv_sec * 1000 + t->tv_nsec / 1000000));
  return 0;
}

#else

#include <pthread.h>
#include <time.h>

#endif

#endif

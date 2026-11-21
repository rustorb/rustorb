/* CORBA_free and the allocation functions: every block starts with a
   header naming the descriptor of what it holds. */

#include "internal.h"

#ifdef _WIN32
#include <windows.h>
static SRWLOCK lock = SRWLOCK_INIT;
void rustorb_c_lock(void) { AcquireSRWLockExclusive(&lock); }
void rustorb_c_unlock(void) { ReleaseSRWLockExclusive(&lock); }
#else
#include <pthread.h>
static pthread_mutex_t lock = PTHREAD_MUTEX_INITIALIZER;
void rustorb_c_lock(void) { pthread_mutex_lock(&lock); }
void rustorb_c_unlock(void) { pthread_mutex_unlock(&lock); }
#endif

#define MAGIC 0x52434d45u /* "RCME" */
#define SEQUENCE_BLOCK ((const rustorb_c_type*)&sequence_block)

typedef struct header_fields {
  uint32_t magic;
  CORBA_unsigned_long count;
  const rustorb_c_type* type;
} header_fields;

/* Padded so that what follows is aligned for any type. */
typedef union header {
  header_fields h;
  long double ld;
  long long ll;
  void* p;
  char pad[32];
} header;

static const char sequence_block = 0;

static void* alloc_block(const rustorb_c_type* type, CORBA_unsigned_long count, size_t size) {
  header* h = (header*)calloc(1, sizeof(header) + size);
  if (!h) return NULL;
  h->h.magic = MAGIC;
  h->h.count = count;
  h->h.type = type;
  return h + 1;
}

void* rustorb_c_alloc(const rustorb_c_type* type, CORBA_unsigned_long count) {
  size_t size = type ? type->size * (size_t)count : (size_t)count;
  if (type && type->size && count > (size_t)-1 / 2 / type->size) return NULL;
  return alloc_block(type, count, size ? size : 1);
}

void* rustorb_c_sequence_alloc(void) {
  return alloc_block(SEQUENCE_BLOCK, 1, sizeof(CORBA_sequence_octet));
}

static void free_sequence(void* p) {
  CORBA_sequence_octet* s = (CORBA_sequence_octet*)p;
  if (s->_release && s->_buffer) CORBA_free(s->_buffer);
  s->_buffer = NULL;
  s->_length = s->_maximum = 0;
}

static int union_entry(const rustorb_c_type* type, const void* p, int64_t* index);

void rustorb_c_free_value(const rustorb_c_type* type, void* p) {
  CORBA_unsigned_long i;
  if (!type || !p) return;
  switch (type->kind) {
    case CORBA_tk_string:
    case CORBA_tk_wstring:
      CORBA_free(*(void**)p);
      *(void**)p = NULL;
      break;
    case CORBA_tk_objref:
    case CORBA_tk_abstract_interface:
    case CORBA_tk_local_interface:
    case CORBA_tk_TypeCode: {
      CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
      CORBA_Object_release(*(CORBA_Object*)p, &ev);
      *(CORBA_Object*)p = NULL;
      break;
    }
    case CORBA_tk_any: {
      CORBA_any* a = (CORBA_any*)p;
      CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
      if (a->_release && a->_value) CORBA_free(a->_value);
      CORBA_Object_release(a->_type, &ev);
      if (a->_rustorb) rorb_any_release((rorb_any*)a->_rustorb);
      memset(a, 0, sizeof *a);
      break;
    }
    case CORBA_tk_struct:
    case CORBA_tk_except:
      for (i = 0; i < type->count; i++)
        rustorb_c_free_value(type->members[i].type, (char*)p + type->members[i].offset);
      break;
    case CORBA_tk_union: {
      int64_t e;
      if (union_entry(type, p, &e))
        rustorb_c_free_value(type->members[e].type, (char*)p + type->members[e].offset);
      memset(p, 0, type->size);
      break;
    }
    case CORBA_tk_sequence:
      free_sequence(p);
      break;
    case CORBA_tk_array:
      for (i = 0; i < type->length; i++)
        rustorb_c_free_value(type->content, (char*)p + i * type->content->size);
      break;
    case CORBA_tk_alias:
      rustorb_c_free_value(type->content, p);
      break;
    default:
      break;
  }
}

/* The union entry selected by the discriminator of the union at p. */
static int union_entry(const rustorb_c_type* type, const void* p, int64_t* index) {
  uint64_t label = rustorb_c_label(type->content, p);
  CORBA_unsigned_long i;
  for (i = 0; i < type->count; i++) {
    if ((CORBA_long)i != type->default_index && type->members[i].label == label) {
      *index = i;
      return 1;
    }
  }
  if (type->default_index >= 0) {
    *index = type->default_index;
    return 1;
  }
  return 0;
}

int rustorb_c_union_entry(const rustorb_c_type* type, const void* p, int64_t* index) {
  return union_entry(type, p, index);
}

void CORBA_free(void* storage) {
  header* h;
  if (!storage) return;
  h = (header*)storage - 1;
  if (h->h.magic != MAGIC) return; /* not the ORB's: leave it */
  if (h->h.type == SEQUENCE_BLOCK) {
    free_sequence(storage);
  } else if (h->h.type) {
    CORBA_unsigned_long i;
    for (i = 0; i < h->h.count; i++)
      rustorb_c_free_value(h->h.type, (char*)storage + i * h->h.type->size);
  }
  h->h.magic = 0;
  free(h);
}

CORBA_char* CORBA_string_alloc(CORBA_unsigned_long len) {
  return (CORBA_char*)alloc_block(NULL, 0, (size_t)len + 1);
}

CORBA_wchar* CORBA_wstring_alloc(CORBA_unsigned_long len) {
  return (CORBA_wchar*)alloc_block(NULL, 0, ((size_t)len + 1) * sizeof(CORBA_wchar));
}

CORBA_char* CORBA_string_dup(const CORBA_char* s) {
  size_t len;
  CORBA_char* r;
  if (!s) return NULL;
  len = strlen(s);
  r = CORBA_string_alloc((CORBA_unsigned_long)len);
  if (r) memcpy(r, s, len);
  return r;
}

CORBA_unsigned_long CORBA_wstring_len(const CORBA_wchar* s) {
  CORBA_unsigned_long n = 0;
  if (!s) return 0;
  while (s[n]) n++;
  return n;
}

CORBA_wchar* CORBA_wstring_dup(const CORBA_wchar* s) {
  CORBA_unsigned_long len;
  CORBA_wchar* r;
  if (!s) return NULL;
  len = CORBA_wstring_len(s);
  r = CORBA_wstring_alloc(len);
  if (r) memcpy(r, s, len * sizeof(CORBA_wchar));
  return r;
}

char* rustorb_c_strndup(const char* s, size_t len) {
  char* r = CORBA_string_alloc((CORBA_unsigned_long)len);
  if (r && len) memcpy(r, s, len);
  return r;
}

void CORBA_sequence_set_release(void* seq, CORBA_boolean release) {
  if (seq) ((CORBA_sequence_octet*)seq)->_release = release;
}

CORBA_boolean CORBA_sequence_get_release(void* seq) {
  return seq ? ((CORBA_sequence_octet*)seq)->_release : CORBA_FALSE;
}

CORBA_sequence_octet* CORBA_sequence_octet__alloc(void) {
  return (CORBA_sequence_octet*)rustorb_c_sequence_alloc();
}
CORBA_octet* CORBA_sequence_octet_allocbuf(CORBA_unsigned_long len) {
  return (CORBA_octet*)rustorb_c_alloc(&CORBA_octet__desc, len);
}
CORBA_sequence_string* CORBA_sequence_string__alloc(void) {
  return (CORBA_sequence_string*)rustorb_c_sequence_alloc();
}
CORBA_char** CORBA_sequence_string_allocbuf(CORBA_unsigned_long len) {
  return (CORBA_char**)rustorb_c_alloc(&CORBA_string__desc, len);
}
CORBA_PolicyList* CORBA_PolicyList__alloc(void) {
  return (CORBA_PolicyList*)rustorb_c_sequence_alloc();
}
CORBA_Policy* CORBA_sequence_CORBA_Policy_allocbuf(CORBA_unsigned_long len) {
  return (CORBA_Policy*)rustorb_c_alloc(&CORBA_Object__desc, len);
}
CORBA_any* CORBA_any__alloc(void) { return (CORBA_any*)rustorb_c_alloc(&CORBA_any__desc, 1); }

void CORBA_any_set_release(CORBA_any* any, CORBA_boolean release) {
  if (any) any->_release = release;
}
CORBA_boolean CORBA_any_get_release(CORBA_any* any) {
  return any ? any->_release : CORBA_FALSE;
}

PortableServer_ObjectId* rustorb_c_object_id(rorb_bytes bytes) {
  PortableServer_ObjectId* id = CORBA_sequence_octet__alloc();
  if (id) {
    id->_buffer = CORBA_sequence_octet_allocbuf((CORBA_unsigned_long)bytes.len);
    if (bytes.len) memcpy(id->_buffer, bytes.data, bytes.len);
    id->_length = id->_maximum = (CORBA_unsigned_long)bytes.len;
    id->_release = CORBA_TRUE;
  }
  rorb_bytes_free(bytes);
  return id;
}

PortableServer_ObjectId* PortableServer_string_to_ObjectId(const CORBA_char* str) {
  size_t len = str ? strlen(str) : 0;
  PortableServer_ObjectId* id = CORBA_sequence_octet__alloc();
  if (!id) return NULL;
  id->_buffer = CORBA_sequence_octet_allocbuf((CORBA_unsigned_long)len);
  if (len) memcpy(id->_buffer, str, len);
  id->_length = id->_maximum = (CORBA_unsigned_long)len;
  id->_release = CORBA_TRUE;
  return id;
}

CORBA_char* PortableServer_ObjectId_to_string(const PortableServer_ObjectId* id) {
  if (!id) return CORBA_string_dup("");
  return rustorb_c_strndup((const char*)id->_buffer, id->_length);
}

/* --- copies ------------------------------------------------------------------ */

void rustorb_c_copy_value(const rustorb_c_type* type, void* dst, const void* src) {
  CORBA_unsigned_long i;
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  switch (type->kind) {
    case CORBA_tk_string:
      *(CORBA_char**)dst = CORBA_string_dup(*(CORBA_char* const*)src);
      break;
    case CORBA_tk_wstring:
      *(CORBA_wchar**)dst = CORBA_wstring_dup(*(CORBA_wchar* const*)src);
      break;
    case CORBA_tk_objref:
    case CORBA_tk_abstract_interface:
    case CORBA_tk_local_interface:
    case CORBA_tk_TypeCode:
      *(CORBA_Object*)dst = CORBA_Object_duplicate(*(CORBA_Object const*)src, &ev);
      break;
    case CORBA_tk_any: {
      const CORBA_any* s = (const CORBA_any*)src;
      CORBA_any* d = (CORBA_any*)dst;
      memset(d, 0, sizeof *d);
      d->_type = CORBA_Object_duplicate(s->_type, &ev);
      if (s->_rustorb) d->_rustorb = rorb_any_clone((rorb_any*)s->_rustorb);
      if (s->_value && s->_type) {
        rorb_typecode* tc = rustorb_c_tc_handle(s->_type);
        const rustorb_c_type* t = s->_type->desc ? s->_type->desc : rustorb_c_desc_for_tc(tc);
        if (t) {
          d->_value = rustorb_c_copy(t, s->_value);
          d->_release = CORBA_TRUE;
        }
      }
      break;
    }
    case CORBA_tk_struct:
    case CORBA_tk_except:
      for (i = 0; i < type->count; i++)
        rustorb_c_copy_value(type->members[i].type, (char*)dst + type->members[i].offset,
                             (const char*)src + type->members[i].offset);
      break;
    case CORBA_tk_union: {
      int64_t e;
      memset(dst, 0, type->size);
      rustorb_c_copy_value(type->content, dst, src);
      if (rustorb_c_union_entry(type, src, &e))
        rustorb_c_copy_value(type->members[e].type, (char*)dst + type->members[e].offset,
                             (const char*)src + type->members[e].offset);
      break;
    }
    case CORBA_tk_sequence: {
      const CORBA_sequence_octet* s = (const CORBA_sequence_octet*)src;
      CORBA_sequence_octet* d = (CORBA_sequence_octet*)dst;
      memset(d, 0, sizeof *d);
      d->_release = CORBA_TRUE;
      if (!s->_length || !s->_buffer || !type->content) break;
      d->_buffer = (CORBA_octet*)rustorb_c_alloc(type->content, s->_length);
      if (!d->_buffer) break;
      d->_length = d->_maximum = s->_length;
      for (i = 0; i < s->_length; i++)
        rustorb_c_copy_value(type->content, (char*)d->_buffer + i * type->content->size,
                             (const char*)s->_buffer + i * type->content->size);
      break;
    }
    case CORBA_tk_array:
      for (i = 0; i < type->length; i++)
        rustorb_c_copy_value(type->content, (char*)dst + i * type->content->size,
                             (const char*)src + i * type->content->size);
      break;
    case CORBA_tk_alias:
      rustorb_c_copy_value(type->content, dst, src);
      break;
    default:
      memcpy(dst, src, type->size);
      break;
  }
}

void* rustorb_c_copy(const rustorb_c_type* type, const void* value) {
  void* r;
  if (!type || !value) return NULL;
  r = rustorb_c_alloc(type, 1);
  if (r) rustorb_c_copy_value(type, r, value);
  return r;
}

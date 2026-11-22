/* CORBA_Environment and the exceptions of the C ABI. */

#include "internal.h"

static const rustorb_c_member system_exception_members[] = {
    {"minor", &CORBA_unsigned_long__desc, offsetof(CORBA_SystemException, minor), 0},
    {"completed", &CORBA_unsigned_long__desc, offsetof(CORBA_SystemException, completed), 0},
};
/* Only for its size: system exceptions are not described by descriptors. */
static const rustorb_c_type system_exception_desc = {
    CORBA_tk_struct, sizeof(CORBA_SystemException), NULL, NULL, NULL, 0, 2,
    system_exception_members, NULL, -1, 0, 0, NULL};

CORBA_char* CORBA_exception_id(CORBA_Environment* ev) {
  return ev && ev->_major != CORBA_NO_EXCEPTION ? ev->_id : NULL;
}

void* CORBA_exception_value(CORBA_Environment* ev) {
  return ev && ev->_major != CORBA_NO_EXCEPTION ? ev->_params : NULL;
}

static void drop_anys(CORBA_Environment* ev);

void CORBA_exception_free(CORBA_Environment* ev) {
  if (!ev) return;
  drop_anys(ev);
  CORBA_free(ev->_params);
  CORBA_free(ev->_id);
  ev->_major = CORBA_NO_EXCEPTION;
  ev->_id = NULL;
  ev->_params = NULL;
}

void CORBA_exception_init(CORBA_Environment* ev) {
  if (!ev) return;
  ev->_major = CORBA_NO_EXCEPTION;
  ev->_id = NULL;
  ev->_params = NULL;
}

void CORBA_exception_set(CORBA_Environment* ev, CORBA_exception_type major, const CORBA_char* id,
                         void* param) {
  if (!ev) {
    CORBA_free(param);
    return;
  }
  /* The functions of the ORB set _major to CORBA_NO_EXCEPTION first, so
     that an Environment not initialised is fine. */
  if (ev->_major != CORBA_NO_EXCEPTION) CORBA_exception_free(ev);
  if (major == CORBA_NO_EXCEPTION) {
    CORBA_free(param);
    return;
  }
  ev->_major = major;
  ev->_id = CORBA_string_dup(id ? id : "");
  ev->_params = param;
}

CORBA_SystemException* CORBA_SystemException__alloc(void) {
  return (CORBA_SystemException*)rustorb_c_alloc(&system_exception_desc, 1);
}

void CORBA_exception_set_system(CORBA_Environment* ev, const CORBA_char* id,
                                CORBA_unsigned_long minor, CORBA_completion_status completed) {
  CORBA_SystemException* v = CORBA_SystemException__alloc();
  if (v) {
    v->minor = minor;
    v->completed = completed;
  }
  CORBA_exception_set(ev, CORBA_SYSTEM_EXCEPTION, id, v);
}

void rustorb_c_ev_clear(CORBA_Environment* ev) {
  if (ev) ev->_major = CORBA_NO_EXCEPTION;
}

void rustorb_c_ev_system(CORBA_Environment* ev, const char* id, CORBA_unsigned_long minor,
                         CORBA_completion_status completed) {
  CORBA_exception_set_system(ev, id, minor, completed);
}

/* The user exceptions of the pseudo objects, by rorb_exception.local. */
static const char* const local_ids[] = {
    NULL,
    "IDL:omg.org/PortableServer/POA/AdapterAlreadyExists:1.0",
    "IDL:omg.org/PortableServer/POA/AdapterNonExistent:1.0",
    "IDL:omg.org/PortableServer/POA/InvalidPolicy:1.0",
    "IDL:omg.org/PortableServer/POA/ServantAlreadyActive:1.0",
    "IDL:omg.org/PortableServer/POA/ObjectAlreadyActive:1.0",
    "IDL:omg.org/PortableServer/POA/ServantNotActive:1.0",
    "IDL:omg.org/PortableServer/POA/ObjectNotActive:1.0",
    "IDL:omg.org/PortableServer/POA/WrongPolicy:1.0",
    "IDL:omg.org/PortableServer/POA/WrongAdapter:1.0",
    "IDL:omg.org/PortableServer/POAManager/AdapterInactive:1.0",
    "IDL:omg.org/PortableServer/POA/NoServant:1.0",
    "IDL:omg.org/PortableServer/Current/NoContext:1.0",
    "IDL:omg.org/CORBA/ORB/InvalidName:1.0",
    "IDL:omg.org/CORBA/TypeCode/BadKind:1.0",
    "IDL:omg.org/CORBA/TypeCode/Bounds:1.0",
    "IDL:omg.org/CORBA/PolicyError:1.0",
    "IDL:omg.org/DynamicAny/DynAny/InvalidValue:1.0",
    "IDL:omg.org/DynamicAny/DynAny/TypeMismatch:1.0",
    "IDL:omg.org/DynamicAny/DynAnyFactory/InconsistentTypeCode:1.0",
    "IDL:omg.org/DynamicAny/MustTruncate:1.0",
    "IDL:omg.org/PortableInterceptor/InvalidSlot:1.0",
    "IDL:omg.org/PortableInterceptor/ORBInitInfo/DuplicateName:1.0",
    "IDL:omg.org/IOP/CodecFactory/UnknownEncoding:1.0",
    "IDL:omg.org/IOP/Codec/InvalidTypeForEncoding:1.0",
    "IDL:omg.org/IOP/Codec/FormatMismatch:1.0",
    "IDL:omg.org/IOP/Codec/TypeMismatch:1.0",
    "IDL:omg.org/PortableServer/POAManagerFactory/ManagerAlreadyExists:1.0",
};

static const rustorb_c_member invalid_policy_members[] = {
    {"index", &CORBA_unsigned_short__desc, offsetof(PortableServer_POA_InvalidPolicy, index), 0},
};
static rustorb_c_cache invalid_policy_cache;
static const rustorb_c_type invalid_policy_desc = {
    CORBA_tk_except, sizeof(PortableServer_POA_InvalidPolicy),
    "IDL:omg.org/PortableServer/POA/InvalidPolicy:1.0", "InvalidPolicy", NULL, 0, 1,
    invalid_policy_members, NULL, -1, 0, 0, &invalid_policy_cache};
static const rustorb_c_member policy_error_members[] = {
    {"reason", &CORBA_short__desc, offsetof(CORBA_PolicyError, reason), 0},
};
static rustorb_c_cache policy_error_cache;
static const rustorb_c_type policy_error_desc = {
    CORBA_tk_except, sizeof(CORBA_PolicyError), "IDL:omg.org/CORBA/PolicyError:1.0",
    "PolicyError", NULL, 0, 1, policy_error_members, NULL, -1, 0, 0, &policy_error_cache};

void rustorb_c_ev_status(CORBA_Environment* ev, rorb_status status, rorb_exception* exc) {
  switch (status) {
    case RORB_OK:
      break;
    case RORB_SYSTEM_EXCEPTION:
      rustorb_c_ev_system(ev, exc->repository_id ? exc->repository_id : ex_CORBA_UNKNOWN,
                          exc->minor, (CORBA_completion_status)exc->completed);
      break;
    case RORB_LOCAL_EXCEPTION: {
      void* param = NULL;
      const char* id = exc->local < sizeof local_ids / sizeof local_ids[0]
                           ? local_ids[exc->local]
                           : NULL;
      if (!id) {
        rustorb_c_ev_system(ev, ex_CORBA_UNKNOWN, 0, CORBA_COMPLETED_NO);
        break;
      }
      if (exc->local == RORB_INVALID_POLICY) {
        PortableServer_POA_InvalidPolicy* v =
            (PortableServer_POA_InvalidPolicy*)rustorb_c_alloc(&invalid_policy_desc, 1);
        if (v) v->index = (CORBA_unsigned_short)exc->index;
        param = v;
      } else if (exc->local == RORB_POLICY_ERROR) {
        CORBA_PolicyError* v = (CORBA_PolicyError*)rustorb_c_alloc(&policy_error_desc, 1);
        if (v) v->reason = (CORBA_short)exc->index;
        param = v;
      }
      CORBA_exception_set(ev, CORBA_USER_EXCEPTION, id, param);
      break;
    }
    default:
      rustorb_c_ev_system(ev, ex_CORBA_UNKNOWN, 0, CORBA_COMPLETED_MAYBE);
      break;
  }
  rorb_exception_clear(exc);
}

int rustorb_c_failed(CORBA_Environment* ev, rorb_status status, rorb_exception* exc) {
  if (status == RORB_OK) return 0;
  rustorb_c_ev_status(ev, status, exc);
  return 1;
}

/* --- CORBA_exception_as_any (1.22) ----------------------------------------------- */

/* The CORBA_any of an Environment stays valid until CORBA_exception_free
   (the mapping: the caller does not own it). CORBA_Environment has only
   _major, _id and _params, so the anys are kept here, by the Environment
   and the _id they were made for. */
typedef struct held_any {
  CORBA_Environment* ev;
  const CORBA_char* id;
  CORBA_any* any;
  struct held_any* next;
} held_any;
static held_any* held;

/* Descriptors made for exceptions: the system exceptions (minor and
   completed) and user exceptions without members that no generated code
   describes (those of the pseudo objects). Kept for the life of the
   program. */
typedef struct made_except {
  rustorb_c_type type;
  rustorb_c_cache cache;
  struct made_except* next;
} made_except;
static made_except* made_excepts;

static const char* const completion_names[] = {"COMPLETED_YES", "COMPLETED_NO",
                                               "COMPLETED_MAYBE"};
static rustorb_c_cache completion_cache;
static const rustorb_c_type completion_desc = {
    CORBA_tk_enum, sizeof(CORBA_completion_status), "IDL:omg.org/CORBA/CompletionStatus:1.0",
    "CompletionStatus", NULL, 0, 3, NULL, completion_names, -1, 0, 0, &completion_cache};
static const rustorb_c_member system_members[] = {
    {"minor", &CORBA_unsigned_long__desc, offsetof(CORBA_SystemException, minor), 0},
    {"completed", &completion_desc, offsetof(CORBA_SystemException, completed), 0},
};

/* The simple name of a repository id: IDL:omg.org/CORBA/BAD_PARAM:1.0 ->
   BAD_PARAM. */
static char* simple_name(const char* id) {
  const char* start = strrchr(id, '/');
  const char* end;
  char* r;
  size_t n;
  start = start ? start + 1 : (strncmp(id, "IDL:", 4) == 0 ? id + 4 : id);
  end = strrchr(start, ':');
  n = end ? (size_t)(end - start) : strlen(start);
  r = (char*)malloc(n + 1);
  if (!r) return NULL;
  memcpy(r, start, n);
  r[n] = 0;
  return r;
}

/* Under the lock. */
static const rustorb_c_type* except_desc(const char* id, int system) {
  made_except* m;
  char* name;
  char* rid;
  for (m = made_excepts; m; m = m->next)
    if (strcmp(m->type.id, id) == 0 && (m->type.count == 2) == (system != 0)) return &m->type;
  m = (made_except*)calloc(1, sizeof *m);
  name = simple_name(id);
  rid = (char*)malloc(strlen(id) + 1);
  if (!m || !name || !rid) {
    free(m);
    free(name);
    free(rid);
    return NULL;
  }
  strcpy(rid, id);
  m->type.kind = CORBA_tk_except;
  m->type.id = rid;
  m->type.name = name;
  m->type.default_index = -1;
  m->type.cache = &m->cache;
  if (system) {
    m->type.size = sizeof(CORBA_SystemException);
    m->type.count = 2;
    m->type.members = system_members;
  } else {
    m->type.size = 1;
  }
  m->next = made_excepts;
  made_excepts = m;
  return &m->type;
}

static const rustorb_c_type* desc_of_exception(CORBA_Environment* ev) {
  const rustorb_c_type* t = NULL;
  if (ev->_major == CORBA_SYSTEM_EXCEPTION) {
    rustorb_c_lock();
    t = except_desc(ev->_id, 1);
    rustorb_c_unlock();
    return t;
  }
  if (strcmp(ev->_id, invalid_policy_desc.id) == 0) return &invalid_policy_desc;
  if (strcmp(ev->_id, policy_error_desc.id) == 0) return &policy_error_desc;
  t = rustorb_c_desc_for_id(ev->_id);
  if (t) return t->kind == CORBA_tk_except ? t : NULL;
  /* An exception no generated code describes: only one without members
     (no _params) can be put in an any. */
  if (ev->_params) return NULL;
  rustorb_c_lock();
  t = except_desc(ev->_id, 0);
  rustorb_c_unlock();
  return t;
}

CORBA_any* CORBA_exception_as_any(CORBA_Environment* ev) {
  static CORBA_octet no_members;
  const rustorb_c_type* t;
  held_any* h;
  CORBA_any* any;
  CORBA_TypeCode tc;
  if (!ev || ev->_major == CORBA_NO_EXCEPTION || !ev->_id) return NULL;
  rustorb_c_lock();
  for (h = held; h; h = h->next)
    if (h->ev == ev && h->id == ev->_id) break;
  rustorb_c_unlock();
  if (h) return h->any;
  t = desc_of_exception(ev);
  if (!t) return NULL;
  tc = rustorb_c_type_tc(t, NULL);
  any = CORBA_any__alloc();
  h = (held_any*)calloc(1, sizeof *h);
  if (!tc || !any || !h) {
    CORBA_free(any);
    free(h);
    return NULL;
  }
  any->_type = tc;
  /* The value stays the Environment's. */
  any->_value = ev->_params ? ev->_params : (t->count == 0 ? (void*)&no_members : NULL);
  any->_release = CORBA_FALSE;
  if (!any->_value) {
    CORBA_free(any);
    free(h);
    return NULL;
  }
  h->ev = ev;
  h->id = ev->_id;
  h->any = any;
  rustorb_c_lock();
  h->next = held;
  held = h;
  rustorb_c_unlock();
  return any;
}

static void drop_anys(CORBA_Environment* ev) {
  held_any** p;
  held_any* gone = NULL;
  rustorb_c_lock();
  for (p = &held; *p;) {
    held_any* h = *p;
    if (h->ev == ev) {
      *p = h->next;
      h->next = gone;
      gone = h;
    } else {
      p = &h->next;
    }
  }
  rustorb_c_unlock();
  while (gone) {
    held_any* h = gone;
    gone = h->next;
    CORBA_free(h->any);
    free(h);
  }
}

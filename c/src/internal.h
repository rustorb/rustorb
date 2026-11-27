/* What the sources of librustorb_c share. */

#ifndef RUSTORB_C_INTERNAL_H
#define RUSTORB_C_INTERNAL_H

#include <rustorb/runtime.h>
#include <rustorb/rustorb.h>

#include <stdlib.h>
#include <string.h>

/* What a CORBA_Object is. */
enum {
  OBJ_REF = 1,
  OBJ_ORB,
  OBJ_POA,
  OBJ_POA_MANAGER,
  OBJ_TYPECODE,
  OBJ_POLICY,
  OBJ_CURRENT,
  OBJ_DYNANY_FACTORY,
  OBJ_DYNANY,
  /* A local object: handle is its implementation (a servant initialised
     with rustorb_c_local_init), not owned. */
  OBJ_LOCAL,
  /* The infos of the interceptor points (pi.c): handle is the core's,
     borrowed during the call. */
  OBJ_CLIENT_INFO,
  OBJ_SERVER_INFO,
  OBJ_IOR_INFO,
  OBJ_INIT_INFO,
  /* handle is a rustorb_c_shared of an rorb_codec / rorb_pi_current. */
  OBJ_CODEC,
  OBJ_PI_CURRENT,
  /* handle is an rorb_orb (owned), or with flags 1 an rorb_orb_init_info
     (borrowed). */
  OBJ_CODEC_FACTORY
};

/* A handle of the core without a clone function, shared by the duplicates
   of a CORBA_Object. */
typedef struct rustorb_c_shared {
  int refs;
  void* handle;
  void (*release)(void* handle);
} rustorb_c_shared;
rustorb_c_shared* rustorb_c_shared_new(void* handle, void (*release)(void* handle));
void rustorb_c_shared_ref(rustorb_c_shared* s);
void rustorb_c_shared_unref(rustorb_c_shared* s);

struct CORBA_Object_struct {
  int kind;
  /* TC_long and the TypeCodes of descriptors: never freed. */
  int immortal;
  void* handle;
  /* A TypeCode: the descriptor of its type, if known. */
  const rustorb_c_type* desc;
  /* A policy. */
  CORBA_unsigned_long policy_type;
  uint64_t policy_value;
  /* See the kinds. */
  int flags;
};

CORBA_Object rustorb_c_wrap(int kind, void* handle);
CORBA_TypeCode rustorb_c_wrap_tc(rorb_typecode* tc, const rustorb_c_type* desc);
/* The rorb_typecode of a TypeCode, or NULL. */
rorb_typecode* rustorb_c_tc_handle(CORBA_TypeCode tc);
/* The descriptor of a TypeCode (generated, built in or made for an
   anonymous type), or NULL when the program has none. */
const rustorb_c_type* rustorb_c_desc_for_tc(const rorb_typecode* tc);

/* A send-time error. */
typedef struct rustorb_c_err {
  const char* id;
  CORBA_unsigned_long minor;
} rustorb_c_err;

/* 0, or -1 with *err set. */
int rustorb_c_write(rorb_out* out, const rustorb_c_type* type, const void* p, rustorb_c_err* err);
/* Into zeroed storage; errors fail the stream. */
void rustorb_c_read(rorb_in* in, const rustorb_c_type* type, void* p);

/* rustorb_c_write and _read of the value of an any on the wire (an
   exception's with its repository id first; not in an rorb_any, whose
   values are as arguments). */
int rustorb_c_write_value(rorb_out* out, const rustorb_c_type* type, const void* p,
                          rustorb_c_err* err);
void rustorb_c_read_value(rorb_in* in, const rustorb_c_type* type, void* p);

/* The rorb_any of a CORBA_any (new), or NULL with *err set. */
rorb_any* rustorb_c_any_to_raw(const CORBA_any* any, rustorb_c_err* err);
/* Reads the value of raw (of the type of the descriptor) into zeroed
   storage: 0, or -1 (storage left zeroed). */
int rustorb_c_raw_value(const rorb_any* raw, const rustorb_c_type* type, void* value);
/* A CORBA_any holding the value of raw, into storage that holds nothing:
   0, or -1 (storage left empty). */
int rustorb_c_any_from_raw(const rorb_any* raw, CORBA_any* any);

/* The label of the discriminator value at p (see rustorb_c_member). */
uint64_t rustorb_c_label(const rustorb_c_type* disc, const void* p);
/* The union entry selected by the union at p: 1 and *index, or 0. */
int rustorb_c_union_entry(const rustorb_c_type* type, const void* p, int64_t* index);

/* A named type registered (rustorb_c_register_module), by repository id,
   or NULL. */
const rustorb_c_type* rustorb_c_desc_for_id(const char* id);

/* Environments. */
void rustorb_c_ev_clear(CORBA_Environment* ev);
void rustorb_c_ev_system(CORBA_Environment* ev, const char* id, CORBA_unsigned_long minor,
                         CORBA_completion_status completed);
/* Sets ev from a status other than RORB_OK and clears exc. */
void rustorb_c_ev_status(CORBA_Environment* ev, rorb_status status, rorb_exception* exc);
/* Returns 1 (and sets ev) if status is not RORB_OK. */
int rustorb_c_failed(CORBA_Environment* ev, rorb_status status, rorb_exception* exc);

/* Whether values of the type own nothing (see memory.c). */
int rustorb_c_plain(const rustorb_c_type* type);
/* rustorb_c_alloc for values the caller fills at once: not zeroed when
   they own nothing. */
void* rustorb_c_alloc_filled(const rustorb_c_type* type, CORBA_unsigned_long count);

char* rustorb_c_strndup(const char* s, size_t len);
PortableServer_ObjectId* rustorb_c_object_id(rorb_bytes bytes);

/* The rorb_servant of a servant (initialised with __init), or NULL. */
rorb_servant* rustorb_c_servant_handle(PortableServer_Servant servant);
/* A new handle of it (released by its taker), or NULL. */
rorb_servant* rustorb_c_servant_ref(PortableServer_Servant servant);

/* A PortableServer_DynamicImpl (dii.c): its servant data, and the
   dispatch of its requests. */
void rustorb_c_servant_init_dynamic(PortableServer_Servant servant, const char* primary,
                                    CORBA_Environment* ev);
rorb_status rustorb_c_dsi_dispatch(PortableServer_Servant servant, const char* op, size_t len,
                                   rorb_in* in, rorb_server_reply* reply);

/* The repository ids of a local object's implementation: 1 if id is one
   of them. */
int rustorb_c_local_is_a(CORBA_Object obj, const char* id);
const char* rustorb_c_local_id(CORBA_Object obj);

/* A PICurrent object over current (taken), or nil. */
CORBA_Object rustorb_c_pi_current(rorb_pi_current* current);
/* A policy object of a type and its raw value. */
CORBA_Policy rustorb_c_policy(CORBA_PolicyType type, uint64_t value);

/* One global lock for the caches. */
void rustorb_c_lock(void);
void rustorb_c_unlock(void);

#define RUSTORB_C_ARG_CHECK(ev, cond, id)            \
  do {                                                \
    if (!(cond)) {                                    \
      rustorb_c_ev_system((ev), (id), 0, CORBA_COMPLETED_NO); \
      return;                                         \
    }                                                 \
  } while (0)

#endif

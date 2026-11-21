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
  OBJ_CURRENT
};

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

/* The rorb_any of a CORBA_any (new), or NULL with *err set. */
rorb_any* rustorb_c_any_to_raw(const CORBA_any* any, rustorb_c_err* err);

/* The label of the discriminator value at p (see rustorb_c_member). */
uint64_t rustorb_c_label(const rustorb_c_type* disc, const void* p);
/* The union entry selected by the union at p: 1 and *index, or 0. */
int rustorb_c_union_entry(const rustorb_c_type* type, const void* p, int64_t* index);

/* Environments. */
void rustorb_c_ev_clear(CORBA_Environment* ev);
void rustorb_c_ev_system(CORBA_Environment* ev, const char* id, CORBA_unsigned_long minor,
                         CORBA_completion_status completed);
/* Sets ev from a status other than RORB_OK and clears exc. */
void rustorb_c_ev_status(CORBA_Environment* ev, rorb_status status, rorb_exception* exc);
/* Returns 1 (and sets ev) if status is not RORB_OK. */
int rustorb_c_failed(CORBA_Environment* ev, rorb_status status, rorb_exception* exc);

char* rustorb_c_strndup(const char* s, size_t len);
PortableServer_ObjectId* rustorb_c_object_id(rorb_bytes bytes);

/* The rorb_servant of a servant (initialised with __init), or NULL. */
rorb_servant* rustorb_c_servant_handle(PortableServer_Servant servant);

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

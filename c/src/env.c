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

void CORBA_exception_free(CORBA_Environment* ev) {
  if (!ev) return;
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
static const rustorb_c_type invalid_policy_desc = {
    CORBA_tk_except, sizeof(PortableServer_POA_InvalidPolicy),
    "IDL:omg.org/PortableServer/POA/InvalidPolicy:1.0", "InvalidPolicy", NULL, 0, 1,
    invalid_policy_members, NULL, -1, 0, 0, NULL};
static const rustorb_c_member policy_error_members[] = {
    {"reason", &CORBA_short__desc, offsetof(CORBA_PolicyError, reason), 0},
};
static const rustorb_c_type policy_error_desc = {
    CORBA_tk_except, sizeof(CORBA_PolicyError), "IDL:omg.org/CORBA/PolicyError:1.0",
    "PolicyError", NULL, 0, 1, policy_error_members, NULL, -1, 0, 0, NULL};

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

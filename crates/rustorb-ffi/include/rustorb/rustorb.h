/*
 * rustorb.h: the stable C ABI of rustorb (crates/rustorb-ffi).
 *
 * The C++ binding (cpp/include/rustorb) is written on top of this header,
 * and so will the Java one be. It is not meant to be used directly by
 * applications.
 *
 * Conventions
 *
 * - Handles (rorb_orb, rorb_object, rorb_poa, rorb_poa_manager,
 *   rorb_servant) are reference counted on the Rust side. A function that
 *   returns a handle gives the caller one reference, which it gives back
 *   with the matching *_release function. *_clone adds a reference.
 *   Handles may be used from any thread.
 * - Fallible functions return an rorb_status and fill the rorb_exception
 *   the caller passes (never NULL). After RORB_SYSTEM_EXCEPTION or
 *   RORB_LOCAL_EXCEPTION the caller frees it with rorb_exception_clear.
 * - Strings are UTF-8 and passed as a pointer and a length, without a
 *   terminating NUL. Wide strings are UTF-16 code units.
 * - Memory returned in an rorb_bytes is freed with rorb_bytes_free.
 * - Streams (rorb_out, rorb_in) and server replies (rorb_server_reply) are
 *   only valid during the callback that receives them. Errors on a stream
 *   are sticky: after the first one, writes do nothing and reads return
 *   zeros, and the error is reported when the callback returns.
 * - Callbacks must not unwind (throw a C++ exception) into Rust.
 *
 * Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.
 */

#ifndef RUSTORB_RUSTORB_H
#define RUSTORB_RUSTORB_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Version of this ABI; rorb_abi_version() returns the library's. */
#define RORB_ABI_VERSION 1

typedef struct rorb_orb rorb_orb;
typedef struct rorb_object rorb_object;
typedef struct rorb_poa rorb_poa;
typedef struct rorb_poa_manager rorb_poa_manager;
typedef struct rorb_servant rorb_servant;
typedef struct rorb_out rorb_out;
typedef struct rorb_in rorb_in;
typedef struct rorb_server_reply rorb_server_reply;

/* --- status and exceptions ------------------------------------------- */

typedef int32_t rorb_status;

/* Success. */
#define RORB_OK 0
/* A CORBA system exception, described in the rorb_exception. */
#define RORB_SYSTEM_EXCEPTION 1
/* A user exception, taken by the read_user_exception callback. */
#define RORB_USER_EXCEPTION 2
/* An exception of a local interface (POA, POAManager, Current, ORB),
   named by rorb_exception.local. */
#define RORB_LOCAL_EXCEPTION 3
/* A callback failed and keeps its own error (a C++ exception, say). */
#define RORB_CALLBACK_FAILED 4

/* CORBA::CompletionStatus */
#define RORB_COMPLETED_YES 0
#define RORB_COMPLETED_NO 1
#define RORB_COMPLETED_MAYBE 2

/* rorb_exception.local after RORB_LOCAL_EXCEPTION */
#define RORB_ADAPTER_ALREADY_EXISTS 1 /* POA::AdapterAlreadyExists */
#define RORB_ADAPTER_NON_EXISTENT 2   /* POA::AdapterNonExistent */
#define RORB_INVALID_POLICY 3         /* POA::InvalidPolicy (index in .index) */
#define RORB_SERVANT_ALREADY_ACTIVE 4 /* POA::ServantAlreadyActive */
#define RORB_OBJECT_ALREADY_ACTIVE 5  /* POA::ObjectAlreadyActive */
#define RORB_SERVANT_NOT_ACTIVE 6     /* POA::ServantNotActive */
#define RORB_OBJECT_NOT_ACTIVE 7      /* POA::ObjectNotActive */
#define RORB_WRONG_POLICY 8           /* POA::WrongPolicy */
#define RORB_WRONG_ADAPTER 9          /* POA::WrongAdapter */
#define RORB_ADAPTER_INACTIVE 10      /* POAManager::AdapterInactive */
#define RORB_NO_SERVANT 11            /* POA::NoServant */
#define RORB_NO_CONTEXT 12            /* Current::NoContext */
#define RORB_INVALID_NAME 13          /* ORB::InvalidName */

typedef struct rorb_exception {
  /* RORB_SYSTEM_EXCEPTION: the repository id, such as
     "IDL:omg.org/CORBA/TRANSIENT:1.0" (owned, NUL terminated). */
  char* repository_id;
  size_t repository_id_len;
  uint32_t minor;
  uint32_t completed;
  /* RORB_LOCAL_EXCEPTION: which one, and the policy index of
     RORB_INVALID_POLICY. */
  uint32_t local;
  uint32_t index;
} rorb_exception;

/* Frees what an rorb_exception holds and zeroes it. */
void rorb_exception_clear(rorb_exception* exc);

typedef struct rorb_bytes {
  uint8_t* data;
  size_t len;
} rorb_bytes;

void rorb_bytes_free(rorb_bytes bytes);

uint32_t rorb_abi_version(void);

/* --- ORB --------------------------------------------------------------- */

/* CORBA::ORB_init: removes the ORB options (-ORB...) from argv and
   *argc; argv[0] is kept. */
rorb_status rorb_orb_init(int* argc, char** argv, rorb_orb** orb, rorb_exception* exc);
rorb_orb* rorb_orb_clone(rorb_orb* orb);
void rorb_orb_release(rorb_orb* orb);

/* *obj is NULL for a nil reference. */
rorb_status rorb_orb_string_to_object(rorb_orb* orb, const char* s, size_t len,
                                      rorb_object** obj, rorb_exception* exc);
/* obj may be NULL (a nil reference). */
rorb_status rorb_orb_object_to_string(rorb_orb* orb, rorb_object* obj, rorb_bytes* s,
                                      rorb_exception* exc);
/* Not for "RootPOA" and "POACurrent", which are local objects: see
   rorb_orb_root_poa. Unknown names give RORB_INVALID_NAME. */
rorb_status rorb_orb_resolve_initial_references(rorb_orb* orb, const char* name, size_t len,
                                                rorb_object** obj, rorb_exception* exc);
/* Calls each(context, name, len) for each initial service. */
void rorb_orb_list_initial_services(rorb_orb* orb, void* context,
                                    void (*each)(void* context, const char* name, size_t len));
rorb_status rorb_orb_root_poa(rorb_orb* orb, rorb_poa** poa, rorb_exception* exc);
/* Blocks until the ORB is shut down, running MAIN_THREAD_MODEL requests. */
void rorb_orb_run(rorb_orb* orb);
rorb_status rorb_orb_work_pending(rorb_orb* orb, int32_t* pending, rorb_exception* exc);
rorb_status rorb_orb_perform_work(rorb_orb* orb, rorb_exception* exc);
rorb_status rorb_orb_shutdown(rorb_orb* orb, int32_t wait_for_completion, rorb_exception* exc);
rorb_status rorb_orb_destroy(rorb_orb* orb, rorb_exception* exc);
/* The RootPOA of the first ORB initialised and not yet destroyed (the
   default POA of servants). */
rorb_status rorb_default_root_poa(rorb_poa** poa, rorb_exception* exc);

/* --- object references -------------------------------------------------- */

rorb_object* rorb_object_clone(rorb_object* obj);
void rorb_object_release(rorb_object* obj);
rorb_orb* rorb_object_orb(rorb_object* obj);
/* The repository id in the IOR (may be empty), valid while obj lives. */
const char* rorb_object_type_id(rorb_object* obj, size_t* len);
rorb_status rorb_object_is_a(rorb_object* obj, const char* id, size_t len, int32_t* result,
                             rorb_exception* exc);
rorb_status rorb_object_non_existent(rorb_object* obj, int32_t* result, rorb_exception* exc);
int32_t rorb_object_is_equivalent(rorb_object* a, rorb_object* b);
/* The same value for equivalent references, below maximum. */
uint32_t rorb_object_hash(rorb_object* obj, uint32_t maximum);

/* How to make one invocation.

   write_args writes the in and inout arguments; it may run more than once
   (when the request is forwarded or sent again). read_reply reads the
   result and the inout and out arguments. read_user_exception is called
   for a user exception with its repository id; it returns
   RORB_USER_EXCEPTION when it took the exception, RORB_SYSTEM_EXCEPTION
   when the operation does not declare it (the invocation then fails with
   UNKNOWN), or RORB_CALLBACK_FAILED. The other callbacks return RORB_OK
   or RORB_CALLBACK_FAILED. Any callback may be NULL. */
typedef struct rorb_invocation {
  const char* operation;
  size_t operation_len;
  int32_t oneway;
  /* Reply timeout in microseconds; negative uses the ORB's setting, and 0
     waits forever. */
  int64_t timeout_us;
  void* context;
  rorb_status (*write_args)(void* context, rorb_out* out);
  rorb_status (*read_reply)(void* context, rorb_in* in);
  rorb_status (*read_user_exception)(void* context, const char* id, size_t id_len, rorb_in* in);
} rorb_invocation;

/* Returns RORB_OK, RORB_SYSTEM_EXCEPTION, RORB_USER_EXCEPTION or
   RORB_CALLBACK_FAILED. A stream error in a read callback is MARSHAL. */
rorb_status rorb_object_invoke(rorb_object* obj, const rorb_invocation* inv, rorb_exception* exc);

/* --- streams --------------------------------------------------------------- */

/* Element kinds of rorb_out_write_array / rorb_in_read_array. */
#define RORB_PRIM_OCTET 0 /* also char */
#define RORB_PRIM_SHORT 1
#define RORB_PRIM_USHORT 2
#define RORB_PRIM_LONG 3
#define RORB_PRIM_ULONG 4
#define RORB_PRIM_LONGLONG 5
#define RORB_PRIM_ULONGLONG 6
#define RORB_PRIM_FLOAT 7
#define RORB_PRIM_DOUBLE 8

void rorb_out_write_boolean(rorb_out* out, int32_t v);
void rorb_out_write_octet(rorb_out* out, uint8_t v);
void rorb_out_write_char(rorb_out* out, char v);
void rorb_out_write_wchar(rorb_out* out, uint16_t v);
void rorb_out_write_short(rorb_out* out, int16_t v);
void rorb_out_write_ushort(rorb_out* out, uint16_t v);
void rorb_out_write_long(rorb_out* out, int32_t v);
void rorb_out_write_ulong(rorb_out* out, uint32_t v);
void rorb_out_write_longlong(rorb_out* out, int64_t v);
void rorb_out_write_ulonglong(rorb_out* out, uint64_t v);
void rorb_out_write_float(rorb_out* out, float v);
void rorb_out_write_double(rorb_out* out, double v);
/* A string in the negotiated code set; s must be UTF-8. */
void rorb_out_write_string(rorb_out* out, const char* s, size_t len);
void rorb_out_write_wstring(rorb_out* out, const uint16_t* s, size_t len);
/* obj may be NULL (a nil reference). */
void rorb_out_write_object(rorb_out* out, rorb_object* obj);
/* count elements of kind, in the host's representation. */
void rorb_out_write_array(rorb_out* out, int32_t kind, const void* data, size_t count);
/* Whether an error happened on the stream. */
int32_t rorb_out_failed(rorb_out* out);

int32_t rorb_in_read_boolean(rorb_in* in);
uint8_t rorb_in_read_octet(rorb_in* in);
char rorb_in_read_char(rorb_in* in);
uint16_t rorb_in_read_wchar(rorb_in* in);
int16_t rorb_in_read_short(rorb_in* in);
uint16_t rorb_in_read_ushort(rorb_in* in);
int32_t rorb_in_read_long(rorb_in* in);
uint32_t rorb_in_read_ulong(rorb_in* in);
int64_t rorb_in_read_longlong(rorb_in* in);
uint64_t rorb_in_read_ulonglong(rorb_in* in);
float rorb_in_read_float(rorb_in* in);
double rorb_in_read_double(rorb_in* in);
/* A string as UTF-8, valid until the next call on the stream. */
const char* rorb_in_read_string(rorb_in* in, size_t* len);
/* A wstring as UTF-16, valid until the next call on the stream. */
const uint16_t* rorb_in_read_wstring(rorb_in* in, size_t* len);
/* NULL for a nil reference (or an error). */
rorb_object* rorb_in_read_object(rorb_in* in);
void rorb_in_read_array(rorb_in* in, int32_t kind, void* data, size_t count);
/* A sequence length, checked against the bytes left for elements of at
   least min_element_size bytes. */
uint32_t rorb_in_read_length(rorb_in* in, size_t min_element_size);
/* Marks the stream as failed (MARSHAL), for a value the caller rejects. */
void rorb_in_fail(rorb_in* in);
int32_t rorb_in_failed(rorb_in* in);

/* --- servants --------------------------------------------------------------- */

/* The callbacks of a servant. self is the pointer given to
   rorb_servant_get. dispatch reads the arguments from in and writes the
   result to reply; it returns RORB_OK (also after
   rorb_server_reply_user_exception) or RORB_SYSTEM_EXCEPTION (after
   rorb_server_reply_system_exception). _is_a, _non_existent, _interface
   and _repository_id are answered by the ORB and do not reach dispatch. */
typedef struct rorb_servant_vtable {
  rorb_status (*dispatch)(void* self, const char* op, size_t op_len, rorb_in* in,
                          rorb_server_reply* reply);
  int32_t (*is_a)(void* self, const char* id, size_t len);
  /* The Rust side took a reference to self... */
  void (*add_ref)(void* self);
  /* ...and gives it back. */
  void (*remove_ref)(void* self);
} rorb_servant_vtable;

/* The Rust side of the servant self, made the first time (then
   vtable->add_ref(self) is called) and the same while the POA or a handle
   holds it. The vtable must outlive the servant. */
rorb_servant* rorb_servant_get(void* self, const char* repository_id, size_t len,
                               const rorb_servant_vtable* vtable);
void rorb_servant_release(rorb_servant* servant);
/* If the calling thread runs a request for servant, *obj is the target
   object and the result is 1; otherwise 0. */
int32_t rorb_servant_current_reference(rorb_servant* servant, rorb_object** obj);

rorb_out* rorb_server_reply_output(rorb_server_reply* reply);
/* Replaces the reply with the user exception id; its members are written
   to the stream returned. */
rorb_out* rorb_server_reply_user_exception(rorb_server_reply* reply, const char* id,
                                           size_t len);
void rorb_server_reply_system_exception(rorb_server_reply* reply, const char* id, size_t len,
                                        uint32_t minor, uint32_t completed);

/* --- POA ------------------------------------------------------------------- */

/* Policy types (CORBA::PolicyType) and values (the IDL enumerators in
   order, as in PortableServer.idl). */
#define RORB_THREAD_POLICY_ID 16
#define RORB_LIFESPAN_POLICY_ID 17
#define RORB_ID_UNIQUENESS_POLICY_ID 18
#define RORB_ID_ASSIGNMENT_POLICY_ID 19
#define RORB_IMPLICIT_ACTIVATION_POLICY_ID 20
#define RORB_SERVANT_RETENTION_POLICY_ID 21
#define RORB_REQUEST_PROCESSING_POLICY_ID 22

typedef struct rorb_policy {
  uint32_t type;
  uint32_t value;
} rorb_policy;

rorb_poa* rorb_poa_clone(rorb_poa* poa);
void rorb_poa_release(rorb_poa* poa);
/* Valid while poa lives. */
const char* rorb_poa_name(rorb_poa* poa, size_t* len);
/* NULL for the RootPOA. */
rorb_poa* rorb_poa_parent(rorb_poa* poa);
void rorb_poa_children(rorb_poa* poa, void* context, void (*each)(void* context, rorb_poa* child));
rorb_poa_manager* rorb_poa_the_manager(rorb_poa* poa);
/* The seven policies of the POA, in the order of the policy ids. */
void rorb_poa_policies(rorb_poa* poa, rorb_policy policies[7]);
/* manager may be NULL for a new one. */
rorb_status rorb_poa_create_poa(rorb_poa* poa, const char* name, size_t len,
                                rorb_poa_manager* manager, const rorb_policy* policies,
                                size_t count, rorb_poa** child, rorb_exception* exc);
rorb_status rorb_poa_find_poa(rorb_poa* poa, const char* name, size_t len, int32_t activate_it,
                              rorb_poa** child, rorb_exception* exc);
rorb_status rorb_poa_destroy(rorb_poa* poa, int32_t etherealize, int32_t wait_for_completion,
                             rorb_exception* exc);
rorb_status rorb_poa_activate_object(rorb_poa* poa, rorb_servant* servant, rorb_bytes* id,
                                     rorb_exception* exc);
rorb_status rorb_poa_activate_object_with_id(rorb_poa* poa, const uint8_t* id, size_t len,
                                             rorb_servant* servant, rorb_exception* exc);
rorb_status rorb_poa_deactivate_object(rorb_poa* poa, const uint8_t* id, size_t len,
                                       rorb_exception* exc);
rorb_status rorb_poa_servant_to_id(rorb_poa* poa, rorb_servant* servant, rorb_bytes* id,
                                   rorb_exception* exc);
rorb_status rorb_poa_servant_to_reference(rorb_poa* poa, rorb_servant* servant,
                                          rorb_object** obj, rorb_exception* exc);
/* *self is a servant made with rorb_servant_get, with a reference added
   for the caller (vtable->add_ref). A servant of another kind is
   OBJ_ADAPTER. */
rorb_status rorb_poa_id_to_servant(rorb_poa* poa, const uint8_t* id, size_t len, void** self,
                                   rorb_exception* exc);
rorb_status rorb_poa_id_to_reference(rorb_poa* poa, const uint8_t* id, size_t len,
                                     rorb_object** obj, rorb_exception* exc);
rorb_status rorb_poa_reference_to_id(rorb_poa* poa, rorb_object* obj, rorb_bytes* id,
                                     rorb_exception* exc);
rorb_status rorb_poa_reference_to_servant(rorb_poa* poa, rorb_object* obj, void** self,
                                          rorb_exception* exc);
rorb_status rorb_poa_create_reference(rorb_poa* poa, const char* intf, size_t len,
                                      rorb_object** obj, rorb_exception* exc);
rorb_status rorb_poa_create_reference_with_id(rorb_poa* poa, const uint8_t* id, size_t id_len,
                                              const char* intf, size_t len, rorb_object** obj,
                                              rorb_exception* exc);
/* The default servant (USE_DEFAULT_SERVANT). */
rorb_status rorb_poa_get_servant(rorb_poa* poa, void** self, rorb_exception* exc);
rorb_status rorb_poa_set_servant(rorb_poa* poa, rorb_servant* servant, rorb_exception* exc);

/* PortableServer::Current; outside a request RORB_NO_CONTEXT. */
rorb_status rorb_current_get_poa(rorb_poa** poa, rorb_exception* exc);
rorb_status rorb_current_get_object_id(rorb_bytes* id, rorb_exception* exc);
rorb_status rorb_current_get_reference(rorb_object** obj, rorb_exception* exc);
rorb_status rorb_current_get_servant(void** self, rorb_exception* exc);

/* POAManager::State */
#define RORB_POA_MANAGER_HOLDING 0
#define RORB_POA_MANAGER_ACTIVE 1
#define RORB_POA_MANAGER_DISCARDING 2
#define RORB_POA_MANAGER_INACTIVE 3

rorb_poa_manager* rorb_poa_manager_clone(rorb_poa_manager* manager);
void rorb_poa_manager_release(rorb_poa_manager* manager);
uint32_t rorb_poa_manager_get_state(rorb_poa_manager* manager);
rorb_status rorb_poa_manager_activate(rorb_poa_manager* manager, rorb_exception* exc);
rorb_status rorb_poa_manager_hold_requests(rorb_poa_manager* manager,
                                           int32_t wait_for_completion, rorb_exception* exc);
rorb_status rorb_poa_manager_discard_requests(rorb_poa_manager* manager,
                                              int32_t wait_for_completion, rorb_exception* exc);
rorb_status rorb_poa_manager_deactivate(rorb_poa_manager* manager, int32_t etherealize,
                                        int32_t wait_for_completion, rorb_exception* exc);

#ifdef __cplusplus
}
#endif

#endif

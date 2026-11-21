/*
 * rustorb/dii.h: the Dynamic Invocation and Skeleton Interfaces of the OMG
 * C Language Mapping 1.0 (CORBA_NVList, CORBA_NamedValue, CORBA_Request,
 * CORBA_ServerRequest and PortableServer_DynamicImpl) on rustorb.
 *
 * Values go in and out as CORBA_any (a CORBA_NamedValue's argument):
 *
 * - An in value is read from the storage _value points to when the
 *   request is sent; with CORBA_IN_COPY_VALUE the list copies it first.
 * - An out or inout value, and the result, are written after the call into
 *   the storage _value points to (for an inout one, what it held is freed
 *   first), or, when _value is NULL, into storage the ORB allocates
 *   (_release set; freed with the list, or by CORBA_NVList_free_memory).
 *   The servant side of the DSI is the same, the ORB reading the in and
 *   inout values into the list and sending the out and inout ones from it.
 *   A value of a type the
 *   program has no generated code for stays in the any as it came, as for
 *   any other CORBA_any.
 *
 * Contexts are not supported (ctx must be NULL). See docs/c-binding.md.
 *
 * Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.
 */

#ifndef RUSTORB_C_DII_H
#define RUSTORB_C_DII_H

#include <rustorb/orb.h>
#include <rustorb/poa.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef CORBA_unsigned_long CORBA_Flags;

/* Argument modes (CORBA::ARG_IN...). */
#define CORBA_ARG_IN 1u
#define CORBA_ARG_OUT 2u
#define CORBA_ARG_INOUT 3u
/* CORBA_NVList_add_item and CORBA_Request_add_arg: copy the in value. */
#define CORBA_IN_COPY_VALUE 0x8u
/* CORBA_Request_send: a oneway call. */
#define CORBA_INV_NO_RESPONSE 0x20u
/* CORBA_Request_get_response: return at once, doing nothing, if the reply
   has not come (CORBA_Request_poll_response tells). */
#define CORBA_RESP_NO_WAIT 0x40u

typedef struct CORBA_Context_struct* CORBA_Context;

typedef struct CORBA_NamedValue {
  CORBA_Identifier name;
  CORBA_any argument;
  CORBA_long len;
  CORBA_Flags arg_modes;
} CORBA_NamedValue;

typedef struct CORBA_NVList_struct* CORBA_NVList;
typedef struct CORBA_Request_struct* CORBA_Request;
typedef struct CORBA_ServerRequest_struct* CORBA_ServerRequest;

/* --- NVList -------------------------------------------------------------- */

/* An empty list (count is a hint). */
void CORBA_ORB_create_list(CORBA_ORB orb, CORBA_long count, CORBA_NVList* new_list,
                           CORBA_Environment* ev);
/* Adds item_name (copied) of item_type (duplicated) with the value at
   value: in, out or inout (item_flags), and CORBA_IN_COPY_VALUE. value may
   be NULL: for an out item, or one of the list given to
   CORBA_ServerRequest_arguments (sending an in or inout one without a
   value is BAD_PARAM). value_len is kept as len. */
void CORBA_NVList_add_item(CORBA_NVList list, const CORBA_char* item_name,
                           CORBA_TypeCode item_type, void* value, CORBA_long value_len,
                           CORBA_Flags item_flags, CORBA_Environment* ev);
void CORBA_NVList_get_count(CORBA_NVList list, CORBA_long* count, CORBA_Environment* ev);
/* rustorb: item index (borrowed; NULL with BAD_PARAM past the end). */
CORBA_NamedValue* CORBA_NVList_item(CORBA_NVList list, CORBA_unsigned_long index,
                                    CORBA_Environment* ev);
/* Frees the values the ORB allocated for the items (those with _release
   set) and sets their _value to NULL. */
void CORBA_NVList_free_memory(CORBA_NVList list, CORBA_Environment* ev);
/* Frees the list, its items and the values it owns. */
void CORBA_NVList_free(CORBA_NVList list, CORBA_Environment* ev);

/* --- Request ------------------------------------------------------------- */

/* A request for operation on obj. arg_list (may be NULL: then
   CORBA_Request_add_arg) becomes the request's, freed with it. result
   (may be NULL: a void result, or one CORBA_Request_result gives) stays
   the caller's: its argument._type is the return type and gets the
   value. req_flags is 0. */
void CORBA_Object_create_request(CORBA_Object obj, CORBA_Context ctx,
                                 const CORBA_char* operation, CORBA_NVList arg_list,
                                 CORBA_NamedValue* result, CORBA_Request* request,
                                 CORBA_Flags req_flags, CORBA_Environment* ev);
/* CORBA_NVList_add_item on the request's arguments. */
void CORBA_Request_add_arg(CORBA_Request req, const CORBA_char* name, CORBA_TypeCode arg_type,
                           void* value, CORBA_long len, CORBA_Flags arg_flags,
                           CORBA_Environment* ev);
/* rustorb: a user exception the operation may raise. One that is not
   given comes back as UNKNOWN (minor 0x4f4d0001). */
void CORBA_Request_add_exception(CORBA_Request req, CORBA_TypeCode exception_type,
                                 CORBA_Environment* ev);
/* rustorb: the request's arguments and result (borrowed). The result is
   the one given to create_request, or the request's own (_type tk_void
   until set). */
CORBA_NVList CORBA_Request_arguments(CORBA_Request req, CORBA_Environment* ev);
CORBA_NamedValue* CORBA_Request_result(CORBA_Request req, CORBA_Environment* ev);
/* Sends the request and waits for the reply. A user exception of the list
   is a CORBA_USER_EXCEPTION in ev: its id, and its value as the generated
   code's struct (NULL if the program has no generated code for it). */
void CORBA_Request_invoke(CORBA_Request req, CORBA_Flags invoke_flags, CORBA_Environment* ev);
/* Sends without waiting (CORBA_INV_NO_RESPONSE: a oneway call). */
void CORBA_Request_send(CORBA_Request req, CORBA_Flags invoke_flags, CORBA_Environment* ev);
/* rustorb: whether the reply of a send has come (BAD_INV_ORDER when
   nothing was sent). */
CORBA_boolean CORBA_Request_poll_response(CORBA_Request req, CORBA_Environment* ev);
/* Waits for the reply of a send; the outcome is that of invoke. With
   CORBA_RESP_NO_WAIT and no reply yet, returns with nothing done. */
void CORBA_Request_get_response(CORBA_Request req, CORBA_Flags response_flags,
                                CORBA_Environment* ev);
/* Frees the request and its arguments. */
void CORBA_Request_delete(CORBA_Request req, CORBA_Environment* ev);

/* --- ServerRequest --------------------------------------------------------- */

/* The operation (borrowed; _get_x for an attribute). */
CORBA_Identifier CORBA_ServerRequest_operation(CORBA_ServerRequest req, CORBA_Environment* ev);
/* Reads the in and inout arguments into the items of parameters, whose
   types and modes the servant gives (with NULL values: the ORB allocates
   them, and zeroed storage for the out ones, which the servant sets). The list becomes the request's: the out and inout values are
   sent from it when invoke returns, and it is freed with the request.
   Once (else BAD_INV_ORDER); a list that does not fit the request is
   MARSHAL (BAD_PARAM for a wrong one). */
void CORBA_ServerRequest_arguments(CORBA_ServerRequest req, CORBA_NVList parameters,
                                   CORBA_Environment* ev);
/* The result (copied). */
void CORBA_ServerRequest_set_result(CORBA_ServerRequest req, const CORBA_any* value,
                                    CORBA_Environment* ev);
/* major CORBA_USER_EXCEPTION: value is an any of the user exception
   (copied). major CORBA_SYSTEM_EXCEPTION: value->_type gives the id and
   value->_value is a CORBA_SystemException (or NULL). */
void CORBA_ServerRequest_set_exception(CORBA_ServerRequest req, CORBA_exception_type major,
                                       const CORBA_any* value, CORBA_Environment* ev);
/* rustorb: the reply is the system exception id. */
void CORBA_ServerRequest_set_system_exception(CORBA_ServerRequest req, const CORBA_char* id,
                                              CORBA_unsigned_long minor,
                                              CORBA_completion_status completed,
                                              CORBA_Environment* ev);
/* The names of the C mapping 1.0. */
#define CORBA_ServerRequest_params CORBA_ServerRequest_arguments
#define CORBA_ServerRequest_result CORBA_ServerRequest_set_result
#define CORBA_ServerRequest_exception CORBA_ServerRequest_set_exception

/* --- DynamicImpl ----------------------------------------------------------- */

/* Called for each request; the reply is what it set on request (nothing:
   a void result with the out values of the arguments). */
typedef void (*PortableServer_DynamicImplRoutine)(PortableServer_Servant servant,
                                                  CORBA_ServerRequest request);
/* The repository id of the servant's references (CORBA_string_dup; the
   ORB frees it). Called once, from __init, with a NULL id and POA. */
typedef CORBA_RepositoryId (*PortableServer_PrimaryInterfaceRoutine)(
    PortableServer_Servant servant, PortableServer_ObjectId* oid, PortableServer_POA poa,
    CORBA_Environment* ev);

typedef struct PortableServer_DynamicImpl__epv {
  void* _private;
  PortableServer_DynamicImplRoutine invoke;
  PortableServer_PrimaryInterfaceRoutine primary_interface;
} PortableServer_DynamicImpl__epv;

typedef struct POA_PortableServer_DynamicImpl__vepv {
  PortableServer_ServantBase__epv* _base_epv;
  PortableServer_DynamicImpl__epv* PortableServer_DynamicImpl_epv;
} POA_PortableServer_DynamicImpl__vepv;

typedef struct POA_PortableServer_DynamicImpl {
  void* _private;
  POA_PortableServer_DynamicImpl__vepv* vepv;
} POA_PortableServer_DynamicImpl;
typedef POA_PortableServer_DynamicImpl PortableServer_DynamicImpl;

/* The servant answers _is_a for its primary interface (and
   CORBA::Object). */
void PortableServer_DynamicImpl__init(PortableServer_Servant servant, CORBA_Environment* ev);
void PortableServer_DynamicImpl__fini(PortableServer_Servant servant, CORBA_Environment* ev);

#ifdef __cplusplus
}
#endif

#endif /* RUSTORB_C_DII_H */

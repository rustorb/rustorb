/*
 * rustorb/messaging.h: the part of the Messaging module that callback AMI
 * uses (docs/ami.md), for the code of rustorb-idl -bc -Wbami.
 *
 * Messaging::ReplyHandler is the base of the AMI_<I>Handler interfaces.
 * The C mapping has no valuetypes; Messaging::ExceptionHolder is the
 * struct Messaging_ExceptionHolder with the state of the valuetype (sent
 * as the valuetype), and an <op>_excep operation of a handler receives it
 * as an in struct: const Messaging_ExceptionHolder*.
 * Messaging_ExceptionHolder_raise_exception sets an environment to the
 * exception it holds.
 *
 * Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.
 */

#ifndef RUSTORB_C_MESSAGING_H
#define RUSTORB_C_MESSAGING_H

#include <rustorb/runtime.h>

#ifdef __cplusplus
extern "C" {
#endif

/* --- Messaging::ReplyHandler --------------------------------------------- */

#ifndef _Messaging_ReplyHandler_defined
#define _Messaging_ReplyHandler_defined
typedef CORBA_Object Messaging_ReplyHandler;
extern const rustorb_c_type Messaging_ReplyHandler__desc;
#define TC_Messaging_ReplyHandler rustorb_c_type_tc(&Messaging_ReplyHandler__desc, NULL)
#endif

/* No operations: the slot of ReplyHandler in the vepv of a handler. */
typedef struct POA_Messaging_ReplyHandler__epv {
  void* _private;
} POA_Messaging_ReplyHandler__epv;

typedef struct POA_Messaging_ReplyHandler__vepv {
  PortableServer_ServantBase__epv* _base_epv;
  POA_Messaging_ReplyHandler__epv* Messaging_ReplyHandler_epv;
} POA_Messaging_ReplyHandler__vepv;

typedef struct POA_Messaging_ReplyHandler {
  void* _private;
  POA_Messaging_ReplyHandler__vepv* vepv;
} POA_Messaging_ReplyHandler;

/* --- Messaging::ExceptionHolder ------------------------------------------ */

/* The state of the valuetype: marshaled_exception holds the exception as
   in a reply body (the repository id, then the members of a user
   exception, or the minor code and completion status of a system
   exception), plain CDR from offset 0 in the byte order byte_order says
   (TRUE: little endian), with the code sets UTF-8 and UTF-16. */
typedef struct Messaging_ExceptionHolder {
  CORBA_boolean is_system_exception;
  CORBA_boolean byte_order;
  CORBA_sequence_octet marshaled_exception;
} Messaging_ExceptionHolder;

extern const rustorb_c_type Messaging_ExceptionHolder__desc;
#define TC_Messaging_ExceptionHolder rustorb_c_type_tc(&Messaging_ExceptionHolder__desc, NULL)
Messaging_ExceptionHolder* Messaging_ExceptionHolder__alloc(void);

/* Sets ev to the exception the holder holds: a system exception, or a
   user exception of the generated code (read with its descriptor; one
   whose file has not been used in this program yet, or not known at all,
   is UNKNOWN minor 0x4f4d0001). MARSHAL when marshaled_exception does not
   read. */
void Messaging_ExceptionHolder_raise_exception(const Messaging_ExceptionHolder* holder,
                                               CORBA_Environment* ev);

/* Like raise_exception, but a user exception whose TypeCode is not one of
   the count TypeCodes of list is UNKNOWN minor 0x4f4d0001. */
void Messaging_ExceptionHolder_raise_exception_with_list(const Messaging_ExceptionHolder* holder,
                                                         const CORBA_TypeCode* list,
                                                         CORBA_unsigned_long count,
                                                         CORBA_Environment* ev);

/* --- for the generated code ----------------------------------------------- */

/* A sendc_ operation (rustorb-idl -Wbami): sends op on obj without waiting.
   ins points to the values of the in and inout parameters of op, in
   order (as rustorb_c_invoke's args); they are copied. When the reply
   comes, on a thread of the ORB's reply pool, reply is called on handler
   with the result and the inout and out values, or excep with the
   ExceptionHolder; with a nil handler the reply is dropped. ev gets only
   what is known before sending (BAD_INV_ORDER once the ORB is shut
   down). */
void rustorb_c_sendc(CORBA_Object obj, const rustorb_c_operation* op, void** ins,
                     CORBA_Object handler, const rustorb_c_operation* reply,
                     const rustorb_c_operation* excep, CORBA_Environment* ev);

#ifdef __cplusplus
}
#endif

#endif /* RUSTORB_C_MESSAGING_H */

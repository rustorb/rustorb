/*
 * rustorb/pi.h: Portable Interceptors and IOP::Codec on the C mapping
 * (docs/portable-interceptors.md, docs/c-binding.md).
 *
 * The C mapping 1.0 has neither. rustorb maps the IDL of the
 * PortableInterceptor and IOP modules as rustorb-idl -bc would:
 *
 * - the interceptors and ORB initializers are local interfaces: the
 *   application fills a POA_PortableInterceptor_<I> struct (vepv with the
 *   Interceptor epv, _get_name and destroy, then the interceptor's own),
 *   calls POA_..._<I>__init and makes the local object with
 *   PortableInterceptor_<I>__create; the struct must outlive the ORB;
 * - RequestInfo, ClientRequestInfo, ServerRequestInfo, IORInfo and
 *   ORBInitInfo are pseudo objects valid only during the call of the point
 *   that receives them (no need to release them; a duplicate is no longer
 *   valid after the call either). The RequestInfo functions take a client
 *   or a server info;
 * - an interceptor point raises PortableInterceptor_ForwardRequest or a
 *   system exception through its CORBA_Environment; any other user
 *   exception is UNKNOWN;
 * - the PICurrent and the CodecFactory come from
 *   CORBA_ORB_resolve_initial_references ("PICurrent", "CodecFactory") or
 *   from the ORBInitInfo.
 *
 * Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.
 */

#ifndef RUSTORB_C_PI_H
#define RUSTORB_C_PI_H

#include <rustorb/poa.h>

#ifdef __cplusplus
extern "C" {
#endif

/* --- IOP ------------------------------------------------------------------ */

typedef CORBA_unsigned_long IOP_ProfileId;
typedef CORBA_unsigned_long IOP_ComponentId;
typedef CORBA_unsigned_long IOP_ServiceId;
#define IOP_TAG_INTERNET_IOP 0U
#define IOP_TAG_MULTIPLE_COMPONENTS 1U

typedef struct IOP_TaggedProfile {
  IOP_ProfileId tag;
  CORBA_sequence_octet profile_data;
} IOP_TaggedProfile;

typedef struct IOP_TaggedComponent {
  IOP_ComponentId tag;
  CORBA_sequence_octet component_data;
} IOP_TaggedComponent;
RUSTORB_C_SEQUENCE(CORBA_sequence_IOP_TaggedComponent, IOP_TaggedComponent);
typedef CORBA_sequence_IOP_TaggedComponent IOP_TaggedComponentSeq;

typedef struct IOP_ServiceContext {
  IOP_ServiceId context_id;
  CORBA_sequence_octet context_data;
} IOP_ServiceContext;

IOP_TaggedProfile* IOP_TaggedProfile__alloc(void);
IOP_TaggedComponent* IOP_TaggedComponent__alloc(void);
IOP_TaggedComponentSeq* IOP_TaggedComponentSeq__alloc(void);
IOP_TaggedComponent* CORBA_sequence_IOP_TaggedComponent_allocbuf(CORBA_unsigned_long len);
IOP_ServiceContext* IOP_ServiceContext__alloc(void);

typedef CORBA_short IOP_EncodingFormat;
#define IOP_ENCODING_CDR_ENCAPS 0

typedef struct IOP_Encoding {
  IOP_EncodingFormat format;
  CORBA_octet major_version;
  CORBA_octet minor_version;
} IOP_Encoding;

typedef CORBA_Object IOP_Codec;
typedef CORBA_Object IOP_CodecFactory;

/* The exceptions of Codec and CodecFactory have no members. */
#define ex_IOP_Codec_InvalidTypeForEncoding "IDL:omg.org/IOP/Codec/InvalidTypeForEncoding:1.0"
#define ex_IOP_Codec_FormatMismatch "IDL:omg.org/IOP/Codec/FormatMismatch:1.0"
#define ex_IOP_Codec_TypeMismatch "IDL:omg.org/IOP/Codec/TypeMismatch:1.0"
#define ex_IOP_CodecFactory_UnknownEncoding "IDL:omg.org/IOP/CodecFactory/UnknownEncoding:1.0"

/* The results are freed with CORBA_free. */
CORBA_sequence_octet* IOP_Codec_encode(IOP_Codec codec, const CORBA_any* data,
                                       CORBA_Environment* ev);
CORBA_any* IOP_Codec_decode(IOP_Codec codec, const CORBA_sequence_octet* data,
                            CORBA_Environment* ev);
CORBA_sequence_octet* IOP_Codec_encode_value(IOP_Codec codec, const CORBA_any* data,
                                             CORBA_Environment* ev);
CORBA_any* IOP_Codec_decode_value(IOP_Codec codec, const CORBA_sequence_octet* data,
                                  CORBA_TypeCode tc, CORBA_Environment* ev);

IOP_Codec IOP_CodecFactory_create_codec(IOP_CodecFactory factory, const IOP_Encoding* enc,
                                        CORBA_Environment* ev);

/* --- PortableInterceptor: types ------------------------------------------- */

typedef CORBA_unsigned_long PortableInterceptor_SlotId;
typedef CORBA_short PortableInterceptor_ReplyStatus;
#define PortableInterceptor_SUCCESSFUL 0
#define PortableInterceptor_SYSTEM_EXCEPTION 1
#define PortableInterceptor_USER_EXCEPTION 2
#define PortableInterceptor_LOCATION_FORWARD 3
#define PortableInterceptor_TRANSPORT_RETRY 4
#define PortableInterceptor_UNKNOWN 5

typedef CORBA_short Messaging_SyncScope;

typedef CORBA_unsigned_long CORBA_ParameterMode;
#define CORBA_PARAM_IN 0U
#define CORBA_PARAM_OUT 1U
#define CORBA_PARAM_INOUT 2U

typedef struct Dynamic_Parameter {
  CORBA_any argument;
  CORBA_ParameterMode mode;
} Dynamic_Parameter;
RUSTORB_C_SEQUENCE(CORBA_sequence_Dynamic_Parameter, Dynamic_Parameter);
typedef CORBA_sequence_Dynamic_Parameter Dynamic_ParameterList;
RUSTORB_C_SEQUENCE(CORBA_sequence_CORBA_TypeCode, CORBA_TypeCode);
typedef CORBA_sequence_CORBA_TypeCode Dynamic_ExceptionList;

#define ex_PortableInterceptor_ForwardRequest "IDL:omg.org/PortableInterceptor/ForwardRequest:1.0"
typedef struct PortableInterceptor_ForwardRequest {
  CORBA_Object forward;
} PortableInterceptor_ForwardRequest;
PortableInterceptor_ForwardRequest* PortableInterceptor_ForwardRequest__alloc(void);

/* No members. */
#define ex_PortableInterceptor_InvalidSlot "IDL:omg.org/PortableInterceptor/InvalidSlot:1.0"
#define ex_PortableInterceptor_ORBInitInfo_DuplicateName \
  "IDL:omg.org/PortableInterceptor/ORBInitInfo/DuplicateName:1.0"
#define ex_PortableInterceptor_ORBInitInfo_InvalidName \
  "IDL:omg.org/PortableInterceptor/ORBInitInfo/InvalidName:1.0"

typedef CORBA_Object PortableInterceptor_Interceptor;
typedef CORBA_Object PortableInterceptor_ClientRequestInterceptor;
typedef CORBA_Object PortableInterceptor_ServerRequestInterceptor;
typedef CORBA_Object PortableInterceptor_IORInterceptor;
typedef CORBA_Object PortableInterceptor_ORBInitializer;
typedef CORBA_Object PortableInterceptor_RequestInfo;
typedef CORBA_Object PortableInterceptor_ClientRequestInfo;
typedef CORBA_Object PortableInterceptor_ServerRequestInfo;
typedef CORBA_Object PortableInterceptor_IORInfo;
typedef CORBA_Object PortableInterceptor_ORBInitInfo;
typedef CORBA_Object PortableInterceptor_Current;

/* --- the implementations ---------------------------------------------------- */

typedef struct POA_PortableInterceptor_Interceptor__epv {
  void* _private;
  /* The name (freed by the ORB; "" or NULL for an anonymous one). */
  CORBA_char* (*_get_name)(PortableServer_Servant servant, CORBA_Environment* ev);
  /* When the ORB is destroyed (may be NULL). */
  void (*destroy)(PortableServer_Servant servant, CORBA_Environment* ev);
} POA_PortableInterceptor_Interceptor__epv;

typedef struct POA_PortableInterceptor_ClientRequestInterceptor__epv {
  void* _private;
  void (*send_request)(PortableServer_Servant servant, PortableInterceptor_ClientRequestInfo ri,
                       CORBA_Environment* ev);
  /* Never called (time-independent invocation is not supported). */
  void (*send_poll)(PortableServer_Servant servant, PortableInterceptor_ClientRequestInfo ri,
                    CORBA_Environment* ev);
  void (*receive_reply)(PortableServer_Servant servant, PortableInterceptor_ClientRequestInfo ri,
                        CORBA_Environment* ev);
  void (*receive_exception)(PortableServer_Servant servant,
                            PortableInterceptor_ClientRequestInfo ri, CORBA_Environment* ev);
  void (*receive_other)(PortableServer_Servant servant, PortableInterceptor_ClientRequestInfo ri,
                        CORBA_Environment* ev);
} POA_PortableInterceptor_ClientRequestInterceptor__epv;

typedef struct POA_PortableInterceptor_ClientRequestInterceptor__vepv {
  PortableServer_ServantBase__epv* _base_epv;
  POA_PortableInterceptor_Interceptor__epv* PortableInterceptor_Interceptor_epv;
  POA_PortableInterceptor_ClientRequestInterceptor__epv*
      PortableInterceptor_ClientRequestInterceptor_epv;
} POA_PortableInterceptor_ClientRequestInterceptor__vepv;

typedef struct POA_PortableInterceptor_ClientRequestInterceptor {
  void* _private;
  POA_PortableInterceptor_ClientRequestInterceptor__vepv* vepv;
} POA_PortableInterceptor_ClientRequestInterceptor;

typedef struct POA_PortableInterceptor_ServerRequestInterceptor__epv {
  void* _private;
  void (*receive_request_service_contexts)(PortableServer_Servant servant,
                                           PortableInterceptor_ServerRequestInfo ri,
                                           CORBA_Environment* ev);
  void (*receive_request)(PortableServer_Servant servant, PortableInterceptor_ServerRequestInfo ri,
                          CORBA_Environment* ev);
  void (*send_reply)(PortableServer_Servant servant, PortableInterceptor_ServerRequestInfo ri,
                     CORBA_Environment* ev);
  void (*send_exception)(PortableServer_Servant servant, PortableInterceptor_ServerRequestInfo ri,
                         CORBA_Environment* ev);
  void (*send_other)(PortableServer_Servant servant, PortableInterceptor_ServerRequestInfo ri,
                     CORBA_Environment* ev);
} POA_PortableInterceptor_ServerRequestInterceptor__epv;

typedef struct POA_PortableInterceptor_ServerRequestInterceptor__vepv {
  PortableServer_ServantBase__epv* _base_epv;
  POA_PortableInterceptor_Interceptor__epv* PortableInterceptor_Interceptor_epv;
  POA_PortableInterceptor_ServerRequestInterceptor__epv*
      PortableInterceptor_ServerRequestInterceptor_epv;
} POA_PortableInterceptor_ServerRequestInterceptor__vepv;

typedef struct POA_PortableInterceptor_ServerRequestInterceptor {
  void* _private;
  POA_PortableInterceptor_ServerRequestInterceptor__vepv* vepv;
} POA_PortableInterceptor_ServerRequestInterceptor;

typedef struct POA_PortableInterceptor_IORInterceptor__epv {
  void* _private;
  void (*establish_components)(PortableServer_Servant servant, PortableInterceptor_IORInfo info,
                               CORBA_Environment* ev);
} POA_PortableInterceptor_IORInterceptor__epv;

typedef struct POA_PortableInterceptor_IORInterceptor__vepv {
  PortableServer_ServantBase__epv* _base_epv;
  POA_PortableInterceptor_Interceptor__epv* PortableInterceptor_Interceptor_epv;
  POA_PortableInterceptor_IORInterceptor__epv* PortableInterceptor_IORInterceptor_epv;
} POA_PortableInterceptor_IORInterceptor__vepv;

typedef struct POA_PortableInterceptor_IORInterceptor {
  void* _private;
  POA_PortableInterceptor_IORInterceptor__vepv* vepv;
} POA_PortableInterceptor_IORInterceptor;

typedef struct POA_PortableInterceptor_ORBInitializer__epv {
  void* _private;
  void (*pre_init)(PortableServer_Servant servant, PortableInterceptor_ORBInitInfo info,
                   CORBA_Environment* ev);
  void (*post_init)(PortableServer_Servant servant, PortableInterceptor_ORBInitInfo info,
                    CORBA_Environment* ev);
} POA_PortableInterceptor_ORBInitializer__epv;

typedef struct POA_PortableInterceptor_ORBInitializer__vepv {
  PortableServer_ServantBase__epv* _base_epv;
  POA_PortableInterceptor_ORBInitializer__epv* PortableInterceptor_ORBInitializer_epv;
} POA_PortableInterceptor_ORBInitializer__vepv;

typedef struct POA_PortableInterceptor_ORBInitializer {
  void* _private;
  POA_PortableInterceptor_ORBInitializer__vepv* vepv;
} POA_PortableInterceptor_ORBInitializer;

#define RUSTORB_C_PI_LOCAL(I)                                                              \
  void POA_PortableInterceptor_##I##__init(PortableServer_Servant servant,                \
                                           CORBA_Environment* ev);                        \
  void POA_PortableInterceptor_##I##__fini(PortableServer_Servant servant,                \
                                           CORBA_Environment* ev);                        \
  PortableInterceptor_##I PortableInterceptor_##I##__create(PortableServer_Servant impl, \
                                                            CORBA_Environment* ev)
RUSTORB_C_PI_LOCAL(ClientRequestInterceptor);
RUSTORB_C_PI_LOCAL(ServerRequestInterceptor);
RUSTORB_C_PI_LOCAL(IORInterceptor);
RUSTORB_C_PI_LOCAL(ORBInitializer);
#undef RUSTORB_C_PI_LOCAL

/* Interceptor::name of any of the three interceptors. */
CORBA_char* PortableInterceptor_Interceptor__get_name(PortableInterceptor_Interceptor obj,
                                                      CORBA_Environment* ev);

/* PortableInterceptor::register_orb_initializer: for the ORBs created
   after it (CORBA_ORB_init). */
void PortableInterceptor_register_orb_initializer(PortableInterceptor_ORBInitializer init,
                                                  CORBA_Environment* ev);

/* --- RequestInfo (client and server) ------------------------------------------- */

/* Attributes not available at the point are BAD_INV_ORDER (minor 14);
   arguments, exceptions and result are NO_RESOURCES (minor 1), but for the
   DII and the DSI. Strings, sequences and anys are the caller's, to
   CORBA_free; object references to release. */
CORBA_unsigned_long PortableInterceptor_RequestInfo__get_request_id(
    PortableInterceptor_RequestInfo ri, CORBA_Environment* ev);
CORBA_char* PortableInterceptor_RequestInfo__get_operation(PortableInterceptor_RequestInfo ri,
                                                           CORBA_Environment* ev);
Dynamic_ParameterList* PortableInterceptor_RequestInfo__get_arguments(
    PortableInterceptor_RequestInfo ri, CORBA_Environment* ev);
Dynamic_ExceptionList* PortableInterceptor_RequestInfo__get_exceptions(
    PortableInterceptor_RequestInfo ri, CORBA_Environment* ev);
CORBA_any* PortableInterceptor_RequestInfo__get_result(PortableInterceptor_RequestInfo ri,
                                                       CORBA_Environment* ev);
CORBA_boolean PortableInterceptor_RequestInfo__get_response_expected(
    PortableInterceptor_RequestInfo ri, CORBA_Environment* ev);
Messaging_SyncScope PortableInterceptor_RequestInfo__get_sync_scope(
    PortableInterceptor_RequestInfo ri, CORBA_Environment* ev);
PortableInterceptor_ReplyStatus PortableInterceptor_RequestInfo__get_reply_status(
    PortableInterceptor_RequestInfo ri, CORBA_Environment* ev);
CORBA_Object PortableInterceptor_RequestInfo__get_forward_reference(
    PortableInterceptor_RequestInfo ri, CORBA_Environment* ev);
CORBA_any* PortableInterceptor_RequestInfo_get_slot(PortableInterceptor_RequestInfo ri,
                                                    PortableInterceptor_SlotId id,
                                                    CORBA_Environment* ev);
/* BAD_PARAM (minor 26) when the context is not there. */
IOP_ServiceContext* PortableInterceptor_RequestInfo_get_request_service_context(
    PortableInterceptor_RequestInfo ri, IOP_ServiceId id, CORBA_Environment* ev);
IOP_ServiceContext* PortableInterceptor_RequestInfo_get_reply_service_context(
    PortableInterceptor_RequestInfo ri, IOP_ServiceId id, CORBA_Environment* ev);

/* --- ClientRequestInfo ------------------------------------------------------- */

CORBA_Object PortableInterceptor_ClientRequestInfo__get_target(
    PortableInterceptor_ClientRequestInfo ri, CORBA_Environment* ev);
CORBA_Object PortableInterceptor_ClientRequestInfo__get_effective_target(
    PortableInterceptor_ClientRequestInfo ri, CORBA_Environment* ev);
IOP_TaggedProfile* PortableInterceptor_ClientRequestInfo__get_effective_profile(
    PortableInterceptor_ClientRequestInfo ri, CORBA_Environment* ev);
CORBA_any* PortableInterceptor_ClientRequestInfo__get_received_exception(
    PortableInterceptor_ClientRequestInfo ri, CORBA_Environment* ev);
CORBA_char* PortableInterceptor_ClientRequestInfo__get_received_exception_id(
    PortableInterceptor_ClientRequestInfo ri, CORBA_Environment* ev);
/* BAD_PARAM (minor 28) when the effective profile has no such component. */
IOP_TaggedComponent* PortableInterceptor_ClientRequestInfo_get_effective_component(
    PortableInterceptor_ClientRequestInfo ri, IOP_ComponentId id, CORBA_Environment* ev);
IOP_TaggedComponentSeq* PortableInterceptor_ClientRequestInfo_get_effective_components(
    PortableInterceptor_ClientRequestInfo ri, IOP_ComponentId id, CORBA_Environment* ev);
/* INV_POLICY (minor 2) for a type that is not a client policy; nil when
   none of the type is in effect. */
CORBA_Policy PortableInterceptor_ClientRequestInfo_get_request_policy(
    PortableInterceptor_ClientRequestInfo ri, CORBA_PolicyType type, CORBA_Environment* ev);
/* Only in send_request. */
void PortableInterceptor_ClientRequestInfo_add_request_service_context(
    PortableInterceptor_ClientRequestInfo ri, const IOP_ServiceContext* service_context,
    CORBA_boolean replace, CORBA_Environment* ev);

/* --- ServerRequestInfo --------------------------------------------------------- */

CORBA_any* PortableInterceptor_ServerRequestInfo__get_sending_exception(
    PortableInterceptor_ServerRequestInfo ri, CORBA_Environment* ev);
CORBA_sequence_octet* PortableInterceptor_ServerRequestInfo__get_object_id(
    PortableInterceptor_ServerRequestInfo ri, CORBA_Environment* ev);
CORBA_sequence_octet* PortableInterceptor_ServerRequestInfo__get_adapter_id(
    PortableInterceptor_ServerRequestInfo ri, CORBA_Environment* ev);
CORBA_char* PortableInterceptor_ServerRequestInfo__get_server_id(
    PortableInterceptor_ServerRequestInfo ri, CORBA_Environment* ev);
CORBA_char* PortableInterceptor_ServerRequestInfo__get_orb_id(
    PortableInterceptor_ServerRequestInfo ri, CORBA_Environment* ev);
CORBA_sequence_string* PortableInterceptor_ServerRequestInfo__get_adapter_name(
    PortableInterceptor_ServerRequestInfo ri, CORBA_Environment* ev);
CORBA_char* PortableInterceptor_ServerRequestInfo__get_target_most_derived_interface(
    PortableInterceptor_ServerRequestInfo ri, CORBA_Environment* ev);
CORBA_Policy PortableInterceptor_ServerRequestInfo_get_server_policy(
    PortableInterceptor_ServerRequestInfo ri, CORBA_PolicyType type, CORBA_Environment* ev);
void PortableInterceptor_ServerRequestInfo_set_slot(PortableInterceptor_ServerRequestInfo ri,
                                                    PortableInterceptor_SlotId id,
                                                    const CORBA_any* data, CORBA_Environment* ev);
CORBA_boolean PortableInterceptor_ServerRequestInfo_target_is_a(
    PortableInterceptor_ServerRequestInfo ri, const CORBA_char* id, CORBA_Environment* ev);
void PortableInterceptor_ServerRequestInfo_add_reply_service_context(
    PortableInterceptor_ServerRequestInfo ri, const IOP_ServiceContext* service_context,
    CORBA_boolean replace, CORBA_Environment* ev);

/* --- IORInfo ------------------------------------------------------------------- */

CORBA_Policy PortableInterceptor_IORInfo_get_effective_policy(PortableInterceptor_IORInfo info,
                                                              CORBA_PolicyType type,
                                                              CORBA_Environment* ev);
void PortableInterceptor_IORInfo_add_ior_component(PortableInterceptor_IORInfo info,
                                                   const IOP_TaggedComponent* a_component,
                                                   CORBA_Environment* ev);
/* BAD_PARAM (minor 29) but for IOP_TAG_INTERNET_IOP. */
void PortableInterceptor_IORInfo_add_ior_component_to_profile(
    PortableInterceptor_IORInfo info, const IOP_TaggedComponent* a_component,
    IOP_ProfileId profile_id, CORBA_Environment* ev);

/* --- ORBInitInfo ----------------------------------------------------------------- */

CORBA_sequence_string* PortableInterceptor_ORBInitInfo__get_arguments(
    PortableInterceptor_ORBInitInfo info, CORBA_Environment* ev);
CORBA_char* PortableInterceptor_ORBInitInfo__get_orb_id(PortableInterceptor_ORBInitInfo info,
                                                        CORBA_Environment* ev);
/* Valid during the initializer's call, like the info. */
IOP_CodecFactory PortableInterceptor_ORBInitInfo__get_codec_factory(
    PortableInterceptor_ORBInitInfo info, CORBA_Environment* ev);
void PortableInterceptor_ORBInitInfo_register_initial_reference(
    PortableInterceptor_ORBInitInfo info, const CORBA_char* id, CORBA_Object obj,
    CORBA_Environment* ev);
/* Only in post_init. */
CORBA_Object PortableInterceptor_ORBInitInfo_resolve_initial_references(
    PortableInterceptor_ORBInitInfo info, const CORBA_char* id, CORBA_Environment* ev);
/* The interceptor is a local object of PortableInterceptor_<I>__create;
   its implementation must outlive the ORB. */
void PortableInterceptor_ORBInitInfo_add_client_request_interceptor(
    PortableInterceptor_ORBInitInfo info, PortableInterceptor_ClientRequestInterceptor interceptor,
    CORBA_Environment* ev);
void PortableInterceptor_ORBInitInfo_add_server_request_interceptor(
    PortableInterceptor_ORBInitInfo info, PortableInterceptor_ServerRequestInterceptor interceptor,
    CORBA_Environment* ev);
void PortableInterceptor_ORBInitInfo_add_ior_interceptor(
    PortableInterceptor_ORBInitInfo info, PortableInterceptor_IORInterceptor interceptor,
    CORBA_Environment* ev);
PortableInterceptor_SlotId PortableInterceptor_ORBInitInfo_allocate_slot_id(
    PortableInterceptor_ORBInitInfo info, CORBA_Environment* ev);

/* --- PICurrent ------------------------------------------------------------------- */

CORBA_any* PortableInterceptor_Current_get_slot(PortableInterceptor_Current current,
                                                PortableInterceptor_SlotId id,
                                                CORBA_Environment* ev);
void PortableInterceptor_Current_set_slot(PortableInterceptor_Current current,
                                          PortableInterceptor_SlotId id, const CORBA_any* data,
                                          CORBA_Environment* ev);

#ifdef __cplusplus
}
#endif

#endif /* RUSTORB_C_PI_H */

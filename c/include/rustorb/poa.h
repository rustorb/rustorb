/*
 * rustorb/poa.h: the PortableServer module of the OMG C Language Mapping
 * 1.0 on rustorb. Operations of the POA, the POAManager and Current are
 * named <interface>_<operation> (PortableServer_POA_activate_object...),
 * attributes PortableServer_POA__get_<name>.
 *
 * A servant is a struct whose first members are _private (the ORB's) and
 * vepv, as POA_<Interface> of the generated code; POA_<Interface>__init
 * makes it usable and POA_<Interface>__fini (after it is deactivated)
 * releases what the ORB keeps for it.
 *
 * Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.
 */

#ifndef RUSTORB_C_POA_H
#define RUSTORB_C_POA_H

#include <rustorb/orb.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef CORBA_Object PortableServer_POA;
typedef CORBA_Object PortableServer_POAManager;
typedef CORBA_Object PortableServer_Current;
typedef CORBA_sequence_octet PortableServer_ObjectId;

#ifndef _CORBA_sequence_PortableServer_POA_defined
#define _CORBA_sequence_PortableServer_POA_defined
RUSTORB_C_SEQUENCE(CORBA_sequence_PortableServer_POA, PortableServer_POA);
#endif
typedef CORBA_sequence_PortableServer_POA PortableServer_POAList;

/* --- servants (1.26) ------------------------------------------------------- */

typedef void* PortableServer_Servant;

typedef struct PortableServer_ServantBase__epv {
  void* _private;
  /* Not called by rustorb. */
  void (*finalize)(PortableServer_Servant servant, CORBA_Environment* ev);
  /* NULL for the RootPOA. */
  PortableServer_POA (*default_POA)(PortableServer_Servant servant, CORBA_Environment* ev);
} PortableServer_ServantBase__epv;

typedef struct PortableServer_ServantBase__vepv {
  PortableServer_ServantBase__epv* _base_epv;
} PortableServer_ServantBase__vepv;

typedef struct PortableServer_ServantBase {
  void* _private;
  PortableServer_ServantBase__vepv* vepv;
} PortableServer_ServantBase;

void PortableServer_ServantBase__init(PortableServer_Servant servant, CORBA_Environment* ev);
void PortableServer_ServantBase__fini(PortableServer_Servant servant, CORBA_Environment* ev);
/* The default POA of the servant: its default_POA, else the RootPOA. */
PortableServer_POA PortableServer_ServantBase__default_POA(PortableServer_Servant servant,
                                                           CORBA_Environment* ev);
/* rustorb: the reference of the object the calling thread runs a request
   for on servant, else nil (PortableServer::Servant::_this in a request). */
CORBA_Object PortableServer_ServantBase__this(PortableServer_Servant servant,
                                              CORBA_Environment* ev);

/* --- exceptions -------------------------------------------------------------- */

#define ex_PortableServer_ForwardRequest "IDL:omg.org/PortableServer/ForwardRequest:1.0"
#define ex_PortableServer_POAManager_AdapterInactive \
  "IDL:omg.org/PortableServer/POAManager/AdapterInactive:1.0"
#define ex_PortableServer_POA_AdapterAlreadyExists \
  "IDL:omg.org/PortableServer/POA/AdapterAlreadyExists:1.0"
#define ex_PortableServer_POA_AdapterNonExistent \
  "IDL:omg.org/PortableServer/POA/AdapterNonExistent:1.0"
/* Its value is a PortableServer_POA_InvalidPolicy. */
#define ex_PortableServer_POA_InvalidPolicy "IDL:omg.org/PortableServer/POA/InvalidPolicy:1.0"
#define ex_PortableServer_POA_NoServant "IDL:omg.org/PortableServer/POA/NoServant:1.0"
#define ex_PortableServer_POA_ObjectAlreadyActive \
  "IDL:omg.org/PortableServer/POA/ObjectAlreadyActive:1.0"
#define ex_PortableServer_POA_ObjectNotActive "IDL:omg.org/PortableServer/POA/ObjectNotActive:1.0"
#define ex_PortableServer_POA_ServantAlreadyActive \
  "IDL:omg.org/PortableServer/POA/ServantAlreadyActive:1.0"
#define ex_PortableServer_POA_ServantNotActive \
  "IDL:omg.org/PortableServer/POA/ServantNotActive:1.0"
#define ex_PortableServer_POA_WrongAdapter "IDL:omg.org/PortableServer/POA/WrongAdapter:1.0"
#define ex_PortableServer_POA_WrongPolicy "IDL:omg.org/PortableServer/POA/WrongPolicy:1.0"
#define ex_PortableServer_Current_NoContext "IDL:omg.org/PortableServer/Current/NoContext:1.0"

typedef struct PortableServer_POA_InvalidPolicy {
  CORBA_unsigned_short index;
} PortableServer_POA_InvalidPolicy;

/* --- policies ------------------------------------------------------------------ */

typedef CORBA_Policy PortableServer_ThreadPolicy;
typedef CORBA_Policy PortableServer_LifespanPolicy;
typedef CORBA_Policy PortableServer_IdUniquenessPolicy;
typedef CORBA_Policy PortableServer_IdAssignmentPolicy;
typedef CORBA_Policy PortableServer_ImplicitActivationPolicy;
typedef CORBA_Policy PortableServer_ServantRetentionPolicy;
typedef CORBA_Policy PortableServer_RequestProcessingPolicy;

#define PortableServer_THREAD_POLICY_ID 16
#define PortableServer_LIFESPAN_POLICY_ID 17
#define PortableServer_ID_UNIQUENESS_POLICY_ID 18
#define PortableServer_ID_ASSIGNMENT_POLICY_ID 19
#define PortableServer_IMPLICIT_ACTIVATION_POLICY_ID 20
#define PortableServer_SERVANT_RETENTION_POLICY_ID 21
#define PortableServer_REQUEST_PROCESSING_POLICY_ID 22

typedef enum PortableServer_ThreadPolicyValue {
  PortableServer_ORB_CTRL_MODEL,
  PortableServer_SINGLE_THREAD_MODEL,
  PortableServer_MAIN_THREAD_MODEL
} PortableServer_ThreadPolicyValue;
typedef enum PortableServer_LifespanPolicyValue {
  PortableServer_TRANSIENT,
  PortableServer_PERSISTENT
} PortableServer_LifespanPolicyValue;
typedef enum PortableServer_IdUniquenessPolicyValue {
  PortableServer_UNIQUE_ID,
  PortableServer_MULTIPLE_ID
} PortableServer_IdUniquenessPolicyValue;
typedef enum PortableServer_IdAssignmentPolicyValue {
  PortableServer_USER_ID,
  PortableServer_SYSTEM_ID
} PortableServer_IdAssignmentPolicyValue;
typedef enum PortableServer_ImplicitActivationPolicyValue {
  PortableServer_IMPLICIT_ACTIVATION,
  PortableServer_NO_IMPLICIT_ACTIVATION
} PortableServer_ImplicitActivationPolicyValue;
typedef enum PortableServer_ServantRetentionPolicyValue {
  PortableServer_RETAIN,
  PortableServer_NON_RETAIN
} PortableServer_ServantRetentionPolicyValue;
typedef enum PortableServer_RequestProcessingPolicyValue {
  PortableServer_USE_ACTIVE_OBJECT_MAP_ONLY,
  PortableServer_USE_DEFAULT_SERVANT,
  PortableServer_USE_SERVANT_MANAGER
} PortableServer_RequestProcessingPolicyValue;

PortableServer_ThreadPolicy PortableServer_POA_create_thread_policy(
    PortableServer_POA poa, PortableServer_ThreadPolicyValue value, CORBA_Environment* ev);
PortableServer_LifespanPolicy PortableServer_POA_create_lifespan_policy(
    PortableServer_POA poa, PortableServer_LifespanPolicyValue value, CORBA_Environment* ev);
PortableServer_IdUniquenessPolicy PortableServer_POA_create_id_uniqueness_policy(
    PortableServer_POA poa, PortableServer_IdUniquenessPolicyValue value, CORBA_Environment* ev);
PortableServer_IdAssignmentPolicy PortableServer_POA_create_id_assignment_policy(
    PortableServer_POA poa, PortableServer_IdAssignmentPolicyValue value, CORBA_Environment* ev);
PortableServer_ImplicitActivationPolicy PortableServer_POA_create_implicit_activation_policy(
    PortableServer_POA poa, PortableServer_ImplicitActivationPolicyValue value,
    CORBA_Environment* ev);
PortableServer_ServantRetentionPolicy PortableServer_POA_create_servant_retention_policy(
    PortableServer_POA poa, PortableServer_ServantRetentionPolicyValue value,
    CORBA_Environment* ev);
PortableServer_RequestProcessingPolicy PortableServer_POA_create_request_processing_policy(
    PortableServer_POA poa, PortableServer_RequestProcessingPolicyValue value,
    CORBA_Environment* ev);

/* The value of a POA policy (any of the seven). */
CORBA_unsigned_long rustorb_POA_policy_value(CORBA_Policy policy, CORBA_Environment* ev);
#define PortableServer_ThreadPolicy__get_value(p, ev) \
  ((PortableServer_ThreadPolicyValue)rustorb_POA_policy_value((p), (ev)))
#define PortableServer_LifespanPolicy__get_value(p, ev) \
  ((PortableServer_LifespanPolicyValue)rustorb_POA_policy_value((p), (ev)))
#define PortableServer_IdUniquenessPolicy__get_value(p, ev) \
  ((PortableServer_IdUniquenessPolicyValue)rustorb_POA_policy_value((p), (ev)))
#define PortableServer_IdAssignmentPolicy__get_value(p, ev) \
  ((PortableServer_IdAssignmentPolicyValue)rustorb_POA_policy_value((p), (ev)))
#define PortableServer_ImplicitActivationPolicy__get_value(p, ev) \
  ((PortableServer_ImplicitActivationPolicyValue)rustorb_POA_policy_value((p), (ev)))
#define PortableServer_ServantRetentionPolicy__get_value(p, ev) \
  ((PortableServer_ServantRetentionPolicyValue)rustorb_POA_policy_value((p), (ev)))
#define PortableServer_RequestProcessingPolicy__get_value(p, ev) \
  ((PortableServer_RequestProcessingPolicyValue)rustorb_POA_policy_value((p), (ev)))

/* --- POAManager ------------------------------------------------------------------ */

typedef enum PortableServer_POAManager_State {
  PortableServer_POAManager_HOLDING,
  PortableServer_POAManager_ACTIVE,
  PortableServer_POAManager_DISCARDING,
  PortableServer_POAManager_INACTIVE
} PortableServer_POAManager_State;

void PortableServer_POAManager_activate(PortableServer_POAManager m, CORBA_Environment* ev);
void PortableServer_POAManager_hold_requests(PortableServer_POAManager m,
                                             CORBA_boolean wait_for_completion,
                                             CORBA_Environment* ev);
void PortableServer_POAManager_discard_requests(PortableServer_POAManager m,
                                                CORBA_boolean wait_for_completion,
                                                CORBA_Environment* ev);
void PortableServer_POAManager_deactivate(PortableServer_POAManager m, CORBA_boolean etherealize,
                                          CORBA_boolean wait_for_completion,
                                          CORBA_Environment* ev);
PortableServer_POAManager_State PortableServer_POAManager_get_state(PortableServer_POAManager m,
                                                                    CORBA_Environment* ev);
/* To CORBA_free. */
CORBA_char* PortableServer_POAManager_get_id(PortableServer_POAManager m, CORBA_Environment* ev);

/* --- POA --------------------------------------------------------------------------- */

PortableServer_POA PortableServer_POA_create_POA(PortableServer_POA poa,
                                                 const CORBA_char* adapter_name,
                                                 PortableServer_POAManager a_POAManager,
                                                 const CORBA_PolicyList* policies,
                                                 CORBA_Environment* ev);
PortableServer_POA PortableServer_POA_find_POA(PortableServer_POA poa,
                                               const CORBA_char* adapter_name,
                                               CORBA_boolean activate_it, CORBA_Environment* ev);
void PortableServer_POA_destroy(PortableServer_POA poa, CORBA_boolean etherealize_objects,
                                CORBA_boolean wait_for_completion, CORBA_Environment* ev);
CORBA_char* PortableServer_POA__get_the_name(PortableServer_POA poa, CORBA_Environment* ev);
PortableServer_POA PortableServer_POA__get_the_parent(PortableServer_POA poa,
                                                      CORBA_Environment* ev);
PortableServer_POAList* PortableServer_POA__get_the_children(PortableServer_POA poa,
                                                             CORBA_Environment* ev);
PortableServer_POAManager PortableServer_POA__get_the_POAManager(PortableServer_POA poa,
                                                                 CORBA_Environment* ev);

PortableServer_Servant PortableServer_POA_get_servant(PortableServer_POA poa,
                                                      CORBA_Environment* ev);
void PortableServer_POA_set_servant(PortableServer_POA poa, PortableServer_Servant servant,
                                    CORBA_Environment* ev);
PortableServer_ObjectId* PortableServer_POA_activate_object(PortableServer_POA poa,
                                                            PortableServer_Servant servant,
                                                            CORBA_Environment* ev);
void PortableServer_POA_activate_object_with_id(PortableServer_POA poa,
                                                const PortableServer_ObjectId* id,
                                                PortableServer_Servant servant,
                                                CORBA_Environment* ev);
void PortableServer_POA_deactivate_object(PortableServer_POA poa,
                                          const PortableServer_ObjectId* oid,
                                          CORBA_Environment* ev);
CORBA_Object PortableServer_POA_create_reference(PortableServer_POA poa, const CORBA_char* intf,
                                                 CORBA_Environment* ev);
CORBA_Object PortableServer_POA_create_reference_with_id(PortableServer_POA poa,
                                                         const PortableServer_ObjectId* oid,
                                                         const CORBA_char* intf,
                                                         CORBA_Environment* ev);
PortableServer_ObjectId* PortableServer_POA_servant_to_id(PortableServer_POA poa,
                                                          PortableServer_Servant servant,
                                                          CORBA_Environment* ev);
CORBA_Object PortableServer_POA_servant_to_reference(PortableServer_POA poa,
                                                     PortableServer_Servant servant,
                                                     CORBA_Environment* ev);
PortableServer_Servant PortableServer_POA_reference_to_servant(PortableServer_POA poa,
                                                               CORBA_Object reference,
                                                               CORBA_Environment* ev);
PortableServer_ObjectId* PortableServer_POA_reference_to_id(PortableServer_POA poa,
                                                            CORBA_Object reference,
                                                            CORBA_Environment* ev);
PortableServer_Servant PortableServer_POA_id_to_servant(PortableServer_POA poa,
                                                        const PortableServer_ObjectId* oid,
                                                        CORBA_Environment* ev);
CORBA_Object PortableServer_POA_id_to_reference(PortableServer_POA poa,
                                                const PortableServer_ObjectId* oid,
                                                CORBA_Environment* ev);

/* --- servant managers and adapter activators ------------------------------------------ */

/* PortableServer::ServantActivator, ServantLocator and AdapterActivator
   are local interfaces: implemented, as the local interfaces of -bc, by
   a struct of the POA_ form (_private, vepv) initialised with its __init,
   whose local object __create makes (the struct must outlive the POAs that
   use it). incarnate and preinvoke raise ForwardRequest with
   CORBA_exception_set(ev, CORBA_USER_EXCEPTION,
   ex_PortableServer_ForwardRequest, a PortableServer_ForwardRequest*), or
   a system exception; etherealize, postinvoke and unknown_adapter cannot
   raise. A servant they return is initialised (POA_<I>__init or
   PortableServer_DynamicImpl__init). */

typedef CORBA_Object PortableServer_ServantManager;
typedef CORBA_Object PortableServer_ServantActivator;
typedef CORBA_Object PortableServer_ServantLocator;
typedef CORBA_Object PortableServer_AdapterActivator;
typedef void* PortableServer_ServantLocator_Cookie;

typedef struct PortableServer_ForwardRequest {
  CORBA_Object forward_reference;
} PortableServer_ForwardRequest;
PortableServer_ForwardRequest* PortableServer_ForwardRequest__alloc(void);

typedef struct POA_PortableServer_ServantManager__epv {
  void* _private;
} POA_PortableServer_ServantManager__epv;

typedef struct POA_PortableServer_ServantActivator__epv {
  void* _private;
  PortableServer_Servant (*incarnate)(PortableServer_Servant _servant,
                                      const PortableServer_ObjectId* oid,
                                      PortableServer_POA adapter, CORBA_Environment* ev);
  void (*etherealize)(PortableServer_Servant _servant, const PortableServer_ObjectId* oid,
                      PortableServer_POA adapter, PortableServer_Servant serv,
                      CORBA_boolean cleanup_in_progress, CORBA_boolean remaining_activations,
                      CORBA_Environment* ev);
} POA_PortableServer_ServantActivator__epv;

typedef struct POA_PortableServer_ServantActivator__vepv {
  PortableServer_ServantBase__epv* _base_epv;
  POA_PortableServer_ServantManager__epv* PortableServer_ServantManager_epv;
  POA_PortableServer_ServantActivator__epv* PortableServer_ServantActivator_epv;
} POA_PortableServer_ServantActivator__vepv;

typedef struct POA_PortableServer_ServantActivator {
  void* _private;
  POA_PortableServer_ServantActivator__vepv* vepv;
} POA_PortableServer_ServantActivator;

typedef struct POA_PortableServer_ServantLocator__epv {
  void* _private;
  /* *the_cookie is given to postinvoke as it is. */
  PortableServer_Servant (*preinvoke)(PortableServer_Servant _servant,
                                      const PortableServer_ObjectId* oid,
                                      PortableServer_POA adapter, const CORBA_char* operation,
                                      PortableServer_ServantLocator_Cookie* the_cookie,
                                      CORBA_Environment* ev);
  void (*postinvoke)(PortableServer_Servant _servant, const PortableServer_ObjectId* oid,
                     PortableServer_POA adapter, const CORBA_char* operation,
                     PortableServer_ServantLocator_Cookie the_cookie,
                     PortableServer_Servant the_servant, CORBA_Environment* ev);
} POA_PortableServer_ServantLocator__epv;

typedef struct POA_PortableServer_ServantLocator__vepv {
  PortableServer_ServantBase__epv* _base_epv;
  POA_PortableServer_ServantManager__epv* PortableServer_ServantManager_epv;
  POA_PortableServer_ServantLocator__epv* PortableServer_ServantLocator_epv;
} POA_PortableServer_ServantLocator__vepv;

typedef struct POA_PortableServer_ServantLocator {
  void* _private;
  POA_PortableServer_ServantLocator__vepv* vepv;
} POA_PortableServer_ServantLocator;

typedef struct POA_PortableServer_AdapterActivator__epv {
  void* _private;
  /* Creates the child name of parent (create_POA) and returns TRUE, or
     returns FALSE. */
  CORBA_boolean (*unknown_adapter)(PortableServer_Servant _servant, PortableServer_POA parent,
                                   const CORBA_char* name, CORBA_Environment* ev);
} POA_PortableServer_AdapterActivator__epv;

typedef struct POA_PortableServer_AdapterActivator__vepv {
  PortableServer_ServantBase__epv* _base_epv;
  POA_PortableServer_AdapterActivator__epv* PortableServer_AdapterActivator_epv;
} POA_PortableServer_AdapterActivator__vepv;

typedef struct POA_PortableServer_AdapterActivator {
  void* _private;
  POA_PortableServer_AdapterActivator__vepv* vepv;
} POA_PortableServer_AdapterActivator;

void POA_PortableServer_ServantActivator__init(PortableServer_Servant servant,
                                               CORBA_Environment* ev);
void POA_PortableServer_ServantActivator__fini(PortableServer_Servant servant,
                                               CORBA_Environment* ev);
PortableServer_ServantActivator PortableServer_ServantActivator__create(
    PortableServer_Servant impl, CORBA_Environment* ev);
void POA_PortableServer_ServantLocator__init(PortableServer_Servant servant,
                                             CORBA_Environment* ev);
void POA_PortableServer_ServantLocator__fini(PortableServer_Servant servant,
                                             CORBA_Environment* ev);
PortableServer_ServantLocator PortableServer_ServantLocator__create(PortableServer_Servant impl,
                                                                    CORBA_Environment* ev);
void POA_PortableServer_AdapterActivator__init(PortableServer_Servant servant,
                                               CORBA_Environment* ev);
void POA_PortableServer_AdapterActivator__fini(PortableServer_Servant servant,
                                               CORBA_Environment* ev);
PortableServer_AdapterActivator PortableServer_AdapterActivator__create(
    PortableServer_Servant impl, CORBA_Environment* ev);

/* The operations, called as the POA calls them. */
PortableServer_Servant PortableServer_ServantActivator_incarnate(
    PortableServer_ServantActivator obj, const PortableServer_ObjectId* oid,
    PortableServer_POA adapter, CORBA_Environment* ev);
void PortableServer_ServantActivator_etherealize(PortableServer_ServantActivator obj,
                                                 const PortableServer_ObjectId* oid,
                                                 PortableServer_POA adapter,
                                                 PortableServer_Servant serv,
                                                 CORBA_boolean cleanup_in_progress,
                                                 CORBA_boolean remaining_activations,
                                                 CORBA_Environment* ev);
PortableServer_Servant PortableServer_ServantLocator_preinvoke(
    PortableServer_ServantLocator obj, const PortableServer_ObjectId* oid,
    PortableServer_POA adapter, const CORBA_char* operation,
    PortableServer_ServantLocator_Cookie* the_cookie, CORBA_Environment* ev);
void PortableServer_ServantLocator_postinvoke(PortableServer_ServantLocator obj,
                                              const PortableServer_ObjectId* oid,
                                              PortableServer_POA adapter,
                                              const CORBA_char* operation,
                                              PortableServer_ServantLocator_Cookie the_cookie,
                                              PortableServer_Servant the_servant,
                                              CORBA_Environment* ev);
CORBA_boolean PortableServer_AdapterActivator_unknown_adapter(PortableServer_AdapterActivator obj,
                                                              PortableServer_POA parent,
                                                              const CORBA_char* name,
                                                              CORBA_Environment* ev);

/* USE_SERVANT_MANAGER: an activator with RETAIN, a locator with NON_RETAIN
   (else WrongPolicy); once (BAD_INV_ORDER). */
void PortableServer_POA_set_servant_manager(PortableServer_POA poa,
                                            PortableServer_ServantManager imgr,
                                            CORBA_Environment* ev);
/* Nil when there is none. */
PortableServer_ServantManager PortableServer_POA_get_servant_manager(PortableServer_POA poa,
                                                                     CORBA_Environment* ev);
/* Nil removes it. */
void PortableServer_POA__set_the_activator(PortableServer_POA poa,
                                           PortableServer_AdapterActivator activator,
                                           CORBA_Environment* ev);
PortableServer_AdapterActivator PortableServer_POA__get_the_activator(PortableServer_POA poa,
                                                                      CORBA_Environment* ev);

/* --- Current ------------------------------------------------------------------------- */

PortableServer_POA PortableServer_Current_get_POA(PortableServer_Current current,
                                                  CORBA_Environment* ev);
PortableServer_ObjectId* PortableServer_Current_get_object_id(PortableServer_Current current,
                                                              CORBA_Environment* ev);
CORBA_Object PortableServer_Current_get_reference(PortableServer_Current current,
                                                  CORBA_Environment* ev);
PortableServer_Servant PortableServer_Current_get_servant(PortableServer_Current current,
                                                          CORBA_Environment* ev);

/* --- object ids ------------------------------------------------------------------------ */

/* rustorb: an ObjectId of the bytes of a string and back (to CORBA_free). */
PortableServer_ObjectId* PortableServer_string_to_ObjectId(const CORBA_char* str);
CORBA_char* PortableServer_ObjectId_to_string(const PortableServer_ObjectId* id);

#ifdef __cplusplus
}
#endif

#endif /* RUSTORB_C_POA_H */

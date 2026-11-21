/*
 * rustorb/runtime.h: what the code of rustorb-idl -bc uses. Not for
 * applications: the names here may change with rustorb-idl.
 *
 * Each IDL type has a descriptor (rustorb_c_type) with its TypeCode
 * parameters and its C layout (sizeof, offsetof). One generic reader,
 * writer and freer in librustorb_c walks the descriptors, for stubs,
 * skeletons, CORBA_any and CORBA_free alike (docs/c-binding.md).
 *
 * Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.
 */

#ifndef RUSTORB_C_RUNTIME_H
#define RUSTORB_C_RUNTIME_H

#include <rustorb/orb.h>
#include <rustorb/poa.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct rustorb_c_type rustorb_c_type;
typedef struct rustorb_c_module rustorb_c_module;

/* A member of a struct or an exception, or one label of a union (a member
   with several labels has one entry for each, the default member one with
   any label). offset is the offsetof of the member (in a union,
   offsetof(T, _u.member)). */
typedef struct rustorb_c_member {
  const char* name;
  const rustorb_c_type* type;
  size_t offset;
  /* The label as rorb_tc_union takes it (two's complement for signed
     values, an enumerator its position). */
  uint64_t label;
} rustorb_c_member;

/* Where the TypeCode of a descriptor is kept once made. Zeroed. */
typedef struct rustorb_c_cache {
  CORBA_TypeCode tc;
  int registered;
} rustorb_c_cache;

struct rustorb_c_type {
  /* A CORBA_TCKind. */
  CORBA_unsigned_long kind;
  /* sizeof the C type. */
  size_t size;
  /* Named types. */
  const char* id;
  const char* name;
  /* The element of a sequence or an array, the original type of an alias,
     the discriminator of a union. */
  const rustorb_c_type* content;
  /* The bound of a string or a sequence (0 for none), the length of an
     array. */
  CORBA_unsigned_long length;
  /* Members (struct, exception, union entries) or enumerators. */
  CORBA_unsigned_long count;
  const rustorb_c_member* members;
  const char* const* enumerators;
  /* The union entry of the default member, or -1. */
  CORBA_long default_index;
  CORBA_unsigned_short digits;
  CORBA_short scale;
  rustorb_c_cache* cache;
};

/* The descriptors of the basic types (and CORBA_Object, CORBA_TypeCode,
   CORBA_any). */
extern const rustorb_c_type CORBA_short__desc;
extern const rustorb_c_type CORBA_long__desc;
extern const rustorb_c_type CORBA_long_long__desc;
extern const rustorb_c_type CORBA_unsigned_short__desc;
extern const rustorb_c_type CORBA_unsigned_long__desc;
extern const rustorb_c_type CORBA_unsigned_long_long__desc;
extern const rustorb_c_type CORBA_float__desc;
extern const rustorb_c_type CORBA_double__desc;
extern const rustorb_c_type CORBA_long_double__desc;
extern const rustorb_c_type CORBA_boolean__desc;
extern const rustorb_c_type CORBA_char__desc;
extern const rustorb_c_type CORBA_wchar__desc;
extern const rustorb_c_type CORBA_octet__desc;
extern const rustorb_c_type CORBA_string__desc;
extern const rustorb_c_type CORBA_wstring__desc;
extern const rustorb_c_type CORBA_any__desc;
extern const rustorb_c_type CORBA_TypeCode__desc;
extern const rustorb_c_type CORBA_Object__desc;
/* Any sequence, for allocating sequences of sequences (no element type). */
extern const rustorb_c_type CORBA_sequence__desc;

/* The named types of one IDL file, registered (so that an any of one of
   them read from the wire gets its C value) on the first stub call,
   servant or TypeCode of the file. */
struct rustorb_c_module {
  const rustorb_c_type* const* types;
  CORBA_unsigned_long count;
  int* registered;
};
void rustorb_c_register_module(const rustorb_c_module* module);

/* The TypeCode of a descriptor (borrowed, like TC_long). */
CORBA_TypeCode rustorb_c_type_tc(const rustorb_c_type* type, const rustorb_c_module* module);

/* Storage for count values of type, zeroed, that CORBA_free frees with
   what it holds; NULL type for plain bytes. */
void* rustorb_c_alloc(const rustorb_c_type* type, CORBA_unsigned_long count);
/* A sequence struct (CORBA_sequence_T__alloc). */
void* rustorb_c_sequence_alloc(void);
/* Frees what the value at p holds (not p itself) and zeroes it. */
void rustorb_c_free_value(const rustorb_c_type* type, void* p);

/* rustorb: a deep copy of the value at value of type (the descriptor of a
   generated type, <Name>__desc), freed with CORBA_free; and a copy into
   storage dst that holds nothing. */
void* rustorb_c_copy(const rustorb_c_type* type, const void* value);
void rustorb_c_copy_value(const rustorb_c_type* type, void* dst, const void* src);

/* --- operations ----------------------------------------------------------- */

#define RUSTORB_C_IN 1
#define RUSTORB_C_OUT 2
#define RUSTORB_C_INOUT 3

/* args[i] of an operation points to the value of parameter i (for an in
   struct, the struct; for a string, the CORBA_char* cell), except when
   indirect: a variable-length out parameter or result, where it points to
   the T* cell that gets storage the ORB allocates. */
typedef struct rustorb_c_param {
  const rustorb_c_type* type;
  int mode;
  int indirect;
} rustorb_c_param;

typedef void (*rustorb_c_skel)(PortableServer_Servant servant, void** args, void* result,
                               CORBA_Environment* ev);

typedef struct rustorb_c_operation {
  /* The name on the wire (_get_x for an attribute). */
  const char* name;
  int oneway;
  /* NULL for void. */
  const rustorb_c_type* result;
  int result_indirect;
  CORBA_unsigned_long param_count;
  const rustorb_c_param* params;
  CORBA_unsigned_long exception_count;
  const rustorb_c_type* const* exceptions;
  const rustorb_c_module* module;
} rustorb_c_operation;

/* Calls op on obj. */
void rustorb_c_invoke(CORBA_Object obj, const rustorb_c_operation* op, void** args, void* result,
                      CORBA_Environment* ev);

/* rustorb_c_invoke without the hook. */
void rustorb_c_invoke_static(CORBA_Object obj, const rustorb_c_operation* op, void** args,
                             void* result, CORBA_Environment* ev);
/* A function rustorb_c_invoke calls first (NULL for none): when it returns
   nonzero, it has made the call. For tests that send the calls of
   generated stubs another way (the DII adapter of
   interop/impls/rustorbc); not thread-safe to change during calls. */
typedef int (*rustorb_c_invoke_hook)(CORBA_Object obj, const rustorb_c_operation* op,
                                     void** args, void* result, CORBA_Environment* ev);
void rustorb_c_set_invoke_hook(rustorb_c_invoke_hook hook);

typedef struct rustorb_c_entry {
  const rustorb_c_operation* op;
  rustorb_c_skel skel;
} rustorb_c_entry;

/* A servant class: the repository ids (its own first, then those of the
   bases) and its operations, own and inherited. */
typedef struct rustorb_c_class {
  const char* const* ids;
  CORBA_unsigned_long id_count;
  const rustorb_c_entry* entries;
  CORBA_unsigned_long entry_count;
  const rustorb_c_module* module;
} rustorb_c_class;

/* POA_<Interface>__init and __fini. */
void rustorb_c_servant_init(PortableServer_Servant servant, const rustorb_c_class* cls,
                            CORBA_Environment* ev);
void rustorb_c_servant_fini(PortableServer_Servant servant, CORBA_Environment* ev);
/* POA_<Local>__init of a local interface: the implementation of local
   objects (no servant of the ORB; __fini is rustorb_c_servant_fini). */
void rustorb_c_local_init(PortableServer_Servant servant, const rustorb_c_class* cls,
                          CORBA_Environment* ev);
/* <Local>__create: a local object whose operations call impl (borrowed:
   it must outlive the object's references). BAD_PARAM for an impl not
   initialised by rustorb_c_local_init. */
CORBA_Object rustorb_c_local_object(PortableServer_Servant impl, CORBA_Environment* ev);

/* The class of a servant initialised with POA_<Interface>__init (NULL for
   others, a PortableServer_DynamicImpl say). */
const rustorb_c_class* rustorb_c_servant_class(PortableServer_Servant servant);

#ifdef __cplusplus
}
#endif

#endif /* RUSTORB_C_RUNTIME_H */

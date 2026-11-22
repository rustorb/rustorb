/* The IDL 4 types of rustorb-idl -bc (cpp/tests/idl/Idl4.idl, as the C++,
 * Java and .NET tests): int8, bitmask and bitset, in calls to a servant
 * in this process, in anys and on the wire (IOP::Codec).
 *
 * Licensed under MIT OR Apache-2.0, like the rest of the rustorb core. */

#include <stdio.h>
#include <string.h>

#include <rustorb/pi.h>

#include "Idl4.h"

static int failures;

#define CHECK(cond)                                                    \
  do {                                                                 \
    if (!(cond)) {                                                     \
      failures++;                                                      \
      fprintf(stderr, "%s:%d: CHECK(%s) failed\n", __FILE__, __LINE__, #cond); \
    }                                                                  \
  } while (0)

/* --- the servant -------------------------------------------------------------------- */

static Idl4_Settings echo_settings(PortableServer_Servant s, const Idl4_Settings* v,
                                   CORBA_Environment* ev) {
  (void)s;
  (void)ev;
  return *v;
}

static Idl4_Flags echo_flags(PortableServer_Servant s, Idl4_Flags f, Idl4_Wide* w,
                             CORBA_Environment* ev) {
  (void)s;
  (void)ev;
  *w = Idl4_LOW | Idl4_HIGH;
  return f;
}

static Idl4_Header echo_header(PortableServer_Servant s, Idl4_Header h, CORBA_Environment* ev) {
  (void)s;
  (void)ev;
  return h;
}

/* -v, and every element of b negated. */
static CORBA_int8 echo_int8(PortableServer_Servant s, CORBA_int8 v, Idl4_Bytes* b,
                            CORBA_Environment* ev) {
  CORBA_unsigned_long i;
  (void)s;
  (void)ev;
  for (i = 0; i < b->_length; i++) b->_buffer[i] = (CORBA_int8)-b->_buffer[i];
  return (CORBA_int8)-v;
}

static CORBA_any* echo_any(PortableServer_Servant s, const CORBA_any* a, CORBA_Environment* ev) {
  (void)s;
  (void)ev;
  return (CORBA_any*)rustorb_c_copy(&CORBA_any__desc, a);
}

static PortableServer_ServantBase__epv base_epv = {NULL, NULL, NULL};
static POA_Idl4_Echo__epv echo_epv = {NULL, echo_settings, echo_flags, echo_header, echo_int8,
                                      echo_any};
static POA_Idl4_Echo__vepv echo_vepv = {&base_epv, &echo_epv};
static POA_Idl4_Echo servant = {NULL, &echo_vepv};

/* --- tests ----------------------------------------------------------------------------- */

static void test_types(void) {
  Idl4_Flags f = Idl4_READ;
  Idl4_Header h = 0;
  CHECK(Idl4_SMALLEST == -128);
  CHECK(Idl4_READ == 1 && Idl4_WRITE == 8 && Idl4_EXEC == 16);
  CHECK(Idl4_HIGH == (1ULL << 39));
  CHECK(sizeof(Idl4_Flags) == 1 && sizeof(Idl4_Wide) == 8 && sizeof(Idl4_Header) == 2);
  CHECK((f & Idl4_READ) && !(f & Idl4_WRITE));
  f = Idl4_WRITE | Idl4_EXEC;
  CHECK(f == 24);
  Idl4_Header_set_kind(&h, 5);
  Idl4_Header_set_delta(&h, -3);
  Idl4_Header_set_last(&h, CORBA_TRUE);
  /* kind 0-2, delta 3-7, 2 unnamed bits, last 10. */
  CHECK(h == (5 | (0x1d << 3) | (1 << 10)));
  CHECK(Idl4_Header_kind(h) == 5 && Idl4_Header_delta(h) == -3 && Idl4_Header_last(h));
  Idl4_Header_set_delta(&h, 15);
  CHECK(Idl4_Header_delta(h) == 15 && Idl4_Header_kind(h) == 5);
  Idl4_Header_set_kind(&h, 9); /* only the low 3 bits */
  CHECK(Idl4_Header_kind(h) == 1 && Idl4_Header_delta(h) == 15);
  CHECK(sizeof(Idl4_Header_delta(h)) == sizeof(CORBA_short));
}

static void test_calls(Idl4_Echo echo) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  Idl4_Settings s, back;
  Idl4_Wide w = 0;
  Idl4_Header h = 0;
  Idl4_Bytes* b = Idl4_Bytes__alloc();
  CORBA_int8 r;
  memset(&s, 0, sizeof s);
  s.id = 1;
  s.offset = -5;
  s.raw = 250;
  s.flags = Idl4_EXEC;
  s.wide = Idl4_HIGH;
  Idl4_Header_set_kind(&s.header, 5);
  Idl4_Header_set_delta(&s.header, -3);
  Idl4_Header_set_last(&s.header, 1);
  back = Idl4_Echo_echo_settings(echo, &s, &ev);
  CHECK(ev._major == CORBA_NO_EXCEPTION);
  CHECK(back.offset == -5 && back.raw == 250 && back.flags == Idl4_EXEC && back.wide == Idl4_HIGH);
  CHECK(Idl4_Header_delta(back.header) == -3 && Idl4_Header_kind(back.header) == 5 &&
        Idl4_Header_last(back.header));

  CHECK(Idl4_Echo_echo_flags(echo, Idl4_EXEC, &w, &ev) == Idl4_EXEC);
  CHECK(w == (Idl4_LOW | Idl4_HIGH));
  Idl4_Header_set_delta(&h, -16);
  CHECK(Idl4_Header_delta(Idl4_Echo_echo_header(echo, h, &ev)) == -16);

  b->_buffer = CORBA_sequence_int8_allocbuf(3);
  b->_length = b->_maximum = 3;
  b->_release = CORBA_TRUE;
  b->_buffer[0] = -1;
  b->_buffer[1] = 2;
  b->_buffer[2] = 127;
  r = Idl4_Echo_echo_int8(echo, -127, b, &ev);
  CHECK(ev._major == CORBA_NO_EXCEPTION && r == 127);
  CHECK(b->_length == 3 && b->_buffer[0] == 1 && b->_buffer[1] == -2 && b->_buffer[2] == -127);
  CORBA_free(b);
  CORBA_exception_free(&ev);
}

static void test_any(Idl4_Echo echo, IOP_Codec codec) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  CORBA_any a, *r;
  Idl4_Header h = 0;
  Idl4_Flags f = Idl4_WRITE;
  CORBA_sequence_octet* data;
  /* A bitset is its holder: tk_ushort. */
  Idl4_Header_set_kind(&h, 3);
  a._type = TC_Idl4_Header;
  a._value = &h;
  a._release = CORBA_FALSE;
  a._rustorb = NULL;
  r = Idl4_Echo_echo_any(echo, &a, &ev);
  CHECK(ev._major == CORBA_NO_EXCEPTION && r);
  if (r) {
    CHECK(CORBA_TypeCode_kind(r->_type, &ev) == CORBA_tk_ushort);
    CHECK(r->_value && Idl4_Header_kind(*(Idl4_Header*)r->_value) == 3);
    CORBA_free(r);
  }
  a._type = TC_Idl4_Flags;
  a._value = &f;
  CHECK(CORBA_TypeCode_kind(a._type, &ev) == CORBA_tk_octet);
  data = IOP_Codec_encode_value(codec, &a, &ev);
  /* The byte-order octet, then the bits. */
  CHECK(ev._major == CORBA_NO_EXCEPTION && data && data->_length == 2 && data->_buffer[1] == 8);
  CORBA_free(data);
  CORBA_exception_free(&ev);
}

int main(int argc, char** argv) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  CORBA_ORB orb = CORBA_ORB_init(&argc, argv, "", &ev);
  PortableServer_POA poa;
  PortableServer_POAManager manager;
  PortableServer_ObjectId* oid;
  IOP_CodecFactory factory;
  IOP_Codec codec;
  IOP_Encoding enc = {IOP_ENCODING_CDR_ENCAPS, 1, 2};
  Idl4_Echo echo;
  if (ev._major != CORBA_NO_EXCEPTION) return 1;
  poa = CORBA_ORB_resolve_initial_references(orb, "RootPOA", &ev);
  manager = PortableServer_POA__get_the_POAManager(poa, &ev);
  PortableServer_POAManager_activate(manager, &ev);
  POA_Idl4_Echo__init(&servant, &ev);
  oid = PortableServer_POA_activate_object(poa, &servant, &ev);
  echo = PortableServer_POA_id_to_reference(poa, oid, &ev);
  factory = (IOP_CodecFactory)CORBA_ORB_resolve_initial_references(orb, "CodecFactory", &ev);
  codec = IOP_CodecFactory_create_codec(factory, &enc, &ev);
  if (ev._major != CORBA_NO_EXCEPTION) {
    fprintf(stderr, "setup: %s\n", CORBA_exception_id(&ev));
    return 1;
  }

  test_types();
  test_calls(echo);
  test_any(echo, codec);

  CORBA_Object_release(codec, &ev);
  CORBA_Object_release(factory, &ev);
  CORBA_Object_release(echo, &ev);
  PortableServer_POA_deactivate_object(poa, oid, &ev);
  CORBA_free(oid);
  CORBA_Object_release(manager, &ev);
  CORBA_Object_release(poa, &ev);
  CORBA_ORB_destroy(orb, &ev);
  CORBA_Object_release(orb, &ev);
  POA_Idl4_Echo__fini(&servant, &ev);
  if (failures) {
    fprintf(stderr, "%d failure(s)\n", failures);
    return 1;
  }
  return 0;
}

/* Serves Example::Echo and binds it in the naming service as "Echo".
 *
 *   rustorb-idl -bc -C gen ../idl/Echo.idl
 *   cc -std=c99 -I$PREFIX/include -Igen server.c gen/Echo.c \
 *      $PREFIX/lib/librustorb_c.a $PREFIX/lib/librustorb_ffi.a -lpthread -ldl -lm -o server
 *   ./server -ORBInitRef NameService=corbaloc::localhost:2809/NameService
 *
 * (PREFIX is where the C package is unpacked, or `cmake --install` put it.
 * On macOS link -framework CoreFoundation -framework Security instead of
 * -ldl.) */

#include <stdio.h>
#include <stdlib.h>

#include <rustorb/CosNaming.h>

#include "Echo.h"

static CORBA_ORB orb;

static void check(CORBA_Environment* ev, const char* what) {
  if (ev->_major == CORBA_NO_EXCEPTION) return;
  fprintf(stderr, "server: %s: %s\n", what, CORBA_exception_id(ev));
  exit(1);
}

static CORBA_char* echo(PortableServer_Servant self, const CORBA_char* message,
                        CORBA_Environment* ev) {
  (void)self;
  (void)ev;
  printf("server: %s\n", message);
  fflush(stdout);
  return CORBA_string_dup(message);
}

static void shutdown_orb(PortableServer_Servant self, CORBA_Environment* ev) {
  (void)self;
  CORBA_ORB_shutdown(orb, CORBA_FALSE, ev);
}

static PortableServer_ServantBase__epv base_epv = {NULL, NULL, NULL};
static POA_Example_Echo__epv echo_epv = {NULL, echo, shutdown_orb};
static POA_Example_Echo__vepv echo_vepv = {&base_epv, &echo_epv};
static POA_Example_Echo servant = {NULL, &echo_vepv};

int main(int argc, char** argv) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  PortableServer_POA poa;
  PortableServer_POAManager mgr;
  CosNaming_NamingContext root;
  CosNaming_NameComponent component;
  CosNaming_Name name;
  CORBA_Object echo_ref;

  orb = CORBA_ORB_init(&argc, argv, "", &ev);
  check(&ev, "ORB_init");
  poa = (PortableServer_POA)CORBA_ORB_resolve_initial_references(orb, "RootPOA", &ev);
  check(&ev, "RootPOA");
  mgr = PortableServer_POA__get_the_POAManager(poa, &ev);
  PortableServer_POAManager_activate(mgr, &ev);
  check(&ev, "activate");
  POA_Example_Echo__init(&servant, &ev);
  echo_ref = PortableServer_POA_servant_to_reference(poa, &servant, &ev);
  check(&ev, "servant_to_reference");

  root = (CosNaming_NamingContext)CORBA_ORB_resolve_initial_references(orb, "NameService", &ev);
  check(&ev, "NameService");
  component.id = "Echo";
  component.kind = "";
  name._maximum = name._length = 1;
  name._buffer = &component;
  name._release = CORBA_FALSE;
  CosNaming_NamingContext_rebind(root, &name, echo_ref, &ev);
  check(&ev, "rebind");
  printf("server: ready\n");
  fflush(stdout);

  CORBA_ORB_run(orb, &ev);
  check(&ev, "run");
  CORBA_Object_release(echo_ref, &ev);
  CORBA_Object_release(root, &ev);
  CORBA_Object_release(mgr, &ev);
  CORBA_Object_release(poa, &ev);
  CORBA_ORB_destroy(orb, &ev);
  CORBA_Object_release(orb, &ev);
  return 0;
}

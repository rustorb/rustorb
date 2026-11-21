/* The interop cases of interop/impls/rustorbc against its Echo servant, in
 * one process: the client calls go over GIOP to the same ORB.
 *
 * Licensed under MIT OR Apache-2.0, like the rest of the rustorb core. */

#include <stdio.h>
#include <string.h>

#include "cases.h"
#include "echo_impl.h"

static int check_ev(CORBA_Environment* ev, const char* what) {
  if (ev->_major == CORBA_NO_EXCEPTION) return 1;
  fprintf(stderr, "%s: %s\n", what, CORBA_exception_id(ev));
  CORBA_exception_free(ev);
  return 0;
}

int main(int argc, char** argv) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  CORBA_ORB orb = CORBA_ORB_init(&argc, argv, "", &ev);
  PortableServer_POA poa;
  PortableServer_POAManager manager;
  PortableServer_Servant echo_servant;
  PortableServer_ObjectId* oid;
  CORBA_Object ref, echo;
  CORBA_char* ior;
  runner r;
  if (!check_ev(&ev, "ORB_init")) return 1;
  poa = CORBA_ORB_resolve_initial_references(orb, "RootPOA", &ev);
  if (!check_ev(&ev, "RootPOA")) return 1;
  manager = PortableServer_POA__get_the_POAManager(poa, &ev);
  PortableServer_POAManager_activate(manager, &ev);
  if (!check_ev(&ev, "activate")) return 1;

  echo_servant = echo_impl_new(orb, &ev);
  if (!check_ev(&ev, "Echo servant")) return 1;
  oid = PortableServer_POA_activate_object(poa, echo_servant, &ev);
  if (!check_ev(&ev, "activate_object")) return 1;
  ref = PortableServer_POA_id_to_reference(poa, oid, &ev);
  if (!check_ev(&ev, "id_to_reference")) return 1;
  /* Through a stringified reference, as a client of another process. */
  ior = CORBA_ORB_object_to_string(orb, ref, &ev);
  if (!check_ev(&ev, "object_to_string")) return 1;
  echo = CORBA_ORB_string_to_object(orb, ior, &ev);
  if (!check_ev(&ev, "string_to_object")) return 1;
  if (!CORBA_Object_is_a(echo, "IDL:rustorb.org/InteropTest/Echo:1.0", &ev) ||
      !check_ev(&ev, "is_a")) {
    fprintf(stderr, "is_a failed\n");
    return 1;
  }

  memset(&r, 0, sizeof r);
  cases_run(&r, echo, poa);
  cases_summary(&r);

  CORBA_Object_release(echo, &ev);
  CORBA_Object_release(ref, &ev);
  CORBA_free(ior);
  PortableServer_POA_deactivate_object(poa, oid, &ev);
  check_ev(&ev, "deactivate_object");
  CORBA_free(oid);
  CORBA_Object_release(manager, &ev);
  CORBA_Object_release(poa, &ev);
  CORBA_ORB_destroy(orb, &ev);
  check_ev(&ev, "destroy");
  CORBA_Object_release(orb, &ev);
  echo_impl_free(echo_servant);
  cases_fini();
  return r.failed == 0 && r.passed == 79 ? 0 : 1;
}

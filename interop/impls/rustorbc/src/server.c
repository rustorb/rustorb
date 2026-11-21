/* The C InteropTest server (rustorbc): serves Echo under --name in the
 * naming service until a client calls shutdown.
 *
 * Licensed under MIT OR Apache-2.0, like the rest of the rustorb core. */

#include <stdio.h>

#include "common.h"
#include "echo_impl.h"

int main(int argc, char** argv) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  options opt;
  CORBA_ORB orb;
  PortableServer_POA poa;
  PortableServer_Servant servant;
  PortableServer_ObjectId* oid;
  CosNaming_NamingContext root;
  CORBA_Object ref;
  parse_args("server", argc, argv, &opt, 0);
  orb = orb_init(&opt);
  poa = persistent_poa(orb, "InteropTest");
  servant = echo_impl_new(orb, &ev);
  die_on(&ev, "Echo servant");
  oid = PortableServer_string_to_ObjectId("Echo");
  PortableServer_POA_activate_object_with_id(poa, oid, servant, &ev);
  die_on(&ev, "activate_object_with_id");
  ref = PortableServer_POA_id_to_reference(poa, oid, &ev);
  die_on(&ev, "id_to_reference");
  root = root_context(orb, opt.timeout);
  bind_name(root, opt.name, ref);
  printf("server ready: %s\n", opt.name);
  fflush(stdout);
  CORBA_ORB_run(orb, &ev);
  die_on(&ev, "run");
  CORBA_Object_release(root, &ev);
  CORBA_Object_release(ref, &ev);
  CORBA_free(oid);
  CORBA_Object_release(poa, &ev);
  CORBA_ORB_destroy(orb, &ev);
  CORBA_Object_release(orb, &ev);
  echo_impl_free(servant);
  printf("server stopped\n");
  return 0;
}

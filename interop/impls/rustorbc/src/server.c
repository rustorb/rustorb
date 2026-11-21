/* The C InteropTest server (rustorbc): serves Echo under --name in the
 * naming service until a client calls shutdown.
 *
 * Licensed under MIT OR Apache-2.0, like the rest of the rustorb core. */

#include <stdio.h>

#include "anytest.h"
#include "common.h"
#include "echo_impl.h"
#include "dii_adapters.h"

/* The servant to activate: s, or with RUSTORB_DII a DSI servant running
   it (rustorbc-dii). */
static PortableServer_Servant wrap(PortableServer_Servant s, PortableServer_Servant* dsi) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  *dsi = NULL;
  if (!dii_enabled()) return s;
  *dsi = dii_servant_new(s, &ev);
  die_on(&ev, "DSI servant");
  return *dsi;
}

int main(int argc, char** argv) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  options opt;
  CORBA_ORB orb;
  PortableServer_POA poa;
  PortableServer_Servant servant;
  PortableServer_ObjectId* oid;
  CosNaming_NamingContext root;
  CORBA_Object ref;
  PortableServer_Servant any_servant = NULL;
  PortableServer_Servant dsi = NULL, any_dsi = NULL;
  parse_args("server", argc, argv, &opt, 0);
  orb = orb_init(&opt);
  poa = persistent_poa(orb, opt.any_suite ? "AnyTest" : "InteropTest");
  servant = echo_impl_new(orb, &ev);
  die_on(&ev, "Echo servant");
  oid = PortableServer_string_to_ObjectId("Echo");
  PortableServer_POA_activate_object_with_id(poa, oid, wrap(servant, &dsi), &ev);
  die_on(&ev, "activate_object_with_id");
  ref = PortableServer_POA_id_to_reference(poa, oid, &ev);
  die_on(&ev, "id_to_reference");
  root = root_context(orb, opt.timeout);
  if (opt.any_suite) {
    /* The stage 2 suite: AnyEcho at the name, with the Echo object next to
       it (at name-Echo) for the client to shut the server down. */
    char echo_name[512];
    PortableServer_ObjectId* any_oid = PortableServer_string_to_ObjectId("AnyEcho");
    CORBA_Object any_ref;
    anytest_init(orb);
    any_servant = anytest_servant_new(&ev);
    die_on(&ev, "AnyEcho servant");
    PortableServer_POA_activate_object_with_id(poa, any_oid, wrap(any_servant, &any_dsi), &ev);
    die_on(&ev, "activate_object_with_id");
    any_ref = PortableServer_POA_id_to_reference(poa, any_oid, &ev);
    die_on(&ev, "id_to_reference");
    bind_name(root, opt.name, any_ref);
    snprintf(echo_name, sizeof echo_name, "%s-Echo", opt.name);
    bind_name(root, echo_name, ref);
    CORBA_Object_release(any_ref, &ev);
    CORBA_free(any_oid);
  } else {
    bind_name(root, opt.name, ref);
  }
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
  dii_servant_free(dsi);
  dii_servant_free(any_dsi);
  echo_impl_free(servant);
  if (any_servant) {
    anytest_servant_free(any_servant);
    anytest_fini();
  }
  printf("server stopped\n");
  return 0;
}

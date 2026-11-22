/* The rustorb C side of run-ami-probe.sh: callback AMI (docs/ami.md)
 * against the Echo server of any implementation (ami_cases.c), on the
 * stubs of InteropTest.idl generated with -Wbami (build.sh).
 *
 *   ami_probe REFERENCE [ORB options]
 *
 * REFERENCE names the Echo object, such as corbaname::127.0.0.1:2809#ami/Echo.
 *
 * Licensed under MIT OR Apache-2.0, like the rest of the rustorb core. */

#include <stdio.h>
#include <stdlib.h>

#include "ami_cases.h"

static void emit(const char* line) {
  printf("%s\n", line);
  fflush(stdout);
}

static void die_on(CORBA_Environment* ev, const char* what) {
  if (ev->_major == CORBA_NO_EXCEPTION) return;
  fprintf(stderr, "%s: %s\n", what, CORBA_exception_id(ev));
  exit(1);
}

int main(int argc, char** argv) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  char* args[64];
  int n = 0, i, status;
  CORBA_ORB orb;
  PortableServer_POA poa;
  PortableServer_POAManager mgr;
  InteropTest_Echo echo;
  if (argc < 2 || argc > 60) {
    fprintf(stderr, "usage: ami_probe REFERENCE [ORB options]\n");
    return 2;
  }
  args[n++] = argv[0];
  args[n++] = "-ORBendPoint";
  args[n++] = "giop:tcp:127.0.0.1:";
  for (i = 2; i < argc; i++) args[n++] = argv[i];
  args[n] = NULL;
  orb = CORBA_ORB_init(&n, args, "", &ev);
  die_on(&ev, "ORB_init");
  poa = (PortableServer_POA)CORBA_ORB_resolve_initial_references(orb, "RootPOA", &ev);
  die_on(&ev, "RootPOA");
  mgr = PortableServer_POA__get_the_POAManager(poa, &ev);
  die_on(&ev, "the_POAManager");
  PortableServer_POAManager_activate(mgr, &ev);
  die_on(&ev, "activate");
  echo = CORBA_ORB_string_to_object(orb, argv[1], &ev);
  die_on(&ev, "string_to_object");
  status = ami_cases(poa, echo, emit);
  CORBA_Object_release(echo, &ev);
  CORBA_Object_release(mgr, &ev);
  CORBA_Object_release(poa, &ev);
  CORBA_ORB_destroy(orb, &ev);
  CORBA_Object_release(orb, &ev);
  return status;
}

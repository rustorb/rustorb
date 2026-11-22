/* The release workflow builds this against an unpacked rustorb-c archive
 * and runs it: an ORB starts, gives its RootPOA and is destroyed. */
#include <stdio.h>

#include <rustorb/orb.h>

int main(int argc, char** argv) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  CORBA_ORB orb = CORBA_ORB_init(&argc, argv, "", &ev);
  CORBA_Object poa;
  if (ev._major != CORBA_NO_EXCEPTION) return 1;
  poa = CORBA_ORB_resolve_initial_references(orb, "RootPOA", &ev);
  if (ev._major != CORBA_NO_EXCEPTION || poa == CORBA_OBJECT_NIL) return 2;
  CORBA_Object_release(poa, &ev);
  CORBA_ORB_destroy(orb, &ev);
  if (ev._major != CORBA_NO_EXCEPTION) return 3;
  puts("rustorb-c: ok");
  return 0;
}

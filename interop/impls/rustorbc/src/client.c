/* The C InteropTest client (rustorbc): resolves Echo from the naming
 * service, runs every case and exits non-zero if one fails.
 *
 * Licensed under MIT OR Apache-2.0, like the rest of the rustorb core. */

#include <stdio.h>
#include <string.h>

#include "cases.h"
#include "common.h"

int main(int argc, char** argv) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  options opt;
  CORBA_ORB orb;
  PortableServer_POA poa;
  CosNaming_NamingContext root;
  InteropTest_Echo echo;
  runner r;
  parse_args("client", argc, argv, &opt, 0);
  orb = orb_init(&opt);
  poa = persistent_poa(orb, "InteropTestClient");
  root = root_context(orb, opt.timeout);
  echo = resolve_retry(root, opt.name, opt.timeout);

  memset(&r, 0, sizeof r);
  cases_run(&r, echo, poa);
  if (opt.shutdown_server) {
    InteropTest_Echo_shutdown(echo, &ev);
    if (ev._major != CORBA_NO_EXCEPTION) {
      fprintf(stderr, "shutdown: %s\n", CORBA_exception_id(&ev));
      CORBA_exception_free(&ev);
    }
  }
  cases_summary(&r);

  CORBA_Object_release(echo, &ev);
  CORBA_Object_release(root, &ev);
  CORBA_Object_release(poa, &ev);
  CORBA_ORB_destroy(orb, &ev);
  CORBA_Object_release(orb, &ev);
  cases_fini();
  return r.failed == 0 ? 0 : 1;
}

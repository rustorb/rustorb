/* The C InteropTest client (rustorbc): resolves Echo from the naming
 * service, runs every case and exits non-zero if one fails.
 *
 * Licensed under MIT OR Apache-2.0, like the rest of the rustorb core. */

#include <stdio.h>
#include <string.h>

#include "anytest.h"
#include "anytest_hidden.h"
#include "cases.h"
#include "common.h"
#include "dii_adapters.h"

/* The stage 2 suite against the AnyEcho object at opt->name; the server's
   Echo object is at opt->name + "-Echo". */
static int any_client(CORBA_ORB orb, options* opt) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  CosNaming_NamingContext root = root_context(orb, opt->timeout);
  AnyTest_AnyEcho echo = resolve_retry(root, opt->name, opt->timeout);
  runner r;
  char echo_name[512];
  anytest_init(orb);
  memset(&r, 0, sizeof r);
  anytest_run(&r, echo, anytest_hidden);
  if (opt->shutdown_server) {
    InteropTest_Echo e;
    snprintf(echo_name, sizeof echo_name, "%s-Echo", opt->name);
    e = resolve_retry(root, echo_name, opt->timeout);
    InteropTest_Echo_shutdown(e, &ev);
    if (ev._major != CORBA_NO_EXCEPTION) {
      fprintf(stderr, "shutdown: %s\n", CORBA_exception_id(&ev));
      CORBA_exception_free(&ev);
    }
    CORBA_Object_release(e, &ev);
  }
  cases_summary(&r);
  anytest_fini();
  CORBA_Object_release(echo, &ev);
  CORBA_Object_release(root, &ev);
  CORBA_ORB_destroy(orb, &ev);
  CORBA_Object_release(orb, &ev);
  return r.failed == 0 ? 0 : 1;
}

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
  if (dii_enabled()) {
    /* rustorbc-dii: every call of the suites through the DII. */
    dii_client_install(&InteropTest_idl__module, 0);
    dii_client_install(&AnyTest_idl__module, 0);
  }
  if (opt.any_suite) return any_client(orb, &opt);
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

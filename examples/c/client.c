/* Looks "Echo" up in the naming service and calls it.
 *
 *   cc -std=c99 -I$PREFIX/include -Igen client.c gen/Echo.c \
 *      $PREFIX/lib/librustorb_c.a $PREFIX/lib/librustorb_ffi.a -lpthread -ldl -lm -o client
 *   ./client -ORBInitRef NameService=corbaloc::localhost:2809/NameService [--shutdown]
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <rustorb/CosNaming.h>

#include "Echo.h"

static void check(CORBA_Environment* ev, const char* what) {
  if (ev->_major == CORBA_NO_EXCEPTION) return;
  fprintf(stderr, "client: %s: %s\n", what, CORBA_exception_id(ev));
  exit(1);
}

int main(int argc, char** argv) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  CORBA_ORB orb;
  CosNaming_NamingContextExt root;
  Example_Echo echo;
  CORBA_char* reply;
  int shutdown = 0, i;

  orb = CORBA_ORB_init(&argc, argv, "", &ev);
  check(&ev, "ORB_init");
  for (i = 1; i < argc; i++)
    if (strcmp(argv[i], "--shutdown") == 0) shutdown = 1;

  root = (CosNaming_NamingContextExt)CORBA_ORB_resolve_initial_references(orb, "NameService", &ev);
  check(&ev, "NameService");
  echo = (Example_Echo)CosNaming_NamingContextExt_resolve_str(root, "Echo", &ev);
  check(&ev, "resolve_str");

  reply = Example_Echo_echo(echo, "hello from C", &ev);
  check(&ev, "echo");
  printf("client: %s\n", reply);
  CORBA_free(reply);
  if (shutdown) {
    Example_Echo_shutdown(echo, &ev);
    check(&ev, "shutdown");
  }
  CORBA_Object_release(echo, &ev);
  CORBA_Object_release(root, &ev);
  CORBA_ORB_destroy(orb, &ev);
  CORBA_Object_release(orb, &ev);
  return 0;
}

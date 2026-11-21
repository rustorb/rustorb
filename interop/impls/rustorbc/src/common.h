/* Helpers shared by the C InteropTest server, client and naming check
 * (rustorbc): the same options and naming service handling as the other
 * implementations, on the OMG C mapping.
 *
 * Licensed under MIT OR Apache-2.0, like the rest of the rustorb core. */

#ifndef RUSTORBC_COMMON_H
#define RUSTORBC_COMMON_H

#include <rustorb/CosNaming.h>

typedef struct options {
  const char* naming;
  const char* name;
  const char* host;
  int port;
  double timeout;
  int shutdown_server;
  /* What is not ours goes to CORBA_ORB_init. */
  int orb_argc;
  char* orb_argv[64];
} options;

/* Parses the options shared with the other implementations (exits with 2 on
   a bad one). naming_only: the naming check's options (--naming,
   --timeout). */
void parse_args(const char* prog, int argc, char** argv, options* opt, int naming_only);

/* The ORB, with the naming service, the endpoint and the GIOP limit, as the
   other rustorb implementations make it (exits with 1 on failure). */
CORBA_ORB orb_init(options* opt);

/* Exits with 1 and a message if ev holds an exception. */
void die_on(CORBA_Environment* ev, const char* what);

/* The last part of a repository id without the version ("BAD_PARAM" for
   IDL:omg.org/CORBA/BAD_PARAM:1.0), in a static buffer. */
const char* exception_name(const char* id);
/* The exception as the other implementations print it:
   "BAD_PARAM(completed=COMPLETED_NO)", or the user exception's name. */
void format_exception(CORBA_Environment* ev, char* buf, size_t size);

/* path ("a/b.kind") as a name: components split at '/', id and kind at the
   first '.' if split_kind. Freed with CORBA_free. */
CosNaming_Name* to_name(const char* path, int split_kind);

/* The root naming context, retried until the timeout. */
CosNaming_NamingContext root_context(CORBA_ORB orb, double timeout);
/* Resolves path, retried until the timeout (the server may not have bound it
   yet). */
CORBA_Object resolve_retry(CosNaming_NamingContext root, const char* path, double timeout);
/* Binds obj at path, creating intermediate contexts as needed. */
void bind_name(CosNaming_NamingContext root, const char* path, CORBA_Object obj);

/* A PERSISTENT / USER_ID child POA, so that object keys are fixed and GIOP
   captures are reproducible between runs; the manager is activated. */
PortableServer_POA persistent_poa(CORBA_ORB orb, const char* name);

#endif

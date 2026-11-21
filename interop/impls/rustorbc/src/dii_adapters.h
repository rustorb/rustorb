/* The rustorbc-dii implementation: the C client and server of this
 * directory with every call of the basic and any suites made through the
 * DII and their servants run through the DSI (<rustorb/dii.h>).
 *
 * With RUSTORB_DII set, the client sends each call of the generated stubs
 * of InteropTest.idl and AnyTest.idl as a CORBA_Request built from the
 * TypeCodes of the operation's parameters (a hook of rustorb_c_invoke),
 * and the server activates each servant inside a PortableServer_DynamicImpl
 * that reads the arguments with CORBA_ServerRequest_arguments and calls the
 * static servant's skeleton. The messages are those of the static stubs
 * and skeletons. (The callback objects of the client stay static.)
 *
 * Licensed under MIT OR Apache-2.0, like the rest of the rustorb core. */

#ifndef RUSTORBC_DII_ADAPTERS_H
#define RUSTORBC_DII_ADAPTERS_H

#include <rustorb/dii.h>
#include <rustorb/runtime.h>

/* Whether RUSTORB_DII is set (and not "0"). */
int dii_enabled(void);

/* Sends the stub calls of the operations of module through the DII (up to
   8 modules); deferred: with CORBA_Request_send and get_response. */
void dii_client_install(const rustorb_c_module* module, int deferred);
void dii_client_uninstall(void);

/* A DSI servant that runs the requests with the skeletons of target (an
   initialised servant of the generated code, which stays the caller's). */
PortableServer_Servant dii_servant_new(PortableServer_Servant target, CORBA_Environment* ev);
void dii_servant_free(PortableServer_Servant servant);

#endif

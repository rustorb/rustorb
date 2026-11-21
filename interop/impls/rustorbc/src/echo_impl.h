/* The InteropTest::Echo servant of the C server (rustorbc), on the OMG C
 * mapping. Behaves as the comments in idl/InteropTest.idl say.
 *
 * Licensed under MIT OR Apache-2.0, like the rest of the rustorb core. */

#ifndef RUSTORBC_ECHO_IMPL_H
#define RUSTORBC_ECHO_IMPL_H

#include "InteropTest.h"

/* A new Echo servant (POA_InteropTest_Echo__init done) whose shutdown()
   shuts orb down. */
PortableServer_Servant echo_impl_new(CORBA_ORB orb, CORBA_Environment* ev);
/* __fini and free, after it is deactivated. */
void echo_impl_free(PortableServer_Servant servant);

#endif

/* The stage 2 tests (idl/AnyTest.idl) of the C client and server
 * (rustorbc): the table of values, the AnyEcho servant and the client
 * cases, those of the other implementations (impls/omniorbpy/anytest.py).
 *
 * Licensed under MIT OR Apache-2.0, like the rest of the rustorb core. */

#ifndef RUSTORBC_ANYTEST_H
#define RUSTORBC_ANYTEST_H

#include <rustorb/dynany.h>

#include "AnyTest.h"
#include "cases.h"

/* Sets the ORB (for the DynAnyFactory and the TypeCode of fixed<31, 4>);
   before anything else. */
void anytest_init(CORBA_ORB orb);
void anytest_fini(void);

/* A new AnyEcho servant (POA_AnyTest_AnyEcho__init done). */
PortableServer_Servant anytest_servant_new(CORBA_Environment* ev);
/* __fini and free, after it is deactivated. */
void anytest_servant_free(PortableServer_Servant servant);

/* Runs the stage 2 cases against echo. The values of AnyTestHidden.idl
   come from hidden (anytest_hidden.c, linked into the client only), which
   returns NULL for other names. */
typedef CORBA_any* (*anytest_hidden_fn)(const char* name);
void anytest_run(runner* r, AnyTest_AnyEcho echo, anytest_hidden_fn hidden);

#endif

/* The values of idl/AnyTestHidden.idl (the client's only).
 *
 * Licensed under MIT OR Apache-2.0, like the rest of the rustorb core. */

#ifndef RUSTORBC_ANYTEST_HIDDEN_H
#define RUSTORBC_ANYTEST_HIDDEN_H

#include <rustorb/orb.h>

/* The value named name (secret, pick, moods), or NULL. */
CORBA_any* anytest_hidden(const char* name);

#endif

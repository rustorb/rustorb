/* The test cases of the C client, the same as those of the other clients
 * (case names included), on the OMG C mapping.
 *
 * Licensed under MIT OR Apache-2.0, like the rest of the rustorb core. */

#ifndef RUSTORBC_CASES_H
#define RUSTORBC_CASES_H

#include "InteropTest.h"

typedef struct runner {
  int passed;
  int failed;
  char failed_names[8192];
} runner;

/* Runs every case against echo; poa is where the callback object goes. */
void cases_run(runner* r, InteropTest_Echo echo, PortableServer_POA poa);
/* Releases the callback servant (after cases_run, once the POA has let it
   go: after the ORB is destroyed). */
void cases_fini(void);
/* Prints the summary line(s). */
void cases_summary(const runner* r);

#endif

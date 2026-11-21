/* The cases of run-ami-probe.sh on the C mapping (callback AMI,
 * docs/ami.md): sendc_ requests to an Echo object, printing what the
 * AMI_EchoHandler gets, as impls/rustorbpy/ami_probe.py does. ami_probe.c
 * runs them against a server; c/tests/test_ami.c in one process.
 *
 * Licensed under MIT OR Apache-2.0, like the rest of the rustorb core. */

#ifndef RUSTORBC_AMI_CASES_H
#define RUSTORBC_AMI_CASES_H

#include "InteropTest.h"

/* Runs the cases, with the handler and the callback on poa (active),
   writing each line with emit; returns 0, or 1 if a reply did not come. */
int ami_cases(PortableServer_POA poa, InteropTest_Echo echo, void (*emit)(const char* line));

#endif

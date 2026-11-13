// Part of the IDL to Java mapping (org.omg.Messaging), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.
package org.omg.Messaging;

/** The operations of Messaging::SyncScopePolicy. */
public interface SyncScopePolicyOperations extends org.omg.CORBA.PolicyOperations {
    /** SYNC_NONE.value to SYNC_WITH_TARGET.value. */
    short synchronization();
}

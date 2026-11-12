// Part of the IDL to Java mapping (org.omg.Messaging), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.
package org.omg.Messaging;

/**
 * Messaging::SyncScopePolicy: how far a oneway request goes before the call
 * returns (docs/sync-scope.md). Made by ORB.create_policy from a SyncScope (a
 * short, SyncScopeHelper) in an any, and applied with
 * Object._set_policy_override, the ORBPolicyManager or the PolicyCurrent.
 * With SYNC_WITH_SERVER and SYNC_WITH_TARGET a oneway call waits for the
 * server's answer, and raises the system exceptions of a two-way call
 * (OBJECT_NOT_EXIST, say).
 */
public interface SyncScopePolicy extends SyncScopePolicyOperations,
        org.omg.CORBA.Policy, org.omg.CORBA.portable.IDLEntity {}

// Part of the IDL to Java mapping (org.omg.BiDirPolicy), written for rustorb
// from the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.BiDirPolicy;

/**
 * BiDirPolicy::BidirectionalPolicy: made by ORB.create_policy from a
 * BidirectionalPolicyValue (an unsigned short) in an any. BOTH on a POA lets
 * the objects of the POA take requests over the connections their ORB opened;
 * BOTH on the ORBPolicyManager is -ORBofferBiDirectionalGIOP 1 and
 * -ORBacceptBiDirectionalGIOP 1.
 */
public interface BidirectionalPolicy extends BidirectionalPolicyOperations,
        org.omg.CORBA.Policy, org.omg.CORBA.portable.IDLEntity {}

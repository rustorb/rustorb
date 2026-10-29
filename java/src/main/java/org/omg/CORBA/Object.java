// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA;

/** An object reference (CORBA::Object). null is the nil reference. */
public interface Object {
    boolean _is_a(String repositoryIdentifier);

    boolean _is_equivalent(Object other);

    boolean _non_existent();

    int _hash(int maximum);

    Object _duplicate();

    void _release();

    Object _get_interface_def();
}

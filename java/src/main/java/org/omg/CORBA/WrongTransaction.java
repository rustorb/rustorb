// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA;


/** CORBA::WrongTransaction (Request.get_response, ORB.get_next_response).
 * rustorb has no transactions and never raises it. */
public final class WrongTransaction extends UserException {
    private static final long serialVersionUID = 1L;

    public WrongTransaction() {
        super("IDL:omg.org/CORBA/WrongTransaction:1.0");
    }

    public WrongTransaction(String reason) {
        super(reason);
    }
}
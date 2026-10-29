// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA.portable;

/** The ORB's implementation of an object reference (ObjectImpl). */
public abstract class Delegate {
    public org.omg.CORBA.Object duplicate(org.omg.CORBA.Object self) {
        return self;
    }

    public void release(org.omg.CORBA.Object self) {}

    public abstract boolean is_a(org.omg.CORBA.Object self, String repository_id);

    public abstract boolean non_existent(org.omg.CORBA.Object self);

    public abstract boolean is_equivalent(org.omg.CORBA.Object self, org.omg.CORBA.Object rhs);

    public abstract int hash(org.omg.CORBA.Object self, int max);

    public org.omg.CORBA.Object get_interface_def(org.omg.CORBA.Object self) {
        throw new org.omg.CORBA.NO_IMPLEMENT();
    }

    public org.omg.CORBA.ORB orb(org.omg.CORBA.Object self) {
        throw new org.omg.CORBA.NO_IMPLEMENT();
    }

    public OutputStream request(org.omg.CORBA.Object self, String operation,
            boolean responseExpected) {
        throw new org.omg.CORBA.NO_IMPLEMENT();
    }

    public InputStream invoke(org.omg.CORBA.Object self, OutputStream output)
            throws ApplicationException, RemarshalException {
        throw new org.omg.CORBA.NO_IMPLEMENT();
    }

    public void releaseReply(org.omg.CORBA.Object self, InputStream input) {}

    public boolean is_local(org.omg.CORBA.Object self) {
        return false;
    }

    public ServantObject servant_preinvoke(org.omg.CORBA.Object self, String operation,
            Class<?> expectedType) {
        return null;
    }

    public void servant_postinvoke(org.omg.CORBA.Object self, ServantObject servant) {}

    public String toString(org.omg.CORBA.Object self) {
        return self.getClass().getName() + ":" + this.toString();
    }

    public int hashCode(org.omg.CORBA.Object self) {
        return System.identityHashCode(self);
    }

    public boolean equals(org.omg.CORBA.Object self, java.lang.Object obj) {
        return self == obj;
    }
}

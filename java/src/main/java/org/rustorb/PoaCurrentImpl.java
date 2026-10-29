// PortableServer::Current: the request the calling thread runs.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.rustorb;

import org.omg.CORBA.UserException;
import org.omg.PortableServer.Current;
import org.omg.PortableServer.CurrentPackage.NoContext;
import org.omg.PortableServer.POA;
import org.omg.PortableServer.Servant;

final class PoaCurrentImpl extends org.omg.CORBA.LocalObject implements Current {
    static final PoaCurrentImpl INSTANCE = new PoaCurrentImpl();

    private PoaCurrentImpl() {}

    @Override
    public boolean _is_a(String id) {
        return id.equals("IDL:omg.org/PortableServer/Current:1.0")
                || id.equals("IDL:omg.org/CORBA/Current:1.0")
                || id.equals("IDL:omg.org/CORBA/LocalObject:1.0")
                || id.equals("IDL:omg.org/CORBA/Object:1.0");
    }

    @Override
    public POA get_POA() throws NoContext {
        try {
            return PoaImpl.of(Native.currentGetPoa());
        } catch (NoContext e) {
            throw e;
        } catch (UserException e) {
            throw Bridge.unexpected(e);
        }
    }

    @Override
    public byte[] get_object_id() throws NoContext {
        try {
            return Native.currentGetObjectId();
        } catch (NoContext e) {
            throw e;
        } catch (UserException e) {
            throw Bridge.unexpected(e);
        }
    }

    @Override
    public org.omg.CORBA.Object get_reference() throws NoContext {
        try {
            return ObjectStub.of(Native.currentGetReference(), OrbImpl.defaultOrb());
        } catch (NoContext e) {
            throw e;
        } catch (UserException e) {
            throw Bridge.unexpected(e);
        }
    }

    @Override
    public Servant get_servant() throws NoContext {
        try {
            return (Servant) Native.currentGetServant();
        } catch (NoContext e) {
            throw e;
        } catch (UserException e) {
            throw Bridge.unexpected(e);
        }
    }
}

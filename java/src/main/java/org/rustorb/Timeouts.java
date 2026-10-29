// Client call timeouts: a rustorb extension, with omniORB's names.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.rustorb;

import java.lang.ref.Reference;

import org.omg.CORBA.BAD_INV_ORDER;
import org.omg.CORBA.BAD_PARAM;
import org.omg.CORBA.CompletionStatus;

/**
 * Client call timeouts in milliseconds. A call waits as long as the timeout
 * of the object reference says, else that of the calling thread, else that
 * of the ORB (also set by the ORB property or option
 * {@code ORBclientCallTimeOutPeriod}); the ORB's covers connecting too,
 * unless a connect timeout is set. When it expires the call throws
 * {@code org.omg.CORBA.TIMEOUT}: {@code COMPLETED_MAYBE} once the request
 * was sent (and a CancelRequest goes to the server), {@code COMPLETED_NO}
 * before. 0 means no limit for the ORB, and unsets the timeout of a
 * reference or a thread.
 *
 * <p>The methods without an ORB use the first ORB initialised and not
 * destroyed ({@code BAD_INV_ORDER} if there is none).
 */
public final class Timeouts {
    private Timeouts() {}

    public static void setClientCallTimeout(org.omg.CORBA.ORB orb, int millisecs) {
        Native.orbSetCallTimeout(orb(orb).handle(), micros(millisecs));
    }

    public static void setClientCallTimeout(int millisecs) {
        setClientCallTimeout(defaultOrb(), millisecs);
    }

    /**
     * The call timeout of the reference {@code obj}, shared by the stubs
     * narrowed from it; 0 unsets it.
     */
    public static void setClientCallTimeout(org.omg.CORBA.Object obj, int millisecs) {
        if (obj == null) {
            throw new BAD_PARAM("a nil reference", 0, CompletionStatus.COMPLETED_NO);
        }
        ObjectDelegateImpl d = ObjectDelegateImpl.of(obj);
        try {
            Native.objectSetCallTimeout(d.handle(), millisecs == 0 ? -1 : micros(millisecs));
        } finally {
            Reference.reachabilityFence(d);
        }
    }

    /**
     * The call timeout of the calling thread, for every ORB; 0 unsets it.
     * (omniORB honours it only with {@code -ORBsupportPerThreadTimeOut 1}.)
     */
    public static void setClientThreadCallTimeout(int millisecs) {
        Native.setThreadCallTimeout(millisecs == 0 ? -1 : micros(millisecs));
    }

    public static void setClientConnectTimeout(org.omg.CORBA.ORB orb, int millisecs) {
        Native.orbSetConnectTimeout(orb(orb).handle(), micros(millisecs));
    }

    public static void setClientConnectTimeout(int millisecs) {
        setClientConnectTimeout(defaultOrb(), millisecs);
    }

    /** The ORB's call timeout in milliseconds; 0 for none. */
    public static int clientCallTimeout(org.omg.CORBA.ORB orb) {
        return (int) (Native.orbCallTimeout(orb(orb).handle()) / 1000);
    }

    /** The ORB's connect timeout in milliseconds; 0 for none. */
    public static int clientConnectTimeout(org.omg.CORBA.ORB orb) {
        return (int) (Native.orbConnectTimeout(orb(orb).handle()) / 1000);
    }

    private static long micros(int millisecs) {
        if (millisecs < 0) {
            throw new BAD_PARAM("a negative timeout", 0, CompletionStatus.COMPLETED_NO);
        }
        return millisecs * 1000L;
    }

    private static OrbImpl orb(org.omg.CORBA.ORB orb) {
        if (orb instanceof OrbImpl) {
            return (OrbImpl) orb;
        }
        throw new BAD_PARAM("not an ORB of rustorb", 0, CompletionStatus.COMPLETED_NO);
    }

    private static OrbImpl defaultOrb() {
        OrbImpl orb = OrbImpl.defaultOrb();
        if (orb == null) {
            throw new BAD_INV_ORDER("no ORB is initialised", 0, CompletionStatus.COMPLETED_NO);
        }
        return orb;
    }
}

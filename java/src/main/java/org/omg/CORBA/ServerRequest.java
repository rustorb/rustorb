// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA;

/**
 * CORBA::ServerRequest: the request that a
 * PortableServer.DynamicImplementation servant receives in invoke.
 *
 * Its calls are made in this order: arguments once (BAD_INV_ORDER the
 * second time), then set_result or set_exception (set_result before
 * arguments is BAD_INV_ORDER). The servant sets the out and inout values
 * in the anys of the NVList it gave to arguments; they are sent when
 * invoke returns. A system exception is thrown from invoke instead.
 */
public abstract class ServerRequest {
    public abstract String operation();

    /** Reads the in and inout arguments into the anys of args, whose types
     * (their values are ignored) and flags are those of the operation's
     * parameters, in order. Arguments that do not fit them are MARSHAL. */
    public abstract void arguments(NVList args);

    public abstract void set_result(Any any);

    /** any holds a user exception (else BAD_PARAM). */
    public abstract void set_exception(Any any);

    public abstract Context ctx();

    /** @deprecated use operation() */
    @Deprecated
    public String op_name() {
        return operation();
    }

    /** @deprecated use arguments(NVList) */
    @Deprecated
    public void params(NVList params) {
        arguments(params);
    }

    /** @deprecated use set_result(Any) */
    @Deprecated
    public void result(Any any) {
        set_result(any);
    }

    /** @deprecated use set_exception(Any) */
    @Deprecated
    public void except(Any any) {
        set_exception(any);
    }
}

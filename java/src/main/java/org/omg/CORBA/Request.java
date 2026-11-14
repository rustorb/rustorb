// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA;

/**
 * CORBA::Request: a call made through the DII (Object._request,
 * Object._create_request).
 *
 * The arguments are the items of arguments(), whose anys hold the values
 * of the in and inout arguments and the types of the out ones. After the
 * call the out and inout anys and return_value() hold the values of the
 * reply, and env() holds the exception of the call, or null: a user
 * exception of exceptions() is an UnknownUserException whose except holds
 * it, one not in the list is UNKNOWN (minor 0x4f4d0001, COMPLETED_YES).
 * A system exception is put in env() too, not thrown.
 */
public abstract class Request {
    public abstract Object target();

    public abstract String operation();

    public abstract NVList arguments();

    public abstract NamedValue result();

    public abstract Environment env();

    public abstract ExceptionList exceptions();

    public abstract ContextList contexts();

    public abstract Context ctx();

    public abstract void ctx(Context c);

    public abstract Any add_in_arg();

    public abstract Any add_named_in_arg(String name);

    public abstract Any add_inout_arg();

    public abstract Any add_named_inout_arg(String name);

    /** The any of an out argument: give it the type (Any.type(tc)). */
    public abstract Any add_out_arg();

    public abstract Any add_named_out_arg(String name);

    public abstract void set_return_type(TypeCode tc);

    public abstract Any return_value();

    public abstract void invoke();

    public abstract void send_oneway();

    /** Sends the request without waiting for the reply (BAD_INV_ORDER while
     * one is outstanding). */
    public abstract void send_deferred();

    /** Whether the reply of send_deferred has arrived (BAD_INV_ORDER when
     * nothing was sent deferred). */
    public abstract boolean poll_response();

    /** Waits for the reply of send_deferred and takes it, as invoke. */
    public abstract void get_response() throws WrongTransaction;
}

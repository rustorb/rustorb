// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA;


/** CORBA::Context. rustorb has no contexts: ORB.get_default_context is
 * NO_IMPLEMENT. */
public abstract class Context {
    public abstract String context_name();

    public abstract Context parent();

    public abstract Context create_child(String child_ctx_name);

    public abstract void set_one_value(String propname, Any propvalue);

    public abstract void set_values(NVList values);

    public abstract void delete_values(String propname);

    public abstract NVList get_values(String start_scope, int op_flags, String pattern);
}
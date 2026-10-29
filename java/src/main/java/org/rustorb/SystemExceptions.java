// The system exceptions by repository id.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.rustorb;

import org.omg.CORBA.CompletionStatus;
import org.omg.CORBA.SystemException;

final class SystemExceptions {
    private SystemExceptions() {}

    private static final String PREFIX = "IDL:omg.org/CORBA/";

    /** The exception for a repository id; UNKNOWN for one that is not a
     * standard system exception. */
    static SystemException make(String id, int minor, CompletionStatus completed) {
        String name = id;
        if (name.startsWith(PREFIX) && name.endsWith(":1.0")) {
            name = name.substring(PREFIX.length(), name.length() - 4);
        }
        switch (name) {
            case "UNKNOWN":
                return new org.omg.CORBA.UNKNOWN("", minor, completed);
            case "BAD_PARAM":
                return new org.omg.CORBA.BAD_PARAM("", minor, completed);
            case "NO_MEMORY":
                return new org.omg.CORBA.NO_MEMORY("", minor, completed);
            case "IMP_LIMIT":
                return new org.omg.CORBA.IMP_LIMIT("", minor, completed);
            case "COMM_FAILURE":
                return new org.omg.CORBA.COMM_FAILURE("", minor, completed);
            case "INV_OBJREF":
                return new org.omg.CORBA.INV_OBJREF("", minor, completed);
            case "NO_PERMISSION":
                return new org.omg.CORBA.NO_PERMISSION("", minor, completed);
            case "INTERNAL":
                return new org.omg.CORBA.INTERNAL("", minor, completed);
            case "MARSHAL":
                return new org.omg.CORBA.MARSHAL("", minor, completed);
            case "INITIALIZE":
                return new org.omg.CORBA.INITIALIZE("", minor, completed);
            case "NO_IMPLEMENT":
                return new org.omg.CORBA.NO_IMPLEMENT("", minor, completed);
            case "BAD_TYPECODE":
                return new org.omg.CORBA.BAD_TYPECODE("", minor, completed);
            case "BAD_OPERATION":
                return new org.omg.CORBA.BAD_OPERATION("", minor, completed);
            case "NO_RESOURCES":
                return new org.omg.CORBA.NO_RESOURCES("", minor, completed);
            case "NO_RESPONSE":
                return new org.omg.CORBA.NO_RESPONSE("", minor, completed);
            case "PERSIST_STORE":
                return new org.omg.CORBA.PERSIST_STORE("", minor, completed);
            case "BAD_INV_ORDER":
                return new org.omg.CORBA.BAD_INV_ORDER("", minor, completed);
            case "TRANSIENT":
                return new org.omg.CORBA.TRANSIENT("", minor, completed);
            case "FREE_MEM":
                return new org.omg.CORBA.FREE_MEM("", minor, completed);
            case "INV_IDENT":
                return new org.omg.CORBA.INV_IDENT("", minor, completed);
            case "INV_FLAG":
                return new org.omg.CORBA.INV_FLAG("", minor, completed);
            case "INTF_REPOS":
                return new org.omg.CORBA.INTF_REPOS("", minor, completed);
            case "BAD_CONTEXT":
                return new org.omg.CORBA.BAD_CONTEXT("", minor, completed);
            case "OBJ_ADAPTER":
                return new org.omg.CORBA.OBJ_ADAPTER("", minor, completed);
            case "DATA_CONVERSION":
                return new org.omg.CORBA.DATA_CONVERSION("", minor, completed);
            case "OBJECT_NOT_EXIST":
                return new org.omg.CORBA.OBJECT_NOT_EXIST("", minor, completed);
            case "TRANSACTION_REQUIRED":
                return new org.omg.CORBA.TRANSACTION_REQUIRED("", minor, completed);
            case "TRANSACTION_ROLLEDBACK":
                return new org.omg.CORBA.TRANSACTION_ROLLEDBACK("", minor, completed);
            case "INVALID_TRANSACTION":
                return new org.omg.CORBA.INVALID_TRANSACTION("", minor, completed);
            case "INV_POLICY":
                return new org.omg.CORBA.INV_POLICY("", minor, completed);
            case "CODESET_INCOMPATIBLE":
                return new org.omg.CORBA.CODESET_INCOMPATIBLE("", minor, completed);
            case "REBIND":
                return new org.omg.CORBA.REBIND("", minor, completed);
            case "TIMEOUT":
                return new org.omg.CORBA.TIMEOUT("", minor, completed);
            case "TRANSACTION_UNAVAILABLE":
                return new org.omg.CORBA.TRANSACTION_UNAVAILABLE("", minor, completed);
            case "TRANSACTION_MODE":
                return new org.omg.CORBA.TRANSACTION_MODE("", minor, completed);
            case "BAD_QOS":
                return new org.omg.CORBA.BAD_QOS("", minor, completed);
            case "INVALID_ACTIVITY":
                return new org.omg.CORBA.INVALID_ACTIVITY("", minor, completed);
            case "ACTIVITY_COMPLETED":
                return new org.omg.CORBA.ACTIVITY_COMPLETED("", minor, completed);
            case "ACTIVITY_REQUIRED":
                return new org.omg.CORBA.ACTIVITY_REQUIRED("", minor, completed);
            case "THREAD_CANCELLED":
                return new org.omg.CORBA.THREAD_CANCELLED("", minor, completed);
            default:
                return new org.omg.CORBA.UNKNOWN(id, minor, completed);
        }
    }

    /** The repository id of a system exception. */
    static String id(SystemException e) {
        Class<?> c = e.getClass();
        if (c.getPackage() != null && "org.omg.CORBA".equals(c.getPackage().getName())) {
            return PREFIX + c.getSimpleName() + ":1.0";
        }
        return PREFIX + "UNKNOWN:1.0";
    }
}

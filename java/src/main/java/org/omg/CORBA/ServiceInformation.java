// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA;

/** CORBA::ServiceInformation */
public final class ServiceInformation implements org.omg.CORBA.portable.IDLEntity {
    public int[] service_options;
    public ServiceDetail[] service_details;

    public ServiceInformation() {}

    public ServiceInformation(int[] service_options, ServiceDetail[] service_details) {
        this.service_options = service_options;
        this.service_details = service_details;
    }
}

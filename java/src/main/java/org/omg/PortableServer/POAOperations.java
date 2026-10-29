// Part of the IDL to Java mapping (org.omg.PortableServer), written for
// rustorb from the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.PortableServer;

import org.omg.PortableServer.POAPackage.AdapterAlreadyExists;
import org.omg.PortableServer.POAPackage.AdapterNonExistent;
import org.omg.PortableServer.POAPackage.InvalidPolicy;
import org.omg.PortableServer.POAPackage.NoServant;
import org.omg.PortableServer.POAPackage.ObjectAlreadyActive;
import org.omg.PortableServer.POAPackage.ObjectNotActive;
import org.omg.PortableServer.POAPackage.ServantAlreadyActive;
import org.omg.PortableServer.POAPackage.ServantNotActive;
import org.omg.PortableServer.POAPackage.WrongAdapter;
import org.omg.PortableServer.POAPackage.WrongPolicy;

/** The operations of PortableServer::POA. */
public interface POAOperations {
    POA create_POA(String adapter_name, POAManager a_POAManager, org.omg.CORBA.Policy[] policies)
            throws AdapterAlreadyExists, InvalidPolicy;

    POA find_POA(String adapter_name, boolean activate_it) throws AdapterNonExistent;

    void destroy(boolean etherealize_objects, boolean wait_for_completion);

    ThreadPolicy create_thread_policy(ThreadPolicyValue value);

    LifespanPolicy create_lifespan_policy(LifespanPolicyValue value);

    IdUniquenessPolicy create_id_uniqueness_policy(IdUniquenessPolicyValue value);

    IdAssignmentPolicy create_id_assignment_policy(IdAssignmentPolicyValue value);

    ImplicitActivationPolicy create_implicit_activation_policy(ImplicitActivationPolicyValue value);

    ServantRetentionPolicy create_servant_retention_policy(ServantRetentionPolicyValue value);

    RequestProcessingPolicy create_request_processing_policy(RequestProcessingPolicyValue value);

    String the_name();

    POA the_parent();

    POA[] the_children();

    POAManager the_POAManager();

    AdapterActivator the_activator();

    void the_activator(AdapterActivator newThe_activator);

    ServantManager get_servant_manager() throws WrongPolicy;

    void set_servant_manager(ServantManager imgr) throws WrongPolicy;

    Servant get_servant() throws NoServant, WrongPolicy;

    void set_servant(Servant p_servant) throws WrongPolicy;

    byte[] activate_object(Servant p_servant) throws ServantAlreadyActive, WrongPolicy;

    void activate_object_with_id(byte[] id, Servant p_servant)
            throws ServantAlreadyActive, ObjectAlreadyActive, WrongPolicy;

    void deactivate_object(byte[] oid) throws ObjectNotActive, WrongPolicy;

    org.omg.CORBA.Object create_reference(String intf) throws WrongPolicy;

    org.omg.CORBA.Object create_reference_with_id(byte[] oid, String intf);

    byte[] servant_to_id(Servant p_servant) throws ServantNotActive, WrongPolicy;

    org.omg.CORBA.Object servant_to_reference(Servant p_servant)
            throws ServantNotActive, WrongPolicy;

    Servant reference_to_servant(org.omg.CORBA.Object reference)
            throws ObjectNotActive, WrongPolicy, WrongAdapter;

    byte[] reference_to_id(org.omg.CORBA.Object reference) throws WrongAdapter, WrongPolicy;

    Servant id_to_servant(byte[] oid) throws ObjectNotActive, WrongPolicy;

    org.omg.CORBA.Object id_to_reference(byte[] oid) throws ObjectNotActive, WrongPolicy;

    byte[] id();
}

// Two ORBs in one process for the tests: a server ORB listening on the
// loopback interface, running on its own thread, and a client ORB.

package org.rustorb.test;

import org.omg.CORBA.ORB;
import org.omg.PortableServer.POA;
import org.omg.PortableServer.POAHelper;

final class Orbs implements AutoCloseable {
    final ORB server;
    final POA serverPoa;
    final ORB client;
    final POA clientPoa;
    private final Thread thread;

    Orbs() throws Exception {
        server = ORB.init(new String[] {"-ORBendPoint", "giop:tcp:127.0.0.1:"}, null);
        serverPoa = POAHelper.narrow(server.resolve_initial_references("RootPOA"));
        serverPoa.the_POAManager().activate();
        thread = new Thread(server::run, "server ORB");
        thread.start();
        client = ORB.init(new String[0], null);
        clientPoa = POAHelper.narrow(client.resolve_initial_references("RootPOA"));
        clientPoa.the_POAManager().activate();
    }

    @Override
    public void close() throws Exception {
        server.shutdown(true);
        thread.join();
        server.destroy();
        client.destroy();
    }
}

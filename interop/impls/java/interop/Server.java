// Java implementation of the InteropTest::Echo server (rustorbjava and
// jacorb).

package interop;

import java.nio.charset.StandardCharsets;

import org.omg.CORBA.ORB;
import org.omg.CosNaming.NamingContext;
import org.omg.PortableServer.POA;

public final class Server {
    private Server() {}

    public static void main(String[] argv) throws Exception {
        Common.Options opt = Common.parseArgs("server", argv);
        ORB orb = Common.orbInit(opt, "InteropTest");
        byte[] oid = "Echo".getBytes(StandardCharsets.US_ASCII);
        if (opt.suite.equals("any")) {
            // The stage 2 suite: AnyTest::AnyEcho, with an Echo object next
            // to it for the client to shut the server down.
            POA poa = Common.persistentPoa(orb, "AnyTest");
            byte[] anyOid = "AnyEcho".getBytes(StandardCharsets.US_ASCII);
            poa.activate_object_with_id(anyOid, new AnyTests.AnyEchoImpl(orb));
            poa.activate_object_with_id(oid, new EchoImpl(orb));
            NamingContext root = Common.rootContext(orb, opt.timeout);
            Common.bindName(root, opt.name, poa.id_to_reference(anyOid));
            Common.bindName(root, opt.name + "-Echo", poa.id_to_reference(oid));
        } else {
            POA poa = Common.persistentPoa(orb, "InteropTest");
            poa.activate_object_with_id(oid, new EchoImpl(orb));
            Common.bindName(Common.rootContext(orb, opt.timeout), opt.name, poa.id_to_reference(oid));
        }
        System.out.println("server ready: " + opt.name);
        System.out.flush();
        orb.run();
        orb.destroy();
        System.out.println("server stopped");
        System.exit(0);
    }
}

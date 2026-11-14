// Java implementation of the InteropTest::Echo server (rustorbjava and
// jacorb). With RUSTORB_DII set (the rustorbjava-dii implementation), the
// servants of the basic, any and value suites run behind the DSI
// (DiiAdapters).

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
        if (opt.suite.equals("value")) {
            // The valuetype suite: ValueTest::ValueEcho, which also stops the
            // server.
            ValueTests.registerFactories(orb);
            POA poa = Common.persistentPoa(orb, "ValueTest");
            byte[] valueOid = "ValueEcho".getBytes(StandardCharsets.US_ASCII);
            poa.activate_object_with_id(valueOid, DiiAdapters.servant(orb, new ValueTests.ValueEchoImpl(orb)));
            Common.bindName(Common.rootContext(orb, opt.timeout), opt.name,
                    poa.id_to_reference(valueOid));
        } else if (opt.suite.equals("custom")) {
            // The custom valuetype suite: CustomTest::CustomEcho, which also
            // stops the server.
            CustomTests.registerFactories(orb);
            POA poa = Common.persistentPoa(orb, "CustomTest");
            byte[] customOid = "CustomEcho".getBytes(StandardCharsets.US_ASCII);
            poa.activate_object_with_id(customOid, new CustomTests.CustomEchoImpl(orb));
            Common.bindName(Common.rootContext(orb, opt.timeout), opt.name,
                    poa.id_to_reference(customOid));
        } else if (opt.suite.equals("abstract")) {
            // The abstract interface suite: AbstractTest::AbstractEcho, which
            // also stops the server.
            AbstractTests.registerFactories(orb);
            POA poa = Common.persistentPoa(orb, "AbstractTest");
            byte[] abstractOid = "AbstractEcho".getBytes(StandardCharsets.US_ASCII);
            poa.activate_object_with_id(abstractOid, new AbstractTests.AbstractEchoImpl(orb));
            Common.bindName(Common.rootContext(orb, opt.timeout), opt.name,
                    poa.id_to_reference(abstractOid));
        } else if (opt.suite.equals("any")) {
            // The stage 2 suite: AnyTest::AnyEcho, with an Echo object next
            // to it for the client to shut the server down.
            POA poa = Common.persistentPoa(orb, "AnyTest");
            byte[] anyOid = "AnyEcho".getBytes(StandardCharsets.US_ASCII);
            poa.activate_object_with_id(anyOid, DiiAdapters.servant(orb, new AnyTests.AnyEchoImpl(orb)));
            poa.activate_object_with_id(oid, DiiAdapters.servant(orb, new EchoImpl(orb)));
            NamingContext root = Common.rootContext(orb, opt.timeout);
            Common.bindName(root, opt.name, poa.id_to_reference(anyOid));
            Common.bindName(root, opt.name + "-Echo", poa.id_to_reference(oid));
        } else {
            POA poa = Common.persistentPoa(orb, "InteropTest");
            poa.activate_object_with_id(oid, DiiAdapters.servant(orb, new EchoImpl(orb)));
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

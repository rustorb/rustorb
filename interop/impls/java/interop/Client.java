// Java implementation of the InteropTest client (rustorbjava and jacorb).
//
// Resolves the Echo object from the naming service, runs every test case
// against it and exits non-zero if any case fails.

package interop;

import AnyTest.AnyEcho;
import AnyTest.AnyEchoHelper;
import InteropTest.Echo;
import InteropTest.EchoHelper;
import org.omg.CORBA.ORB;
import org.omg.CosNaming.NamingContext;
import org.omg.PortableServer.POA;

public final class Client {
    private Client() {}

    /** The stage 2 suite (AnyTest.idl) against the AnyEcho object at
     * opt.name; the server's Echo object is at opt.name + "-Echo". */
    private static int runAny(ORB orb, NamingContext root, Common.Options opt) throws Exception {
        org.omg.CORBA.Object obj = Common.retry("the AnyEcho server",
                () -> root.resolve(Common.toName(opt.name)), opt.timeout);
        AnyEcho echo = AnyEchoHelper.narrow(obj);
        Common.retry("the AnyEcho server", echo::_non_existent, opt.timeout);

        Cases.Runner r = new Cases.Runner();
        AnyTests.run(r, orb, echo);
        if (opt.shutdownServer) {
            EchoHelper.narrow(root.resolve(Common.toName(opt.name + "-Echo"))).shutdown();
        }
        r.summary();
        return r.failed().isEmpty() ? 0 : 1;
    }

    public static void main(String[] argv) throws Exception {
        Common.Options opt = Common.parseArgs("client", argv);
        ORB orb = Common.orbInit(opt, "InteropTestClient");
        int status;
        try {
            POA poa = Common.persistentPoa(orb, "InteropTestClient");
            NamingContext root = Common.rootContext(orb, opt.timeout);
            if (opt.suite.equals("any")) {
                status = runAny(orb, root, opt);
                orb.destroy();
                System.exit(status);
            }
            org.omg.CORBA.Object obj = Common.retry("the Echo server",
                    () -> root.resolve(Common.toName(opt.name)), opt.timeout);
            Echo echo = EchoHelper.narrow(obj);
            Common.retry("the Echo server", echo::_non_existent, opt.timeout);

            Cases.Runner r = new Cases.Runner();
            Cases.run(r, echo, poa);
            if (opt.shutdownServer) {
                echo.shutdown();
            }
            r.summary();
            status = r.failed().isEmpty() ? 0 : 1;
        } catch (org.omg.CORBA.SystemException e) {
            System.err.println("CORBA exception: " + e);
            e.printStackTrace();
            status = 1;
        }
        orb.destroy();
        System.exit(status);
    }
}

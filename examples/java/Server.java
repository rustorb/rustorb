// Serves Example::Echo and binds it in the naming service as "Echo".
//
//   rustorb-idl -bjava -C gen ../idl/Echo.idl
//   javac --release 22 -cp rustorb-0.1.0.jar -d classes Server.java Client.java $(find gen -name '*.java')
//   java --enable-native-access=ALL-UNNAMED -Drustorb.ffi.library=path/to/librustorb_ffi.so \
//        -cp rustorb-0.1.0.jar:classes Server -ORBInitRef NameService=corbaloc::localhost:2809/NameService

import Example.Echo;
import Example.EchoHelper;
import Example.EchoPOA;
import org.omg.CORBA.ORB;
import org.omg.CosNaming.NameComponent;
import org.omg.CosNaming.NamingContext;
import org.omg.CosNaming.NamingContextHelper;
import org.omg.PortableServer.POA;
import org.omg.PortableServer.POAHelper;

public final class Server {
    private Server() {}

    static final class EchoImpl extends EchoPOA {
        private final ORB orb;

        EchoImpl(ORB orb) {
            this.orb = orb;
        }

        @Override
        public String echo(String message) {
            System.out.println("server: " + message);
            return message;
        }

        @Override
        public void shutdown() {
            orb.shutdown(false);
        }
    }

    public static void main(String[] args) throws Exception {
        ORB orb = ORB.init(args, null);
        POA poa = POAHelper.narrow(orb.resolve_initial_references("RootPOA"));
        poa.the_POAManager().activate();
        Echo echo = EchoHelper.narrow(poa.servant_to_reference(new EchoImpl(orb)));

        NamingContext root = NamingContextHelper.narrow(orb.resolve_initial_references("NameService"));
        root.rebind(new NameComponent[] {new NameComponent("Echo", "")}, echo);
        System.out.println("server: ready");

        orb.run();
        orb.destroy();
    }
}

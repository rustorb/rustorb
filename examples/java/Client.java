// Looks "Echo" up in the naming service and calls it.
//
//   java --enable-native-access=ALL-UNNAMED -Drustorb.ffi.library=path/to/librustorb_ffi.so \
//        -cp rustorb-0.1.0.jar:classes Client -ORBInitRef NameService=corbaloc::localhost:2809/NameService [--shutdown]

import Example.Echo;
import Example.EchoHelper;
import java.util.Arrays;
import org.omg.CORBA.ORB;
import org.omg.CosNaming.NamingContextExt;
import org.omg.CosNaming.NamingContextExtHelper;

public final class Client {
    private Client() {}

    public static void main(String[] args) throws Exception {
        ORB orb = ORB.init(args, null);
        boolean shutdown = Arrays.asList(args).contains("--shutdown");

        NamingContextExt root = NamingContextExtHelper.narrow(orb.resolve_initial_references("NameService"));
        Echo echo = EchoHelper.narrow(root.resolve_str("Echo"));

        System.out.println("client: " + echo.echo("hello from Java"));
        if (shutdown) {
            echo.shutdown();
        }
        orb.destroy();
    }
}

// The Java side of scripts/idle-probe (run-idle-probe.sh): calls Echo, waits
// longer than the idle timeout of the client or the server, checks that the
// connection to the server was closed, and calls again.
//
//   IdleProbe --naming CORBALOC --name PATH --server-port N --pause S [ORB options]

package interop;

import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.util.ArrayList;
import java.util.List;

import InteropTest.Echo;
import InteropTest.EchoHelper;
import org.omg.CORBA.ORB;
import org.omg.CosNaming.NamingContext;

public final class IdleProbe {
    private IdleProbe() {}

    /** The client's TCP connections to the server port, but those in
     * TIME-WAIT, as ss(8) counts them. */
    static int connections(int port) throws Exception {
        Process p = new ProcessBuilder("ss", "-Htn", "exclude", "time-wait",
                "( dport = :" + port + " )").redirectErrorStream(true).start();
        int n = 0;
        try (BufferedReader r = new BufferedReader(new InputStreamReader(p.getInputStream()))) {
            while (r.readLine() != null) {
                ++n;
            }
        }
        p.waitFor();
        return n;
    }

    public static void main(String[] argv) throws Exception {
        int serverPort = 2810;
        double pause = 4.0;
        List<String> rest = new ArrayList<>();
        for (int i = 0; i < argv.length; ++i) {
            if (argv[i].equals("--server-port")) {
                serverPort = Integer.parseInt(argv[++i]);
            } else if (argv[i].equals("--pause")) {
                pause = Double.parseDouble(argv[++i]);
            } else {
                rest.add(argv[i]);
            }
        }
        Common.Options opt = Common.parseArgs("IdleProbe", rest.toArray(new String[0]));
        ORB orb = Common.orbInit(opt, "IdleProbe");
        NamingContext root = Common.rootContext(orb, opt.timeout);
        Echo echo = EchoHelper.narrow(Common.retry("the Echo server",
                () -> root.resolve(Common.toName(opt.name)), opt.timeout));
        boolean ok = true;
        int got = echo.echo_long(1);
        int open = connections(serverPort);
        System.out.println("call 1: " + (got == 1 ? "ok" : "FAIL") + ", connections: " + open);
        ok &= got == 1 && open == 1;
        Thread.sleep((long) (pause * 1000));
        open = connections(serverPort);
        System.out.println("idle: connections: " + open);
        ok &= open == 0;
        got = echo.echo_long(2);
        open = connections(serverPort);
        System.out.println("call 2: " + (got == 2 ? "ok" : "FAIL") + ", connections: " + open);
        ok &= got == 2 && open == 1;
        System.out.println(ok ? "PASS" : "FAIL");
        orb.destroy();
        System.exit(ok ? 0 : 1);
    }
}

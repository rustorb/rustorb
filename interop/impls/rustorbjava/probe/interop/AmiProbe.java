// The rustorb Java side of run-ami-probe.sh: callback AMI (docs/ami.md)
// against the Echo server of any implementation, printing the same lines
// as impls/rustorbpy/ami_probe.py. On the stubs of InteropTest.idl
// generated with -Wbami (build.sh): sendc_ is on the stub class _EchoStub.
//
//   AmiProbe REFERENCE [ORB options]
//
// REFERENCE names the Echo object, such as corbaname::127.0.0.1:2809#ami/Echo.

package interop;

import java.lang.reflect.Proxy;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.BlockingQueue;
import java.util.concurrent.LinkedBlockingQueue;
import java.util.concurrent.TimeUnit;

import InteropTest.AMI_EchoHandler;
import InteropTest.AMI_EchoHandlerHelper;
import InteropTest.AMI_EchoHandlerOperations;
import InteropTest.AMI_EchoHandlerPOATie;
import InteropTest.Callback;
import InteropTest.CallbackHelper;
import InteropTest.CallbackPOA;
import InteropTest.DivideByZero;
import InteropTest.EchoHelper;
import InteropTest.Oops;
import InteropTest._EchoStub;
import org.omg.CORBA.CompletionStatus;
import org.omg.CORBA.ORB;
import org.omg.CORBA.SystemException;
import org.omg.Messaging.ExceptionHolder;
import org.omg.PortableServer.POA;
import org.omg.PortableServer.POAHelper;
import org.rustorb.Timeouts;

public final class AmiProbe {
    static final long WAIT = 10;

    static final BlockingQueue<String> lines = new LinkedBlockingQueue<>();

    static String completed(CompletionStatus c) {
        switch (c.value()) {
            case CompletionStatus._COMPLETED_YES: return "COMPLETED_YES";
            case CompletionStatus._COMPLETED_NO: return "COMPLETED_NO";
            case CompletionStatus._COMPLETED_MAYBE: return "COMPLETED_MAYBE";
            default: return "?";
        }
    }

    /** What raise_exception() throws, as a probe line. */
    static String describe(ExceptionHolder holder) {
        if (holder == null) {
            return "nil holder";
        }
        try {
            holder.raise_exception();
            return "nothing raised";
        } catch (Oops e) {
            return "Oops " + e.code + " " + e.reason;
        } catch (DivideByZero e) {
            return "DivideByZero";
        } catch (SystemException e) {
            return e.getClass().getSimpleName() + " " + e.minor + " " + completed(e.completed);
        } catch (Throwable e) {
            return "unexpected " + e;
        }
    }

    /**
     * The handler: the operations the probe expects put their line into the
     * queue; any other one puts "unexpected OP". A proxy of the operations
     * interface, behind the generated tie class.
     */
    static AMI_EchoHandlerOperations handler() {
        return (AMI_EchoHandlerOperations) Proxy.newProxyInstance(
                AmiProbe.class.getClassLoader(),
                new Class<?>[] {AMI_EchoHandlerOperations.class},
                (proxy, method, args) -> {
                    String op = method.getName();
                    if (method.getDeclaringClass() == Object.class) {
                        switch (op) {
                            case "hashCode": return System.identityHashCode(proxy);
                            case "equals": return proxy == args[0];
                            default: return "AmiProbe.handler";
                        }
                    }
                    Object[] a = args == null ? new Object[0] : args;
                    switch (op) {
                        case "echo_long":
                        case "echo_string":
                        case "get_counter":
                        case "get_name":
                            lines.add(op + ": " + a[0]);
                            break;
                        case "swap":
                            lines.add(op + ": " + a[0] + " " + a[1] + " " + a[2]);
                            break;
                        case "divide":
                            lines.add(op + ": " + a[0] + " " + a[1]);
                            break;
                        case "set_counter":
                            lines.add(op);
                            break;
                        case "divide_excep":
                        case "fail_excep":
                        case "fail_system_excep":
                        case "trigger_excep":
                            lines.add(op + ": " + describe((ExceptionHolder) a[0]));
                            break;
                        default:
                            lines.add("unexpected " + op);
                            break;
                    }
                    return null;
                });
    }

    /** Answers after 2 s, longer than the call timeout of the last case. */
    static final class SlowCallback extends CallbackPOA {
        @Override
        public String notify(String message) {
            try {
                Thread.sleep(2000);
            } catch (InterruptedException e) {
                Thread.currentThread().interrupt();
            }
            return "ack:" + message;
        }
    }

    static int status = 0;

    static void show(String what) throws InterruptedException {
        String line = lines.poll(WAIT, TimeUnit.SECONDS);
        if (line == null) {
            line = what + ": no reply";
            status = 1;
        }
        System.out.println(line);
    }

    public static void main(String[] argv) throws Exception {
        if (argv.length < 1) {
            System.err.println("usage: AmiProbe REFERENCE [ORB options]");
            System.exit(2);
        }
        List<String> args = new ArrayList<>(List.of("-ORBendPoint", "giop:tcp:127.0.0.1:"));
        args.addAll(List.of(argv).subList(1, argv.length));
        ORB orb = ORB.init(args.toArray(new String[0]), null);
        POA poa = POAHelper.narrow(orb.resolve_initial_references("RootPOA"));
        poa.the_POAManager().activate();
        _EchoStub echo = (_EchoStub) EchoHelper.narrow(orb.string_to_object(argv[0]));
        AMI_EchoHandler h = AMI_EchoHandlerHelper.narrow(
                poa.servant_to_reference(new AMI_EchoHandlerPOATie(handler())));

        echo.sendc_echo_long(h, 42);
        show("echo_long");
        echo.sendc_echo_string(h, "hello AMI");
        show("echo_string");
        echo.sendc_swap(h, 1, 2);
        show("swap");
        echo.sendc_divide(h, 7, 2);
        show("divide");
        echo.sendc_divide(h, 1, 0);
        show("divide_excep");
        echo.sendc_fail(h, 7, "bad");
        show("fail_excep");
        echo.sendc_fail_system(h);
        show("fail_system_excep");
        echo.sendc_set_counter(h, 5);
        show("set_counter");
        echo.sendc_get_counter(h);
        show("get_counter");
        echo.sendc_get_name(h);
        show("get_name");
        // A nil handler: the request is sent, the reply dropped.
        echo.sendc_set_counter(null, 9);
        long deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(WAIT);
        while (echo.counter() != 9 && System.nanoTime() < deadline) {
            Thread.sleep(10);
        }
        System.out.println("nil handler: counter " + echo.counter());
        // The server calls the slow callback; the call times out first.
        Callback cb = CallbackHelper.narrow(poa.servant_to_reference(new SlowCallback()));
        echo.register_callback(cb);
        Timeouts.setClientCallTimeout(echo, 500);
        echo.sendc_trigger(h, "late");
        Timeouts.setClientCallTimeout(echo, 0);
        show("trigger_excep");
        // Let the server's call of the callback finish before going away.
        Thread.sleep(2000);
        orb.destroy();
        System.exit(status);
    }
}

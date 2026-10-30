// rustorb Java: runs the CosNaming operation sequence of
// impls/omniorbpy/naming_check.py with the CosNaming stubs of the rustorb
// Java runtime, printing the same lines, so that the output can be compared
// with expected/naming-check.txt (run-naming-check.sh).
//
//   NamingCheck --naming corbaloc::naming:2809/NameService [--timeout S]

package interop;

import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;

import org.omg.CORBA.ORB;
import org.omg.CORBA.SystemException;
import org.omg.CORBA.UserException;
import org.omg.CosNaming.Binding;
import org.omg.CosNaming.BindingIterator;
import org.omg.CosNaming.BindingIteratorHolder;
import org.omg.CosNaming.BindingListHolder;
import org.omg.CosNaming.BindingType;
import org.omg.CosNaming.NameComponent;
import org.omg.CosNaming.NamingContext;
import org.omg.CosNaming.NamingContextExt;
import org.omg.CosNaming.NamingContextExtHelper;
import org.omg.CosNaming.NamingContextHelper;
import org.omg.CosNaming.NamingContextPackage.CannotProceed;
import org.omg.CosNaming.NamingContextPackage.NotFound;
import org.omg.CosNaming.NamingContextPackage.NotFoundReason;

public final class NamingCheck {
    private NamingCheck() {}

    interface Step {
        String run() throws Exception;
    }

    static String fmtName(NameComponent[] name) {
        if (name.length == 0) {
            return "<empty>";
        }
        StringBuilder s = new StringBuilder();
        for (NameComponent c : name) {
            if (s.length() > 0) {
                s.append('/');
            }
            s.append(c.id);
            if (!c.kind.isEmpty()) {
                s.append('.').append(c.kind);
            }
        }
        return s.toString();
    }

    static String why(NotFoundReason why) {
        switch (why.value()) {
            case NotFoundReason._missing_node:
                return "missing_node";
            case NotFoundReason._not_context:
                return "not_context";
            default:
                return "not_object";
        }
    }

    static String fmtExc(Exception e) {
        if (e instanceof NotFound) {
            NotFound nf = (NotFound) e;
            return "NotFound(" + why(nf.why) + ", rest=" + fmtName(nf.rest_of_name) + ")";
        }
        if (e instanceof CannotProceed) {
            return "CannotProceed(rest=" + fmtName(((CannotProceed) e).rest_of_name) + ")";
        }
        if (e instanceof SystemException) {
            String[] completed = {"COMPLETED_YES", "COMPLETED_NO", "COMPLETED_MAYBE"};
            return e.getClass().getSimpleName() + "(completed="
                    + completed[((SystemException) e).completed.value()] + ")";
        }
        return e.getClass().getSimpleName();
    }

    /** Python's repr of a string, for the plain strings used here. */
    static String pyRepr(String s) {
        return "'" + s.replace("\\", "\\\\").replace("'", "\\'") + "'";
    }

    static String fmtComponents(NameComponent[] name) {
        List<String> parts = new ArrayList<>();
        for (NameComponent c : name) {
            parts.add("(" + pyRepr(c.id) + ", " + pyRepr(c.kind) + ")");
        }
        return "[" + String.join(", ", parts) + "]";
    }

    static String fmtBindings(List<Binding> bl) {
        List<String> items = new ArrayList<>();
        for (Binding b : bl) {
            String type = b.binding_type.value() == BindingType._nobject ? "nobject" : "ncontext";
            // Sorted as the (name, type) tuples of the omniORBpy check.
            items.add(fmtName(b.binding_name) + "\u0000" + type);
        }
        items.sort(null);
        List<String> out = new ArrayList<>();
        for (String i : items) {
            out.add(i.replace('\u0000', ':'));
        }
        return "[" + String.join(", ", out) + "]";
    }

    /** path as a name: components split at '/', id and kind at the first '.'. */
    static NameComponent[] n(String path) {
        List<NameComponent> name = new ArrayList<>();
        for (String c : path.split("/")) {
            if (c.isEmpty()) {
                continue;
            }
            int dot = c.indexOf('.');
            name.add(dot < 0 ? new NameComponent(c, "")
                    : new NameComponent(c.substring(0, dot), c.substring(dot + 1)));
        }
        return name.toArray(new NameComponent[0]);
    }

    /** Runs f and prints its line; false if it raised. */
    static boolean step(String label, Step f) throws Exception {
        String out;
        boolean ok = true;
        try {
            out = f.run();
        } catch (UserException | SystemException e) {
            out = "raises " + fmtExc(e);
            ok = false;
        }
        System.out.println(String.format("%-40s -> %s", label, out));
        return ok;
    }

    public static void main(String[] argv) throws Exception {
        Common.Options opt = new Common.Options();
        for (int i = 0; i < argv.length; ++i) {
            if (argv[i].equals("--naming") && i + 1 < argv.length) {
                opt.naming = argv[++i];
            } else if (argv[i].equals("--timeout") && i + 1 < argv.length) {
                opt.timeout = Double.parseDouble(argv[++i]);
            } else {
                opt.orbArgs.add(argv[i]);
            }
        }
        if (opt.naming == null) {
            System.err.println("usage: NamingCheck --naming CORBALOC [--timeout S]");
            System.exit(2);
        }
        ORB orb = Common.orbInit(opt, "naming-check");
        NamingContext root = Common.rootContext(orb, opt.timeout);
        NamingContextExt ext = NamingContextExtHelper.narrow(root);

        NamingContext[] t = new NamingContext[1];
        if (!step("bind_new_context test", () -> {
            t[0] = root.bind_new_context(n("test"));
            return "ok";
        })) {
            System.exit(1);
        }
        // Any object reference will do. The test context is used rather than
        // the root (as the omniORBpy check does) because its IOR comes from
        // the naming service, while the root's was made from the corbaloc
        // URL, and _is_equivalent compares IORs (as the Rust check).
        org.omg.CORBA.Object obj = t[0];
        NamingContext test = t[0];

        step("bind test/a", () -> { root.bind(n("test/a"), obj); return "ok"; });
        step("bind test/a (again)", () -> { root.bind(n("test/a"), obj); return "ok"; });
        step("rebind test/a", () -> { root.rebind(n("test/a"), obj); return "ok"; });
        step("resolve test/a", () -> equivalent(root.resolve(n("test/a")), obj));
        step("bind test/b.kind", () -> { test.bind(n("b.kind"), obj); return "ok"; });
        step("resolve test/b (no kind)", () -> { root.resolve(n("test/b")); return "ok"; });
        step("resolve test/b.kind", () -> equivalent(root.resolve(n("test/b.kind")), obj));
        step("resolve test/missing", () -> { root.resolve(n("test/missing")); return "ok"; });
        step("resolve test/missing/deeper",
                () -> { root.resolve(n("test/missing/deeper")); return "ok"; });
        step("resolve test/a/x (not a context)", () -> { root.resolve(n("test/a/x")); return "ok"; });
        step("resolve <empty>", () -> { root.resolve(new NameComponent[0]); return "ok"; });
        step("bind <empty>", () -> { root.bind(new NameComponent[0], obj); return "ok"; });

        NamingContext[] sub = new NamingContext[1];
        NamingContext[] loose = new NamingContext[1];
        step("bind_new_context test/sub",
                () -> { sub[0] = root.bind_new_context(n("test/sub")); return "ok"; });
        step("bind_new_context test/sub (again)",
                () -> { root.bind_new_context(n("test/sub")); return "ok"; });
        step("bind test/sub/c", () -> { root.bind(n("test/sub/c"), obj); return "ok"; });
        step("resolve test/sub (is a context)", () -> isContext(root.resolve(n("test/sub"))));
        step("new_context", () -> { loose[0] = root.new_context(); return "ok"; });
        step("bind_context test/loose",
                () -> { root.bind_context(n("test/loose"), loose[0]); return "ok"; });
        step("rebind_context test/loose",
                () -> { root.rebind_context(n("test/loose"), loose[0]); return "ok"; });

        step("list test (how_many=10)", () -> {
            BindingListHolder bl = new BindingListHolder();
            BindingIteratorHolder it = new BindingIteratorHolder();
            test.list(10, bl, it);
            return fmtBindings(Arrays.asList(bl.value)) + (it.value != null ? " + iterator" : "");
        });
        step("list test (how_many=1) + iterator", () -> {
            BindingListHolder bl = new BindingListHolder();
            BindingIteratorHolder ith = new BindingIteratorHolder();
            test.list(1, bl, ith);
            List<Binding> names = new ArrayList<>(Arrays.asList(bl.value));
            StringBuilder steps = new StringBuilder("first=" + bl.value.length);
            BindingIterator it = ith.value;
            while (it != null) {
                BindingListHolder chunk = new BindingListHolder();
                boolean more = it.next_n(2, chunk);
                steps.append(" next_n(2)=").append(more ? "True" : "False").append('/')
                        .append(chunk.value.length);
                names.addAll(Arrays.asList(chunk.value));
                if (!more) {
                    it.destroy();
                    break;
                }
            }
            return fmtBindings(names) + " " + steps;
        });

        step("destroy test/sub (not empty)", () -> { sub[0].destroy(); return "ok"; });
        step("unbind test/sub/c", () -> { root.unbind(n("test/sub/c")); return "ok"; });
        step("destroy test/sub", () -> { sub[0].destroy(); return "ok"; });
        step("resolve test/sub (dangling)",
                () -> { root.resolve(n("test/sub")); return "still bound"; });
        step("unbind test/sub", () -> { root.unbind(n("test/sub")); return "ok"; });
        step("unbind test/a", () -> { root.unbind(n("test/a")); return "ok"; });
        step("unbind test/a (again)", () -> { root.unbind(n("test/a")); return "ok"; });
        step("resolve test/a", () -> { root.resolve(n("test/a")); return "ok"; });

        if (ext != null) {
            step("to_string [a.b, c/d, .e]", () -> ext.to_string(new NameComponent[] {
                new NameComponent("a", "b"), new NameComponent("c/d", ""), new NameComponent("", "e")}));
            step("to_name a.b/c\\/d/.e", () -> fmtComponents(ext.to_name("a.b/c\\/d/.e")));
            step("to_url host:2809 a/b", () -> ext.to_url("host:2809", "a/b"));
            step("to_url :host:2809 a/b c", () -> ext.to_url(":host:2809", "a/b c"));
            step("resolve_str test/b.kind", () -> equivalent(ext.resolve_str("test/b.kind"), obj));
            step("to_name <invalid>", () -> fmtComponents(ext.to_name("a//b")));
        } else {
            System.out.println("root context is not a NamingContextExt");
        }

        // corbaname: URLs, resolved by the ORB through the naming service.
        String base = opt.naming.startsWith("corbaloc:")
                ? "corbaname:" + opt.naming.substring("corbaloc:".length()) : opt.naming;
        step("corbaname #test/b.kind",
                () -> equivalent(orb.string_to_object(base + "#test/b.kind"), obj));
        step("corbaname #test%2Fb%2ekind",
                () -> equivalent(orb.string_to_object(base + "#test%2Fb%2ekind"), obj));
        step("corbaname rir: #test/b.kind",
                () -> equivalent(orb.string_to_object("corbaname:rir:#test/b.kind"), obj));
        step("corbaname (no name)", () -> isContext(orb.string_to_object(base)));
        step("corbaname #test/loose", () -> isContext(orb.string_to_object(base + "#test/loose")));
        step("corbaname #test/missing",
                () -> { orb.string_to_object(base + "#test/missing"); return "ok"; });
        step("corbaname #test//b (invalid)",
                () -> { orb.string_to_object(base + "#test//b"); return "ok"; });

        step("list test (final)", () -> {
            BindingListHolder bl = new BindingListHolder();
            BindingIteratorHolder it = new BindingIteratorHolder();
            test.list(100, bl, it);
            return fmtBindings(Arrays.asList(bl.value));
        });
        System.out.flush();
        orb.destroy();
    }

    static String equivalent(org.omg.CORBA.Object o, org.omg.CORBA.Object obj) {
        return o != null && o._is_equivalent(obj) ? "equivalent" : "different object";
    }

    static String isContext(org.omg.CORBA.Object o) {
        try {
            return NamingContextHelper.narrow(o) != null ? "NamingContext" : "not a context";
        } catch (org.omg.CORBA.BAD_PARAM e) {
            return "not a context";
        }
    }
}

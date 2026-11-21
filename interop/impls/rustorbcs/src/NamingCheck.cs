// rustorb .NET: runs the CosNaming operation sequence of
// impls/omniorbpy/naming_check.py with the CosNaming stubs of the rustorb
// .NET runtime, printing the same lines, so that the output can be compared
// with expected/naming-check.txt (run-naming-check.sh).
//
//   rustorbcs naming-check --naming corbaloc::naming:2809/NameService [--timeout S]
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

using System;
using System.Collections.Generic;
using System.Globalization;
using System.Linq;
using CosNaming;
using Omg.Types;

namespace Interop
{
    public static class NamingCheck
    {
        private static string FmtName(IEnumerable<NameComponent> name)
        {
            var parts = name.Select(c => c.kind.Length > 0 ? c.id + "." + c.kind : c.id).ToList();
            return parts.Count == 0 ? "<empty>" : string.Join("/", parts);
        }

        private static string Why(NamingContext.NotFoundReason why) => why switch
        {
            NamingContext.NotFoundReason.missing_node => "missing_node",
            NamingContext.NotFoundReason.not_context => "not_context",
            _ => "not_object",
        };

        private static string FmtExc(Exception e) => e switch
        {
            NamingContext.NotFound nf => "NotFound(" + Why(nf.why) + ", rest=" + FmtName(nf.rest_of_name) + ")",
            NamingContext.CannotProceed cp => "CannotProceed(rest=" + FmtName(cp.rest_of_name) + ")",
            Corba.SystemException s => s.GetType().Name + "(completed=" + s.completed + ")",
            _ => e.GetType().Name,
        };

        /// <summary>Python's repr of a string, for the plain strings used here.</summary>
        private static string PyRepr(string s) => "'" + s.Replace("\\", "\\\\").Replace("'", "\\'") + "'";

        private static string FmtComponents(IEnumerable<NameComponent> name) =>
            "[" + string.Join(", ", name.Select(c => "(" + PyRepr(c.id) + ", " + PyRepr(c.kind) + ")")) + "]";

        private static string FmtBindings(IEnumerable<Binding> bl)
        {
            // Sorted as the (name, type) tuples of the omniORBpy check.
            var items = bl.Select(b => FmtName(b.binding_name) + "\u0000"
                + (b.binding_type == BindingType.nobject ? "nobject" : "ncontext")).ToList();
            items.Sort(StringComparer.Ordinal);
            return "[" + string.Join(", ", items.Select(i => i.Replace('\u0000', ':'))) + "]";
        }

        /// <summary>path as a name: components split at '/', id and kind at
        /// the first '.'.</summary>
        private static ISequence<NameComponent> N(string path)
        {
            var name = new Sequence<NameComponent>();
            foreach (string c in path.Split('/'))
            {
                if (c.Length == 0)
                {
                    continue;
                }
                int dot = c.IndexOf('.');
                name.Add(dot < 0 ? new NameComponent(c, "") : new NameComponent(c.Substring(0, dot), c.Substring(dot + 1)));
            }
            return name;
        }

        /// <summary>Runs f and prints its line; false if it raised.</summary>
        private static bool Step(string label, Func<string> f)
        {
            string output;
            bool ok = true;
            try
            {
                output = f();
            }
            catch (Exception e) when (e is Corba.UserException || e is Corba.SystemException)
            {
                output = "raises " + FmtExc(e);
                ok = false;
            }
            Console.WriteLine(label.PadRight(40) + " -> " + output);
            return ok;
        }

        private static string Equivalent(Corba.IObject o, Corba.IObject obj) =>
            o != null && o._is_equivalent(obj) ? "equivalent" : "different object";

        private static string IsContext(Corba.IObject o)
        {
            try
            {
                return NamingContextHelper.narrow(o) != null ? "NamingContext" : "not a context";
            }
            catch (Corba.BAD_PARAM)
            {
                return "not a context";
            }
        }

        public static int Run(string[] argv)
        {
            var opt = new Options();
            for (int i = 0; i < argv.Length; ++i)
            {
                if (argv[i] == "--naming" && i + 1 < argv.Length)
                {
                    opt.Naming = argv[++i];
                }
                else if (argv[i] == "--timeout" && i + 1 < argv.Length)
                {
                    opt.Timeout = double.Parse(argv[++i], CultureInfo.InvariantCulture);
                }
                else
                {
                    opt.OrbArgs.Add(argv[i]);
                }
            }
            if (opt.Naming == null)
            {
                Console.Error.WriteLine("usage: rustorbcs naming-check --naming CORBALOC [--timeout S]");
                return 2;
            }
            Corba.ORB orb = Common.OrbInit(opt);
            INamingContext root = Common.RootContext(orb, opt.Timeout);
            INamingContextExt ext = NamingContextExtHelper.narrow(root);

            INamingContext test = null;
            if (!Step("bind_new_context test", () =>
            {
                test = root.bind_new_context(N("test"));
                return "ok";
            }))
            {
                return 1;
            }
            // Any object reference will do. The test context is used rather
            // than the root (as the omniORBpy check does) because its IOR
            // comes from the naming service, while the root's was made from
            // the corbaloc URL, and _is_equivalent compares IORs.
            Corba.IObject obj = test;

            Step("bind test/a", () => { root.bind(N("test/a"), obj); return "ok"; });
            Step("bind test/a (again)", () => { root.bind(N("test/a"), obj); return "ok"; });
            Step("rebind test/a", () => { root.rebind(N("test/a"), obj); return "ok"; });
            Step("resolve test/a", () => Equivalent(root.resolve(N("test/a")), obj));
            Step("bind test/b.kind", () => { test.bind(N("b.kind"), obj); return "ok"; });
            Step("resolve test/b (no kind)", () => { root.resolve(N("test/b")); return "ok"; });
            Step("resolve test/b.kind", () => Equivalent(root.resolve(N("test/b.kind")), obj));
            Step("resolve test/missing", () => { root.resolve(N("test/missing")); return "ok"; });
            Step("resolve test/missing/deeper", () => { root.resolve(N("test/missing/deeper")); return "ok"; });
            Step("resolve test/a/x (not a context)", () => { root.resolve(N("test/a/x")); return "ok"; });
            Step("resolve <empty>", () => { root.resolve(new Sequence<NameComponent>()); return "ok"; });
            Step("bind <empty>", () => { root.bind(new Sequence<NameComponent>(), obj); return "ok"; });

            INamingContext sub = null, loose = null;
            Step("bind_new_context test/sub", () => { sub = root.bind_new_context(N("test/sub")); return "ok"; });
            Step("bind_new_context test/sub (again)", () => { root.bind_new_context(N("test/sub")); return "ok"; });
            Step("bind test/sub/c", () => { root.bind(N("test/sub/c"), obj); return "ok"; });
            Step("resolve test/sub (is a context)", () => IsContext(root.resolve(N("test/sub"))));
            Step("new_context", () => { loose = root.new_context(); return "ok"; });
            Step("bind_context test/loose", () => { root.bind_context(N("test/loose"), loose); return "ok"; });
            Step("rebind_context test/loose", () => { root.rebind_context(N("test/loose"), loose); return "ok"; });

            Step("list test (how_many=10)", () =>
            {
                test.list(10, out ISequence<Binding> bl, out IBindingIterator it);
                return FmtBindings(bl) + (it != null ? " + iterator" : "");
            });
            Step("list test (how_many=1) + iterator", () =>
            {
                test.list(1, out ISequence<Binding> bl, out IBindingIterator it);
                var names = new List<Binding>(bl);
                string steps = "first=" + bl.Count;
                while (it != null)
                {
                    bool more = it.next_n(2, out ISequence<Binding> chunk);
                    steps += " next_n(2)=" + (more ? "True" : "False") + "/" + chunk.Count;
                    names.AddRange(chunk);
                    if (!more)
                    {
                        it.destroy();
                        break;
                    }
                }
                return FmtBindings(names) + " " + steps;
            });

            Step("destroy test/sub (not empty)", () => { sub.destroy(); return "ok"; });
            Step("unbind test/sub/c", () => { root.unbind(N("test/sub/c")); return "ok"; });
            Step("destroy test/sub", () => { sub.destroy(); return "ok"; });
            Step("resolve test/sub (dangling)", () => { root.resolve(N("test/sub")); return "still bound"; });
            Step("unbind test/sub", () => { root.unbind(N("test/sub")); return "ok"; });
            Step("unbind test/a", () => { root.unbind(N("test/a")); return "ok"; });
            Step("unbind test/a (again)", () => { root.unbind(N("test/a")); return "ok"; });
            Step("resolve test/a", () => { root.resolve(N("test/a")); return "ok"; });

            if (ext != null)
            {
                Step("to_string [a.b, c/d, .e]", () => ext.to_string(new Sequence<NameComponent>
                {
                    new NameComponent("a", "b"), new NameComponent("c/d", ""), new NameComponent("", "e"),
                }));
                Step("to_name a.b/c\\/d/.e", () => FmtComponents(ext.to_name("a.b/c\\/d/.e")));
                Step("to_url host:2809 a/b", () => ext.to_url("host:2809", "a/b"));
                Step("to_url :host:2809 a/b c", () => ext.to_url(":host:2809", "a/b c"));
                Step("resolve_str test/b.kind", () => Equivalent(ext.resolve_str("test/b.kind"), obj));
                Step("to_name <invalid>", () => FmtComponents(ext.to_name("a//b")));
            }
            else
            {
                Console.WriteLine("root context is not a NamingContextExt");
            }

            // corbaname: URLs, resolved by the ORB through the naming service.
            string b = opt.Naming.StartsWith("corbaloc:", StringComparison.Ordinal)
                ? "corbaname:" + opt.Naming.Substring("corbaloc:".Length) : opt.Naming;
            Step("corbaname #test/b.kind", () => Equivalent(orb.string_to_object(b + "#test/b.kind"), obj));
            Step("corbaname #test%2Fb%2ekind", () => Equivalent(orb.string_to_object(b + "#test%2Fb%2ekind"), obj));
            Step("corbaname rir: #test/b.kind", () => Equivalent(orb.string_to_object("corbaname:rir:#test/b.kind"), obj));
            Step("corbaname (no name)", () => IsContext(orb.string_to_object(b)));
            Step("corbaname #test/loose", () => IsContext(orb.string_to_object(b + "#test/loose")));
            Step("corbaname #test/missing", () => { orb.string_to_object(b + "#test/missing"); return "ok"; });
            Step("corbaname #test//b (invalid)", () => { orb.string_to_object(b + "#test//b"); return "ok"; });

            Step("list test (final)", () =>
            {
                test.list(100, out ISequence<Binding> bl, out _);
                return FmtBindings(bl);
            });
            Console.Out.Flush();
            orb.destroy();
            return 0;
        }
    }
}

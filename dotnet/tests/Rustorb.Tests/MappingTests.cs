// The constructs of cpp/tests/idl/CxxMapping.idl that InteropTest.idl does
// not use, through rustorb-idl -bcsharp stubs and skeletons over GIOP.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

using System;
using System.Collections.Generic;
using System.Linq;
using CxxMapping;
using Omg.Types;
using Xunit;

namespace Rustorb.Tests
{
    public sealed class LaterImpl : LaterPOA
    {
        public override ILater next() => LaterHelper.narrow(_this_object());
    }

    public sealed class TypesImpl : TypesPOA
    {
        public override Tree echo_tree(Tree t) => t;
        public override Node echo_node(Node n) => n;
        public override ByBool echo_by_bool(ByBool u) => u;
        public override ByChar echo_by_char(ByChar u) => u;
        public override ByEnum echo_by_enum(ByEnum u) => u;
        public override ByULongLong echo_by_ulonglong(ByULongLong u) => u;
        public override WithArray echo_with_array(WithArray u) => u;
        public override Holder echo_holder(Holder h) => h;
        public override Outer echo_outer(Outer o) => o;
        public override Keywords echo_keywords(Keywords k) => k;
        public override GlobalPoint echo_global(GlobalPoint p) => p;
        public override string echo_short_string(string s) => s;

        public override decimal echo_money(decimal m, ref decimal total, out decimal whole)
        {
            total += m;
            whole = Math.Truncate(m);
            return m;
        }

        public override Price echo_price(Price p) => p;

        public override decimal echo_quad(decimal q, ref decimal twice, out decimal negated)
        {
            twice *= 2;
            negated = -q;
            return q;
        }

        public override Measure echo_measure(Measure m) => m;

        public override void fail(int code)
        {
            if (code == 0)
            {
                throw new Empty();
            }
            throw new Failure(code, "failed", Fruit.CHERRY);
        }

        public override ISequence<ILater> laters(int n)
        {
            var s = new Sequence<ILater>();
            for (int i = 0; i < n; i++)
            {
                s.Add(LaterHelper.narrow(_default_POA().servant_to_reference(new LaterImpl())));
            }
            return s;
        }

        public string deleted;
        public override void delete(string what) => deleted = what;
    }

    public sealed class MultiImpl : MultiPOA
    {
        public override int id => 5;
        public override string label { get; set; } = "multi";

        public override void base_op(bool fail)
        {
            if (fail)
            {
                throw new Base.BaseError("asked to");
            }
        }

        public override int derived_op(int a, ref int b, out int c)
        {
            c = a * b;
            b = a + b;
            return a;
        }

        public override void notify(string msg)
        {
        }

        public override IBase as_base() => BaseHelper.narrow(_this_object());
        public override string other_op() => "other";
        public override Base.BaseInfo multi_op(Base.BaseInfo info) => new Base.BaseInfo(info.id + 1);
    }

    public sealed class MappingFixture : IDisposable
    {
        public Corba.ORB Orb { get; }
        public TypesImpl TypesServant { get; } = new TypesImpl();
        public ITypes Types { get; }
        public IMulti Multi { get; }

        public MappingFixture()
        {
            Orb = Corba.ORB.init(new string[0]);
            var poa = PortableServer.POAHelper.narrow(Orb.resolve_initial_references("RootPOA"));
            poa.the_POAManager.activate();
            Types = TypesHelper.narrow(Orb.string_to_object(
                Orb.object_to_string(poa.servant_to_reference(TypesServant))));
            Multi = MultiHelper.narrow(Orb.string_to_object(
                Orb.object_to_string(poa.servant_to_reference(new MultiImpl()))));
        }

        public void Dispose()
        {
            Orb.shutdown(true);
            Orb.destroy();
        }
    }

    public class MappingTests : IClassFixture<MappingFixture>
    {
        private readonly MappingFixture f;
        private ITypes T => f.Types;

        public MappingTests(MappingFixture fixture)
        {
            f = fixture;
        }

        [Fact]
        public void Constants_()
        {
            Assert.Equal(10, global::Constants.GLOBAL_LIMIT);
            Assert.Equal((short)-3, CxxMapping.Constants.C_SHORT);
            Assert.Equal(18000000000000000000UL, CxxMapping.Constants.C_ULONGLONG);
            Assert.Equal(0.25, CxxMapping.Constants.C_DOUBLE);
            Assert.Equal("abcd", CxxMapping.Constants.C_STRING);
            Assert.Equal(2097149, CxxMapping.Constants.C_EXPR);
            Assert.Equal(Fruit.BANANA, CxxMapping.Constants.C_ENUM);
            Assert.Equal(12.50m, CxxMapping.Constants.C_FIXED);
            Assert.Equal(-1.5m, CxxMapping.Constants.C_MONEY);
            Assert.Equal(1, CxxMapping.Constants.REOPENED);
            Assert.Equal(7, Base.BASE_CONST);
        }

        [Fact]
        public void Recursive()
        {
            var t = new Tree("root", new Sequence<Tree> { new Tree("a", new Sequence<Tree>()), new Tree() });
            t.children[1].children.Add(new Tree("deep", new Sequence<Tree>()));
            Assert.Equal(t, T.echo_tree(t));

            var leaf = new Node();
            leaf.leaf = 3;
            var n = new Node();
            n.Setbranch(new[] { leaf, new Node() });
            Node n2 = T.echo_node(n);
            Assert.Equal(n, n2);
            Assert.Equal(3, n2.branch[0].leaf);
            var d = new Node();
            d._default((short)9);
            Assert.Equal((short)9, T.echo_node(d).Discriminator);
            Assert.Equal(Corba.TCKind.tk_sequence, NodeHelper.type().member_type(1).kind());
        }

        [Fact]
        public void Unions()
        {
            var b = new ByBool();
            b.no = "nein";
            Assert.Equal("nein", T.echo_by_bool(b).no);
            Assert.False(T.echo_by_bool(b).Discriminator);
            var c = new ByChar();
            c.Setbc("see", 'c');
            Assert.Equal('c', T.echo_by_char(c).Discriminator);
            c._default('z');
            Assert.Equal('z', T.echo_by_char(c).Discriminator);
            var e = new ByEnum();
            e.Setother(true, Fruit.BANANA);
            Assert.Equal(e, T.echo_by_enum(e));
            var u = new ByULongLong();
            u.seven = 7.5;
            Assert.Equal(7.5, T.echo_by_ulonglong(u).seven);
            u._default();
            Assert.NotEqual(7UL, T.echo_by_ulonglong(u).Discriminator);
            var w = new WithArray();
            w.cells = new[] { new short[] { 1, 2, 3 }, new short[] { 4, 5, 6 } };
            Assert.Equal(w, T.echo_with_array(w));
            w.two = new Pair(1, 2);
            Assert.Equal(new Pair(1, 2), T.echo_with_array(w).two);
        }

        [Fact]
        public void Holder_()
        {
            var h = new Holder();
            h.numbers = new[] { 1, 2, 3 };
            h.pairs.Add(new Pair(4, 5));
            h.tag = "eight ch";
            h.bounded.Add(1);
            h.wide = "wïde";
            h.flags.Add(true);
            h.wchars.Add('é');
            h.nested.Add(new Sequence<string> { "x", "y" });
            h.target = T;
            h.choice.apple_count = 2;
            Holder h2 = T.echo_holder(h);
            Assert.Equal(h, h2);
            Assert.True(h2.target._is_equivalent(T));

            h.tag = "nine char";
            Assert.Throws<Corba.BAD_PARAM>(() => T.echo_holder(h));
            Assert.Throws<Corba.BAD_PARAM>(() => T.echo_short_string("too long, sure"));
            h.tag = "";
            for (int i = 0; i < 5; i++)
            {
                h.bounded.Add(i);
            }
            Assert.Throws<Corba.BAD_PARAM>(() => T.echo_holder(h));
        }

        [Fact]
        public void Nested()
        {
            var o = new Outer(new Outer.Inner(4), Outer.Mode.OFF);
            Assert.Equal(o, T.echo_outer(o));
            var k = new Keywords(1, "ns");
            Keywords k2 = T.echo_keywords(k);
            Assert.Equal(1, k2.@class);
            Assert.Equal("ns", k2.@namespace);
            Assert.Equal(new GlobalPoint(1, -1), T.echo_global(new GlobalPoint(1, -1)));
            T.delete("it");
            Assert.Equal("it", f.TypesServant.deleted);
        }

        [Fact]
        public void FixedAndLongDouble()
        {
            decimal total = 1.25m;
            Assert.Equal(123.45m, T.echo_money(123.45m, ref total, out decimal whole));
            Assert.Equal(124.70m, total);
            Assert.Equal(123m, whole);
            Assert.Throws<Corba.BAD_PARAM>(() => T.echo_money(1.234m, ref total, out _));
            Assert.Throws<Corba.BAD_PARAM>(() => T.echo_money(1234m, ref total, out _));

            var p = new Price(-9.99m, 0.0000000000000000000000000001m, new Sequence<decimal> { 1m, 2.5m });
            Assert.Equal(p, T.echo_price(p));

            decimal twice = 1.5m;
            Assert.Equal(2.5m, T.echo_quad(2.5m, ref twice, out decimal neg));
            Assert.Equal(3m, twice);
            Assert.Equal(-2.5m, neg);
            var m = new Measure(1e20m, new Sequence<decimal> { 0.1m, -7m });
            Assert.Equal(m, T.echo_measure(m));
        }

        [Fact]
        public void Exceptions()
        {
            Assert.Throws<Empty>(() => T.fail(0));
            var e = Assert.Throws<Failure>(() => T.fail(3));
            Assert.Equal(3, e.code);
            Assert.Equal(Fruit.CHERRY, e.kind);
            Assert.Throws<Base.BaseError>(() => f.Multi.base_op(true));
        }

        [Fact]
        public void Inheritance()
        {
            IMulti m = f.Multi;
            Assert.Equal(5, m.id);
            m.label = "changed";
            Assert.Equal("changed", m.label);
            int b = 3;
            Assert.Equal(2, m.derived_op(2, ref b, out int c));
            Assert.Equal(5, b);
            Assert.Equal(6, c);
            Assert.Equal("other", m.other_op());
            Assert.Equal(8, m.multi_op(new Base.BaseInfo(7)).id);
            m.notify("hi");
            IBase asBase = m.as_base();
            Assert.True(asBase._is_a(DerivedHelper.id()));
            IDerived derived = DerivedHelper.narrow(asBase);
            Assert.Equal(5, derived.id);
            IOther other = OtherHelper.narrow(asBase);
            Assert.Equal("other", other.other_op());
        }

        [Fact]
        public void ObjectSequences()
        {
            ISequence<ILater> ls = T.laters(3);
            Assert.Equal(3, ls.Count);
            Assert.True(ls[1].next()._is_equivalent(ls[1]));
            Assert.False(ls[0]._is_equivalent(ls[1]));
        }

        [Fact]
        public void TypeCodes()
        {
            Corba.TypeCode tc = HolderHelper.type();
            Assert.Equal("IDL:rustorb.org/CxxMapping/Holder:1.0", tc.id());
            Assert.Equal(11u, tc.member_count());
            Assert.Equal("tag", tc.member_name(3));
            Corba.TypeCode tag = tc.member_type(3);
            Assert.Equal(Corba.TCKind.tk_alias, tag.kind());
            Assert.Equal(8u, tag.content_type().length());
            Corba.TypeCode money = MoneyHelper.type().unaliased();
            Assert.Equal((ushort)5, money.fixed_digits());
            Assert.Equal((short)2, money.fixed_scale());
            Assert.True(ByEnumHelper.type().default_index() == 2);

            var a = new Corba.Any();
            var h = new Holder();
            h.choice.cherry_name = "kirsche";
            HolderHelper.insert(a, h);
            Holder back = HolderHelper.extract(a);
            Assert.Equal(h, back);
            var a2 = a.copy();
            Assert.Equal(a, a2);
            a.insert_fixed(12.30m, MoneyHelper.type());
            Assert.Equal(12.30m, a.extract_fixed());
            a.insert_longdouble(0.5m);
            Assert.Equal(0.5m, a.extract_longdouble());
        }
    }
}

// Valuetypes, value boxes, custom valuetypes and abstract interfaces through
// the ORB in one process, with the code rustorb-idl -bcsharp generates from
// interop/idl/ValueTest.idl, CustomTest.idl and AbstractTest.idl
// (interop/run-matrix.sh --suite value|custom|abstract covers the other
// ORBs).
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

using System;
using System.Collections.Generic;
using System.Linq;
using AbstractTest;
using CustomTest;
using DynamicAny;
using Omg.Types;
using ValueTest;
using Xunit;
using Point = ValueTest.Point;

namespace Rustorb.Tests
{
    public sealed class TallyImpl : Tally
    {
        public override int count() => n;
    }

    public sealed class AccountImpl : Account
    {
        public override int doubled() => balance * 2;
        public override void init(int balance) => this.balance = balance;
    }

    public sealed class BlobImpl : Blob
    {
        public override void marshal(Corba.DataOutputStream os)
        {
            os.write_long(n);
            os.write_string(label);
            os.write_Value(pt);
            os.write_ulong((uint)values.Count);
            os.write_long_array(values, 0, (uint)values.Count);
        }

        public override void unmarshal(Corba.DataInputStream @is)
        {
            n = @is.read_long();
            label = @is.read_string();
            pt = (Point)@is.read_Value();
            uint len = @is.read_ulong();
            var v = new Sequence<int>();
            @is.read_long_array(v, 0, len);
            values = v;
        }
    }

    public sealed class CardImpl : Card
    {
        public override string name() => title;
    }

    public sealed class PersonImpl : PersonPOA
    {
        public override string name() => "alice";
        public override int age() => 30;
    }

    /// <summary>A value of a type nobody has a factory of.</summary>
    public sealed class Stranger : OBV_ValueTest.Point
    {
        private static readonly string[] ids = { "IDL:Nobody/Stranger:1.0" };

        public override string[] _truncatable_ids() => ids;
    }

    public sealed class ValueEchoImpl : ValueEchoPOA
    {
        public override Point echo_point(Point p) => p;
        public override Pair echo_pair(Pair p) => p;
        public override ValueTest.Node echo_node(ValueTest.Node n) => n;
        public override Shape echo_shape(Shape s) => s;
        public override ISequence<Point> echo_points(ISequence<Point> s) => s;
        public override bool same(Point a, Point b) => ReferenceEquals(a, b);
        public override int count_nodes(ValueTest.Node n) => 0;
        public override LongBox echo_long_box(LongBox b) => b;
        public override StringBox echo_string_box(StringBox b) => b;
        public override PairBox echo_pair_box(PairBox b) => b;
        public override Corba.Any echo_any(Corba.Any a) => a;
        public override Tally echo_tally(Tally t) => t;
        public override int call_count(ICounter c) => c.count();
        public override int account_doubled(Account a) => a.doubled();
        public override Account make_account(int balance) => null;
        public override void shutdown() { }
    }

    public sealed class CustomEchoImpl : CustomEchoPOA
    {
        public override Blob echo_blob(Blob b) => b;
        public override BlobPair echo_blob_pair(BlobPair p) => p;
        public override ISequence<Blob> echo_blobs(ISequence<Blob> s) => s;
        public override void shutdown() { }
    }

    public sealed class AbstractEchoImpl : AbstractEchoPOA
    {
        public override INamed echo_named(INamed n) => n;
        public override string name_of(INamed n) => n.name();
        public override bool is_object(INamed n) => n is Corba.IObject;
        public override NamedPair echo_pair(NamedPair p) => p;
        public override ISequence<INamed> echo_seq(ISequence<INamed> s) => s;
        public override IPerson make_person(string name) => null;
        public override Corba.Any echo_any(Corba.Any a) => a;
        public override void shutdown() { }
    }

    public sealed class ValueFixture : IDisposable
    {
        public Corba.ORB Orb { get; }
        public IValueEcho Values { get; }
        public ICustomEcho Custom { get; }
        public IAbstractEcho Abstract { get; }

        private T Remote<T>(PortableServer.POA poa, PortableServer.Servant s, Func<Corba.IObject, T> narrow) =>
            narrow(Orb.string_to_object(Orb.object_to_string(poa.servant_to_reference(s))));

        public ValueFixture()
        {
            Orb = Corba.ORB.init(new string[0]);
            Orb.register_value_factory(TallyHelper.id(), new Corba.ValueFactory(() => new TallyImpl()));
            Orb.register_value_factory(AccountHelper.id(), new Corba.ValueFactory(() => new AccountImpl()));
            Orb.register_value_factory(BlobHelper.id(), new Corba.ValueFactory(() => new BlobImpl()));
            Orb.register_value_factory(CardHelper.id(), new Corba.ValueFactory(() => new CardImpl()));
            var poa = PortableServer.POAHelper.narrow(Orb.resolve_initial_references("RootPOA"));
            poa.the_POAManager.activate();
            Values = Remote(poa, new ValueEchoImpl(), ValueEchoHelper.narrow);
            Custom = Remote(poa, new CustomEchoImpl(), CustomEchoHelper.narrow);
            Abstract = Remote(poa, new AbstractEchoImpl(), AbstractEchoHelper.narrow);
        }

        public void Dispose()
        {
            Orb.shutdown(true);
            Orb.destroy();
        }
    }

    public class ValueTests : IClassFixture<ValueFixture>
    {
        private readonly ValueFixture f;

        public ValueTests(ValueFixture fixture)
        {
            f = fixture;
        }

        private static OBV_ValueTest.Point3 Point3() => new OBV_ValueTest.Point3(1, 2, 3, "three");

        [Fact]
        public void PlainAndNull()
        {
            Point got = f.Values.echo_point(new OBV_ValueTest.Point(1, -2));
            Assert.IsType<OBV_ValueTest.Point>(got);
            Assert.Equal((1, -2), (got.x, got.y));
            Assert.Null(f.Values.echo_point(null));
        }

        [Fact]
        public void Truncatable()
        {
            // The default factory of Point3 makes the OBV_ class.
            var got = Assert.IsType<OBV_ValueTest.Point3>(f.Values.echo_point(Point3()));
            Assert.Equal((1, 2, 3), (got.x, got.y, got.z));
        }

        [Fact]
        public void SharingAndCycles()
        {
            var p = new OBV_ValueTest.Point(5, 6);
            Pair pair = f.Values.echo_pair(new Pair(p, p));
            Assert.Same(pair.first, pair.second);
            Assert.False(f.Values.same(p, new OBV_ValueTest.Point(5, 6)));
            Assert.True(f.Values.same(p, p));

            var a = new OBV_ValueTest.Node(1, null);
            a.next = new OBV_ValueTest.Node(2, a);
            ValueTest.Node n = f.Values.echo_node(a);
            Assert.Equal((1, 2), (n.id, n.next.id));
            Assert.Same(n, n.next.next);

            var c = new OBV_ValueTest.Point(0, 0);
            Shape s = f.Values.echo_shape(new OBV_ValueTest.Shape("square", new Sequence<Point> { c, Point3() }, c, 1.5));
            Assert.Same(s.center, s.points[0]);
            Assert.IsType<OBV_ValueTest.Point3>(s.points[1]);
        }

        [Fact]
        public void Boxes()
        {
            Assert.Equal(7, f.Values.echo_long_box(new LongBox(7)).value);
            Assert.Null(f.Values.echo_long_box(null));
            Assert.Equal("boxed", f.Values.echo_string_box(new StringBox("boxed")).value);
            PairBox pb = f.Values.echo_pair_box(new PairBox(new Pair(new OBV_ValueTest.Point(1, 2), null)));
            Assert.Equal(2, pb.value.first.y);
            Assert.Null(pb.value.second);
        }

        [Fact]
        public void Factories()
        {
            var t = new TallyImpl { n = 3 };
            Assert.Equal(3, Assert.IsType<TallyImpl>(f.Values.echo_tally(t)).count());
            var acc = (Account)f.Orb.lookup_value_factory(AccountHelper.id()).create_for_unmarshal();
            acc.init(21);
            Assert.Equal(42, f.Values.account_doubled(acc));
            // A valuetype that supports an interface, as a servant.
            ICounter counter = new CounterPOATie(new TallyImpl { n = 5 })._this();
            Assert.Equal(5, f.Values.call_count(counter));
            // Without a factory, the value cannot be read.
            Assert.Null(f.Orb.lookup_value_factory("IDL:Nobody/Stranger:1.0"));
            Assert.Throws<Corba.MARSHAL>(() => f.Values.echo_point(new Stranger()));
        }

        [Fact]
        public void Anys()
        {
            var a = new Corba.Any();
            PointHelper.insert(a, Point3());
            Corba.Any got = f.Values.echo_any(a);
            Assert.True(got.type.equal(PointHelper.type()));
            Assert.IsType<OBV_ValueTest.Point3>(PointHelper.extract(got));

            // The instance inserted is the one extracted, until it travels.
            var p = new OBV_ValueTest.Point(4, 4);
            var local = new Corba.Any();
            PointHelper.insert(local, p);
            Assert.Same(p, PointHelper.extract(local));
            Assert.Same(p, local.extract_Value());

            var box = new Corba.Any();
            StringBoxHelper.insert(box, new StringBox("in an any"));
            Assert.Equal("in an any", StringBoxHelper.extract(f.Values.echo_any(box)).value);
            Assert.Throws<Corba.BAD_OPERATION>(() => NodeHelper.extract(box));
        }

        [Fact]
        public void DynAny()
        {
            var factory = DynAnyFactoryHelper.narrow(f.Orb.resolve_initial_references("DynAnyFactory"));
            // The first member of a Pair: a Point.
            IDynAny d = factory.create_dyn_any_from_type_code(PairHelper.type());
            var p = new OBV_ValueTest.Point(8, 9);
            d.insert_val(p);
            var back = Assert.IsAssignableFrom<Point>(d.get_val());
            Assert.Equal((8, 9), (back.x, back.y));
            Assert.Equal(9, PairHelper.extract(d.to_any()).first.y);
            Assert.Throws<DynamicAny.DynAny.TypeMismatch>(() => d.insert_val("not a value"));
            d.destroy();
        }

        [Fact]
        public void Custom()
        {
            var p = new OBV_ValueTest.Point(5, 6);
            var b = new BlobImpl { n = 7, label = "seven", pt = p, values = new Sequence<int> { 1, 2, 3 } };
            BlobPair got = f.Custom.echo_blob_pair(new BlobPair(b, p));
            var blob = Assert.IsType<BlobImpl>(got.data);
            Assert.Equal((7, "seven"), (blob.n, blob.label));
            Assert.Equal(new[] { 1, 2, 3 }, blob.values.ToArray());
            Assert.Same(blob.pt, got.pt);
            ISequence<Blob> s = f.Custom.echo_blobs(new Sequence<Blob> { b, null, b });
            Assert.Same(s[0], s[2]);
            Assert.Null(s[1]);
        }

        [Fact]
        public void AbstractInterfaces()
        {
            var card = new CardImpl { title = "ace" };
            Assert.Equal("ace", Assert.IsType<CardImpl>(f.Abstract.echo_named(card)).name());
            Assert.False(f.Abstract.is_object(card));
            Assert.Null(f.Abstract.echo_named(null));
            IPerson alice = new PersonImpl()._this();
            INamed n = f.Abstract.echo_named(alice);
            Assert.IsAssignableFrom<Corba.IObject>(n);
            Assert.Equal("alice", n.name());
            Assert.True(f.Abstract.is_object(alice));
            Assert.Equal("alice", f.Abstract.name_of(alice));
            NamedPair pair = f.Abstract.echo_pair(new NamedPair(card, card));
            Assert.Same(pair.first, pair.second);

            var a = new Corba.Any();
            NamedHelper.insert(a, card);
            Corba.Any got = f.Abstract.echo_any(a);
            Assert.True(got.type.equal(NamedHelper.type()));
            Assert.Equal("ace", NamedHelper.extract(got).name());
            Assert.Throws<Corba.BAD_PARAM>(() => NamedHelper.narrow("not a Named"));
        }
    }
}

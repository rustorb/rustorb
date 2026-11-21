// DynamicAny on the types of interop/idl/AnyTest.idl (the cases of
// interop/run-dynany-probe.sh cover the behaviour in detail).
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

using System.Linq;
using AnyTest;
using DynamicAny;
using Xunit;

namespace Rustorb.Tests
{
    public sealed class DynAnyFixture : System.IDisposable
    {
        public Corba.ORB Orb { get; } = Corba.ORB.init(new string[0]);

        public void Dispose() => Orb.destroy();
    }

    public class DynAnyTests : IClassFixture<DynAnyFixture>
    {
        private readonly Corba.ORB orb;

        public DynAnyTests(DynAnyFixture fixture)
        {
            orb = fixture.Orb;
        }

        private IDynAnyFactory Factory() =>
            DynAnyFactoryHelper.narrow(orb.resolve_initial_references("DynAnyFactory"));

        [Fact]
        public void FactoryIsAnInitialReference()
        {
            IDynAnyFactory f = Factory();
            Assert.True(f._is_a("IDL:omg.org/DynamicAny/DynAnyFactory:1.0"));
            Assert.Contains("DynAnyFactory", orb.list_initial_services());
        }

        [Fact]
        public void BuildsAStruct()
        {
            IDynAny d = Factory().create_dyn_any_from_type_code(PointHelper.type());
            IDynStruct s = DynStructHelper.narrow(d);
            Assert.Equal("x", s.current_member_name());
            d.insert_long(1);
            Assert.True(d.next());
            Assert.Equal(Corba.TCKind.tk_double, s.current_member_kind());
            d.insert_double(2.5);
            Point p = PointHelper.extract(d.to_any());
            Assert.Equal(1, p.x);
            Assert.Equal(2.5, p.y);

            var members = s.get_members();
            Assert.Equal(new[] { "x", "y" }, members.Select(m => m.id));
            members[0].value.insert_long(7);
            s.set_members(members);
            Assert.Equal(7, PointHelper.extract(d.to_any()).x);
        }

        [Fact]
        public void TakesApartAUnion()
        {
            var a = new Corba.Any();
            var c = new Choice();
            c.s = "hello";
            ChoiceHelper.insert(a, c);
            IDynUnion u = DynUnionHelper.narrow(Factory().create_dyn_any(a));
            Assert.Equal(2, u.get_discriminator().get_long());
            Assert.Equal("s", u.member_name());
            Assert.Equal("hello", u.member().get_string());
            Assert.Equal(Corba.TCKind.tk_long, u.discriminator_kind());
        }

        [Fact]
        public void SequencesAndEnums()
        {
            IDynAny d = Factory().create_dyn_any_from_type_code(LongSeqHelper.type());
            d.insert_long_seq(new Omg.Types.Sequence<int>(new[] { 1, 2, 3 }));
            IDynSequence s = DynSequenceHelper.narrow(d);
            Assert.Equal(3u, s.get_length());
            Assert.Equal(new[] { 1, 2, 3 }, d.get_long_seq());
            Assert.Equal(new[] { 1, 2, 3 }, LongSeqHelper.extract(d.to_any()));

            IDynEnum e = DynEnumHelper.narrow(Factory().create_dyn_any_from_type_code(ShadeHelper.type()));
            e.set_as_string("DIM");
            Assert.Equal(2u, e.get_as_ulong());
            Assert.Equal(Shade.DIM, ShadeHelper.extract(e.to_any()));
            Assert.Throws<DynAny.InvalidValue>(() => e.set_as_string("PINK"));
        }

        [Fact]
        public void ExceptionsAndIdentity()
        {
            IDynAnyFactory f = Factory();
            IDynAny d = f.create_dyn_any_from_type_code(PointHelper.type());
            Assert.Throws<DynAny.TypeMismatch>(() => d.insert_string("x"));
            Assert.Throws<DynAnyFactory.InconsistentTypeCode>(() =>
                f.create_dyn_any_from_type_code(orb.get_primitive_tc(Corba.TCKind.tk_Principal)));
            IDynAny c1 = d.current_component(), c2 = d.current_component();
            Assert.True(c1._is_equivalent(c2));
            Assert.False(c1._is_equivalent(d.copy().current_component()));
            d.destroy();
            Assert.Throws<Corba.OBJECT_NOT_EXIST>(() => d.component_count());
        }
    }
}

// Local interfaces (idl/LocalTest.idl): implementations derive from the
// generated abstract class on Corba.LocalObject; they are never sent.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

using LocalTest;
using Xunit;

namespace Rustorb.Tests
{
    public sealed class LoudImpl : Loud
    {
        public override int count { get; set; }
        public override IRemote? target => null;

        public override string greet(string who)
        {
            if (who == "")
            {
                throw new Greeter.Busy("nobody");
            }
            count++;
            return "hello " + who;
        }

        public override Greeter.Msg make(out int n)
        {
            n = count;
            return new Greeter.Msg("made");
        }

        public override void shout() => count += 10;
    }

    public sealed class FactoryImpl : Factory
    {
        public override IGreeter? make_greeter(ILoud? like) => like;
    }

    public class LocalTests
    {
        [Fact]
        public void Implementations()
        {
            var loud = new LoudImpl();
            IGreeter g = new FactoryImpl().make_greeter(loud)!;
            Assert.Equal("hello bob", g.greet("bob"));
            Assert.Equal(1, g.count);
            var busy = Assert.Throws<Greeter.Busy>(() => g.greet(""));
            Assert.Equal("nobody", busy.why);
            loud.shout();
            Assert.Equal("made", g.make(out int n).text);
            Assert.Equal(11, n);
            Assert.Equal(3, Greeter.N);
        }

        [Fact]
        public void Objects()
        {
            var loud = new LoudImpl();
            Assert.True(loud._is_a(LoudHelper.id()));
            Assert.True(loud._is_a(GreeterHelper.id()));
            Assert.True(loud._is_a("IDL:omg.org/CORBA/Object:1.0"));
            Assert.False(loud._is_a(FactoryHelper.id()));
            Assert.False(loud._non_existent());
            Assert.Same(loud, GreeterHelper.narrow(loud));
            Assert.Null(LoudHelper.narrow(null));
            Assert.Throws<Corba.BAD_PARAM>(() => FactoryHelper.narrow(loud));
            Assert.Equal(Corba.TCKind.tk_local_interface, LoudHelper.type().kind());
        }

        [Fact]
        public void NotSent()
        {
            var e = Assert.Throws<Corba.MARSHAL>(() => LoudHelper.insert(new Corba.Any(), new LoudImpl()));
            Assert.Equal(4u, e.minor);
        }
    }
}

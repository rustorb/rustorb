// The IDL 4 constructs of cpp/tests/idl/Idl4.idl (those cpp/tests/test_idl4.cc
// covers in C++): int8, bitmask, bitset, @default and @range / @min / @max,
// through the ORB in one process.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

using System;
using Idl4;
using Omg.Types;
using Xunit;

namespace Rustorb.Tests
{
    public sealed class Idl4EchoImpl : Idl4.EchoPOA
    {
        public override Settings echo_settings(Settings s) => s;

        public override Flags echo_flags(Flags f, out Wide w)
        {
            w = Wide.LOW | Wide.HIGH;
            return f;
        }

        public override Header echo_header(Header h) => h;

        public override sbyte echo_int8(sbyte v, ref ISequence<sbyte> b)
        {
            var r = new Sequence<sbyte>(b);
            r.Reverse();
            b = r;
            return (sbyte)-v;
        }

        public override Corba.Any echo_any(Corba.Any a) => a;
    }

    public class Idl4Tests : IClassFixture<EchoFixture>
    {
        private readonly IEcho echo;

        public Idl4Tests(EchoFixture f)
        {
            Corba.Object o = f.Poa.servant_to_reference(new Idl4EchoImpl());
            echo = Idl4.EchoHelper.narrow(f.Orb.string_to_object(f.Orb.object_to_string(o)))!;
        }

        [Fact]
        public void Defaults()
        {
            var s = new Settings();
            Assert.Equal((7, true, 2.5), (s.retries, s.enabled, s.ratio));
            Assert.Equal(0, s.id);
            Assert.Equal(-128, Idl4.Constants.SMALLEST);
        }

        [Fact]
        public void Bitmasks()
        {
            Assert.Equal(1 << 3, (int)Flags.WRITE);
            Assert.Equal(1UL << 39, (ulong)Wide.HIGH);
            Assert.Equal(Flags.READ | Flags.EXEC, echo.echo_flags(Flags.READ | Flags.EXEC, out Wide w));
            Assert.Equal(Wide.LOW | Wide.HIGH, w);
            Assert.True(FlagsHelper.type().equal(Rustorb.TypeCodes.Simple(Corba.TCKind.tk_octet)));
            Assert.True(WideHelper.type().equal(Rustorb.TypeCodes.Simple(Corba.TCKind.tk_ulonglong)));
        }

        [Fact]
        public void Bitsets()
        {
            var h = new Header { kind = 5, delta = -3, last = true };
            Header got = echo.echo_header(h);
            Assert.Equal(h, got);
            Assert.Equal(-3, got.delta);
            // Bits beyond a bitfield's width are not sent.
            Assert.Equal(new Header { kind = 1 }, echo.echo_header(new Header { kind = 9 }));
            Assert.True(HeaderHelper.type().equal(Rustorb.TypeCodes.Simple(Corba.TCKind.tk_ushort)));
        }

        [Fact]
        public void StructsAndInt8()
        {
            var s = new Settings
            {
                id = 1, level = 10, step = 5, offset = -7, raw = 200,
                flags = Flags.WRITE, wide = Wide.HIGH, header = new Header { kind = 2, delta = 15 },
            };
            Assert.Equal(s, echo.echo_settings(s));
            ISequence<sbyte> b = new Sequence<sbyte> { -1, 2, -3 };
            Assert.Equal(-100, echo.echo_int8(100, ref b));
            Assert.Equal(new sbyte[] { -3, 2, -1 }, b);
        }

        [Fact]
        public void Ranges()
        {
            // @range(min=0, max=10) level and @min(1) @max(5) step: values outside are not sent.
            Assert.Throws<Corba.BAD_PARAM>(() => echo.echo_settings(new Settings { level = 11, step = 1 }));
            Assert.Throws<Corba.BAD_PARAM>(() => echo.echo_settings(new Settings { level = 0, step = 0 }));
            Assert.Equal(3, echo.echo_settings(new Settings { step = 3 }).step);
        }

        [Fact]
        public void Anys()
        {
            var a = new Corba.Any();
            HeaderHelper.insert(a, new Header { kind = 7, delta = -16, last = false });
            Assert.Equal(new Header { kind = 7, delta = -16 }, HeaderHelper.extract(echo.echo_any(a)));
            var f = new Corba.Any();
            FlagsHelper.insert(f, Flags.EXEC);
            Assert.Equal(Flags.EXEC, FlagsHelper.extract(echo.echo_any(f)));
            var s = new Corba.Any();
            SettingsHelper.insert(s, new Settings { step = 2, offset = -1 });
            Assert.Equal(-1, SettingsHelper.extract(echo.echo_any(s)).offset);
        }
    }
}

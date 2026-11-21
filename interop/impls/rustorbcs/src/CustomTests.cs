// The custom valuetype tests (idl/CustomTest.idl) on the IDL4 to C#
// mapping: the marshaling of Blob, the CustomEcho servant and the client
// cases, those of impls/java/interop/CustomTests.java.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

using System.Linq;
using CustomTest;
using Omg.Types;

namespace Interop
{
    public static class CustomTests
    {
        /// <summary>Blob, marshaled as CustomTest.idl says.</summary>
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
                pt = (ValueTest.Point)@is.read_Value();
                uint len = @is.read_ulong();
                var v = new Sequence<int>();
                @is.read_long_array(v, 0, len);
                values = v;
            }
        }

        public static void RegisterFactories(Corba.ORB orb)
        {
            ValueTests.RegisterFactories(orb);
            orb.register_value_factory(BlobHelper.id(), new Corba.ValueFactory(() => new BlobImpl()));
        }

        public sealed class CustomEchoImpl : CustomEchoPOA
        {
            private readonly Corba.ORB orb;

            public CustomEchoImpl(Corba.ORB orb)
            {
                this.orb = orb;
            }

            public override Blob echo_blob(Blob b) => b;
            public override BlobPair echo_blob_pair(BlobPair p) => p;
            public override ISequence<Blob> echo_blobs(ISequence<Blob> s) => s;
            public override void shutdown() => orb.shutdown(false);
        }

        private static Blob MakeBlob(int n, string label, ValueTest.Point pt, int[] values) =>
            new BlobImpl { n = n, label = label, pt = pt, values = new Sequence<int>(values) };

        private static bool IsBlob(Blob b, int n, string label, int[] values) =>
            b is BlobImpl && b.n == n && b.label == label && b.values.SequenceEqual(values);

        private static void Check(bool cond, string what)
        {
            if (!cond)
            {
                throw new CheckFailed(what);
            }
        }

        public static void Run(Runner r, ICustomEcho echo)
        {
            r.Run("echo_blob[plain]", () =>
            {
                Blob got = echo.echo_blob(MakeBlob(7, "seven", ValueTests.MakePoint(1, -2), new[] { 1, 2, 3 }));
                Check(IsBlob(got, 7, "seven", new[] { 1, 2, 3 }), "wrong blob");
                Check(ValueTests.IsPoint(got.pt, 1, -2), "wrong point");
            });
            r.Run("echo_blob[null]", () => Check(echo.echo_blob(null) == null, "not null"));
            r.Run("echo_blob[empty]", () =>
            {
                Blob got = echo.echo_blob(MakeBlob(0, "", null, new int[0]));
                Check(IsBlob(got, 0, "", new int[0]), "wrong blob");
                Check(got.pt == null, "not null");
            });
            r.Run("echo_blob[derived]", () =>
            {
                Blob got = echo.echo_blob(MakeBlob(3, "three", ValueTests.MakePoint3(), new[] { -1 }));
                Check(IsBlob(got, 3, "three", new[] { -1 }), "wrong blob");
                Check(ValueTests.IsPoint3(got.pt), "wrong point3");
            });
            r.Run("echo_blob_pair[shared]", () =>
            {
                ValueTest.Point p = ValueTests.MakePoint(5, 6);
                BlobPair got = echo.echo_blob_pair(new BlobPair(MakeBlob(1, "one", p, new[] { 4, 5 }), p));
                Check(IsBlob(got.data, 1, "one", new[] { 4, 5 }), "wrong blob");
                Check(ReferenceEquals(got.data.pt, got.pt), "not one instance");
                Check(ValueTests.IsPoint(got.pt, 5, 6), "wrong point");
            });
            r.Run("echo_blob_pair[distinct]", () =>
            {
                BlobPair got = echo.echo_blob_pair(new BlobPair(
                    MakeBlob(1, "one", ValueTests.MakePoint(5, 6), new int[0]), ValueTests.MakePoint(5, 6)));
                Check(IsBlob(got.data, 1, "one", new int[0]), "wrong blob");
                Check(!ReferenceEquals(got.data.pt, got.pt), "one instance");
            });
            r.Run("echo_blobs", () =>
            {
                int[] many = Enumerable.Range(0, 100).ToArray();
                Blob b = MakeBlob(2, "two", null, many);
                Blob c = MakeBlob(9, "nine", ValueTests.MakePoint(9, 9), new int[0]);
                ISequence<Blob> got = echo.echo_blobs(new Sequence<Blob> { b, c, b, null });
                Check(got.Count == 4, "wrong length");
                Check(IsBlob(got[0], 2, "two", many) && IsBlob(got[1], 9, "nine", new int[0]), "wrong blobs");
                Check(ReferenceEquals(got[0], got[2]) && !ReferenceEquals(got[0], got[1]), "sharing lost");
                Check(got[3] == null, "not null");
            });
        }
    }
}

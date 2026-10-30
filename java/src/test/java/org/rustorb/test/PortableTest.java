// Portable stubs and streams (#45): a stub written the way other IDL
// compilers write them (ObjectImpl._request / _invoke / _releaseReply,
// ApplicationException), a skeleton that starts its reply before calling
// the operation, ORB.create_output_stream and the stream operations of Any.

package org.rustorb.test;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.math.BigDecimal;

import ValueTest.LongBox;
import ValueTest.LongBoxHelper;
import ValueTest.Point;
import ValueTest.Point3;
import ValueTest.PointHelper;
import mapping.CxxMapping.Derived;
import mapping.CxxMapping.DerivedHelper;
import mapping.CxxMapping.Empty;
import mapping.CxxMapping.EmptyHelper;
import mapping.CxxMapping.Failure;
import mapping.CxxMapping.FailureHelper;
import mapping.CxxMapping.Fruit;
import mapping.CxxMapping.Other;
import mapping.CxxMapping.OtherHelper;
import mapping.CxxMapping.Pair;
import mapping.CxxMapping.PairHelper;
import org.junit.jupiter.api.Test;
import org.omg.CORBA.Any;
import org.omg.CORBA.BAD_INV_ORDER;
import org.omg.CORBA.BAD_OPERATION;
import org.omg.CORBA.CompletionStatus;
import org.omg.CORBA.IntHolder;
import org.omg.CORBA.MARSHAL;
import org.omg.CORBA.NO_PERMISSION;
import org.omg.CORBA.ORB;
import org.omg.CORBA.TCKind;
import org.omg.CORBA.TypeCode;
import org.omg.CORBA.UNKNOWN;
import org.omg.CORBA.portable.ApplicationException;
import org.omg.CORBA.portable.InputStream;
import org.omg.CORBA.portable.InvokeHandler;
import org.omg.CORBA.portable.ObjectImpl;
import org.omg.CORBA.portable.OutputStream;
import org.omg.CORBA.portable.RemarshalException;
import org.omg.CORBA.portable.ResponseHandler;
import org.omg.PortableServer.POA;
import org.omg.PortableServer.Servant;

class PortableTest {
    /** A stub of CxxMapping::Derived in the portable form. */
    static final class PortableDerivedStub extends ObjectImpl {
        @Override
        public String[] _ids() {
            return new String[] {DerivedHelper.id()};
        }

        int derived_op(int a, IntHolder b, IntHolder c) throws Failure, Empty {
            while (true) {
                InputStream in = null;
                try {
                    OutputStream out = _request("derived_op", true);
                    out.write_long(a);
                    out.write_long(b.value);
                    in = _invoke(out);
                    int result = in.read_long();
                    b.value = in.read_long();
                    c.value = in.read_long();
                    return result;
                } catch (ApplicationException e) {
                    InputStream ein = e.getInputStream();
                    if (e.getId().equals(FailureHelper.id())) {
                        throw FailureHelper.read(ein);
                    }
                    if (e.getId().equals(EmptyHelper.id())) {
                        throw EmptyHelper.read(ein);
                    }
                    throw new UNKNOWN(e.getId());
                } catch (RemarshalException e) {
                    // again
                } finally {
                    _releaseReply(in);
                }
            }
        }

        void notify_(String msg) {
            try {
                OutputStream out = _request("notify", false);
                out.write_string(msg);
                assertNull(_invoke(out));
            } catch (ApplicationException | RemarshalException e) {
                throw new AssertionError(e);
            }
        }

        String label() {
            InputStream in = null;
            try {
                in = _invoke(_request("_get_label", true));
                return in.read_string();
            } catch (ApplicationException | RemarshalException e) {
                throw new AssertionError(e);
            } finally {
                _releaseReply(in);
            }
        }
    }

    static PortableDerivedStub portable(org.omg.CORBA.Object obj) {
        PortableDerivedStub s = new PortableDerivedStub();
        s._set_delegate(((ObjectImpl) obj)._get_delegate());
        return s;
    }

    static void useStub(PortableDerivedStub stub) throws Exception {
        IntHolder b = new IntHolder(10);
        IntHolder c = new IntHolder();
        assertEquals(4, stub.derived_op(3, b, c));
        assertEquals(13, b.value);
        assertEquals(20, c.value);
        Failure f = assertThrows(Failure.class, () -> stub.derived_op(-2, b, c));
        assertEquals(-2, f.code);
        assertEquals("negative", f.message);
        assertEquals(Fruit.APPLE, f.kind);
        stub.notify_("hello");
        // The oneway call may still be on its way.
        for (int i = 0; i < 100 && !stub.label().equals("hello"); i++) {
            Thread.sleep(10);
        }
        assertEquals("hello", stub.label());
    }

    @Test
    void portableStub() throws Exception {
        try (Orbs orbs = new Orbs()) {
            Derived ref = DerivedHelper.narrow(
                    orbs.serverPoa.servant_to_reference(new MappingTest.MultiImpl()));
            Derived remote = DerivedHelper.narrow(
                    orbs.client.string_to_object(orbs.server.object_to_string(ref)));
            useStub(portable(remote));
            // In the same ORB.
            useStub(portable(ref));
        }
    }

    @Test
    void replyStreamOutsideTheCall() throws Exception {
        try (Orbs orbs = new Orbs()) {
            Derived ref = DerivedHelper.narrow(orbs.client.string_to_object(orbs.server
                    .object_to_string(orbs.serverPoa.servant_to_reference(new MappingTest.MultiImpl()))));
            PortableDerivedStub stub = portable(ref);
            OutputStream out = stub._request("_get_id", true);
            InputStream in = stub._invoke(out);
            assertEquals(5, in.read_long());
            // Past the end of the reply.
            assertThrows(MARSHAL.class, in::read_long);
            stub._releaseReply(in);
            assertThrows(BAD_INV_ORDER.class, in::read_long);
            // A stream of another object.
            PortableDerivedStub other = portable(DerivedHelper.narrow(orbs.client.string_to_object(
                    orbs.server.object_to_string(orbs.serverPoa.servant_to_reference(
                            new MappingTest.MultiImpl())))));
            OutputStream foreign = other._request("_get_id", true);
            assertThrows(org.omg.CORBA.BAD_PARAM.class, () -> stub._invoke(foreign));
        }
    }

    /** A skeleton of CxxMapping::Other that starts the reply before it
     * calls the operation, as some IDL compilers generate. */
    static final class ReplyFirstOther extends Servant implements InvokeHandler {
        @Override
        public String[] _all_interfaces(POA poa, byte[] objectId) {
            return new String[] {OtherHelper.id()};
        }

        String other_op() {
            throw new NO_PERMISSION(7, CompletionStatus.COMPLETED_NO);
        }

        @Override
        public OutputStream _invoke(String op, InputStream in, ResponseHandler rh) {
            if (!op.equals("other_op")) {
                throw new BAD_OPERATION(op, 0, CompletionStatus.COMPLETED_NO);
            }
            OutputStream out = rh.createReply();
            out.write_string(other_op());
            return out;
        }
    }

    @Test
    void exceptionOfTheOperationKeepsItsStatus() throws Exception {
        try (Orbs orbs = new Orbs()) {
            Other ref = OtherHelper.narrow(orbs.client.string_to_object(orbs.server
                    .object_to_string(orbs.serverPoa.servant_to_reference(new ReplyFirstOther()))));
            NO_PERMISSION e = assertThrows(NO_PERMISSION.class, ref::other_op);
            assertEquals(7, e.minor);
            assertEquals(CompletionStatus._COMPLETED_NO, e.completed.value());
        }
    }

    @Test
    void orbOutputStream() throws Exception {
        try (Orbs orbs = new Orbs()) {
            org.omg.CORBA.Object ref = orbs.server.string_to_object(orbs.server
                    .object_to_string(orbs.serverPoa.servant_to_reference(new MappingTest.MultiImpl())));
            OutputStream out = orbs.server.create_output_stream();
            out.write_boolean(true);
            out.write_octet((byte) 7);
            out.write_long(-5);
            out.write_longlong(1L << 40);
            out.write_double(2.5);
            out.write_string("café");
            out.write_wstring("ワイド");
            out.write_long_array(new int[] {9, 1, 2, 3, 9}, 1, 3);
            out.write_fixed(new BigDecimal("-12.34"), (short) 5, (short) 2);
            out.write_Object(ref);
            out.write_Object(null);
            out.write_TypeCode(PairHelper.type());
            Any a = orbs.server.create_any();
            a.insert_long(42);
            out.write_any(a);
            // The stream keeps the value written, not the any's later one.
            a.insert_string("later");
            PairHelper.write(out, new Pair(1, 11));

            InputStream in = out.create_input_stream();
            assertTrue(in.read_boolean());
            assertEquals(7, in.read_octet());
            assertEquals(-5, in.read_long());
            assertEquals(1L << 40, in.read_longlong());
            assertEquals(2.5, in.read_double());
            assertEquals("café", in.read_string());
            assertEquals("ワイド", in.read_wstring());
            int[] longs = new int[3];
            in.read_long_array(longs, 0, 3);
            assertArrayEquals(new int[] {1, 2, 3}, longs);
            assertEquals(new BigDecimal("-12.34"), in.read_fixed((short) 5, (short) 2));
            assertTrue(in.read_Object()._is_equivalent(ref));
            assertNull(in.read_Object());
            assertTrue(in.read_TypeCode().equal(PairHelper.type()));
            assertEquals(42, in.read_any().extract_long());
            Pair p = PairHelper.read(in);
            assertEquals(1, p.first);
            assertEquals(11, p.second);
            assertThrows(MARSHAL.class, in::read_long);
            // Read again from the start.
            assertTrue(out.create_input_stream().read_boolean());
        }
    }

    @Test
    void anyStreams() {
        ORB orb = ORB.init();
        TypeCode tc = PairHelper.type();

        // The value written on the any's stream after type(t).
        Any a = orb.create_any();
        OutputStream out = a.create_output_stream();
        a.type(tc);
        assertTrue(a.type().equal(tc));
        PairHelper.write(out, new Pair(2, 22));
        assertEquals(22, PairHelper.extract(a).second);

        // read_value from the stream of create_output_stream.
        Any b = orb.create_any();
        OutputStream bout = b.create_output_stream();
        b.type(tc);
        PairHelper.write(bout, new Pair(3, 33));
        b.read_value(bout.create_input_stream(), tc);
        assertEquals(3, PairHelper.read(b.create_input_stream()).first);

        // type(t) without a value: the any has none until one is put in.
        Any c = orb.create_any();
        c.insert_long(1);
        c.type(tc);
        assertThrows(BAD_OPERATION.class, () -> PairHelper.extract(c));
        c.insert_long(5);
        assertEquals(TCKind._tk_long, c.type().kind().value());

        // write_value on a stream, and an exception's repository id first.
        OutputStream s = orb.create_output_stream();
        a.write_value(s);
        Any e = orb.create_any();
        FailureHelper.insert(e, new Failure(4, "four", Fruit.CHERRY));
        e.write_value(s);
        InputStream in = s.create_input_stream();
        assertEquals(22, PairHelper.read(in).second);
        Any back = orb.create_any();
        back.read_value(in, FailureHelper.type());
        assertEquals(4, FailureHelper.extract(back).code);
        InputStream ein = e.create_input_stream();
        assertEquals(FailureHelper.id(), ein.read_string());
        assertEquals(4, ein.read_long());
    }

    /** A Point3 whose private member is set. */
    static final class LabeledPoint3 extends Point3 {
        private static final long serialVersionUID = 1L;

        LabeledPoint3() {
            label = "p3";
        }
    }

    @Test
    void valuesInAnys() throws Exception {
        ORB orb = ORB.init();
        Any a = orb.create_any();
        Point3 p3 = new LabeledPoint3();
        p3.x = 1;
        p3.y = 2;
        p3.z = 3;
        a.insert_Value(p3);
        assertEquals("IDL:ValueTest/Point3:1.0", a.type().id());
        Point back = (Point) a.extract_Value();
        assertTrue(back instanceof Point3);
        assertEquals(p3.x, back.x);

        a.insert_Value(p3, PointHelper.type());
        assertTrue(a.type().equal(PointHelper.type()));
        assertEquals(p3.y, ((Point) a.extract_Value()).y);

        a.insert_Value(new LongBox(12), LongBoxHelper.type());
        assertEquals(12, ((LongBox) a.extract_Value()).value);

        a.insert_long(1);
        assertThrows(BAD_OPERATION.class, a::extract_Value);
        assertThrows(MARSHAL.class, () -> a.insert_Value("not a value"));
    }
}

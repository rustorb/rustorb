// The interop test cases of interop/impls/java (the basic suite and the
// any suite) in one process, over IIOP on the loopback interface.

package org.rustorb.test;

import static org.junit.jupiter.api.Assertions.assertEquals;

import java.util.Collections;

import AnyTest.AnyEcho;
import AnyTest.AnyEchoHelper;
import InteropTest.Echo;
import InteropTest.EchoHelper;
import interop.AnyTests;
import interop.Cases;
import interop.EchoImpl;
import org.junit.jupiter.api.Test;

class InteropCasesTest {
    @Test
    void basic() throws Exception {
        try (Orbs orbs = new Orbs()) {
            String ior = orbs.server.object_to_string(
                    orbs.serverPoa.servant_to_reference(new EchoImpl(orbs.server)));
            Echo echo = EchoHelper.narrow(orbs.client.string_to_object(ior));
            Cases.Runner r = new Cases.Runner();
            Cases.run(r, echo, orbs.clientPoa);
            r.summary();
            assertEquals(Collections.emptyList(), r.failed());
        }
    }

    @Test
    void any() throws Exception {
        try (Orbs orbs = new Orbs()) {
            String ior = orbs.server.object_to_string(
                    orbs.serverPoa.servant_to_reference(new AnyTests.AnyEchoImpl(orbs.server)));
            AnyEcho echo = AnyEchoHelper.narrow(orbs.client.string_to_object(ior));
            Cases.Runner r = new Cases.Runner();
            AnyTests.run(r, orbs.client, echo);
            r.summary();
            assertEquals(Collections.emptyList(), r.failed());
        }
    }
}

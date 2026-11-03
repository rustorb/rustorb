"""The omniORBpy side of run-pi-probe.sh: the request and reply service
contexts of impls/rustorb/src/pi_probe.rs, carried with omniORBpy's own
request interceptors (omniORB.interceptors, as its user's guide describes
them: omniORBpy has no Portable Interceptors).

    pi-probe server [ORB options]       prints the IOR of an Echo object
                                        (PiProbeOmni.idl), then serves
    pi-probe client IOR [ORB options]   calls echo

Both sides carry a request service context and a reply service context
with the id CONTEXT, each a string in a CDR encapsulation
(omniORB.cdrMarshal). The server keeps the request context of the upcall
until its reply and answers with "omniorbpy got: <request context>".

omniORBpy cannot add tagged components to its IORs, so the server's IOR
has none; the client reads the component from the IOR it is given itself
(the CDR of the IOR and the IIOP profile, CORBA 3.x 7.6 and 9.7.2) and
prints the same lines as the other probes.
"""

import binascii
import os
import struct
import sys
import threading

# The stubs from omniidl (docker/omniorb), or PI_PROBE_STUBS.
sys.path.insert(0, os.environ.get(
    "PI_PROBE_STUBS", os.path.join(os.path.dirname(os.path.abspath(__file__)), "stubs")))

import omniORB
import omniORB.interceptors
from omniORB import CORBA

CONTEXT = 0x50495052
TAG = 0x50495052
NAME = "omniorbpy"


def encode(s):
    return omniORB.cdrMarshal(CORBA.TC_string, s)


def decode(data):
    try:
        return omniORB.cdrUnmarshal(CORBA.TC_string, data)
    except Exception as e:  # noqa: BLE001 - printed for the comparison
        return "<%r>" % (e,)


class Cdr:
    """Reads CDR from an encapsulation, aligned from its first octet."""

    def __init__(self, data):
        self.data = data
        self.pos = 1
        self.order = "<" if data[0] & 1 else ">"

    def ulong(self):
        self.pos = (self.pos + 3) & ~3
        (v,) = struct.unpack_from(self.order + "I", self.data, self.pos)
        self.pos += 4
        return v

    def ushort(self):
        self.pos = (self.pos + 1) & ~1
        (v,) = struct.unpack_from(self.order + "H", self.data, self.pos)
        self.pos += 2
        return v

    def octets(self):
        n = self.ulong()
        v = self.data[self.pos:self.pos + n]
        self.pos += n
        return v

    def octet(self):
        v = self.data[self.pos]
        self.pos += 1
        return v


def ior_component(ior, tag):
    """The data of the first component tagged tag in an IIOP 1.1+ profile
    of the stringified IOR ior, or None."""
    # The stringified IOR is an encapsulation.
    cdr = Cdr(binascii.unhexlify(ior[4:]))
    cdr.octets()  # type_id
    for _ in range(cdr.ulong()):
        profile_tag = cdr.ulong()
        body = cdr.octets()
        if profile_tag != 0:  # TAG_INTERNET_IOP
            continue
        p = Cdr(body)
        major, minor = p.octet(), p.octet()
        p.octets()  # host
        p.ushort()  # port
        p.octets()  # object_key
        if (major, minor) < (1, 1):
            continue
        for _ in range(p.ulong()):
            component_tag = p.ulong()
            component = p.octets()
            if component_tag == tag:
                return component
    return None


# --- client ------------------------------------------------------------------

seen = {}


def client_send_request(operation, contexts):
    contexts.append((CONTEXT, encode("from " + NAME)))


def client_receive_reply(operation, contexts):
    for context_id, data in contexts:
        if context_id == CONTEXT:
            seen["reply"] = decode(data)


# --- server ------------------------------------------------------------------

upcall = threading.local()


def server_receive_request(operation, contexts):
    upcall.context = None
    for context_id, data in contexts:
        if context_id == CONTEXT:
            upcall.context = decode(data)
    # For clients that cannot read the reply context (run-pi-probe.sh).
    print("request context: %s" % (upcall.context or "<none>"), flush=True)


def server_send_reply(operation, contexts):
    context = getattr(upcall, "context", None)
    if context is not None:
        contexts.append((CONTEXT, encode(NAME + " got: " + context)))


def echo_servant():
    import PiProbeOmni__POA  # noqa: PLC0415

    class Echo(PiProbeOmni__POA.Svc):
        def echo(self, s):
            return s

    return Echo()


def main(argv):
    if len(argv) >= 2 and argv[1] == "server":
        omniORB.interceptors.addServerReceiveRequest(server_receive_request)
        omniORB.interceptors.addServerSendReply(server_send_reply)
    else:
        omniORB.interceptors.addClientSendRequest(client_send_request)
        omniORB.interceptors.addClientReceiveReply(client_receive_reply)
    orb = CORBA.ORB_init(argv)
    if len(argv) >= 2 and argv[1] == "server":
        poa = orb.resolve_initial_references("RootPOA")
        poa._get_the_POAManager().activate()
        obj = poa.id_to_reference(poa.activate_object(echo_servant()))
        print(orb.object_to_string(obj), flush=True)
        orb.run()
    elif len(argv) >= 3 and argv[1] == "client":
        import PiProbeOmni  # noqa: PLC0415

        echo = orb.string_to_object(argv[2])._narrow(PiProbeOmni.Svc)
        try:
            result = echo.echo("hello")
        except CORBA.Exception as e:
            result = getattr(e, "_NP_RepositoryId", repr(e))
        component = ior_component(argv[2], TAG)
        print("reply context: " + seen.get("reply", "<none>"))
        print("component: " + (decode(component) if component is not None else "<none>"))
        print("result: " + result)
        orb.destroy()
    else:
        sys.exit("usage: pi-probe server | client IOR")


if __name__ == "__main__":
    main(sys.argv)

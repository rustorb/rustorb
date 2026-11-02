"""The rustorb Python side of run-pi-probe.sh: the probe of
impls/rustorb/src/pi_probe.rs on rustorb.PortableInterceptor, printing the
same lines.

    pi-probe server [ORB options]       prints the IOR of an Echo object
                                        (DSI), then serves
    pi-probe client IOR [ORB options]   calls echo (DII)
"""

import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path[:0] = [os.path.join(HERE, "lib")]

from rustorb import CORBA, IOP, PortableInterceptor, PortableServer  # noqa: E402

CONTEXT = 0x50495052
TAG = 0x50495052
NAME = "rustorbpy"
ECHO_ID = "IDL:PiProbe/Echo:1.0"


def encode(codec, s):
    return codec.encode_value(CORBA.Any(CORBA.TC_string, s))


def decode(codec, data):
    try:
        return codec.decode_value(data, CORBA.TC_string).value()
    except CORBA.Exception as e:
        return "<%r>" % (e,)


def ex_id(e):
    return getattr(e, "_NP_RepositoryId", repr(e))


class Probe(PortableInterceptor.ClientRequestInterceptor,
            PortableInterceptor.ServerRequestInterceptor,
            PortableInterceptor.IORInterceptor):
    name = "pi-probe"

    def __init__(self, codec, slot):
        self.codec = codec
        self.slot = slot
        self.seen = []

    def send_request(self, ri):
        ri.add_request_service_context(
            IOP.ServiceContext(CONTEXT, encode(self.codec, "from " + NAME)), False)

    def receive_reply(self, ri):
        try:
            context = decode(self.codec, ri.get_reply_service_context(CONTEXT).context_data)
        except CORBA.SystemException as e:
            context = ex_id(e)
        try:
            component = decode(self.codec, ri.get_effective_component(TAG).component_data)
        except CORBA.SystemException as e:
            component = ex_id(e)
        self.seen.append("reply context: " + context)
        self.seen.append("component: " + component)

    def receive_request_service_contexts(self, ri):
        try:
            got = decode(self.codec, ri.get_request_service_context(CONTEXT).context_data)
        except CORBA.SystemException as e:
            got = ex_id(e)
        ri.set_slot(self.slot, CORBA.Any(CORBA.TC_string, got))

    def send_reply(self, ri):
        got = ri.get_slot(self.slot).value()
        ri.add_reply_service_context(
            IOP.ServiceContext(CONTEXT, encode(self.codec, "%s got: %s" % (NAME, got))), False)

    def establish_components(self, info):
        info.add_ior_component(IOP.TaggedComponent(TAG, encode(self.codec, NAME + " component")))


class Init(PortableInterceptor.ORBInitializer):
    probe = None

    def pre_init(self, info):
        codec = info._get_codec_factory().create_codec(
            IOP.Encoding(IOP.ENCODING_CDR_ENCAPS, 1, 2))
        Init.probe = Probe(codec, info.allocate_slot_id())
        info.add_client_request_interceptor(Init.probe)
        info.add_server_request_interceptor(Init.probe)
        info.add_ior_interceptor(Init.probe)


class Echo(PortableServer.DynamicImplementation):
    def __init__(self, orb):
        self.orb = orb

    def _primary_interface(self, oid, poa):
        return ECHO_ID

    def invoke(self, request):
        if request.operation() != "echo":
            raise CORBA.BAD_OPERATION(0, CORBA.COMPLETED_NO)
        args = self.orb.create_list(1)
        args.add_value("s", CORBA.Any(CORBA.TC_string, None), CORBA.ARG_IN)
        request.arguments(args)
        request.set_result(args.item(0).argument)


def main(argv):
    PortableInterceptor.register_orb_initializer(Init())
    orb = CORBA.ORB_init(argv)
    if len(argv) >= 2 and argv[1] == "server":
        poa = orb.resolve_initial_references("RootPOA")
        poa._get_the_POAManager().activate()
        obj = poa.id_to_reference(poa.activate_object(Echo(orb)))
        print(orb.object_to_string(obj), flush=True)
        orb.run()
    elif len(argv) >= 3 and argv[1] == "client":
        obj = orb.string_to_object(argv[2])
        req = obj._request("echo")
        req.add_in_arg(CORBA.Any(CORBA.TC_string, "hello"))
        req.set_return_type(CORBA.TC_string)
        try:
            req.invoke()
            result = req.return_value().value()
        except CORBA.Exception as e:
            result = ex_id(e)
        for line in Init.probe.seen:
            print(line)
        print("result: " + result)
        orb.destroy()
    else:
        sys.stderr.write("usage: pi-probe server|client IOR [ORB options]\n")
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))

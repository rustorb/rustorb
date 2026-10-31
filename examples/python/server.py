"""Serves Example::Echo and binds it in the naming service as "Echo".

    rustorb-idl -bpython -C gen ../idl/Echo.idl "$(rustorb-idl --idl-dir)/CosNaming.idl"
    PYTHONPATH=gen python server.py -ORBInitRef NameService=corbaloc::localhost:2809/NameService
"""

import sys

from rustorb import CORBA, PortableServer  # noqa: F401
import CosNaming
import Example__POA


class EchoImpl(Example__POA.Echo):
    def __init__(self, orb):
        self.orb = orb

    def echo(self, message):
        print("server:", message, flush=True)
        return message

    def shutdown(self):
        self.orb.shutdown(False)


def main():
    orb = CORBA.ORB_init(sys.argv)
    poa = orb.resolve_initial_references("RootPOA")
    poa._get_the_POAManager().activate()
    echo = EchoImpl(orb)._this()

    root = orb.resolve_initial_references("NameService")._narrow(CosNaming.NamingContext)
    root.rebind([CosNaming.NameComponent("Echo", "")], echo)
    print("server: ready", flush=True)

    orb.run()
    orb.destroy()


if __name__ == "__main__":
    main()

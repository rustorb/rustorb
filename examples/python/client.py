"""Looks "Echo" up in the naming service and calls it.

    PYTHONPATH=gen python client.py -ORBInitRef NameService=corbaloc::localhost:2809/NameService [--shutdown]
"""

import sys

from rustorb import CORBA
import CosNaming
import Example


def main():
    orb = CORBA.ORB_init(sys.argv)
    shutdown = "--shutdown" in sys.argv[1:]

    root = orb.resolve_initial_references("NameService")._narrow(CosNaming.NamingContextExt)
    echo = root.resolve_str("Echo")._narrow(Example.Echo)

    print("client:", echo.echo("hello from Python"))
    if shutdown:
        echo.shutdown()
    orb.destroy()


if __name__ == "__main__":
    main()

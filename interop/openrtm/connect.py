"""Connects the ports of two RTCs and activates both RTCs in their first
execution context.

    connect.py NAMESERVER A.rtc B.rtc [PORT_A:PORT_B]...

Without port pairs, the ports that have the same name (after the instance
name) are connected.

Runs on either ORB: ``from omniORB import CORBA`` is omniORBpy or the
rustorb compatibility package. Waits for each name to refer to a live
object, as an earlier run may have left a stale binding.
"""

import sys
import time

from omniORB import CORBA
import RTC
import OpenRTM_aist
import OpenRTM_aist.CORBA_RTCUtil as util


def resolve(naming, name):
    for _ in range(150):
        try:
            obj = naming.resolve(name)._narrow(RTC.RTObject)
            if obj is not None and not obj._non_existent():
                return obj
        except Exception:
            pass
        time.sleep(0.1)
    raise SystemExit("cannot resolve " + name)


def short(port):
    return port.get_port_profile().name.split(".", 1)[1]


def main():
    nameserver, name_a, name_b = sys.argv[1:4]
    pairs = dict(arg.split(":", 1) for arg in sys.argv[4:])
    orb = CORBA.ORB_init([sys.argv[0]])
    naming = OpenRTM_aist.CorbaNaming(orb, nameserver)
    a, b = resolve(naming, name_a), resolve(naming, name_b)
    ports_b = {short(p): p for p in b.get_ports()}
    status = 0
    n = 0
    for p in a.get_ports():
        q = ports_b.get(pairs.get(short(p), short(p)))
        if q is None:
            continue
        prop = OpenRTM_aist.Properties()
        prop.setProperty("dataport.interface_type", "corba_cdr")
        prop.setProperty("dataport.dataflow_type", "push")
        prop.setProperty("dataport.subscription_type", "flush")
        rc = util.connect("connector%d" % n, prop, p, q)
        print("connect", short(p), rc)
        if rc != RTC.RTC_OK:
            status = 1
        n += 1
    for rtc in (a, b):
        # Right after start-up the RTC can still be in the created state,
        # which its execution context leaves on its first run.
        for _ in range(100):
            rc = util.activate(rtc, 0)
            if rc != RTC.PRECONDITION_NOT_MET:
                break
            time.sleep(0.1)
        print("activate", rc)
        if rc != RTC.RTC_OK:
            status = 1
    orb.destroy()
    return status


if __name__ == "__main__":
    sys.exit(main())

"""The rustorb Python side of run-ami-probe.sh: callback AMI (docs/ami.md)
against the Echo server of any implementation. Each case sends one
sendc_ request and prints what the AMI_EchoHandler servant got, in the
lines that run-ami-probe.sh compares (the same in every probe).

    ami_probe.py REFERENCE [ORB options]

REFERENCE names the Echo object, such as corbaname::127.0.0.1:2809#ami/Echo.
"""

import os
import queue
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path[:0] = [os.path.join(HERE, "lib"), os.path.join(HERE, "stubs")]

from rustorb import CORBA, timeouts  # noqa: E402
import InteropTest  # noqa: E402
import InteropTest__POA  # noqa: E402

WAIT = 10
COMPLETED = {
    CORBA.COMPLETED_YES: "COMPLETED_YES",
    CORBA.COMPLETED_NO: "COMPLETED_NO",
    CORBA.COMPLETED_MAYBE: "COMPLETED_MAYBE",
}


def describe(holder):
    """What raise_exception() raises, as a probe line."""
    try:
        holder.raise_exception()
    except InteropTest.Oops as e:
        return "Oops %d %s" % (e.code, e.reason)
    except InteropTest.DivideByZero:
        return "DivideByZero"
    except CORBA.SystemException as e:
        return "%s %d %s" % (type(e).__name__, e.minor, COMPLETED.get(e.completed, "?"))
    except Exception as e:  # noqa: BLE001
        return "unexpected %r" % (e,)
    return "nothing raised"


class Handler(InteropTest__POA.AMI_EchoHandler):
    """Puts one line into a queue for each call."""

    def __init__(self):
        self.lines = queue.Queue()

    def _put(self, line):
        self.lines.put(line)

    def echo_long(self, ami_return_val):
        self._put("echo_long: %d" % ami_return_val)

    def echo_string(self, ami_return_val):
        self._put("echo_string: %s" % ami_return_val)

    def swap(self, ami_return_val, a, b):
        self._put("swap: %d %d %d" % (ami_return_val, a, b))

    def divide(self, ami_return_val, remainder):
        self._put("divide: %d %d" % (ami_return_val, remainder))

    def divide_excep(self, excep_holder):
        self._put("divide_excep: " + describe(excep_holder))

    def fail_excep(self, excep_holder):
        self._put("fail_excep: " + describe(excep_holder))

    def fail_system_excep(self, excep_holder):
        self._put("fail_system_excep: " + describe(excep_holder))

    def set_counter(self):
        self._put("set_counter")

    def get_counter(self, ami_return_val):
        self._put("get_counter: %d" % ami_return_val)

    def get_name(self, ami_return_val):
        self._put("get_name: %s" % ami_return_val)

    def trigger_excep(self, excep_holder):
        self._put("trigger_excep: " + describe(excep_holder))


class SlowCallback(InteropTest__POA.Callback):
    """Answers after 2 s, longer than the call timeout of the last case."""

    def notify(self, message):
        time.sleep(2)
        return "ack:" + message


def main():
    if len(sys.argv) < 2:
        sys.exit("usage: ami_probe.py REFERENCE [ORB options]")
    ref = sys.argv[1]
    orb = CORBA.ORB_init([sys.argv[0], "-ORBendPoint", "giop:tcp:127.0.0.1:"] + sys.argv[2:],
                         CORBA.ORB_ID)
    poa = orb.resolve_initial_references("RootPOA")
    poa._get_the_POAManager().activate()
    echo = orb.string_to_object(ref)._narrow(InteropTest.Echo)
    handler = Handler()
    h = poa.id_to_reference(poa.activate_object(handler))._narrow(InteropTest.AMI_EchoHandler)
    status = 0

    def show(what):
        nonlocal status
        try:
            print(handler.lines.get(timeout=WAIT), flush=True)
        except queue.Empty:
            print("%s: no reply" % what, flush=True)
            status = 1

    echo.sendc_echo_long(h, 42)
    show("echo_long")
    echo.sendc_echo_string(h, "hello AMI")
    show("echo_string")
    echo.sendc_swap(h, 1, 2)
    show("swap")
    echo.sendc_divide(h, 7, 2)
    show("divide")
    echo.sendc_divide(h, 1, 0)
    show("divide_excep")
    echo.sendc_fail(h, 7, "bad")
    show("fail_excep")
    echo.sendc_fail_system(h)
    show("fail_system_excep")
    echo.sendc_set_counter(h, 5)
    show("set_counter")
    echo.sendc_get_counter(h)
    show("get_counter")
    echo.sendc_get_name(h)
    show("get_name")
    # A nil handler: the request is sent, the reply dropped.
    echo.sendc_set_counter(None, 9)
    deadline = time.monotonic() + WAIT
    while echo._get_counter() != 9 and time.monotonic() < deadline:
        time.sleep(0.01)
    print("nil handler: counter %d" % echo._get_counter(), flush=True)
    # The server calls the slow callback; the call times out first.
    cb = poa.id_to_reference(poa.activate_object(SlowCallback()))._narrow(InteropTest.Callback)
    echo.register_callback(cb)
    timeouts.set_object_call_timeout(echo, 0.5)
    echo.sendc_trigger(h, "late")
    timeouts.set_object_call_timeout(echo, None)
    show("trigger_excep")
    # Let the server's call of the callback finish before going away.
    time.sleep(2)
    orb.destroy()
    return status


if __name__ == "__main__":
    sys.exit(main())

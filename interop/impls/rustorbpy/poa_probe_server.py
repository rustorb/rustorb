"""rustorb (Python) server for the POA and thread model probes (PoaProbe.idl),
the same as the omniORBpy one (impls/omniorbpy/poa_probe_server.py) on the
rustorb package.

    poa_probe_server.py --host HOST --port N --ior-file FILE

Publishes Target objects in POAs with different policies and writes the IOR
of the Control object to FILE. scripts/poa-probe drives it; see
run-poa-probe.sh. build.sh puts the rustorb package in lib/ and the stubs in
stubs/.
"""
import argparse
import os
import sys
import threading
import time

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path[:0] = [os.path.join(HERE, "lib"), os.path.join(HERE, "stubs")]

from rustorb import CORBA, PortableServer, PortableServer__POA, timeouts  # noqa: E402
import PoaProbe  # noqa: E402
import PoaProbe__POA  # noqa: E402

START = time.monotonic()
MAIN_THREAD = threading.get_ident()
TARGET_ID = "IDL:rustorb.org/PoaProbe/Target:1.0"


class Threads:
    """Gives threads short, stable labels in the order they are first seen."""

    def __init__(self):
        self._lock = threading.Lock()
        self._labels = {}

    def label(self):
        ident = threading.get_ident()
        if ident == MAIN_THREAD:
            return "main"
        with self._lock:
            if ident not in self._labels:
                self._labels[ident] = "T%d" % (len(self._labels) + 1)
            return self._labels[ident]


THREADS = Threads()
EVENTS = []
EVENTS_LOCK = threading.Lock()


def event(text):
    with EVENTS_LOCK:
        EVENTS.append(text)


def poa_path(poa):
    names = []
    while True:
        parent = poa._get_the_parent()
        if parent is None:
            break
        names.append(poa._get_the_name())
        poa = parent
    return "/".join(reversed(names)) or "RootPOA"


class TargetServant(PoaProbe__POA.Target):

    def __init__(self, current):
        self._current = current

    def work(self, ms):
        start = time.monotonic() - START
        thread = THREADS.label()
        oid = self._current.get_object_id().decode("latin-1", "replace")
        poa = poa_path(self._current.get_POA())
        if ms > 0:
            time.sleep(ms / 1000.0)
        return PoaProbe.CallInfo(thread, oid, poa, start, time.monotonic() - START)

    def post(self, seq, ms):
        if ms > 0:
            time.sleep(ms / 1000.0)
        event("post %d" % seq)

    def relay(self, other, ms):
        outer = THREADS.label()
        timeouts.set_object_call_timeout(other, 3.0)
        return other.work(ms), outer


class Activator(PortableServer__POA.ServantActivator):

    def __init__(self, servant, forward):
        self._servant = servant
        self._forward = forward

    def incarnate(self, oid, adapter):
        name = oid.decode("latin-1", "replace")
        event("incarnate %s" % name)
        if name == "bad":
            raise CORBA.OBJECT_NOT_EXIST(0, CORBA.COMPLETED_NO)
        if name == "fwd":
            raise PortableServer.ForwardRequest(self._forward)
        return self._servant

    def etherealize(self, oid, adapter, servant, cleanup_in_progress, remaining_activations):
        event("etherealize %s" % oid.decode("latin-1", "replace"))


class Locator(PortableServer__POA.ServantLocator):

    def __init__(self, servant, forward):
        self._servant = servant
        self._forward = forward

    def preinvoke(self, oid, adapter, operation):
        name = oid.decode("latin-1", "replace")
        event("preinvoke %s %s" % (name, operation))
        if name == "bad":
            raise CORBA.OBJECT_NOT_EXIST(0, CORBA.COMPLETED_NO)
        if name == "fwd":
            raise PortableServer.ForwardRequest(self._forward)
        return (self._servant, None)

    def postinvoke(self, oid, adapter, operation, cookie, servant):
        event("postinvoke %s %s" % (oid.decode("latin-1", "replace"), operation))


class AdapterActivator(PortableServer__POA.AdapterActivator):
    """Creates POAs named lazy* on demand, with a default servant."""

    def __init__(self, servant):
        self._servant = servant

    def unknown_adapter(self, parent, name):
        event("unknown_adapter %s" % name)
        if not name.startswith("lazy"):
            return False
        poa = parent.create_POA(name, parent._get_the_POAManager(), [
            parent.create_lifespan_policy(PortableServer.PERSISTENT),
            parent.create_id_assignment_policy(PortableServer.USER_ID),
            parent.create_servant_retention_policy(PortableServer.NON_RETAIN),
            parent.create_request_processing_policy(PortableServer.USE_DEFAULT_SERVANT),
            parent.create_id_uniqueness_policy(PortableServer.MULTIPLE_ID),
        ])
        poa.set_servant(self._servant)
        return True


class ControlServant(PoaProbe__POA.Control):

    def __init__(self, orb, root, managed_manager, servant, system_refs):
        self._orb = orb
        self._root = root
        self._managed = managed_manager
        self._servant = servant
        self._system_refs = system_refs

    def _poa(self, path):
        poa = self._root
        try:
            for name in [p for p in path.split("/") if p and p != "RootPOA"]:
                poa = poa.find_POA(name, True)
        except PortableServer.POA.AdapterNonExistent:
            raise PoaProbe.Failed("no POA %s" % path)
        return poa

    def reference(self, poa, oid):
        if poa in self._system_refs:
            return self._orb.object_to_string(self._system_refs[poa])
        ref = self._poa(poa).create_reference_with_id(oid.encode("latin-1"), TARGET_ID)
        return self._orb.object_to_string(ref)

    def set_state(self, state):
        try:
            if state == "active":
                self._managed.activate()
            elif state == "holding":
                self._managed.hold_requests(False)
            elif state == "discarding":
                self._managed.discard_requests(False)
            elif state == "inactive":
                self._managed.deactivate(False, False)
            else:
                raise PoaProbe.Failed("unknown state %s" % state)
        except PortableServer.POAManager.AdapterInactive:
            raise PoaProbe.Failed("AdapterInactive")

    def deactivate_object(self, poa, oid):
        try:
            self._poa(poa).deactivate_object(oid.encode("latin-1"))
        except PortableServer.POA.ObjectNotActive:
            raise PoaProbe.Failed("ObjectNotActive")

    def activate_object(self, poa, oid):
        try:
            self._poa(poa).activate_object_with_id(oid.encode("latin-1"), TargetServant(self._servant._current))
        except (PortableServer.POA.ObjectAlreadyActive,
                PortableServer.POA.ServantAlreadyActive) as e:
            raise PoaProbe.Failed(type(e).__name__)

    def destroy_poa(self, poa):
        self._poa(poa).destroy(True, False)

    def events(self):
        with EVENTS_LOCK:
            result = list(EVENTS)
            del EVENTS[:]
        return result

    def shutdown_inside(self, wait_for_completion):
        try:
            self._orb.shutdown(wait_for_completion)
        except CORBA.SystemException as e:
            return "%s minor 0x%08x" % (e._NP_RepositoryId, e.minor)
        return ""

    def shutdown_later(self, delay_ms):
        def later():
            time.sleep(delay_ms / 1000.0)
            self._orb.shutdown(False)
        threading.Thread(target=later, daemon=True).start()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--host", default="")
    parser.add_argument("--port", type=int, default=0)
    parser.add_argument("--ior-file", required=True)
    args, orb_args = parser.parse_known_args()

    argv = [sys.argv[0]] + orb_args
    if args.port:
        argv += ["-ORBendPoint", "giop:tcp:%s:%d" % (args.host, args.port)]
    orb = CORBA.ORB_init(argv, CORBA.ORB_ID)
    root = orb.resolve_initial_references("RootPOA")
    current = orb.resolve_initial_references("POACurrent")
    root_manager = root._get_the_POAManager()
    root_manager.activate()
    servant = TargetServant(current)

    def policies(*extra, lifespan=PortableServer.PERSISTENT, ids=PortableServer.USER_ID):
        return [root.create_lifespan_policy(lifespan),
                root.create_id_assignment_policy(ids)] + list(extra)

    def create(parent, name, pol, manager=None):
        return parent.create_POA(name, manager or root_manager, pol)

    system_refs = {}
    # RootPOA: TRANSIENT, SYSTEM_ID, IMPLICIT_ACTIVATION.
    system_refs["RootPOA"] = servant._this()

    persistent_user = create(root, "persistent_user", policies())
    persistent_user.activate_object_with_id(b"obj", servant)
    persistent_user.activate_object_with_id(b"victim", TargetServant(current))
    nested = create(persistent_user, "nested", policies())
    nested.activate_object_with_id(b"obj", servant)

    persistent_system = create(root, "persistent_system", policies(ids=PortableServer.SYSTEM_ID))
    oid = persistent_system.activate_object(TargetServant(current))
    system_refs["persistent_system"] = persistent_system.id_to_reference(oid)

    transient_user = create(root, "transient_user", policies(lifespan=PortableServer.TRANSIENT))
    transient_user.activate_object_with_id(b"obj", servant)

    transient_system = create(root, "transient_system", policies(
        lifespan=PortableServer.TRANSIENT, ids=PortableServer.SYSTEM_ID))
    oid = transient_system.activate_object(TargetServant(current))
    system_refs["transient_system"] = transient_system.id_to_reference(oid)

    single = root.create_thread_policy(PortableServer.SINGLE_THREAD_MODEL)
    for name in ("single", "single2"):
        poa = create(root, name, policies(single))
        poa.activate_object_with_id(b"a", servant)
        poa.activate_object_with_id(b"b", TargetServant(current))

    main_poa = create(root, "main", policies(
        root.create_thread_policy(PortableServer.MAIN_THREAD_MODEL)))
    main_poa.activate_object_with_id(b"a", servant)
    main_poa.activate_object_with_id(b"b", TargetServant(current))

    managed_manager = None
    managed = root.create_POA("managed", managed_manager, policies())
    managed_manager = managed._get_the_POAManager()
    managed.activate_object_with_id(b"obj", servant)

    doomed = create(root, "doomed", policies())
    doomed.activate_object_with_id(b"obj", servant)

    forward = persistent_user.id_to_reference(b"obj")
    activator = create(root, "activator", policies(
        root.create_request_processing_policy(PortableServer.USE_SERVANT_MANAGER)))
    activator.set_servant_manager(Activator(servant, forward))
    locator = create(root, "locator", policies(
        root.create_request_processing_policy(PortableServer.USE_SERVANT_MANAGER),
        root.create_servant_retention_policy(PortableServer.NON_RETAIN)))
    locator.set_servant_manager(Locator(servant, forward))

    default = create(root, "default", policies(
        root.create_request_processing_policy(PortableServer.USE_DEFAULT_SERVANT),
        root.create_servant_retention_policy(PortableServer.NON_RETAIN),
        root.create_id_uniqueness_policy(PortableServer.MULTIPLE_ID)))
    default.set_servant(servant)

    root._set_the_activator(AdapterActivator(servant))

    # The Control object has its own POAManager, so that it keeps working
    # whatever the probe does to the others.
    control_poa = root.create_POA("control", None, policies())
    control = ControlServant(orb, root, managed_manager, servant, system_refs)
    control_poa.activate_object_with_id(b"Control", control)
    control_poa._get_the_POAManager().activate()
    managed_manager.activate()

    with open(args.ior_file + ".tmp", "w") as f:
        f.write(orb.object_to_string(control_poa.id_to_reference(b"Control")) + "\n")
    os.replace(args.ior_file + ".tmp", args.ior_file)
    print("server ready", flush=True)

    orb.run()
    print("server stopped", flush=True)
    orb.destroy()
    return 0


if __name__ == "__main__":
    sys.exit(main())

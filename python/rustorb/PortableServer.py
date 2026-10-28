"""The PortableServer module of the IDL to Python mapping.

The base class of servants, the POA, its manager, the policies and
``PortableServer::Current``. The POA is the native module's; this module
wraps it with the standard Python mapping.
"""

from . import CORBA
from . import _types

_ID = "IDL:omg.org/PortableServer/%s:2.3"


def _native():
    if CORBA._native is None:
        raise CORBA.INITIALIZE(0, CORBA.COMPLETED_NO)
    return CORBA._native


def _make_objref(native, repoId=None):
    from . import _rt

    return _rt.make_objref(native, repoId or native.type_id())


def _native_ref(obj):
    """The native reference of an object reference argument."""
    if not isinstance(obj, CORBA.Object) or obj._obj is None:
        raise CORBA.BAD_PARAM(0, CORBA.COMPLETED_NO)
    return obj._obj


class _Exception(CORBA.UserException):
    pass


def _exception(owner, name, members=()):
    cls = type(
        name,
        (_Exception,),
        {
            "_NP_RepositoryId": _ID % ("%s/%s" % (owner, name) if owner else name),
            "_NP_members": tuple(members),
            "__module__": __name__,
            "__qualname__": "%s.%s" % (owner, name) if owner else name,
        },
    )
    return cls


ForwardRequest = _exception(None, "ForwardRequest", ("forward_reference",))

# ---------------------------------------------------------------------------
# Policies


def _policy_enum(name, values):
    enum = _types.Enum(_ID % name, values)
    globals()[name] = enum
    for item in enum._items:
        globals()[item._n] = item
    return enum


_policy_enum("ThreadPolicyValue", ("ORB_CTRL_MODEL", "SINGLE_THREAD_MODEL", "MAIN_THREAD_MODEL"))
_policy_enum("LifespanPolicyValue", ("TRANSIENT", "PERSISTENT"))
_policy_enum("IdUniquenessPolicyValue", ("UNIQUE_ID", "MULTIPLE_ID"))
_policy_enum("IdAssignmentPolicyValue", ("USER_ID", "SYSTEM_ID"))
_policy_enum("ImplicitActivationPolicyValue", ("IMPLICIT_ACTIVATION", "NO_IMPLICIT_ACTIVATION"))
_policy_enum("ServantRetentionPolicyValue", ("RETAIN", "NON_RETAIN"))
_policy_enum(
    "RequestProcessingPolicyValue",
    ("USE_ACTIVE_OBJECT_MAP_ONLY", "USE_DEFAULT_SERVANT", "USE_SERVANT_MANAGER"),
)

THREAD_POLICY_ID = 16
LIFESPAN_POLICY_ID = 17
ID_UNIQUENESS_POLICY_ID = 18
ID_ASSIGNMENT_POLICY_ID = 19
IMPLICIT_ACTIVATION_POLICY_ID = 20
SERVANT_RETENTION_POLICY_ID = 21
REQUEST_PROCESSING_POLICY_ID = 22


def _policy_class(name, policy_type):
    return type(
        name,
        (CORBA.Policy,),
        {
            "_NP_RepositoryId": _ID % name,
            "policy_type": policy_type,
            "__module__": __name__,
        },
    )


ThreadPolicy = _policy_class("ThreadPolicy", THREAD_POLICY_ID)
LifespanPolicy = _policy_class("LifespanPolicy", LIFESPAN_POLICY_ID)
IdUniquenessPolicy = _policy_class("IdUniquenessPolicy", ID_UNIQUENESS_POLICY_ID)
IdAssignmentPolicy = _policy_class("IdAssignmentPolicy", ID_ASSIGNMENT_POLICY_ID)
ImplicitActivationPolicy = _policy_class("ImplicitActivationPolicy", IMPLICIT_ACTIVATION_POLICY_ID)
ServantRetentionPolicy = _policy_class("ServantRetentionPolicy", SERVANT_RETENTION_POLICY_ID)
RequestProcessingPolicy = _policy_class("RequestProcessingPolicy", REQUEST_PROCESSING_POLICY_ID)

# ---------------------------------------------------------------------------
# Servants


class Servant(object):
    """Base of servants. Skeleton classes (``Module__POA.Interface``) derive
    from it, and implementations derive from a skeleton class."""

    _NP_RepositoryId = "IDL:omg.org/CORBA/Object:1.0"

    def _this(self):
        """A reference to this servant's object: inside a request on this
        servant, the object of the request; otherwise the servant is
        activated in (or found in) its default POA."""
        native = _native()
        try:
            if native.current_servant() is self:
                return _make_objref(native.current_reference(), self._NP_RepositoryId)
        except Current.NoContext:
            pass
        return self._default_POA().servant_to_reference(self)

    def _default_POA(self):
        """The RootPOA of the ORB. Override to choose another POA."""
        return CORBA._default_orb()._get_root_poa()

    def _non_existent(self):
        return False

    def _is_a(self, repoId):
        from . import _rt

        return _rt.servant_is_a(self, repoId)


# ---------------------------------------------------------------------------
# POAManager


class POAManager(CORBA.LocalObject):
    """``PortableServer::POAManager``."""

    _NP_RepositoryId = _ID % "POAManager"

    AdapterInactive = _exception("POAManager", "AdapterInactive")
    State = _types.Enum(_ID % "POAManager/State", ("HOLDING", "ACTIVE", "DISCARDING", "INACTIVE"))
    HOLDING, ACTIVE, DISCARDING, INACTIVE = State._items

    def __init__(self, native):
        CORBA.LocalObject.__init__(self)
        self._native = native

    def activate(self):
        self._native.activate()

    def hold_requests(self, wait_for_completion):
        self._native.hold_requests(bool(wait_for_completion))

    def discard_requests(self, wait_for_completion):
        self._native.discard_requests(bool(wait_for_completion))

    def deactivate(self, etherealize_objects, wait_for_completion):
        self._native.deactivate(bool(etherealize_objects), bool(wait_for_completion))

    def get_state(self):
        return POAManager.State._item(self._native.get_state())

    def __eq__(self, other):
        return isinstance(other, POAManager) and self._native == other._native

    def __ne__(self, other):
        return not self.__eq__(other)

    def __hash__(self):
        return 0


# ---------------------------------------------------------------------------
# POA


def _wrap_poa(native):
    return POA(native)


class POA(CORBA.LocalObject):
    """``PortableServer::POA``. Get the RootPOA with
    ``orb.resolve_initial_references("RootPOA")``."""

    _NP_RepositoryId = _ID % "POA"

    AdapterAlreadyExists = _exception("POA", "AdapterAlreadyExists")
    AdapterNonExistent = _exception("POA", "AdapterNonExistent")
    InvalidPolicy = _exception("POA", "InvalidPolicy", ("index",))
    NoServant = _exception("POA", "NoServant")
    ObjectAlreadyActive = _exception("POA", "ObjectAlreadyActive")
    ObjectNotActive = _exception("POA", "ObjectNotActive")
    ServantAlreadyActive = _exception("POA", "ServantAlreadyActive")
    ServantNotActive = _exception("POA", "ServantNotActive")
    WrongAdapter = _exception("POA", "WrongAdapter")
    WrongPolicy = _exception("POA", "WrongPolicy")

    def __init__(self, native):
        CORBA.LocalObject.__init__(self)
        self._native = native

    def __eq__(self, other):
        return isinstance(other, POA) and self._native == other._native

    def __ne__(self, other):
        return not self.__eq__(other)

    def __hash__(self):
        return hash(self._native.the_name())

    def __repr__(self):
        return "<PortableServer.POA %r>" % self._native.the_name()

    # -- attributes --

    def _get_the_name(self):
        return self._native.the_name()

    def _get_the_parent(self):
        parent = self._native.the_parent()
        return None if parent is None else POA(parent)

    def _get_the_children(self):
        return [POA(c) for c in self._native.the_children()]

    def _get_the_POAManager(self):
        return POAManager(self._native.the_poa_manager())

    def _get_the_activator(self):
        return self._native.get_the_activator()

    def _set_the_activator(self, activator):
        self._native.set_the_activator(activator)

    the_name = property(_get_the_name)
    the_parent = property(_get_the_parent)
    the_children = property(_get_the_children)
    the_POAManager = property(_get_the_POAManager)
    the_activator = property(_get_the_activator, _set_the_activator)

    # -- POA creation and destruction --

    def create_POA(self, adapter_name, a_POAManager, policies):
        values = []
        for p in policies:
            if not isinstance(p, CORBA.Policy):
                raise CORBA.BAD_PARAM(0, CORBA.COMPLETED_NO)
            values.append((p.policy_type, int(p.value)))
        manager = None if a_POAManager is None else a_POAManager._native
        return POA(self._native.create_poa(adapter_name, manager, values))

    def find_POA(self, adapter_name, activate_it):
        return POA(self._native.find_poa(adapter_name, bool(activate_it)))

    def destroy(self, etherealize_objects, wait_for_completion):
        self._native.destroy(bool(etherealize_objects), bool(wait_for_completion))

    # -- policy factories --

    def create_thread_policy(self, value):
        return ThreadPolicy(value)

    def create_lifespan_policy(self, value):
        return LifespanPolicy(value)

    def create_id_uniqueness_policy(self, value):
        return IdUniquenessPolicy(value)

    def create_id_assignment_policy(self, value):
        return IdAssignmentPolicy(value)

    def create_implicit_activation_policy(self, value):
        return ImplicitActivationPolicy(value)

    def create_servant_retention_policy(self, value):
        return ServantRetentionPolicy(value)

    def create_request_processing_policy(self, value):
        return RequestProcessingPolicy(value)

    # -- servant managers and the default servant --

    def get_servant_manager(self):
        return self._native.get_servant_manager()

    def set_servant_manager(self, imgr):
        from . import PortableServer__POA

        self._native.set_servant_manager(
            imgr, isinstance(imgr, PortableServer__POA.ServantLocator)
        )

    def get_servant(self):
        return self._native.get_servant()

    def set_servant(self, p_servant):
        self._native.set_servant(p_servant)

    # -- object activation --

    def activate_object(self, p_servant):
        return self._native.activate_object(p_servant)

    def activate_object_with_id(self, id, p_servant):
        self._native.activate_object_with_id(id, p_servant)

    def deactivate_object(self, oid):
        self._native.deactivate_object(oid)

    # -- references --

    def create_reference(self, intf):
        return _make_objref(self._native.create_reference(intf), intf)

    def create_reference_with_id(self, oid, intf):
        return _make_objref(self._native.create_reference_with_id(oid, intf), intf)

    def servant_to_id(self, p_servant):
        return self._native.servant_to_id(p_servant)

    def servant_to_reference(self, p_servant):
        return _make_objref(
            self._native.servant_to_reference(p_servant), p_servant._NP_RepositoryId
        )

    def reference_to_servant(self, reference):
        return self._native.reference_to_servant(_native_ref(reference))

    def reference_to_id(self, reference):
        return self._native.reference_to_id(_native_ref(reference))

    def id_to_servant(self, oid):
        return self._native.id_to_servant(oid)

    def id_to_reference(self, oid):
        return _make_objref(self._native.id_to_reference(oid))


# ---------------------------------------------------------------------------
# Current


class Current(CORBA.LocalObject):
    """``PortableServer::Current``: the request a servant is running, from
    ``orb.resolve_initial_references("POACurrent")``."""

    _NP_RepositoryId = _ID % "Current"

    NoContext = _exception("Current", "NoContext")

    def get_POA(self):
        return POA(_native().current_poa())

    def get_object_id(self):
        return _native().current_object_id()

    def get_reference(self):
        return _make_objref(_native().current_reference())

    def get_servant(self):
        return _native().current_servant()

"""The PortableInterceptor module of the IDL to Python mapping.

Write interceptors by deriving from the local interfaces here and register
an ``ORBInitializer`` before ``CORBA.ORB_init``::

    from rustorb import CORBA, IOP, PortableInterceptor

    class Tracer(PortableInterceptor.ClientRequestInterceptor):
        def _get_name(self):
            return "tracer"

        def send_request(self, ri):
            ri.add_request_service_context(
                IOP.ServiceContext(0x52555354, b"..."), False)

    class Init(PortableInterceptor.ORBInitializer):
        def pre_init(self, info):
            info.add_client_request_interceptor(Tracer())

    PortableInterceptor.register_orb_initializer(Init())
    orb = CORBA.ORB_init(sys.argv)

Attributes are ``_get_<name>()`` methods, as in the mapping. The points
run with the GIL taken on the thread of the call (the caller's for a client
interceptor, the servant's for a server one). An info object is valid only
during its point: kept and used later, it raises ``BAD_INV_ORDER``.
``PICurrent`` (``orb.resolve_initial_references("PICurrent")``) holds the
slots of ``ORBInitInfo.allocate_slot_id``. See
docs/portable-interceptors.md.
"""

import threading as _threading

from . import CORBA, IOP, _types

# ``ReplyStatus``.
SUCCESSFUL = 0
SYSTEM_EXCEPTION = 1
USER_EXCEPTION = 2
LOCATION_FORWARD = 3
TRANSPORT_RETRY = 4
UNKNOWN = 5

# ``AdapterState``.
HOLDING = 0
ACTIVE = 1
DISCARDING = 2
INACTIVE = 3
NON_EXISTENT = 4


class ForwardRequest(CORBA.UserException):
    """Raised by a point to send the request to ``forward`` instead."""

    _NP_RepositoryId = "IDL:omg.org/PortableInterceptor/ForwardRequest:1.0"
    _NP_members = ("forward",)


class InvalidSlot(CORBA.UserException):
    _NP_RepositoryId = "IDL:omg.org/PortableInterceptor/InvalidSlot:1.0"


class Parameter(_types.Struct):
    """``Dynamic::Parameter``: ``argument`` (a ``CORBA.Any``) and ``mode``
    (``CORBA.PARAM_IN``, ``PARAM_OUT`` or ``PARAM_INOUT``, 0 to 2)."""

    _NP_RepositoryId = "IDL:omg.org/Dynamic/Parameter:1.0"
    _NP_members = ("argument", "mode")


PARAM_IN = 0
PARAM_OUT = 1
PARAM_INOUT = 2


# ---------------------------------------------------------------------------
# The interceptors


class Interceptor(CORBA.LocalObject):
    """Base of the interceptors. ``_get_name`` returns the ``name``
    attribute of the object if it has one, else ``""`` (an anonymous
    interceptor); override it to name the interceptor."""

    _NP_RepositoryId = "IDL:omg.org/PortableInterceptor/Interceptor:1.0"

    def _get_name(self):
        name = getattr(self, "name", "")
        return name if isinstance(name, str) else ""

    def destroy(self):
        pass


class ClientRequestInterceptor(Interceptor):
    _NP_RepositoryId = "IDL:omg.org/PortableInterceptor/ClientRequestInterceptor:1.0"

    def send_request(self, ri):
        pass

    def send_poll(self, ri):
        pass

    def receive_reply(self, ri):
        pass

    def receive_exception(self, ri):
        pass

    def receive_other(self, ri):
        pass


class ServerRequestInterceptor(Interceptor):
    _NP_RepositoryId = "IDL:omg.org/PortableInterceptor/ServerRequestInterceptor:1.0"

    def receive_request_service_contexts(self, ri):
        pass

    def receive_request(self, ri):
        pass

    def send_reply(self, ri):
        pass

    def send_exception(self, ri):
        pass

    def send_other(self, ri):
        pass


class IORInterceptor(Interceptor):
    _NP_RepositoryId = "IDL:omg.org/PortableInterceptor/IORInterceptor:1.0"

    def establish_components(self, info):
        pass


class ORBInitializer(CORBA.LocalObject):
    _NP_RepositoryId = "IDL:omg.org/PortableInterceptor/ORBInitializer:1.0"

    def pre_init(self, info):
        pass

    def post_init(self, info):
        pass


# ---------------------------------------------------------------------------
# The info objects


def _objref(native):
    from . import _rt

    return _rt.make_objref(native, CORBA.Object._NP_RepositoryId)


def _invalid_slot(f, *args):
    try:
        return f(*args)
    except IndexError:
        raise InvalidSlot()


class RequestInfo(CORBA.LocalObject):
    """What ``ClientRequestInfo`` and ``ServerRequestInfo`` share."""

    _NP_RepositoryId = "IDL:omg.org/PortableInterceptor/RequestInfo:1.0"

    def __init__(self, native):
        CORBA.LocalObject.__init__(self)
        self._native = native

    def _get_request_id(self):
        return self._native.request_id()

    def _get_operation(self):
        return self._native.operation()

    def _get_arguments(self):
        return [Parameter(a, m) for a, m in self._native.arguments()]

    def _get_exceptions(self):
        return self._native.exceptions()

    def _get_contexts(self):
        return self._native.contexts()

    def _get_operation_context(self):
        return self._native.operation_context()

    def _get_result(self):
        return self._native.result()

    def _get_response_expected(self):
        return self._native.response_expected()

    def _get_sync_scope(self):
        return self._native.sync_scope()

    def _get_reply_status(self):
        return self._native.reply_status()

    def _get_forward_reference(self):
        return _objref(self._native.forward_reference())

    def get_slot(self, id):
        return _invalid_slot(self._native.get_slot, id)

    def get_request_service_context(self, id):
        return IOP.ServiceContext(*self._native.get_request_service_context(id))

    def get_reply_service_context(self, id):
        return IOP.ServiceContext(*self._native.get_reply_service_context(id))


class ClientRequestInfo(RequestInfo):
    _NP_RepositoryId = "IDL:omg.org/PortableInterceptor/ClientRequestInfo:1.0"

    def _get_target(self):
        return _objref(self._native.target())

    def _get_effective_target(self):
        return _objref(self._native.effective_target())

    def _get_effective_profile(self):
        return IOP.TaggedProfile(*self._native.effective_profile())

    def _get_received_exception(self):
        return self._native.received_exception()

    def _get_received_exception_id(self):
        return self._native.received_exception_id()

    def get_effective_component(self, id):
        return IOP.TaggedComponent(*self._native.get_effective_component(id))

    def get_effective_components(self, id):
        return [IOP.TaggedComponent(*c) for c in self._native.get_effective_components(id)]

    def get_request_policy(self, type):
        p = self._native.get_request_policy(type)
        return None if p is None else CORBA._make_policy(*p)

    def add_request_service_context(self, service_context, replace):
        self._native.add_request_service_context(
            service_context.context_id, bytes(service_context.context_data), bool(replace)
        )


class ServerRequestInfo(RequestInfo):
    _NP_RepositoryId = "IDL:omg.org/PortableInterceptor/ServerRequestInfo:1.0"

    def _get_sending_exception(self):
        return self._native.sending_exception()

    def _get_sending_exception_id(self):
        return self._native.sending_exception_id()

    def _get_object_id(self):
        return self._native.object_id()

    def _get_adapter_id(self):
        return self._native.adapter_id()

    def _get_adapter_name(self):
        return self._native.adapter_name()

    def _get_orb_id(self):
        return self._native.orb_id()

    def _get_server_id(self):
        return self._native.server_id()

    def _get_target_most_derived_interface(self):
        return self._native.target_most_derived_interface()

    def get_server_policy(self, type):
        return CORBA._make_policy(*self._native.get_server_policy(type))

    def set_slot(self, id, data):
        _invalid_slot(self._native.set_slot, id, data)

    def target_is_a(self, id):
        return self._native.target_is_a(id)

    def add_reply_service_context(self, service_context, replace):
        self._native.add_reply_service_context(
            service_context.context_id, bytes(service_context.context_data), bool(replace)
        )


class IORInfo(CORBA.LocalObject):
    _NP_RepositoryId = "IDL:omg.org/PortableInterceptor/IORInfo:1.0"

    def __init__(self, native):
        CORBA.LocalObject.__init__(self)
        self._native = native

    def get_effective_policy(self, type):
        return CORBA._make_policy(*self._native.get_effective_policy(type))

    def add_ior_component(self, a_component):
        self._native.add_ior_component(a_component.tag, bytes(a_component.component_data))

    def add_ior_component_to_profile(self, a_component, profile_id):
        self._native.add_ior_component_to_profile(
            a_component.tag, bytes(a_component.component_data), profile_id
        )

    def _get_adapter_name(self):
        """The names of the POAs from the RootPOA down (rustorb)."""
        return self._native.adapter_name()


class Current(CORBA.Current):
    """``PICurrent``: the slots of ``ORBInitInfo.allocate_slot_id``, as the
    calling thread sees them."""

    _NP_RepositoryId = "IDL:omg.org/PortableInterceptor/Current:1.0"

    def __init__(self, native):
        CORBA.Current.__init__(self)
        self._native = native

    def get_slot(self, id):
        return _invalid_slot(self._native.get_slot, id)

    def set_slot(self, id, data):
        _invalid_slot(self._native.set_slot, data, id)


class ORBInitInfo(CORBA.LocalObject):
    _NP_RepositoryId = "IDL:omg.org/PortableInterceptor/ORBInitInfo:1.0"

    class DuplicateName(CORBA.UserException):
        _NP_RepositoryId = "IDL:omg.org/PortableInterceptor/ORBInitInfo/DuplicateName:1.0"
        _NP_members = ("name",)

    class InvalidName(CORBA.UserException):
        _NP_RepositoryId = "IDL:omg.org/PortableInterceptor/ORBInitInfo/InvalidName:1.0"

    def __init__(self, native):
        CORBA.LocalObject.__init__(self)
        self._native = native

    def _get_arguments(self):
        return self._native.arguments()

    def _get_orb_id(self):
        return self._native.orb_id()

    def _get_codec_factory(self):
        return IOP.CodecFactory(self._native.codec_factory())

    def register_initial_reference(self, id, obj):
        try:
            if obj is not None and isinstance(obj, CORBA.LocalObject):
                self._native.register_local_initial_reference(id, obj)
            else:
                self._native.register_initial_reference(
                    id, None if obj is None else obj._orb_object()
                )
        except LookupError:
            raise ORBInitInfo.InvalidName()

    def resolve_initial_references(self, id):
        """Only in ``post_init`` (``BAD_INV_ORDER`` in ``pre_init``)."""
        native = self._native
        if id in ("PICurrent", "CodecFactory") or (
            native.resolve_local_initial_reference(id) is not None
        ):
            # The core raises BAD_INV_ORDER in pre_init for every name, and
            # InvalidName for the empty one in post_init.
            try:
                native.resolve_initial_references("")
            except LookupError:
                pass
            if id == "PICurrent":
                return Current(native.pi_current())
            if id == "CodecFactory":
                return self._get_codec_factory()
            return native.resolve_local_initial_reference(id)
        try:
            return _objref(native.resolve_initial_references(id))
        except LookupError:
            raise ORBInitInfo.InvalidName()

    def _add(self, add, interceptor):
        name = interceptor._get_name()
        try:
            add(_Adapter(interceptor), name)
        except LookupError:
            raise ORBInitInfo.DuplicateName(name)

    def add_client_request_interceptor(self, interceptor):
        self._add(self._native.add_client_request_interceptor, interceptor)

    def add_server_request_interceptor(self, interceptor):
        self._add(self._native.add_server_request_interceptor, interceptor)

    def add_ior_interceptor(self, interceptor):
        self._add(self._native.add_ior_interceptor, interceptor)

    def allocate_slot_id(self):
        return self._native.allocate_slot_id()

    def register_policy_factory(self, type, policy_factory):
        """Not supported yet: ``NO_IMPLEMENT``."""
        self._native.register_policy_factory(type)


class _Adapter(object):
    """What the native module calls: each point with the native info, which
    this wraps in the info object of the mapping."""

    __slots__ = ("_i",)

    def __init__(self, interceptor):
        self._i = interceptor

    def destroy(self):
        self._i.destroy()

    def send_request(self, native):
        self._i.send_request(ClientRequestInfo(native))

    def receive_reply(self, native):
        self._i.receive_reply(ClientRequestInfo(native))

    def receive_exception(self, native):
        self._i.receive_exception(ClientRequestInfo(native))

    def receive_other(self, native):
        self._i.receive_other(ClientRequestInfo(native))

    def receive_request_service_contexts(self, native):
        self._i.receive_request_service_contexts(ServerRequestInfo(native))

    def receive_request(self, native):
        self._i.receive_request(ServerRequestInfo(native))

    def send_reply(self, native):
        self._i.send_reply(ServerRequestInfo(native))

    def send_exception(self, native):
        self._i.send_exception(ServerRequestInfo(native))

    def send_other(self, native):
        self._i.send_other(ServerRequestInfo(native))

    def establish_components(self, native):
        self._i.establish_components(IORInfo(native))


class _InitAdapter(object):
    __slots__ = ("_i",)

    def __init__(self, initializer):
        self._i = initializer

    def pre_init(self, native):
        self._i.pre_init(ORBInitInfo(native))

    def post_init(self, native):
        self._i.post_init(ORBInitInfo(native))


# ---------------------------------------------------------------------------
# Registration

_initializers = []
_initializers_lock = _threading.Lock()


def register_orb_initializer(init):
    """Registers ``init`` (an ``ORBInitializer``) for the ORBs that
    ``CORBA.ORB_init`` creates from now on."""
    with _initializers_lock:
        _initializers.append(init)


def _adapters():
    with _initializers_lock:
        return [_InitAdapter(i) for i in _initializers]

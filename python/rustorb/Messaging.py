"""The Messaging module of the IDL to Python mapping, as far as rustorb
has it: ``RelativeRoundtripTimeoutPolicy``, the standard form of the
client call timeouts (see ``rustorb.timeouts``), and ``ReplyHandler`` and
``ExceptionHolder`` of callback AMI (rustorb's docs/ami.md).

Make the policy with ``orb.create_policy(RELATIVE_RT_TIMEOUT_POLICY_TYPE,
CORBA.Any(TimeBase._tc_TimeT, t))``, ``t`` in units of 100 ns, and apply
it with ``obj._set_policy_overrides``, the ORB's ``ORBPolicyManager`` or
the thread's ``PolicyCurrent``.

Callback AMI: code generated with ``rustorb-idl -bpython -Wbami`` has an
``AMI_<I>Handler`` interface for each interface ``I``, deriving from
``ReplyHandler``, and ``sendc_<op>(ami_handler, in and inout arguments)``
in the stubs. The reply comes to the handler's ``<op>`` (the result, then
the ``inout`` and ``out`` arguments), an exception to its ``<op>_excep``
with an ``ExceptionHolder``; ``raise_exception()`` raises it. Handlers are
servants of ``<Module>__POA.AMI_<I>Handler``; their methods are called on
threads of the ORB.
"""

import logging as _logging
import struct as _struct

from . import CORBA
from . import _rt

RELATIVE_RT_TIMEOUT_POLICY_TYPE = 32


class RelativeRoundtripTimeoutPolicy(CORBA.Policy):
    """How long a call may take: ``relative_expiry``, a ``TimeBase::TimeT``
    (units of 100 ns)."""

    _NP_RepositoryId = "IDL:omg.org/Messaging/RelativeRoundtripTimeoutPolicy:1.0"
    policy_type = RELATIVE_RT_TIMEOUT_POLICY_TYPE

    def _get_relative_expiry(self):
        return self.value

    relative_expiry = property(_get_relative_expiry)


# ---------------------------------------------------------------------------
# Callback AMI


class ReplyHandler(object):
    """``Messaging::ReplyHandler``, the base of the ``AMI_<I>Handler``
    interfaces."""

    _NP_RepositoryId = "IDL:omg.org/Messaging/ReplyHandler:1.0"
    _NP_operations: dict = {}
    _nil = None

    def __init__(self, *args, **kw):
        raise RuntimeError("Cannot construct objects of this type.")


_tc_ReplyHandler = _rt.tc_objref(ReplyHandler._NP_RepositoryId, "ReplyHandler")


class _objref_ReplyHandler(CORBA.Object):
    _NP_RepositoryId = ReplyHandler._NP_RepositoryId
    _NP_operations = ReplyHandler._NP_operations

    def __init__(self, obj):
        CORBA.Object.__init__(self, obj)


_rt.register_interface(ReplyHandler, _objref_ReplyHandler)

# The skeleton is ``Messaging__POA.ReplyHandler``.


class ExceptionHolder(CORBA.ValueBase):
    """``Messaging::ExceptionHolder``: the exception of an AMI reply, given
    to a handler's ``<op>_excep``. ``raise_exception()`` raises it;
    ``raise_exception_with_list(exc_list)`` raises it if it is a system
    exception or its TypeCode is in ``exc_list``, else ``CORBA.UNKNOWN``.

    The state is that of the IDL: ``is_system_exception``, ``byte_order``
    (True for little endian) and ``marshaled_exception``, the exception as
    a reply carries it (the repository id, then the members, or the minor
    code and the completion status), plain CDR from offset 0. It is what a
    handler in another process receives. A holder made by the ORB of this
    process also keeps the exception itself."""

    _NP_RepositoryId = "IDL:omg.org/Messaging/ExceptionHolder:1.0"
    _NP_members = ("is_system_exception", "byte_order", "marshaled_exception")
    _NP_exception = None

    def raise_exception(self):
        raise self._NP_get(None)

    def raise_exception_with_list(self, exc_list):
        raise self._NP_get(list(exc_list))

    def _NP_get(self, exc_list):
        """The exception to raise: with ``exc_list``, the user exceptions
        are those of its TypeCodes."""
        exc = self._NP_exception
        if exc is not None and (
            exc_list is None
            or isinstance(exc, CORBA.SystemException)
            or any(tc._id == exc._NP_RepositoryId for tc in exc_list)
        ):
            return exc
        data = bytes(self.marshaled_exception)
        little = bool(self.byte_order)
        try:
            repo_id, end = _read_string(data, little)
            if self.is_system_exception:
                minor, completed = _struct.unpack_from(
                    "<II" if little else ">II", data, (end + 3) & ~3
                )
                cls = CORBA.system_exceptions.get(repo_id, CORBA.UNKNOWN)
                return cls(minor, CORBA._completion._items[completed])
        except (_struct.error, IndexError, ValueError):
            return CORBA.MARSHAL(0, CORBA.COMPLETED_YES)
        if exc_list is None:
            tc = _rt.typecodes.get(repo_id)
        else:
            tc = next((t for t in exc_list if t._id == repo_id), None)
        if tc is None or tc._k is not CORBA.tk_except or getattr(tc, "_class", None) is None:
            return CORBA.UNKNOWN(_UNLISTED_USER_EXCEPTION, CORBA.COMPLETED_YES)
        orb = _any_orb()
        if orb is None:
            return CORBA.BAD_INV_ORDER(4, CORBA.COMPLETED_NO)
        try:
            return orb._native.unmarshal_exception(tc, data, little)
        except CORBA.SystemException as e:
            return e


#: The minor code of ``UNKNOWN`` for a user exception not in the list.
_UNLISTED_USER_EXCEPTION = 0x4F4D0001

_tc_ExceptionHolder = _rt.declare_value(ExceptionHolder._NP_RepositoryId, "ExceptionHolder")
_rt.define_value(
    _tc_ExceptionHolder,
    ExceptionHolder,
    CORBA.VM_NONE,
    None,
    [
        ("is_system_exception", CORBA.TC_boolean, CORBA.PRIVATE_MEMBER),
        ("byte_order", CORBA.TC_boolean, CORBA.PRIVATE_MEMBER),
        ("marshaled_exception", _rt.tc_sequence(CORBA.TC_octet, 0), CORBA.PRIVATE_MEMBER),
    ],
)
_rt.register_value(_tc_ExceptionHolder)


def _read_string(data, little):
    """A CDR string at offset 0 of ``data``: the string and the offset
    after it."""
    (n,) = _struct.unpack_from("<I" if little else ">I", data, 0)
    if n == 0 or 4 + n > len(data):
        raise ValueError("bad string")
    return data[4 : 4 + n - 1].decode("latin-1"), 4 + n


def _any_orb():
    with CORBA._orbs_lock:
        for orb in CORBA._orbs.values():
            return orb
    return None


_logger = _logging.getLogger("rustorb.ami")


def _NP_deliver(ami_handler, reply, excep):
    """The function the native module calls with the outcome of a
    ``sendc_`` (``ObjRef.sendc``): it calls the handler. What the handler
    raises is dropped (and logged)."""

    def done(kind, value, data, little):
        if ami_handler is None:
            return
        try:
            if kind == 0:
                getattr(ami_handler, reply)(*value)
            else:
                holder = ExceptionHolder(kind == 2, little, data)
                holder._NP_exception = value
                getattr(ami_handler, excep)(holder)
        except Exception:
            _logger.info("the AMI handler's %s raised", reply if kind == 0 else excep, exc_info=True)

    return done

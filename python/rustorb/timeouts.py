"""Client call timeouts, a rustorb extension (the standard mapping has
none).

A call waits for its reply as long as the first of these that is set
says:

1. the calling thread's timeout (:func:`set_thread_call_timeout`),
2. the object reference's (:func:`set_object_call_timeout`), shared by
   the references narrowed from it,
3. the ORB's (:func:`set_call_timeout`, or ``-ORBclientCallTimeOutPeriod``).

When the time is up, the call raises ``CORBA.TIMEOUT`` with
``COMPLETED_MAYBE`` and a ``CancelRequest`` goes to the server.

Timeouts are in seconds. For the ORB, ``None`` (or 0) is no limit; for a
thread or a reference, ``None`` unsets the timeout, so that the next one
applies, and ``math.inf`` is no limit. ``omniORB.setClientCallTimeout``
and the other omniORBpy functions are in the ``omniORB`` compatibility
package.
"""

from . import CORBA, _native

__all__ = [
    "set_call_timeout", "call_timeout", "set_connect_timeout", "connect_timeout",
    "set_thread_call_timeout", "thread_call_timeout",
    "set_object_call_timeout", "object_call_timeout",
]


def _orb(orb):
    return (orb if orb is not None else CORBA._default_orb())._native


def _native_ref(obj):
    if obj is None or getattr(obj, "_obj", None) is None:
        raise CORBA.BAD_PARAM(0, CORBA.COMPLETED_NO)
    return obj._obj


def set_call_timeout(seconds, orb=None):
    """Sets how long calls through ``orb`` (by default the first ORB
    initialised) wait for a reply."""
    _orb(orb).set_call_timeout(seconds)


def call_timeout(orb=None):
    return _orb(orb).call_timeout()


def set_connect_timeout(seconds, orb=None):
    """Sets how long ``orb`` waits for a new connection; expiry raises
    ``CORBA.TRANSIENT``."""
    _orb(orb).set_connect_timeout(seconds)


def connect_timeout(orb=None):
    return _orb(orb).connect_timeout()


def set_thread_call_timeout(seconds):
    """Sets the timeout of calls from the calling thread, through any
    ORB."""
    _native.set_thread_call_timeout(seconds)


def thread_call_timeout():
    return _native.thread_call_timeout()


def set_object_call_timeout(obj, seconds):
    """Sets the timeout of calls on the object reference ``obj``."""
    _native_ref(obj).set_call_timeout(seconds)


def object_call_timeout(obj):
    return _native_ref(obj).call_timeout()



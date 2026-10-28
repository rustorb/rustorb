"""omniORBpy compatibility for rustorb.

Existing omniORBpy applications import ``omniORB``; with this package they
run on rustorb unchanged. ``omniORB.CORBA`` and ``omniORB.PortableServer``
are the rustorb modules themselves, so the code that ``rustorb-idl
-bpython`` generates and the application share their classes.

Besides the standard mapping, the package has the omniORBpy extensions
that applications use, chosen from what OpenRTM-aist-Python calls (see
docs/omniorb-compat.md):

- :func:`cdrMarshal` / :func:`cdrUnmarshal`: a value as CDR bytes
- :mod:`omniORB.any`: ``to_any`` / ``from_any``
- :func:`findType` and :mod:`omniORB.tcInternal`: type descriptors
- the ``omniINSPOA`` initial reference (in rustorb itself)
- minor code constants that applications compare
"""

import sys as _sys

from rustorb import CORBA, PortableServer, PortableServer__POA

for _m in (CORBA, PortableServer, PortableServer__POA):
    _sys.modules[__name__ + "." + _m.__name__.rsplit(".", 1)[1]] = _m
del _m

from . import tcInternal  # noqa: E402

#: omniORB's minor code of a ``COMM_FAILURE`` while waiting for a reply.
#: rustorb raises its own minor codes, so comparisons with it are false and
#: the application takes its general path.
COMM_FAILURE_WaitingForReply = 0x41540007


def _orb():
    # The first ORB initialised, as for omniORBpy's single ORB:
    # BAD_INV_ORDER before ORB_init.
    return CORBA._default_orb()._native


def _endian(endian):
    if endian == -1 or endian is None:
        return None
    return bool(endian)


def cdrMarshal(tc, data, endian=-1):
    """Marshal ``data`` of the type ``tc`` into bytes. With the default
    ``endian`` the bytes are a CDR encapsulation (a byte order octet, then
    the value in the byte order of the host); with ``endian`` true or
    false, the value alone in little or big endian. Strings are ISO
    8859-1 and wide strings UTF-16, like omniORB."""
    return _orb().cdr_marshal(tc, data, _endian(endian))


def cdrUnmarshal(tc, encap, endian=-1):
    """Read what :func:`cdrMarshal` writes. ``MARSHAL`` when ``encap`` is
    too short or longer than the value."""
    if not isinstance(encap, bytes):
        raise TypeError("a bytes-like object is required, not %r" % type(encap).__name__)
    return _orb().cdr_unmarshal(tc, encap, _endian(endian))


def findType(repoId):
    """The type descriptor of the type with the repository ID ``repoId``
    that the generated code defines, or None.

    A descriptor is the kind number (:mod:`omniORB.tcInternal`) for a
    basic type, or a tuple that starts with it: ``(tv_struct, class,
    repoId, name, member name, member descriptor, ...)`` (and the same
    for exceptions), ``(tv_union, class, repoId, name)``, ``(tv_enum,
    repoId, name, items)``, ``(tv_alias, repoId, name, content)``,
    ``(tv_objref, repoId, name)``, ``(tv_string, bound)``,
    ``(tv_wstring, bound)``, ``(tv_sequence, content, bound)``,
    ``(tv_array, content, length)``, ``(tv_fixed, digits, scale)``.
    """
    from rustorb import _rt

    tc = _rt.typecodes.get(repoId)
    if tc is None:
        return None
    return _descriptor(tc, set())


def _descriptor(tc, seen):
    k = tc.kind()
    v = int(k)
    if k in (CORBA.tk_struct, CORBA.tk_except, CORBA.tk_union):
        if id(tc) in seen:
            return (tcInternal.tv__indirect, tc.id())
        seen = seen | {id(tc)}
        if k is CORBA.tk_union:
            return (v, tc._class, tc.id(), tc.name())
        desc = [v, tc._class, tc.id(), tc.name()]
        for i in range(tc.member_count()):
            desc.append(tc.member_name(i))
            desc.append(_descriptor(tc.member_type(i), seen))
        return tuple(desc)
    if k is CORBA.tk_enum:
        return (v, tc.id(), tc.name(), tuple(tc._class._items))
    if k is CORBA.tk_alias:
        return (v, tc.id(), tc.name(), _descriptor(tc.content_type(), seen))
    if k in (CORBA.tk_objref, CORBA.tk_abstract_interface, CORBA.tk_local_interface,
             CORBA.tk_native):
        return (v, tc.id(), tc.name())
    if k in (CORBA.tk_string, CORBA.tk_wstring):
        return (v, tc.length())
    if k in (CORBA.tk_sequence, CORBA.tk_array):
        return (v, _descriptor(tc.content_type(), seen), tc.length())
    if k is CORBA.tk_fixed:
        return (v, tc.fixed_digits(), tc.fixed_scale())
    return v

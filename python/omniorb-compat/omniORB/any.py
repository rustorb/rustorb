"""``omniORB.any``: Python values to and from ``CORBA.Any`` without
writing the TypeCode.

:func:`to_any` chooses the TypeCode from the value: ``long`` for an int
that fits (else ``long long``, then ``unsigned long long``), ``double``,
``boolean``, ``string``, ``CORBA::OctetSeq`` for bytes, the type of a
struct, union, exception, enum item or object reference of the generated
code, a sequence of the items' type when they all have the same one (else
a sequence of ``any``), and a struct made for the occasion for a dict.
"""

from rustorb import CORBA, _rt, _types

_INT_KINDS = (
    (CORBA.TC_long, -(2**31), 2**31 - 1),
    (CORBA.TC_longlong, -(2**63), 2**63 - 1),
    (CORBA.TC_ulonglong, 0, 2**64 - 1),
)

_OCTET_SEQ = CORBA.TypeCode(
    CORBA.tk_alias,
    "IDL:omg.org/CORBA/OctetSeq:1.0",
    "OctetSeq",
    content=CORBA.TypeCode(CORBA.tk_sequence, content=CORBA.TC_octet, length=0),
)

_anonymous = [0]


def _bad_param():
    return CORBA.BAD_PARAM(0, CORBA.COMPLETED_NO)


def _named_tc(repoId):
    tc = _rt.typecodes.get(repoId) or _rt.dynamic_types.get(repoId)
    if tc is None:
        raise _bad_param()
    return tc


def _objref_tc(obj):
    repoId = getattr(obj, "_NP_RepositoryId", None) or CORBA.Object._NP_RepositoryId
    tc = _rt.typecodes.get(repoId)
    if tc is not None and tc.kind() is CORBA.tk_objref:
        return tc
    name = repoId.split(":")[1].rsplit("/", 1)[-1] if repoId.count(":") >= 2 else ""
    return CORBA.TypeCode(CORBA.tk_objref, repoId, name)


def _typed(data):
    """(TypeCode, value) for ``data``."""
    if isinstance(data, CORBA.Any):
        return data.typecode(), data.value()
    if data is None:
        return CORBA.TC_null, None
    if isinstance(data, bool):
        return CORBA.TC_boolean, data
    if isinstance(data, int):
        for tc, lo, hi in _INT_KINDS:
            if lo <= data <= hi:
                return tc, data
        raise _bad_param()
    if isinstance(data, float):
        return CORBA.TC_double, data
    if isinstance(data, str):
        return CORBA.TC_string, data
    if isinstance(data, bytes):
        return _OCTET_SEQ, data
    if isinstance(data, _types.EnumItem):
        return _named_tc(data._parent_id), data
    if isinstance(data, CORBA.Object):
        return _objref_tc(data), data
    if isinstance(data, (list, tuple)):
        return _sequence(list(data))
    if isinstance(data, dict):
        return _struct(data)
    repoId = getattr(type(data), "_NP_RepositoryId", None)
    if repoId is not None and not isinstance(data, CORBA.TypeCode):
        return _named_tc(repoId), data
    raise _bad_param()


def _sequence(items):
    typed = [_typed(item) for item in items]
    if typed and all(tc.equal(typed[0][0]) for tc, _ in typed[1:]):
        content = typed[0][0]
        return CORBA.TypeCode(CORBA.tk_sequence, content=content, length=0), [
            v for _, v in typed
        ]
    return CORBA.TypeCode(CORBA.tk_sequence, content=CORBA.TC_any, length=0), [
        CORBA.Any(tc, v) for tc, v in typed
    ]


def _struct(data):
    _anonymous[0] += 1
    members = []
    values = []
    for name, value in data.items():
        if not isinstance(name, str) or not name.isidentifier():
            raise _bad_param()
        tc, v = _typed(value)
        members.append((name, tc))
        values.append(v)
    tc = CORBA.TypeCode(
        CORBA.tk_struct, "rustorb:UnknownStruct:%d" % _anonymous[0], "", members=members
    )
    cls = _rt.make_class(tc)
    return tc, cls(*values)


def to_any(data):
    """``data`` as a ``CORBA.Any`` (``BAD_PARAM`` for a value without a
    CORBA type, such as a complex number)."""
    tc, value = _typed(data)
    return CORBA.Any(tc, value)


def from_any(any, keep_structs=0):
    """The value of ``any``, with nested ``any`` values unpacked and
    sequences converted item by item. Structs and exceptions become dicts
    of their members, unless ``keep_structs`` is true."""
    if not isinstance(any, CORBA.Any):
        raise _bad_param()
    return _convert(any.typecode(), any.value(), keep_structs)


def _unalias(tc):
    while tc.kind() is CORBA.tk_alias:
        tc = tc.content_type()
    return tc


def _convert(tc, value, keep_structs):
    tc = _unalias(tc)
    k = tc.kind()
    if k is CORBA.tk_any:
        return _convert(value.typecode(), value.value(), keep_structs)
    if k in (CORBA.tk_sequence, CORBA.tk_array):
        if isinstance(value, (bytes, str)):
            return value
        content = tc.content_type()
        return [_convert(content, v, keep_structs) for v in value]
    if k in (CORBA.tk_struct, CORBA.tk_except) and not keep_structs:
        return {
            tc.member_name(i): _convert(
                tc.member_type(i), getattr(value, _rt._py_name(tc.member_name(i))), keep_structs
            )
            for i in range(tc.member_count())
        }
    return value

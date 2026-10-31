"""The CORBA module of the IDL to Python mapping.

TypeCodes, ``Any`` values, the exception hierarchy, object references and
the ORB (``CORBA.ORB_init``). The ORB, invocations and the POA are those
of the native module ``rustorb._native``; the TypeCodes, values and
exceptions are plain Python, so generated code can be imported without
it.
"""

import atexit as _atexit
import builtins as _builtins
import decimal as _decimal
import reprlib as _reprlib
import threading as _threading
import time as _time
from typing import Optional

from . import _types

try:
    from . import _native
except ImportError:  # the pure-Python parts work without it
    _native = None  # type: ignore[assignment]

# ---------------------------------------------------------------------------
# TCKind


_TCKIND_NAMES = (
    "tk_null",
    "tk_void",
    "tk_short",
    "tk_long",
    "tk_ushort",
    "tk_ulong",
    "tk_float",
    "tk_double",
    "tk_boolean",
    "tk_char",
    "tk_octet",
    "tk_any",
    "tk_TypeCode",
    "tk_Principal",
    "tk_objref",
    "tk_struct",
    "tk_union",
    "tk_enum",
    "tk_string",
    "tk_sequence",
    "tk_array",
    "tk_alias",
    "tk_except",
    "tk_longlong",
    "tk_ulonglong",
    "tk_longdouble",
    "tk_wchar",
    "tk_wstring",
    "tk_fixed",
    "tk_value",
    "tk_value_box",
    "tk_native",
    "tk_abstract_interface",
    "tk_local_interface",
)
TCKind = _types.Enum("IDL:omg.org/CORBA/TCKind:1.0", _TCKIND_NAMES)
(
    tk_null,
    tk_void,
    tk_short,
    tk_long,
    tk_ushort,
    tk_ulong,
    tk_float,
    tk_double,
    tk_boolean,
    tk_char,
    tk_octet,
    tk_any,
    tk_TypeCode,
    tk_Principal,
    tk_objref,
    tk_struct,
    tk_union,
    tk_enum,
    tk_string,
    tk_sequence,
    tk_array,
    tk_alias,
    tk_except,
    tk_longlong,
    tk_ulonglong,
    tk_longdouble,
    tk_wchar,
    tk_wstring,
    tk_fixed,
    tk_value,
    tk_value_box,
    tk_native,
    tk_abstract_interface,
    tk_local_interface,
) = TCKind._items

#: Type modifiers of valuetypes (``TypeCode.type_modifier``).
VM_NONE = 0
VM_CUSTOM = 1
VM_ABSTRACT = 2
VM_TRUNCATABLE = 3

#: Visibility of valuetype state members (``TypeCode.member_visibility``).
PRIVATE_MEMBER = 0
PUBLIC_MEMBER = 1

# ---------------------------------------------------------------------------
# Exceptions


class Exception(_builtins.Exception):  # noqa: A001 - the name is fixed by the mapping
    """Base of every CORBA exception."""

    _NP_RepositoryId: Optional[str] = None


class UserException(Exception):
    """Base of the exceptions declared in IDL."""

    _NP_members: tuple = ()

    def __init__(self, *args, **kw):
        _types._init_members(self, type(self).__name__, self._NP_members, args, kw)
        Exception.__init__(self, *[getattr(self, m) for m in self._NP_members])

    def __repr__(self):
        return _types._repr_members(self, self._NP_members)

    __str__ = __repr__

    def __eq__(self, other):
        return type(other) is type(self) and all(
            getattr(self, m) == getattr(other, m) for m in self._NP_members
        )

    def __ne__(self, other):
        return not self.__eq__(other)

    __hash__ = Exception.__hash__


#: The IDL boolean constants of the Python mapping.
TRUE = True
FALSE = False


_completion = _types.Enum(
    "IDL:omg.org/CORBA/completion_status:1.0",
    ("COMPLETED_YES", "COMPLETED_NO", "COMPLETED_MAYBE"),
)
completion_status = _completion
COMPLETED_YES, COMPLETED_NO, COMPLETED_MAYBE = _completion._items


class SystemException(Exception):
    """Base of the standard system exceptions: ``minor`` and ``completed``."""

    def __init__(self, minor=0, completed=COMPLETED_NO):
        Exception.__init__(self, minor, completed)
        self.minor = minor
        self.completed = completed

    def __repr__(self):
        return "CORBA.%s(0x%x, %r)" % (type(self).__name__, self.minor, self.completed)

    __str__ = __repr__

    def __eq__(self, other):
        return (
            type(other) is type(self)
            and self.minor == other.minor
            and self.completed == other.completed
        )

    def __ne__(self, other):
        return not self.__eq__(other)

    __hash__ = Exception.__hash__


#: Repository ID -> class of every system exception.
system_exceptions: dict = {}


def _system_exception(name):
    cls = type(
        name,
        (SystemException,),
        {"_NP_RepositoryId": "IDL:omg.org/CORBA/%s:1.0" % name, "__module__": __name__},
    )
    system_exceptions[cls._NP_RepositoryId] = cls
    return cls


UNKNOWN = _system_exception("UNKNOWN")
BAD_PARAM = _system_exception("BAD_PARAM")
NO_MEMORY = _system_exception("NO_MEMORY")
IMP_LIMIT = _system_exception("IMP_LIMIT")
COMM_FAILURE = _system_exception("COMM_FAILURE")
INV_OBJREF = _system_exception("INV_OBJREF")
NO_PERMISSION = _system_exception("NO_PERMISSION")
INTERNAL = _system_exception("INTERNAL")
MARSHAL = _system_exception("MARSHAL")
INITIALIZE = _system_exception("INITIALIZE")
NO_IMPLEMENT = _system_exception("NO_IMPLEMENT")
BAD_TYPECODE = _system_exception("BAD_TYPECODE")
BAD_OPERATION = _system_exception("BAD_OPERATION")
NO_RESOURCES = _system_exception("NO_RESOURCES")
NO_RESPONSE = _system_exception("NO_RESPONSE")
PERSIST_STORE = _system_exception("PERSIST_STORE")
BAD_INV_ORDER = _system_exception("BAD_INV_ORDER")
TRANSIENT = _system_exception("TRANSIENT")
FREE_MEM = _system_exception("FREE_MEM")
INV_IDENT = _system_exception("INV_IDENT")
INV_FLAG = _system_exception("INV_FLAG")
INTF_REPOS = _system_exception("INTF_REPOS")
BAD_CONTEXT = _system_exception("BAD_CONTEXT")
OBJ_ADAPTER = _system_exception("OBJ_ADAPTER")
DATA_CONVERSION = _system_exception("DATA_CONVERSION")
OBJECT_NOT_EXIST = _system_exception("OBJECT_NOT_EXIST")
TRANSACTION_REQUIRED = _system_exception("TRANSACTION_REQUIRED")
TRANSACTION_ROLLEDBACK = _system_exception("TRANSACTION_ROLLEDBACK")
INVALID_TRANSACTION = _system_exception("INVALID_TRANSACTION")
INV_POLICY = _system_exception("INV_POLICY")
CODESET_INCOMPATIBLE = _system_exception("CODESET_INCOMPATIBLE")
REBIND = _system_exception("REBIND")
TIMEOUT = _system_exception("TIMEOUT")
TRANSACTION_UNAVAILABLE = _system_exception("TRANSACTION_UNAVAILABLE")
TRANSACTION_MODE = _system_exception("TRANSACTION_MODE")
BAD_QOS = _system_exception("BAD_QOS")
INVALID_ACTIVITY = _system_exception("INVALID_ACTIVITY")
ACTIVITY_COMPLETED = _system_exception("ACTIVITY_COMPLETED")
ACTIVITY_REQUIRED = _system_exception("ACTIVITY_REQUIRED")

# ---------------------------------------------------------------------------
# TypeCode


class TypeCode(object):
    """A CORBA TypeCode. Made by the ``TC_*`` constants and by generated
    code (``_tc_Name``); compare with :meth:`equal`."""

    class Bounds(UserException):
        _NP_RepositoryId = "IDL:omg.org/CORBA/TypeCode/Bounds:1.0"

    class BadKind(UserException):
        _NP_RepositoryId = "IDL:omg.org/CORBA/TypeCode/BadKind:1.0"

    def __init__(self, kind, repoId="", name="", **params):
        self._k = kind
        self._id = repoId
        self._name = name
        # members: [(name, TypeCode)]; labels: [value] for unions
        self._members = params.get("members")
        self._labels = params.get("labels")
        self._disc = params.get("discriminator")
        self._default = params.get("default", -1)
        self._content = params.get("content")
        self._length = params.get("length", 0)
        self._digits = params.get("digits", 0)
        self._scale = params.get("scale", 0)
        self._class = params.get("cls")
        # valuetypes: members are [(name, TypeCode, visibility)]
        self._modifier = params.get("modifier", VM_NONE)
        self._base = params.get("base")

    def __repr__(self):
        if self._id:
            return "CORBA.TypeCode(%s, %r)" % (self._k._n, self._id)
        return "CORBA.TypeCode(%s)" % self._k._n

    # -- standard operations --

    def kind(self):
        return self._k

    def _need(self, *kinds):
        if self._k not in kinds:
            raise TypeCode.BadKind()

    def id(self):
        self._need(
            tk_objref,
            tk_struct,
            tk_union,
            tk_enum,
            tk_alias,
            tk_except,
            tk_value,
            tk_value_box,
            tk_native,
            tk_abstract_interface,
            tk_local_interface,
        )
        return self._id

    def name(self):
        self._need(
            tk_objref,
            tk_struct,
            tk_union,
            tk_enum,
            tk_alias,
            tk_except,
            tk_value,
            tk_value_box,
            tk_native,
            tk_abstract_interface,
            tk_local_interface,
        )
        return self._name

    def member_count(self):
        self._need(tk_struct, tk_union, tk_enum, tk_except, tk_value)
        return len(self._members)

    def _member(self, index):
        self._need(tk_struct, tk_union, tk_enum, tk_except, tk_value)
        if not 0 <= index < len(self._members):
            raise TypeCode.Bounds()
        return self._members[index]

    def member_name(self, index):
        m = self._member(index)
        return m if self._k is tk_enum else m[0]

    def member_type(self, index):
        self._need(tk_struct, tk_union, tk_except, tk_value)
        return self._member(index)[1]

    def member_visibility(self, index):
        self._need(tk_value)
        return self._member(index)[2]

    def type_modifier(self):
        self._need(tk_value)
        return self._modifier

    def concrete_base_type(self):
        self._need(tk_value)
        return self._base if self._base is not None else TC_null

    def member_label(self, index):
        self._need(tk_union)
        self._member(index)
        if index == self._default:
            return Any(TC_octet, 0)
        return Any(self._disc, self._labels[index])

    def discriminator_type(self):
        self._need(tk_union)
        return self._disc

    def default_index(self):
        self._need(tk_union)
        return self._default

    def length(self):
        self._need(tk_string, tk_wstring, tk_sequence, tk_array)
        return self._length

    def content_type(self):
        self._need(tk_sequence, tk_array, tk_alias, tk_value_box)
        return self._content

    def fixed_digits(self):
        self._need(tk_fixed)
        return self._digits

    def fixed_scale(self):
        self._need(tk_fixed)
        return self._scale

    def equal(self, other):
        return _tc_equal(self, other, set(), alias=True)

    def equivalent(self, other):
        return _tc_equal(_unalias(self), _unalias(other), set(), alias=False)

    def get_compact_typecode(self):
        return self

    # -- for the ORB --

    def _NP_class(self):
        """The Python class of values of this type (struct, union, exception,
        enum type), or None."""
        return self._class


def _unalias(tc):
    while tc._k is tk_alias:
        tc = tc._content
    return tc


def _tc_equal(a, b, seen, alias):
    if a is b:
        return True
    if not isinstance(b, TypeCode):
        return False
    if not alias:
        a, b = _unalias(a), _unalias(b)
    if a._k is not b._k:
        return False
    key = (id(a), id(b))
    if key in seen:
        return True  # recursive types: assume equal while comparing
    seen.add(key)
    k = a._k
    if k in (
        tk_objref,
        tk_struct,
        tk_union,
        tk_enum,
        tk_alias,
        tk_except,
        tk_value,
        tk_value_box,
        tk_native,
        tk_abstract_interface,
        tk_local_interface,
    ):
        if a._id != b._id or a._name != b._name:
            return False
    if k is tk_value:
        if a._modifier != b._modifier or len(a._members) != len(b._members):
            return False
        if (a._base is None) != (b._base is None):
            return False
        if a._base is not None and not _tc_equal(a._base, b._base, seen, alias):
            return False
        for (an, at, av), (bn, bt, bv) in zip(a._members, b._members):
            if an != bn or av != bv or not _tc_equal(at, bt, seen, alias):
                return False
        return True
    if k is tk_enum:
        return list(a._members) == list(b._members)
    if k in (tk_struct, tk_except, tk_union):
        if len(a._members) != len(b._members):
            return False
        for (an, at), (bn, bt) in zip(a._members, b._members):
            if an != bn or not _tc_equal(at, bt, seen, alias):
                return False
        if k is tk_union:
            return (
                a._default == b._default
                and _tc_equal(a._disc, b._disc, seen, alias)
                and list(a._labels) == list(b._labels)
            )
        return True
    if k in (tk_string, tk_wstring):
        return a._length == b._length
    if k in (tk_sequence, tk_array):
        return a._length == b._length and _tc_equal(a._content, b._content, seen, alias)
    if k in (tk_alias, tk_value_box):
        return _tc_equal(a._content, b._content, seen, alias)
    if k is tk_fixed:
        return a._digits == b._digits and a._scale == b._scale
    return True


TC_null = TypeCode(tk_null)
TC_void = TypeCode(tk_void)
TC_short = TypeCode(tk_short)
TC_long = TypeCode(tk_long)
TC_ushort = TypeCode(tk_ushort)
TC_ulong = TypeCode(tk_ulong)
TC_float = TypeCode(tk_float)
TC_double = TypeCode(tk_double)
TC_boolean = TypeCode(tk_boolean)
TC_char = TypeCode(tk_char)
TC_octet = TypeCode(tk_octet)
TC_any = TypeCode(tk_any)
TC_TypeCode = TypeCode(tk_TypeCode)
TC_Principal = TypeCode(tk_Principal)
TC_Object = TypeCode(tk_objref, "IDL:omg.org/CORBA/Object:1.0", "Object")
TC_string = TypeCode(tk_string)
TC_longlong = TypeCode(tk_longlong)
TC_ulonglong = TypeCode(tk_ulonglong)
TC_longdouble = TypeCode(tk_longdouble)
TC_wchar = TypeCode(tk_wchar)
TC_wstring = TypeCode(tk_wstring)

#: TCKind value -> TypeCode of the kinds without parameters.
_SIMPLE = {
    tc._k._v: tc
    for tc in (
        TC_null,
        TC_void,
        TC_short,
        TC_long,
        TC_ushort,
        TC_ulong,
        TC_float,
        TC_double,
        TC_boolean,
        TC_char,
        TC_octet,
        TC_any,
        TC_TypeCode,
        TC_Principal,
        TC_longlong,
        TC_ulonglong,
        TC_longdouble,
        TC_wchar,
    )
}

# ---------------------------------------------------------------------------
# Any


class Any(object):
    """A value with its TypeCode: ``CORBA.Any(CORBA.TC_long, 1)``."""

    def __init__(self, typecode, value):
        self._t = typecode
        self._v = value

    def typecode(self):
        return self._t

    def value(self):
        return self._v

    def __repr__(self):
        return "CORBA.Any(%r, %r)" % (self._t, self._v)

    def __eq__(self, other):
        return isinstance(other, Any) and self._t.equal(other._t) and self._v == other._v

    def __ne__(self, other):
        return not self.__eq__(other)

    __hash__ = None  # type: ignore[assignment]


# ---------------------------------------------------------------------------
# Fixed point


class fixed(object):
    """An IDL ``fixed`` value: ``CORBA.fixed("12.50")``.

    The value is kept without trailing zeros after the point
    (``fixed("12.50")`` has 3 digits and scale 1). ``value()`` is the value
    as an integer scaled by ``10 ** decimals()``.
    """

    __slots__ = ("_d",)

    def __init__(self, value):
        d = _decimal.Decimal(str(value) if not isinstance(value, _decimal.Decimal) else value)
        # The default context rounds to 28 digits; a fixed has up to 31.
        with _decimal.localcontext() as ctx:
            ctx.prec = 64
            if d == d.to_integral_value():
                d = d.quantize(_decimal.Decimal(1))
            else:
                d = d.normalize()
        self._d = d

    def precision(self):
        t = self._d.as_tuple()
        return max(len(t.digits) + max(0, t.exponent), self.decimals())

    def decimals(self):
        return max(0, -self._d.as_tuple().exponent)

    def value(self):
        with _decimal.localcontext() as ctx:
            ctx.prec = 64
            return int(self._d.scaleb(self.decimals()))

    def __repr__(self):
        return 'fixed("%s")' % self

    def __str__(self):
        return str(self._d)

    def __eq__(self, other):
        if isinstance(other, fixed):
            return self._d == other._d
        return self._d == other

    def __ne__(self, other):
        return not self.__eq__(other)

    def __hash__(self):
        return hash(self._d)

    def __float__(self):
        return float(self._d)


# ---------------------------------------------------------------------------
# Object references


class ValueBase(object):
    """Base class of the classes of valuetypes.

    ``_NP_members`` names the state members, those of the base valuetypes
    first. An instance is made with all of them, by position or by name,
    or with none, to set afterwards (as value factories do).
    """

    _NP_RepositoryId = "IDL:omg.org/CORBA/ValueBase:1.0"
    _NP_members: tuple = ()

    def __init__(self, *args, **kw):
        if not args and not kw:
            return
        names = self._NP_members
        if len(args) > len(names):
            raise TypeError(
                "%s takes %d state members, not %d" % (type(self).__name__, len(names), len(args))
            )
        values = dict(zip(names, args))
        for k, v in kw.items():
            if k not in names or k in values:
                raise TypeError("%s: unexpected or repeated member %r" % (type(self).__name__, k))
            values[k] = v
        if len(values) != len(names):
            missing = [n for n in names if n not in values]
            raise TypeError("%s: missing members %s" % (type(self).__name__, ", ".join(missing)))
        for k, v in values.items():
            setattr(self, k, v)

    @_reprlib.recursive_repr()
    def __repr__(self):
        return "%s(%s)" % (
            type(self).__name__,
            ", ".join("%s=%r" % (n, getattr(self, n, None)) for n in self._NP_members),
        )


TC_ValueBase = TypeCode(tk_value, ValueBase._NP_RepositoryId, "ValueBase", members=[], cls=ValueBase)
_tc_ValueBase = TC_ValueBase
# Any abstract interface: a reference or a value.
TC_AbstractBase = TypeCode(tk_abstract_interface, "IDL:omg.org/CORBA/AbstractBase:1.0", "AbstractBase")
_tc_AbstractBase = TC_AbstractBase


class CustomMarshal(object):
    """Base class of the classes of custom valuetypes (``custom
    valuetype``): the implementation writes the state of an instance in
    :meth:`marshal` and reads it back in :meth:`unmarshal`, in the same
    order. A value factory registered for the type makes the instances
    to unmarshal."""

    def marshal(self, os):
        """Write the state to ``os``, a :class:`DataOutputStream`."""
        raise NO_IMPLEMENT(0, COMPLETED_NO)

    def unmarshal(self, is_):
        """Read the state from ``is_``, a :class:`DataInputStream`."""
        raise NO_IMPLEMENT(0, COMPLETED_NO)


def _data_types():
    return [
        ("any", TC_any),
        ("boolean", TC_boolean),
        ("char", TC_char),
        ("wchar", TC_wchar),
        ("octet", TC_octet),
        ("short", TC_short),
        ("ushort", TC_ushort),
        ("long", TC_long),
        ("ulong", TC_ulong),
        ("longlong", TC_longlong),
        ("ulonglong", TC_ulonglong),
        ("float", TC_float),
        ("double", TC_double),
        ("longdouble", TC_longdouble),
    ]


class DataOutputStream(object):
    """What :meth:`CustomMarshal.marshal` writes to (``CORBA::
    DataOutputStream``): each ``write_<type>`` writes one item as CDR, and
    ``write_<type>_array(seq, offset, length)`` writes ``length`` elements
    of ``seq`` from ``offset``, without a length. Values written with
    :meth:`write_Value` keep their sharing with the rest of the message."""

    def __init__(self):
        #: [(TypeCode, value)] written so far.
        self._items = []

    def _write(self, tc, value):
        self._items.append((tc, value))

    def _write_array(self, tc, seq, offset, length):
        if offset < 0 or length < 0 or offset + length > len(seq):
            raise BAD_PARAM(0, COMPLETED_NO)
        for v in seq[offset:offset + length]:
            self._write(tc, v)

    def write_string(self, value):
        self._write(TC_string, value)

    def write_wstring(self, value):
        self._write(TC_wstring, value)

    def write_Object(self, value):
        self._write(TC_Object, value)

    def write_Value(self, value):
        self._write(TC_ValueBase, value)

    def write_Abstract(self, value):
        self._write(TC_AbstractBase, value)

    def write_TypeCode(self, value):
        self._write(TC_TypeCode, value)


class DataInputStream(object):
    """What :meth:`CustomMarshal.unmarshal` reads from (``CORBA::
    DataInputStream``): each ``read_<type>()`` reads one item, and
    ``read_<type>_array(seq, offset, length)`` reads ``length`` elements
    into ``seq`` from ``offset`` and returns the sequence (``seq`` is an
    inout parameter)."""

    def __init__(self, native):
        self._n = native

    def _read_array(self, tc, seq, offset, length):
        if offset < 0 or length < 0:
            raise BAD_PARAM(0, COMPLETED_NO)
        values = [self._n.read(tc) for _ in range(length)]
        if isinstance(seq, (bytes, str)):
            join = bytes(values) if isinstance(seq, bytes) else "".join(values)
            pad = seq[:0] + (b"\0" if isinstance(seq, bytes) else "\0") * max(0, offset - len(seq))
            return seq[:offset] + pad + join + seq[offset + length:]
        seq = list(seq)
        if len(seq) < offset:
            seq.extend([None] * (offset - len(seq)))
        seq[offset:offset + length] = values
        return seq

    def read_string(self):
        return self._n.read(TC_string)

    def read_wstring(self):
        return self._n.read(TC_wstring)

    def read_Object(self):
        return self._n.read(TC_Object)

    def read_Value(self):
        return self._n.read(TC_ValueBase)

    def read_Abstract(self):
        return self._n.read(TC_AbstractBase)

    def read_TypeCode(self):
        return self._n.read(TC_TypeCode)


def _data_methods():
    def writer(tc):
        return lambda self, value: self._write(tc, value)

    def array_writer(tc):
        return lambda self, seq, offset, length: self._write_array(tc, seq, offset, length)

    def reader(tc):
        return lambda self: self._n.read(tc)

    def array_reader(tc):
        return lambda self, seq, offset, length: self._read_array(tc, seq, offset, length)

    for name, tc in _data_types():
        setattr(DataOutputStream, "write_" + name, writer(tc))
        setattr(DataOutputStream, "write_%s_array" % name, array_writer(tc))
        setattr(DataInputStream, "read_" + name, reader(tc))
        setattr(DataInputStream, "read_%s_array" % name, array_reader(tc))


_data_methods()


class Object(object):
    """Base of object references. A nil reference is ``None``.

    References are made by the ORB; the stub classes generated from IDL
    derive from this class.
    """

    _NP_RepositoryId = "IDL:omg.org/CORBA/Object:1.0"
    #: The nil reference, as for the interface classes of generated code.
    _nil = None

    def __init__(self, obj=None):
        self._obj = obj

    def _orb_object(self):
        if self._obj is None:
            raise NO_IMPLEMENT(0, COMPLETED_NO)
        return self._obj

    def _invoke(self, op, args):
        """Call operation ``op`` (an ``Operation`` of the generated code)."""
        return self._orb_object().invoke(op, args)

    def _is_a(self, repoId):
        return self._orb_object().is_a(repoId)

    def _non_existent(self):
        return self._orb_object().non_existent()

    def _is_equivalent(self, other):
        if other is None:
            return False
        return self._orb_object().is_equivalent(other._orb_object())

    def _hash(self, maximum):
        return self._orb_object().hash(maximum)

    def _duplicate(self, obj=None):
        return self

    def _release(self):
        pass

    def _narrow(self, cls):
        """This reference as an instance of the stub class of ``cls`` (an
        interface class of generated code), or None if the object does not
        have that interface."""
        from . import _rt

        return _rt.narrow(self, cls)

    def _unchecked_narrow(self, cls):
        from . import _rt

        return _rt.narrow(self, cls, checked=False)

    def _get_interface(self):
        raise NO_IMPLEMENT(0, COMPLETED_NO)

    # -- the DII --

    def _request(self, operation):
        """A ``CORBA.Request`` for ``operation``, with no arguments and a
        ``void`` result."""
        return Request(self, operation)

    def _create_request(self, ctx, operation, arg_list, result, exceptions=None, ctxlist=None, req_flags=0):
        """``create_request``: a ``CORBA.Request`` for ``operation`` with
        the arguments ``arg_list`` (an NVList, or None), the result
        ``result`` (a NamedValue, or None for ``void``) and the user
        exceptions ``exceptions`` (TypeCodes). Contexts are not supported:
        ``ctx`` and ``ctxlist`` must be None."""
        if ctx is not None or ctxlist:
            raise NO_IMPLEMENT(0, COMPLETED_NO)
        return Request(self, operation, arg_list, result, exceptions or ())

    # -- policies --

    def _get_client_policy(self, type):
        """The policy of type ``type`` for calls on this reference: the
        reference's override, else the calling thread's, else the ORB's;
        None if there is none."""
        value = self._orb_object().client_policy(type)
        return None if value is None else _make_policy(type, value)

    #: rustorb reads no policies from IORs: the effective policy is the
    #: client's.
    _get_policy = _get_client_policy

    def _set_policy_overrides(self, policies, set_add):
        """A new reference to the object with the overrides ``policies``
        (``SET_OVERRIDE`` replaces this reference's, ``ADD_OVERRIDE`` adds
        to them). NO_PERMISSION for a policy that cannot override, such as
        a POA policy."""
        native = self._orb_object().set_policy_overrides(
            _raw_policies(policies), _override_mode(set_add)
        )
        return type(self)(native)

    def _get_policy_overrides(self, types):
        return [_make_policy(t, v) for t, v in self._orb_object().policy_overrides(list(types))]

    def _validate_connection(self):
        """Binds the reference with a LocateRequest: ``(True, [])``, or the
        system exception of the failure."""
        return self._orb_object().validate_connection(), []

    def __repr__(self):
        return "<%s.%s %s>" % (
            type(self).__module__,
            type(self).__name__,
            "nil" if self._obj is None else repr(self._obj.type_id()),
        )


def is_nil(obj):
    return obj is None


# ---------------------------------------------------------------------------
# The DII and the DSI

#: The modes of an argument (``CORBA::Flags``).
ARG_IN = 1
ARG_OUT = 2
ARG_INOUT = 3


class NamedValue(object):
    """``CORBA::NamedValue``: an argument's ``name``, value (``argument``,
    a ``CORBA.Any``) and mode (``arg_modes``, ``ARG_IN``...). The Any of an
    ``out`` argument gives its type; its value comes with the reply."""

    def __init__(self, name, argument, arg_modes):
        self.name = name
        self.argument = argument
        self.arg_modes = arg_modes

    def __repr__(self):
        return "CORBA.NamedValue(%r, %r, %r)" % (self.name, self.argument, self.arg_modes)


class NVList(object):
    """``CORBA::NVList``, a list of named values."""

    def __init__(self, items=()):
        self._items = list(items)

    def count(self):
        return len(self._items)

    def add(self, flags):
        return self.add_item("", flags)

    def add_item(self, item_name, flags):
        nv = NamedValue(item_name, Any(TC_null, None), flags)
        self._items.append(nv)
        return nv

    def add_value(self, item_name, value, flags):
        nv = NamedValue(item_name, value, flags)
        self._items.append(nv)
        return nv

    def item(self, index):
        try:
            return self._items[index]
        except IndexError:
            raise Bounds()

    def remove(self, index):
        try:
            del self._items[index]
        except IndexError:
            raise Bounds()

    def __len__(self):
        return len(self._items)

    def __iter__(self):
        return iter(self._items)

    def __getitem__(self, index):
        return self._items[index]

    def _modes(self):
        for nv in self._items:
            if nv.arg_modes & 3 not in (ARG_IN, ARG_OUT, ARG_INOUT):
                raise BAD_PARAM(0, COMPLETED_NO)


class Bounds(UserException):
    _NP_RepositoryId = "IDL:omg.org/CORBA/Bounds:1.0"


class Request(object):
    """``CORBA::Request``: a call built at run time (the DII), from
    ``obj._request(operation)`` or ``obj._create_request(...)``.

    Arguments are added with ``add_in_arg(value)``, ``add_inout_arg(value)``
    (``CORBA.Any`` values) and ``add_out_arg(tc)``, or are the NVList given to
    ``_create_request``; ``set_return_type(tc)`` gives the result type
    (``void`` by default) and ``add_exception(tc)`` the user exceptions it
    may raise. After ``invoke()`` (or ``send_deferred()`` and
    ``get_response()``) the ``out`` and ``inout`` arguments hold the
    values sent back and ``return_value()`` the result. A listed user
    exception is raised; an unlisted one is ``UNKNOWN``.
    """

    def __init__(self, target, operation, arguments=None, result=None, exceptions=()):
        self._target = target
        self._operation = operation
        self._arguments = arguments if arguments is not None else NVList()
        self._result = result if result is not None else NamedValue("", Any(TC_void, None), 0)
        self._exceptions = list(exceptions)
        self._native = _native_module().Request(target._orb_object(), operation)

    def target(self):
        return self._target

    def operation(self):
        return self._operation

    def arguments(self):
        return self._arguments

    def result(self):
        return self._result

    def exceptions(self):
        return list(self._exceptions)

    def ctx(self, *args):
        raise NO_IMPLEMENT(0, COMPLETED_NO)

    contexts = ctx

    def add_in_arg(self, value, name=""):
        self._arguments.add_value(name, value, ARG_IN)

    def add_inout_arg(self, value, name=""):
        self._arguments.add_value(name, value, ARG_INOUT)

    def add_out_arg(self, tc, name=""):
        self._arguments.add_value(name, Any(tc, None), ARG_OUT)

    def set_return_type(self, tc):
        self._result.argument = Any(tc, None)

    def add_exception(self, tc):
        self._exceptions.append(tc)

    def return_value(self):
        return self._result.argument

    def _call_args(self):
        self._arguments._modes()
        return (
            [(nv.name, nv.argument, nv.arg_modes & 3) for nv in self._arguments],
            self._result.argument.typecode(),
            list(self._exceptions),
        )

    def _collect(self, outputs):
        result, values = outputs
        for nv, value in zip(self._arguments, values):
            if nv.arg_modes & 3 != ARG_IN and value is not None:
                nv.argument = value
        self._result.argument = result

    def _new_native(self):
        # A new native request for each call: the exception list may grow.
        self._native = _native_module().Request(self._target._orb_object(), self._operation)
        return self._native

    def invoke(self, invoke_flags=0):
        self._collect(self._new_native().invoke(*self._call_args()))

    def send_oneway(self):
        self._new_native().send_oneway(*self._call_args())

    def send_deferred(self):
        self._new_native().send_deferred(*self._call_args())

    def send(self, invoke_flags=0):
        self.send_deferred()

    def poll_response(self):
        return self._native.poll_response()

    def get_response(self, response_flags=0):
        self._collect(self._native.get_response())


class ServerRequest(object):
    """``CORBA::ServerRequest``: a request to a
    ``PortableServer.DynamicImplementation``, given to its ``invoke``.

    ``arguments(nvlist)`` reads the ``in`` and ``inout`` arguments into the
    NVList, whose TypeCodes and modes describe the parameters (the Anys'
    values are ignored). The servant then sets the ``argument`` of the
    ``out`` and ``inout`` items, and calls ``set_result(any)`` or
    ``set_exception(any)`` (an Any of a user exception). A system
    exception is raised from ``invoke``.
    """

    def __init__(self, native):
        self._native = native
        self._params = None
        self._raised = False

    def operation(self):
        return self._native.operation()

    def arguments(self, nvlist):
        if self._params is not None:
            raise BAD_INV_ORDER(0, COMPLETED_NO)
        nvlist._modes()
        values = self._native.arguments(
            [(nv.name, nv.argument.typecode(), nv.arg_modes & 3) for nv in nvlist]
        )
        self._unset = []
        for nv, value in zip(nvlist, values):
            if value is not None:
                nv.argument = value
            self._unset.append(nv.argument if value is None else None)
        self._params = nvlist
        return nvlist

    def set_result(self, value):
        self._native.set_result(value)

    def set_exception(self, value):
        self._native.set_exception(value)
        self._raised = True

    def ctx(self):
        raise NO_IMPLEMENT(0, COMPLETED_NO)

    def _NP_finish(self):
        if self._params is None or self._raised:
            return
        # An out argument the servant did not set (still the Any given to
        # arguments()) is left so: the reply then fails with BAD_PARAM.
        self._native.set_out_values(
            [
                None if nv.arg_modes & 3 == ARG_IN or nv.argument is unset else nv.argument
                for nv, unset in zip(self._params, self._unset)
            ]
        )


class LocalObject(Object):
    """Base of local objects (the POA, its manager, policies): they live in
    this process only and have no IOR."""

    def _is_a(self, repoId):
        return repoId == "IDL:omg.org/CORBA/Object:1.0" or any(
            getattr(c, "_NP_RepositoryId", None) == repoId for c in type(self).__mro__
        )

    def _non_existent(self):
        return False

    def _is_equivalent(self, other):
        return self is other

    def _hash(self, maximum):
        return hash(self) % maximum

    def _narrow(self, cls):
        return self if isinstance(self, cls) else None

    _unchecked_narrow = _narrow


class Policy(LocalObject):
    """``CORBA::Policy``. The POA's ``create_*_policy`` operations make
    them; ``value`` is the enumerator of the policy value."""

    _NP_RepositoryId = "IDL:omg.org/CORBA/Policy:1.0"
    policy_type = 0

    def __init__(self, value):
        LocalObject.__init__(self)
        self.value = value

    def _get_policy_type(self):
        return self.policy_type

    def _get_value(self):
        return self.value

    def copy(self):
        return type(self)(self.value)

    def destroy(self):
        pass

    def _rorb_raw(self):
        """The type and value of the policy for the native module."""
        return self.policy_type, int(self.value)


#: CORBA::PolicyErrorCode
BAD_POLICY = 0
UNSUPPORTED_POLICY = 1
BAD_POLICY_TYPE = 2
BAD_POLICY_VALUE = 3
UNSUPPORTED_POLICY_VALUE = 4


class PolicyError(UserException):
    _NP_RepositoryId = "IDL:omg.org/CORBA/PolicyError:1.0"
    _NP_members = ("reason",)


class InvalidPolicies(UserException):
    _NP_RepositoryId = "IDL:omg.org/CORBA/InvalidPolicies:1.0"
    _NP_members = ("indices",)


_override = _types.Enum("IDL:omg.org/CORBA/SetOverrideType:1.0", ("SET_OVERRIDE", "ADD_OVERRIDE"))
SetOverrideType = _override
SET_OVERRIDE, ADD_OVERRIDE = _override._items


def _override_mode(set_add):
    """True for ADD_OVERRIDE."""
    if set_add is ADD_OVERRIDE:
        return True
    if set_add is SET_OVERRIDE:
        return False
    raise BAD_PARAM(0, COMPLETED_NO)


def _raw_policies(policies):
    raw = []
    for p in policies:
        if not isinstance(p, Policy):
            raise BAD_PARAM(0, COMPLETED_NO)
        raw.append(p._rorb_raw())
    return raw


def _make_policy(type, value):
    """The policy object of a type and value from the native module."""
    if type == 32:
        from . import Messaging

        return Messaging.RelativeRoundtripTimeoutPolicy(value)
    from . import PortableServer

    return PortableServer._make_policy(type, value)


def _native_module():
    if _native is None:
        raise INITIALIZE(0, COMPLETED_NO)
    return _native


class PolicyManager(LocalObject):
    """The policy overrides of the ORB (initial reference
    ``ORBPolicyManager``). ``set_policy_overrides`` raises NO_PERMISSION
    for a policy that cannot override, BAD_PARAM for two of one type."""

    _NP_RepositoryId = "IDL:omg.org/CORBA/PolicyManager:1.0"

    def __init__(self, native):
        LocalObject.__init__(self)
        self._native = native

    def get_policy_overrides(self, ts):
        return [_make_policy(t, v) for t, v in self._native.policy_overrides(list(ts))]

    def set_policy_overrides(self, policies, set_add):
        self._native.set_policy_overrides(_raw_policies(policies), _override_mode(set_add))


class Current(LocalObject):
    _NP_RepositoryId = "IDL:omg.org/CORBA/Current:1.0"


class PolicyCurrent(PolicyManager, Current):
    """The policy overrides of the calling thread (initial reference
    ``PolicyCurrent``), for every ORB."""

    _NP_RepositoryId = "IDL:omg.org/CORBA/PolicyCurrent:1.0"

    def __init__(self):
        LocalObject.__init__(self)

    def get_policy_overrides(self, ts):
        native = _native_module()
        return [_make_policy(t, v) for t, v in native.thread_policy_overrides(list(ts))]

    def set_policy_overrides(self, policies, set_add):
        _native_module().set_thread_policy_overrides(
            _raw_policies(policies), _override_mode(set_add)
        )


class ServiceDetail(_types.Struct):
    _NP_RepositoryId = "IDL:omg.org/CORBA/ServiceDetail:1.0"
    _NP_members = ("service_detail_type", "service_detail")


class ServiceInformation(_types.Struct):
    _NP_RepositoryId = "IDL:omg.org/CORBA/ServiceInformation:1.0"
    _NP_members = ("service_options", "service_details")


# ---------------------------------------------------------------------------
# The ORB

#: The ORB identifier of ``ORB_init``.
ORB_ID = "rustorb"

_orbs: dict = {}
_orbs_lock = _threading.Lock()


class ORB(object):
    """``CORBA::ORB``, made by :func:`ORB_init`."""

    _NP_RepositoryId = "IDL:omg.org/CORBA/ORB:1.0"

    class InvalidName(UserException):
        _NP_RepositoryId = "IDL:omg.org/CORBA/ORB/InvalidName:1.0"

    def __init__(self, native, orb_id):
        self._native = native
        self._id = orb_id
        self._root_poa = None
        self._ins_poa = None
        self._lock = _threading.Lock()
        self._destroyed = False
        # Requests of send_multiple_requests_deferred not yet collected.
        self._deferred = []

    def id(self):
        return self._id

    # -- the DII --

    def create_list(self, count):
        """An empty ``CORBA.NVList`` (``count`` is only a hint)."""
        return NVList()

    def create_operation_list(self, oper):
        raise NO_IMPLEMENT(0, COMPLETED_NO)

    def create_named_value(self, name, value, flags):
        return NamedValue(name, value, flags)

    def create_exception_list(self):
        return []

    def send_multiple_requests_oneway(self, requests):
        for r in requests:
            r.send_oneway()

    def send_multiple_requests_deferred(self, requests):
        with self._lock:
            for r in requests:
                r.send_deferred()
                self._deferred.append(r)

    def poll_next_response(self):
        """Whether a response to ``send_multiple_requests_deferred`` has
        arrived."""
        with self._lock:
            return any(r.poll_response() for r in self._deferred)

    def get_next_response(self):
        """The next request of ``send_multiple_requests_deferred`` whose
        response has arrived; waits for one. Its ``get_response()`` then
        takes the response at once (raising its exception, if any).
        ``BAD_INV_ORDER`` when none is outstanding."""
        while True:
            with self._lock:
                if not self._deferred:
                    raise BAD_INV_ORDER(0, COMPLETED_NO)
                for i, r in enumerate(self._deferred):
                    if r.poll_response():
                        del self._deferred[i]
                        break
                else:
                    r = None
            if r is not None:
                return r
            _time.sleep(0.001)

    def string_to_object(self, s):
        from . import _rt

        native = self._native.string_to_object(s)
        if native is None:
            return None
        return _rt.make_objref(native, Object._NP_RepositoryId)

    def object_to_string(self, obj):
        if obj is None:
            return self._native.object_to_string(None)
        return self._native.object_to_string(obj._orb_object())

    def resolve_initial_references(self, identifier):
        from . import _rt

        if identifier == "RootPOA":
            return self._get_root_poa()
        if identifier == "POACurrent":
            from . import PortableServer

            return PortableServer.Current()
        if identifier == "omniINSPOA":
            return self._get_ins_poa()
        if identifier == "ORBPolicyManager":
            return PolicyManager(self._native)
        if identifier == "PolicyCurrent":
            return PolicyCurrent()
        if identifier == "DynAnyFactory":
            from . import DynamicAny

            return DynamicAny.DynAnyFactory()
        local = self._native.resolve_local_initial_reference(identifier)
        if local is not None:
            return local
        try:
            native = self._native.resolve_initial_references(identifier)
        except LookupError:
            raise ORB.InvalidName()
        return _rt.make_objref(native, Object._NP_RepositoryId)

    #: The names resolve_initial_references answers without the core.
    _LOCAL_SERVICES = (
        "RootPOA",
        "POACurrent",
        "omniINSPOA",
        "ORBPolicyManager",
        "PolicyCurrent",
        "DynAnyFactory",
    )

    def list_initial_services(self):
        return list(self._LOCAL_SERVICES) + list(self._native.list_initial_services())

    def register_initial_reference(self, identifier, obj):
        """Makes ``resolve_initial_references(identifier)`` return
        ``obj``, a reference or a local object. InvalidName for an empty
        name or one that is already an initial reference, BAD_PARAM (minor
        27) for nil."""
        if identifier in self._LOCAL_SERVICES:
            raise ORB.InvalidName()
        try:
            if isinstance(obj, Object) and obj._obj is not None:
                self._native.register_initial_reference(identifier, obj._obj)
            elif obj is None:
                self._native.register_initial_reference(identifier, None)
            else:
                self._native.register_local_initial_reference(identifier, obj)
        except LookupError:
            raise ORB.InvalidName()

    def get_service_information(self, service_type):
        """rustorb offers no services (such as security):
        ``(False, ServiceInformation)``."""
        return False, ServiceInformation([], [])

    def create_policy(self, type, val):
        """A POA policy from its enumerator, or a
        ``Messaging.RelativeRoundtripTimeoutPolicy`` from a
        ``TimeBase::TimeT`` (unsigned long long, in units of 100 ns), in
        the any ``val``. PolicyError: BAD_POLICY for a type rustorb does not
        know, BAD_POLICY_TYPE for an any of the wrong type,
        BAD_POLICY_VALUE for an enumerator out of range."""
        if not isinstance(val, Any):
            raise BAD_PARAM(0, COMPLETED_NO)
        value, reason = self._native.create_policy(type, val)
        if reason is not None:
            raise PolicyError(reason)
        return _make_policy(type, value)

    def _get_root_poa(self):
        with self._lock:
            if self._root_poa is None:
                from . import PortableServer

                self._root_poa = PortableServer._wrap_poa(self._native.root_poa())
            return self._root_poa

    def _get_ins_poa(self):
        # omniORB's "omniINSPOA" (OpenRTM-aist uses it): a child of the
        # RootPOA with its own manager, where references have the object
        # id as their object key, for corbaloc::host:port/ID. The keys
        # stay the same across restarts whatever the lifespan policy, so
        # it is TRANSIENT, which needs no fixed port.
        root = self._get_root_poa()
        with self._lock:
            if self._ins_poa is None:
                from . import PortableServer

                policy = root.create_id_assignment_policy(PortableServer.USER_ID)
                native = root._native.create_poa(
                    "omniINSPOA", None, [(policy.policy_type, int(policy.value))]
                )
                self._ins_poa = PortableServer._INSPOA(native, self._native)
            return self._ins_poa

    def run(self):
        self._native.run()

    def work_pending(self):
        return self._native.work_pending()

    def perform_work(self):
        self._native.perform_work()

    def shutdown(self, wait_for_completion):
        self._native.shutdown(bool(wait_for_completion))

    def destroy(self):
        with _orbs_lock:
            if _orbs.get(self._id) is self:
                del _orbs[self._id]
        self._destroyed = True
        self._native.destroy()

    # -- TypeCodes --

    def create_struct_tc(self, id, name, members):
        return _create_struct_like(tk_struct, id, name, members)

    def create_exception_tc(self, id, name, members):
        return _create_struct_like(tk_except, id, name, members)

    def create_union_tc(self, id, name, discriminator_type, members):
        from . import _rt

        labels, ms, default = [], [], -1
        for i, m in enumerate(members):
            m_name, label, m_type = _member_fields(m, 3)
            if isinstance(label, Any) and label._t._k is tk_octet and label._v == 0:
                if default >= 0:
                    raise BAD_PARAM(0, COMPLETED_NO)
                default = i
                labels.append(None)
            else:
                labels.append(label._v if isinstance(label, Any) else label)
            ms.append((m_name, m_type))
        tc = TypeCode(
            tk_union,
            id,
            name,
            members=ms,
            labels=labels,
            discriminator=discriminator_type,
            default=default,
        )
        _resolve_recursive(tc, id)
        _rt.make_class(tc)
        return tc

    def create_enum_tc(self, id, name, members):
        from . import _rt

        tc = TypeCode(tk_enum, id, name, members=list(members))
        _rt.make_class(tc)
        return tc

    def create_alias_tc(self, id, name, original_type):
        return TypeCode(tk_alias, id, name, content=original_type)

    def create_value_box_tc(self, id, name, boxed_type):
        return TypeCode(tk_value_box, id, name, content=boxed_type)

    def create_value_tc(self, id, name, type_modifier, concrete_base, members):
        """A valuetype TypeCode. ``members`` are :class:`ValueMember`
        (or ``(name, type, access)`` tuples); ``concrete_base`` may be None
        or ``TC_null``."""
        from . import _rt

        if concrete_base is not None and concrete_base._k is tk_null:
            concrete_base = None
        tc = TypeCode(
            tk_value,
            id,
            name,
            modifier=type_modifier,
            base=concrete_base,
            members=[
                (m.name, m.type, m.access) if isinstance(m, ValueMember) else tuple(m)[:3]
                for m in members
            ],
        )
        _resolve_recursive(tc, id)
        _rt.make_class(tc)
        return tc

    # -- value factories --

    def register_value_factory(self, id, factory):
        """Registers ``factory`` (a callable without arguments that
        returns a new instance) for the valuetype ``id``; returns the
        factory it replaces, or None."""
        from . import _rt

        return _rt.register_value_factory(id, factory)

    def unregister_value_factory(self, id):
        from . import _rt

        if _rt.unregister_value_factory(id) is None:
            raise BAD_PARAM(0, COMPLETED_NO)

    def lookup_value_factory(self, id):
        from . import _rt

        return _rt.lookup_value_factory(id)

    def create_interface_tc(self, id, name):
        return TypeCode(tk_objref, id, name)

    def create_abstract_interface_tc(self, id, name):
        return TypeCode(tk_abstract_interface, id, name)

    def create_local_interface_tc(self, id, name):
        return TypeCode(tk_local_interface, id, name)

    def create_native_tc(self, id, name):
        return TypeCode(tk_native, id, name)

    def create_string_tc(self, bound):
        return TypeCode(tk_string, length=bound)

    def create_wstring_tc(self, bound):
        return TypeCode(tk_wstring, length=bound)

    def create_fixed_tc(self, digits, scale):
        return TypeCode(tk_fixed, digits=digits, scale=scale)

    def create_sequence_tc(self, bound, element_type):
        return TypeCode(tk_sequence, content=element_type, length=bound)

    def create_array_tc(self, length, element_type):
        return TypeCode(tk_array, content=element_type, length=length)

    def create_recursive_tc(self, id):
        """A placeholder for the struct or union ``id``, to use in its member
        types; ``create_struct_tc`` and ``create_union_tc`` replace it."""
        tc = TypeCode(tk_null, id)
        tc._recursive = True
        return tc

    def __repr__(self):
        return "<CORBA.ORB %r>" % self._id


class StructMember(_types.Struct):
    """A member for ``ORB.create_struct_tc``: ``name``, ``type`` and
    ``type_def`` (unused, may be None)."""

    _NP_RepositoryId = "IDL:omg.org/CORBA/StructMember:1.0"
    _NP_members = ("name", "type", "type_def")

    def __init__(self, name, type, type_def=None):  # noqa: A002
        self.name = name
        self.type = type
        self.type_def = type_def


class UnionMember(_types.Struct):
    """A member for ``ORB.create_union_tc``: ``name``, ``label`` (an Any;
    the octet 0 for the default member), ``type`` and ``type_def``."""

    _NP_RepositoryId = "IDL:omg.org/CORBA/UnionMember:1.0"
    _NP_members = ("name", "label", "type", "type_def")

    def __init__(self, name, label, type, type_def=None):  # noqa: A002
        self.name = name
        self.label = label
        self.type = type
        self.type_def = type_def


class ValueMember(_types.Struct):
    """A member for ``ORB.create_value_tc``."""

    _NP_RepositoryId = "IDL:omg.org/CORBA/ValueMember:1.0"
    _NP_members = ("name", "id", "defined_in", "version", "type", "type_def", "access")

    def __init__(
        self, name, id, defined_in, version, type, type_def=None, access=PUBLIC_MEMBER
    ):  # noqa: A002
        self.name = name
        self.id = id
        self.defined_in = defined_in
        self.version = version
        self.type = type
        self.type_def = type_def
        self.access = access


def _member_fields(m, n):
    """(name, type) of a StructMember, (name, label, type) of a
    UnionMember, or the same from a tuple."""
    if isinstance(m, StructMember):
        return m.name, m.type
    if isinstance(m, UnionMember):
        return m.name, m.label, m.type
    return tuple(m)[:n]


def _create_struct_like(kind, id, name, members):
    from . import _rt

    tc = TypeCode(kind, id, name, members=[_member_fields(m, 2) for m in members])
    _resolve_recursive(tc, id)
    _rt.make_class(tc)
    return tc


def _resolve_recursive(tc, id):
    """Replace the placeholders of ``create_recursive_tc(id)`` in the member
    types of ``tc`` by ``tc``."""
    seen = set()

    def fix(t):
        if getattr(t, "_recursive", False) and t._id == id:
            return tc
        if id_(t) in seen:
            return t
        seen.add(id_(t))
        if t._content is not None:
            t._content = fix(t._content)
        if t._disc is not None:
            t._disc = fix(t._disc)
        if t._base is not None:
            t._base = fix(t._base)
        if t._members is not None and t._k is not tk_enum:
            t._members = [(m[0], fix(m[1])) + tuple(m[2:]) for m in t._members]
        return t

    id_ = _builtins.id
    seen.add(id_(tc))
    if tc._members is not None:
        tc._members = [(m[0], fix(m[1])) + tuple(m[2:]) for m in tc._members]


def ORB_init(argv=None, orb_identifier=ORB_ID):
    """Initialises the ORB ``orb_identifier``, or returns it if it exists.

    ORB options (``-ORBInitRef``, ``-ORBendPoint``...) are taken from
    ``argv`` and removed from it when it is a list.
    """
    if _native is None:
        raise INITIALIZE(0, COMPLETED_NO)
    if argv is None:
        argv = []
    with _orbs_lock:
        orb = _orbs.get(orb_identifier)
        if orb is not None:
            return orb
        native, rest = _native.Orb.init([str(a) for a in argv])
        if isinstance(argv, list):
            argv[:] = rest
        orb = ORB(native, orb_identifier)
        _orbs[orb_identifier] = orb
    from . import _rt

    _rt.flush_values()
    return orb


def _default_orb():
    """The first ORB that was initialised and is not destroyed."""
    with _orbs_lock:
        for orb in _orbs.values():
            return orb
    raise BAD_INV_ORDER(0, COMPLETED_NO)


@_atexit.register
def _destroy_orbs():
    # Servants run on ORB threads; finish their requests while the
    # interpreter can still run them.
    with _orbs_lock:
        orbs = list(_orbs.values())
    for orb in orbs:
        try:
            orb.destroy()
        except Exception:  # already destroyed
            pass

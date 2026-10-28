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
import threading as _threading
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

# ---------------------------------------------------------------------------
# Exceptions


class Exception(_builtins.Exception):  # noqa: A001 - the name is fixed by the mapping
    """Base of every CORBA exception."""

    _NP_RepositoryId: Optional[str] = None


class UserException(Exception):
    """Base of the exceptions declared in IDL."""

    _NP_members = ()

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
        self._need(tk_struct, tk_union, tk_enum, tk_except)
        return len(self._members)

    def _member(self, index):
        self._need(tk_struct, tk_union, tk_enum, tk_except)
        if not 0 <= index < len(self._members):
            raise TypeCode.Bounds()
        return self._members[index]

    def member_name(self, index):
        m = self._member(index)
        return m if self._k is tk_enum else m[0]

    def member_type(self, index):
        self._need(tk_struct, tk_union, tk_except)
        return self._member(index)[1]

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
        tk_native,
        tk_abstract_interface,
        tk_local_interface,
    ):
        if a._id != b._id or a._name != b._name:
            return False
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


class Object(object):
    """Base of object references. A nil reference is ``None``.

    References are made by the ORB; the stub classes generated from IDL
    derive from this class.
    """

    _NP_RepositoryId = "IDL:omg.org/CORBA/Object:1.0"

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

    def __repr__(self):
        return "<%s.%s %s>" % (
            type(self).__module__,
            type(self).__name__,
            "nil" if self._obj is None else repr(self._obj.type_id()),
        )


def is_nil(obj):
    return obj is None


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
        self._lock = _threading.Lock()
        self._destroyed = False

    def id(self):
        return self._id

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
        try:
            native = self._native.resolve_initial_references(identifier)
        except LookupError:
            raise ORB.InvalidName()
        return _rt.make_objref(native, Object._NP_RepositoryId)

    def list_initial_services(self):
        return ["RootPOA", "POACurrent"] + list(self._native.list_initial_services())

    def _get_root_poa(self):
        with self._lock:
            if self._root_poa is None:
                from . import PortableServer

                self._root_poa = PortableServer._wrap_poa(self._native.root_poa())
            return self._root_poa

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
        if t._members is not None and t._k is not tk_enum:
            t._members = [(n, fix(mt)) for n, mt in t._members]
        return t

    id_ = _builtins.id
    seen.add(id_(tc))
    if tc._members is not None:
        tc._members = [(n, fix(mt)) for n, mt in tc._members]


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

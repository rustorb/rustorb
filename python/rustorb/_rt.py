"""Support for the code generated from IDL by ``rustorb-idl -bpython``.

Generated code calls only the functions and classes of this module, of
:mod:`rustorb.CORBA` and of :mod:`rustorb.PortableServer`. The interface
between the generator and this module is versioned by
:data:`GENERATED_CODE_VERSION`: generated code calls :func:`check_version`
first, and a module generated for another version refuses to load.
"""

import keyword
import sys
import types
from typing import Optional

from . import CORBA
from . import PortableServer
from ._types import _NO_DEFAULT, Enum, EnumItem, Struct, Union  # noqa: F401 (used by generated code)

#: Version of the interface between generated code and this module.
GENERATED_CODE_VERSION = 1


def check_version(version):
    if version != GENERATED_CODE_VERSION:
        raise ImportError(
            "this module was generated for version %r of the rustorb runtime "
            "interface, but rustorb provides version %r; regenerate it with "
            "the rustorb-idl of this rustorb" % (version, GENERATED_CODE_VERSION)
        )


# ---------------------------------------------------------------------------
# Modules
#
# The definitions of an IDL module may come from several IDL files, and the
# Python package of the module (``Module/__init__.py``) imports the
# ``*_idl.py`` file of each. A ``*_idl.py`` file puts its definitions into
# the module object that open_module() returns: the package once it is
# imported, a stand-in module until then (bind_package() moves the names
# over).

_modules: dict = {}


def open_module(name, source=None):
    """The module object of the IDL module (or ``__POA`` module) ``name``
    (``"Outer.Inner"`` for a nested module)."""
    mod = _modules.get(name)
    if mod is None:
        mod = sys.modules.get(name)
        if mod is None or not getattr(mod, "_NP_rustorb_package", False):
            mod = types.ModuleType(name)
            mod.__file__ = source
        _modules[name] = mod
        _attach(name, mod)
    return mod


def bind_package(package):
    """Make the Python package ``package`` (a module object, from the
    package's ``__init__.py``) the module object of its IDL module."""
    name = package.__name__
    package._NP_rustorb_package = True
    old = _modules.get(name)
    if old is not None and old is not package:
        for k, v in vars(old).items():
            if not k.startswith("__"):
                setattr(package, k, v)
    _modules[name] = package
    _attach(name, package)


def _attach(name, mod):
    parent, _, child = name.rpartition(".")
    if parent:
        setattr(open_module(parent), child, mod)


# ---------------------------------------------------------------------------
# TypeCodes

#: Repository ID -> TypeCode of the types defined by generated code.
typecodes: dict = {}


def _named(kind, repoId, name, **params):
    tc = typecodes.get(repoId)
    if tc is not None and tc._k is kind:
        return tc
    tc = CORBA.TypeCode(kind, repoId, name, **params)
    typecodes[repoId] = tc
    return tc


def tc_string(bound=0):
    return CORBA.TC_string if bound == 0 else CORBA.TypeCode(CORBA.tk_string, length=bound)


def tc_wstring(bound=0):
    return CORBA.TC_wstring if bound == 0 else CORBA.TypeCode(CORBA.tk_wstring, length=bound)


def tc_sequence(content, bound=0):
    return CORBA.TypeCode(CORBA.tk_sequence, content=content, length=bound)


def tc_array(content, length):
    return CORBA.TypeCode(CORBA.tk_array, content=content, length=length)


def tc_fixed(digits, scale):
    return CORBA.TypeCode(CORBA.tk_fixed, digits=digits, scale=scale)


def tc_alias(repoId, name, content):
    return _named(CORBA.tk_alias, repoId, name, content=content)


def tc_objref(repoId, name):
    return _named(CORBA.tk_objref, repoId, name)


def tc_abstract_interface(repoId, name):
    return _named(CORBA.tk_abstract_interface, repoId, name)


def tc_local_interface(repoId, name):
    return _named(CORBA.tk_local_interface, repoId, name)


def tc_native(repoId, name):
    return _named(CORBA.tk_native, repoId, name)


def tc_enum(repoId, name, enum):
    return _named(CORBA.tk_enum, repoId, name, members=[i._n for i in enum._items], cls=enum)


def declare_struct(repoId, name):
    """The TypeCode of a struct, before its members are known (for
    recursive types and forward declarations; see :func:`define_struct`)."""
    return _named(CORBA.tk_struct, repoId, name)


def declare_union(repoId, name):
    return _named(CORBA.tk_union, repoId, name)


def declare_except(repoId, name):
    return _named(CORBA.tk_except, repoId, name)


def define_struct(tc, cls, members):
    """Fill the TypeCode of a struct or exception: ``members`` is
    ``[(name, TypeCode)]`` in order."""
    tc._members = list(members)
    tc._class = cls
    return tc


define_except = define_struct


def define_union(tc, cls, discriminator, cases, default_index):
    """Fill the TypeCode of a union. ``cases`` is ``[(label, name,
    TypeCode)]``, one entry for each label as in the TypeCode (the label of
    the ``default`` entry is ignored); ``default_index`` is the position of
    the ``default`` entry, or -1."""
    tc._disc = discriminator
    tc._labels = [label for label, _, _ in cases]
    tc._members = [(name, t) for _, name, t in cases]
    tc._default = default_index
    tc._class = cls
    return tc


# ---------------------------------------------------------------------------
# Types made at run time
#
# TypeCodes that arrive in an ``any`` (and those of ``ORB.create_*_tc``)
# describe types the generated code may not define. Values of a struct,
# union, exception or enum need a class, so one is made from the TypeCode.

#: Repository ID -> TypeCode of the types made at run time (the last one
#: for each id), so that the same type gets the same class each time it
#: arrives.
dynamic_types: dict = {}

_INT_RANGES = {
    CORBA.tk_short: (-(2**15), 2**15 - 1),
    CORBA.tk_ushort: (0, 2**16 - 1),
    CORBA.tk_long: (-(2**31), 2**31 - 1),
    CORBA.tk_ulong: (0, 2**32 - 1),
    CORBA.tk_longlong: (-(2**63), 2**63 - 1),
    CORBA.tk_ulonglong: (0, 2**64 - 1),
}


def _py_name(name):
    if keyword.iskeyword(name):
        return "_" + name
    return name


def _class_name(tc, default):
    name = tc._name or default
    return name if name.isidentifier() else default


def make_class(tc):
    """Make and set the class of the values of ``tc`` (a struct, union,
    exception or enum TypeCode whose parameters are set)."""
    k = tc._k
    if k is CORBA.tk_enum:
        tc._class = Enum(tc._id, list(tc._members))
    elif k in (CORBA.tk_struct, CORBA.tk_except):
        attrs = {
            "_NP_RepositoryId": tc._id,
            "_NP_members": tuple(_py_name(n) for n, _ in tc._members),
        }
        base = CORBA.UserException if k is CORBA.tk_except else Struct
        tc._class = type(_class_name(tc, "Struct"), (base,), attrs)
    elif k is CORBA.tk_union:
        tc._class = _union_class(tc)
    if tc._id:
        dynamic_types[tc._id] = tc
    return tc._class


def _union_class(tc):
    d_to_m = {}
    m_to_d = {}
    def_m = None
    for i, (name, _) in enumerate(tc._members):
        attr = _py_name(name)
        if i == tc._default:
            def_m = attr
            continue
        label = tc._labels[i]
        d_to_m[label] = attr
        m_to_d[attr] = label  # the last label, as generated code does
    attrs = {
        "_NP_RepositoryId": tc._id,
        "_d_to_m": d_to_m,
        "_m_to_d": m_to_d,
        "_def_m": def_m,
        "_def_d": _unused_label(CORBA._unalias(tc._disc), set(d_to_m)),
    }
    return type(_class_name(tc, "Union"), (Union,), attrs)


def _unused_label(disc, used):
    """The first discriminator value no label uses, as generated code
    chooses it, or ``_NO_DEFAULT``."""
    k = disc._k
    if k in _INT_RANGES:
        lo, hi = _INT_RANGES[k]
        v = lo
        while v in used and v <= hi:
            v += 1
        return v if v <= hi else _NO_DEFAULT
    if k is CORBA.tk_boolean:
        candidates = (False, True)
    elif k in (CORBA.tk_char, CORBA.tk_wchar):
        candidates = (chr(c) for c in range(0, 256 if k is CORBA.tk_char else 0x10000))
    elif k is CORBA.tk_enum:
        candidates = disc._class._items
    else:
        return _NO_DEFAULT
    for v in candidates:
        if v not in used:
            return v
    return _NO_DEFAULT


def label_value(disc, n):
    """The discriminator value of the union label ``n`` (a number, as the
    native module gives it) for the discriminator type ``disc``."""
    d = CORBA._unalias(disc)
    if d._k is CORBA.tk_boolean:
        return bool(n)
    if d._k in (CORBA.tk_char, CORBA.tk_wchar):
        return chr(n)
    if d._k is CORBA.tk_enum:
        return d._class._items[n]
    return n


_NAMED = (
    CORBA.tk_objref,
    CORBA.tk_struct,
    CORBA.tk_union,
    CORBA.tk_enum,
    CORBA.tk_alias,
    CORBA.tk_except,
    CORBA.tk_value_box,
    CORBA.tk_native,
    CORBA.tk_abstract_interface,
    CORBA.tk_local_interface,
)
_WITH_CLASS = (CORBA.tk_struct, CORBA.tk_union, CORBA.tk_except, CORBA.tk_enum)


def typecode_from_nodes(nodes):
    """The TypeCode of a list of nodes the native module made from a
    TypeCode it received: tuples that start with the kind and refer to
    other nodes by index; the first node is the TypeCode. Named types that
    generated code (or an earlier call) defines with an equal TypeCode are
    that TypeCode; the others get new classes."""
    n = len(nodes)
    tcs = []
    for node in nodes:
        k = CORBA.TCKind._items[node[0]] if node[0] < len(CORBA.TCKind._items) else None
        simple = CORBA._SIMPLE.get(node[0])
        if simple is not None:
            tcs.append(simple)
        elif k is None:
            # A kind this mapping has no name for (component, home, event).
            tcs.append(CORBA.TypeCode(CORBA.tk_null))
        else:
            tcs.append(CORBA.TypeCode(k))
    final = list(tcs)

    def fill(i):
        node = nodes[i]
        tc = tcs[i]
        k = tc._k
        if k in (CORBA.tk_string, CORBA.tk_wstring):
            tc._length = node[1]
        elif k is CORBA.tk_fixed:
            tc._digits, tc._scale = node[1], node[2]
        elif k in (CORBA.tk_sequence, CORBA.tk_array):
            tc._content, tc._length = final[node[1]], node[2]
        elif k in (CORBA.tk_alias, CORBA.tk_value_box):
            tc._id, tc._name, tc._content = node[1], node[2], final[node[3]]
        elif k is CORBA.tk_enum:
            tc._id, tc._name, tc._members = node[1], node[2], list(node[3])
        elif k in (CORBA.tk_struct, CORBA.tk_except):
            tc._id, tc._name = node[1], node[2]
            tc._members = [(m, final[t]) for m, t in node[3]]
        elif k is CORBA.tk_union:
            tc._id, tc._name = node[1], node[2]
            tc._disc = final[node[3]]
            tc._default = node[4]
            tc._members = [(m, final[t]) for _, m, t in node[5]]
            tc._labels = [
                None if label is None else label_value(tc._disc, label)
                for label, _, _ in node[5]
            ]
        elif len(node) == 3:
            tc._id, tc._name = node[1], node[2]

    fresh = [i for i in range(n) if tcs[i] is not CORBA._SIMPLE.get(nodes[i][0])]
    # Enum discriminators must have their class before the labels are made.
    order = sorted(fresh, key=lambda i: tcs[i]._k is not CORBA.tk_enum)
    for i in order:
        fill(i)
        if tcs[i]._k is CORBA.tk_enum:
            make_class(tcs[i])
    # Named types defined elsewhere with an equal TypeCode are replaced.
    for i in fresh:
        tc = tcs[i]
        if tc._k not in _NAMED or not tc._id:
            continue
        for known in (typecodes.get(tc._id), dynamic_types.get(tc._id)):
            if known is not None and known._k is tc._k and known.equal(tc):
                final[i] = known
                break
    # The others refer to the replacements, and get classes.
    for i in order:
        if final[i] is tcs[i]:
            fill(i)
    for i in fresh:
        tc = final[i]
        if tc is tcs[i] and tc._k in _WITH_CLASS and tc._k is not CORBA.tk_enum:
            make_class(tc)
    return final[0]


# ---------------------------------------------------------------------------
# Operations and interfaces


class Operation(object):
    """An operation (or attribute accessor) of an interface.

    ``in_types`` are the TypeCodes of the ``in`` and ``inout`` arguments in
    order. ``out_types`` are those of the result: the return value (unless
    ``void``) followed by the ``inout`` and ``out`` arguments in order. A
    call returns None, the single result, or a tuple of the results.
    ``exceptions`` are the TypeCodes of the user exceptions it raises.
    """

    __slots__ = ("name", "in_types", "out_types", "exceptions", "oneway", "contexts", "_native")

    def __init__(self, name, in_types, out_types, exceptions=(), oneway=False, contexts=()):
        self.name = name
        self.in_types = tuple(in_types)
        self.out_types = tuple(out_types)
        self.exceptions = tuple(exceptions)
        self.oneway = oneway
        self.contexts = tuple(contexts)
        # The compiled form the native module makes on the first call.
        self._native = None

    def __repr__(self):
        return "<Operation %s>" % self.name


#: Repository ID -> interface class (the abstract class ``Module.Iface``).
interfaces: dict = {}
#: Repository ID -> stub class (``Module._objref_Iface``).
objref_classes: dict = {}
#: Repository ID -> skeleton class (``Module__POA.Iface``).
skeleton_classes: dict = {}


def register_interface(cls, objref_cls):
    interfaces[cls._NP_RepositoryId] = cls
    objref_classes[cls._NP_RepositoryId] = objref_cls


def register_skeleton(skel_cls):
    skeleton_classes[skel_cls._NP_RepositoryId] = skel_cls


def narrow(obj, cls, checked=True):
    if obj is None:
        return None
    repoId = cls._NP_RepositoryId
    objref_cls = objref_classes.get(repoId)
    if objref_cls is None:
        raise CORBA.BAD_PARAM(0, CORBA.COMPLETED_NO)
    if isinstance(obj, objref_cls):
        return obj
    if checked and not obj._is_a(repoId):
        return None
    return objref_cls(obj._obj)


def make_objref(native, repoId):
    """The Python object reference for ``native`` (a reference of the
    native module) whose static type is the interface ``repoId``: an
    instance of the stub class of the most derived interface known, from
    the type id of the IOR, if it derives from the static type; else of the
    static type's stub class, or ``CORBA.Object``."""
    static = objref_classes.get(repoId, CORBA.Object)
    cls = objref_classes.get(native.type_id())
    if cls is None or not issubclass(cls, static):
        cls = static
    return cls(native)


def operation(cls, name):
    """The :class:`Operation` ``name`` of an interface, stub or skeleton
    class, looking through its base interfaces; None if it has none."""
    for c in cls.__mro__:
        op = c.__dict__.get("_NP_operations", {}).get(name)
        if op is not None:
            return op
    return None


def servant_is_a(servant, repoId):
    if repoId == "IDL:omg.org/CORBA/Object:1.0":
        return True
    return any(
        getattr(c, "_NP_RepositoryId", None) == repoId
        for c in type(servant).__mro__
        if issubclass(c, PortableServer.Servant)
    )


class Typedef(object):
    """Class of the names of typedefs that have no Python class of their
    own (``typedef sequence<long> LongSeq;``): they only carry the
    repository ID and cannot be instantiated."""

    _NP_RepositoryId: Optional[str] = None

    def __init__(self, *args, **kw):
        raise RuntimeError("Cannot construct objects of this type.")

"""Support for the code generated from IDL by ``rustorb-idl -bpython``.

Generated code calls only the functions and classes of this module, of
:mod:`rustorb.CORBA` and of :mod:`rustorb.PortableServer`. The interface
between the generator and this module is versioned by
:data:`GENERATED_CODE_VERSION`: generated code calls :func:`check_version`
first, and a module generated for another version refuses to load.
"""

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
# Operations and interfaces


class Operation(object):
    """An operation (or attribute accessor) of an interface.

    ``in_types`` are the TypeCodes of the ``in`` and ``inout`` arguments in
    order. ``out_types`` are those of the result: the return value (unless
    ``void``) followed by the ``inout`` and ``out`` arguments in order. A
    call returns None, the single result, or a tuple of the results.
    ``exceptions`` are the TypeCodes of the user exceptions it raises.
    """

    __slots__ = ("name", "in_types", "out_types", "exceptions", "oneway", "contexts")

    def __init__(self, name, in_types, out_types, exceptions=(), oneway=False, contexts=()):
        self.name = name
        self.in_types = tuple(in_types)
        self.out_types = tuple(out_types)
        self.exceptions = tuple(exceptions)
        self.oneway = oneway
        self.contexts = tuple(contexts)

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

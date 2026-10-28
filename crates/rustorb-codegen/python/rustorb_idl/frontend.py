"""Reading IDL with idl_parser, and a uniform view of its types.

The backends work on the definition nodes of idl_parser, through the
helpers here:

- :func:`load` parses one IDL file (with its includes);
- :func:`definitions` lists the definitions of a scope in the order they
  are written, with reopened modules taken apart (:class:`Item`);
- :func:`type_of` turns the type of a member, argument, typedef, ... into a
  :class:`Type`.
"""

import os

from idl_parser import enum as idl_enum
from idl_parser import exception as idl_exception
from idl_parser import interface as idl_interface
from idl_parser import module as idl_module
from idl_parser import native as idl_native
from idl_parser import parser as idl_parser_mod
from idl_parser import struct as idl_struct
from idl_parser import type as idl_type
from idl_parser import typedef as idl_typedef
from idl_parser import union as idl_union
from idl_parser import const as idl_const
from idl_parser import valuetype as idl_valuetype


class GeneratorError(Exception):
    """An IDL construct the generator cannot handle."""

    def __init__(self, message, node=None):
        where = ""
        if node is not None and getattr(node, "filepath", None):
            line = getattr(node, "line_number", None)
            where = "%s:%s: " % (node.filepath, line if line is not None else "?")
        Exception.__init__(self, where + message)


# ---------------------------------------------------------------------------
# Loading


class Spec(object):
    """A parsed IDL file: ``root`` is the global scope and ``path`` the
    absolute path of the file (definitions of included files have other
    ``filepath``)."""

    def __init__(self, root, path):
        self.root = root
        self.path = path

    def is_main(self, node):
        fp = getattr(node, "filepath", None)
        return fp is not None and os.path.abspath(fp) == self.path

    def included_files(self):
        """The files, other than the main one, whose definitions the main
        file can use, in the order they are first seen."""
        files = []

        def walk(scope):
            for d in scope.definitions:
                fp = getattr(d, "filepath", None)
                if isinstance(d, idl_module.IDLModule):
                    walk(d)
                    continue
                if fp is not None:
                    fp = os.path.abspath(fp)
                    if fp != self.path and fp not in files:
                        files.append(fp)

        walk(self.root)
        return files


def load(path, include_dirs=(), defines=None, encoding=None):
    path = os.path.abspath(path)
    p = idl_parser_mod.IDLParser(
        idl_dirs=[os.path.abspath(d) for d in include_dirs],
        defines=defines,
        encoding=encoding,
    )
    if encoding is None:
        with open(path, "rb") as f:
            data = f.read()
        try:
            text = data.decode("utf-8")
        except UnicodeDecodeError:
            text = data.decode("latin-1")
    else:
        with open(path, encoding=encoding) as f:
            text = f.read()
    try:
        root = p.load(text, include_dirs=[os.path.dirname(path)], filepath=path)
    except idl_exception.IDLParserException as e:
        raise GeneratorError(str(e)) from e
    return Spec(root, path)


# ---------------------------------------------------------------------------
# Definitions in the order they are written


class Item(object):
    """A definition and the IDL modules it is in (``("Mapping", "Inner")``)."""

    def __init__(self, node, modules):
        self.node = node
        self.modules = modules


def _order_key(node, index):
    line = getattr(node, "line_number", None)
    return (line if line is not None else 0, index)


def definitions(spec):
    """The definitions of the main file outside modules, each with the
    modules it is in, in the order they are written. A module opened
    several times contributes its definitions where each opening is."""
    items = []

    def walk(scope, modules):
        for d in scope.definitions:
            if isinstance(d, idl_module.IDLModule):
                walk(d, modules + (d.name,))
            elif spec.is_main(d):
                items.append(Item(d, modules))

    walk(spec.root, ())
    indexed = list(enumerate(items))
    indexed.sort(key=lambda p: _order_key(p[1].node, p[0]))
    return [it for _, it in indexed]


def modules(spec):
    """Every IDL module (as a tuple of names) the main file defines
    something in, outer modules first."""
    found = []
    for it in definitions(spec):
        for i in range(1, len(it.modules) + 1):
            m = it.modules[:i]
            if m not in found:
                found.append(m)
    return found


def members_of(scope):
    """The definitions nested in an interface, struct, union or exception,
    in the order they are written."""
    return list(getattr(scope, "definitions", []) or [])


# ---------------------------------------------------------------------------
# Kinds of definitions


def is_module(n):
    return isinstance(n, idl_module.IDLModule)


def is_interface(n):
    return isinstance(n, idl_interface.IDLInterface) and not is_valuetype(n)


def is_valuetype(n):
    return isinstance(n, idl_valuetype.IDLValueType)


def is_operation(n):
    return isinstance(n, idl_interface.IDLMethod) and not isinstance(n, idl_valuetype.IDLFactory)


def is_attribute(n):
    return isinstance(n, idl_interface.IDLAttribute)


def is_exception(n):
    return isinstance(n, idl_struct.IDLException)


def is_struct(n):
    return isinstance(n, idl_struct.IDLStruct) and not is_exception(n)


def is_union(n):
    return isinstance(n, idl_union.IDLUnion)


def is_enum(n):
    return isinstance(n, idl_enum.IDLEnum)


def is_typedef(n):
    return isinstance(n, idl_typedef.IDLTypedef)


def is_const(n):
    return isinstance(n, idl_const.IDLConst)


def is_native(n):
    return isinstance(n, idl_native.IDLNative)


def is_forward(n):
    return bool(getattr(n, "is_forward", False))


def kind_name(n):
    return type(n).__name__.replace("IDL", "").lower()


def scope_path(node):
    """Names of the scopes a definition is in, from the outermost module,
    and its own name: ``("CosNaming", "NamingContext", "NotFound")``."""
    names = []
    n = node
    while n is not None and n.parent is not None:
        names.append(n.name)
        n = n.parent
    return tuple(reversed(names))


def enclosing_modules(node):
    """The modules around a definition (``("Mapping", "Inner")``)."""
    names = []
    n = node.parent
    while n is not None and n.parent is not None:
        if is_module(n):
            names.append(n.name)
        n = n.parent
    return tuple(reversed(names))


def inner_scope(node):
    """The names of the interfaces / structs / unions / exceptions around
    a definition, inside its innermost module."""
    names = []
    n = node.parent
    while n is not None and n.parent is not None and not is_module(n):
        names.append(n.name)
        n = n.parent
    return tuple(reversed(names))


# ---------------------------------------------------------------------------
# Types

# Basic type names of idl_parser -> names used by the backends
BASIC = {
    "short": "short",
    "int16": "short",
    "long": "long",
    "int32": "long",
    "unsigned short": "ushort",
    "uint16": "ushort",
    "unsigned long": "ulong",
    "uint32": "ulong",
    "long long": "longlong",
    "int64": "longlong",
    "unsigned long long": "ulonglong",
    "uint64": "ulonglong",
    "float": "float",
    "double": "double",
    "long double": "longdouble",
    "boolean": "boolean",
    "char": "char",
    "wchar": "wchar",
    "octet": "octet",
    "uint8": "octet",
    "any": "any",
    "Object": "Object",
    "TypeCode": "TypeCode",
    "Principal": "Principal",
    "void": "void",
}


class Type(object):
    """A type as the backends see it.

    ``kind`` is one of:

    - a basic type name of :data:`BASIC` (``"long"``, ``"any"``, ...);
    - ``"string"`` / ``"wstring"`` with ``bound`` (0 if unbounded);
    - ``"sequence"`` with ``content`` and ``bound``;
    - ``"array"`` with ``content`` and ``length`` (one dimension);
    - ``"fixed"`` with ``digits`` and ``scale``;
    - ``"named"`` with ``node``: a typedef, struct, union, enum,
      exception, interface or native definition.
    """

    def __init__(self, kind, **kw):
        self.kind = kind
        self.bound = kw.get("bound", 0)
        self.content = kw.get("content")
        self.length = kw.get("length", 0)
        self.digits = kw.get("digits", 0)
        self.scale = kw.get("scale", 0)
        self.node = kw.get("node")

    def __repr__(self):
        if self.kind == "named":
            return "Type(named %s)" % "::".join(scope_path(self.node))
        return "Type(%s)" % self.kind

    def resolved(self):
        """This type with typedefs followed."""
        t = self
        while t.kind == "named" and is_typedef(t.node):
            t = type_of(t.node.type, t.node)
        return t


def type_of(t, where=None):
    """The :class:`Type` of an idl_parser type node ``t``."""
    if isinstance(t, idl_type.IDLBasicType):
        obj = t.obj
        if obj is None:
            raise GeneratorError("Can not find the type %s" % t.ref_name, where)
        return type_of(obj, where)
    if isinstance(t, idl_type.IDLVoid):
        return Type("void")
    if isinstance(t, idl_type.IDLSequence):
        return Type("sequence", content=type_of(t.inner_type, where), bound=t.bound or 0)
    if isinstance(t, idl_type.IDLArray):
        return Type("array", content=type_of(t.inner_type, where), length=t.size)
    if isinstance(t, idl_type.IDLFixed):
        return Type("fixed", digits=t.digits, scale=t.scale)
    if isinstance(t, idl_type.IDLMap):
        raise GeneratorError("map types are not supported yet", where)
    if isinstance(t, idl_type.IDLPrimitive):
        name = " ".join(t.name.split())
        if name.startswith("string") or name.startswith("wstring"):
            wide = name.startswith("w")
            return Type("wstring" if wide else "string", bound=t.bound or 0)
        if name in BASIC:
            return Type(BASIC[name])
        raise GeneratorError("the type %s is not supported yet" % name, where)
    if isinstance(
        t,
        (
            idl_typedef.IDLTypedef,
            idl_struct.IDLStruct,
            idl_union.IDLUnion,
            idl_enum.IDLEnum,
            idl_interface.IDLInterface,
            idl_native.IDLNative,
        ),
    ):
        if is_interface(t) and is_forward(t):
            full = _full_definition(t)
            if full is not None:
                t = full
        return Type("named", node=t)
    raise GeneratorError("the type %s is not supported yet" % getattr(t, "name", t), where)


def _full_definition(fwd):
    """The full definition of a forward-declared interface, if there is one."""
    for d in fwd.parent.definitions:
        if is_interface(d) and d.name == fwd.name and not is_forward(d):
            return d
    return None


def repository_id(node):
    rid = node.repository_id
    if rid is None:
        raise GeneratorError("no repository ID for %s" % node.name, node)
    return rid


def raised_exceptions(op):
    """The exceptions an operation raises, as definition nodes."""
    return list(op.exceptions or [])

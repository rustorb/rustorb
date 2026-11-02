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

from idl_parser import bitmask as idl_bitmask
from idl_parser import bitset as idl_bitset
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


def load(path, include_dirs=(), defines=None, encoding=None, giop=True):
    """Parses the IDL file ``path``. With ``giop`` (code for CORBA), the
    annotations GIOP has no form for are refused (:func:`check_annotations`);
    without it (code for DDS only) they are left to the backend."""
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
    if giop:
        check_annotations(root)
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


def is_bitmask(n):
    return isinstance(n, idl_bitmask.IDLBitmask)


def is_bitset(n):
    return isinstance(n, idl_bitset.IDLBitset)


# Types of IDL 4 that only the C++ backend maps.
CXX_ONLY = {"int8": "int8", "bitmask": "bitmask", "bitset": "bitset"}


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
    "int8": "int8",
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
    - ``"bitmask"`` / ``"bitset"`` with ``node`` (IDL 4; only the C++
      backend maps them);
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
        raise GeneratorError("map types have no form in GIOP and are not supported", where)
    if isinstance(t, idl_bitmask.IDLBitmask):
        return Type("bitmask", node=t)
    if isinstance(t, idl_bitset.IDLBitset):
        return Type("bitset", node=t)
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
            full = full_definition(t)
            if full is not None:
                t = full
        return Type("named", node=t)
    raise GeneratorError("the type %s is not supported yet" % getattr(t, "name", t), where)


def full_definition(fwd):
    """The full definition of a forward-declared interface or valuetype,
    if there is one."""
    kind = is_valuetype if is_valuetype(fwd) else is_interface
    for d in fwd.parent.definitions:
        if kind(d) and d.name == fwd.name and not is_forward(d):
            return d
    return None


def value_bases(node):
    """The base valuetypes of a valuetype (abstract and concrete), as
    definition nodes."""
    return [b if is_valuetype(b) else b.obj for b in node.inheritances]


def concrete_base(node):
    """The concrete (stateful) base valuetype of a valuetype, or None."""
    concrete = [b for b in value_bases(node) if not b.is_abstract]
    if len(concrete) > 1:
        raise GeneratorError("a valuetype can inherit from one concrete valuetype only", node)
    return concrete[0] if concrete else None


def state_member_names(node):
    """The names of the state members of a valuetype, those of its
    concrete base valuetypes first."""
    base = concrete_base(node)
    names = state_member_names(base) if base is not None else []
    return names + [m.name for m in node.state_members]


def supported_interfaces(node):
    """The interfaces a valuetype supports, as definition nodes."""
    return [i if is_interface(i) else i.obj for i in node.supports]


def has_behaviour(node):
    """Whether a valuetype or one of its bases has operations, attributes
    or initializers, or supports an interface, which an implementation
    must provide."""
    if node.factories or node.supports or any(
        is_operation(d) or is_attribute(d) for d in node.methods + node.attributes
    ):
        return True
    return any(has_behaviour(b) for b in value_bases(node))


def repository_id(node):
    rid = node.repository_id
    if rid is None:
        raise GeneratorError("no repository ID for %s" % node.name, node)
    return rid


def raised_exceptions(op):
    """The exceptions an operation raises, as definition nodes."""
    return list(op.exceptions or [])


# ---------------------------------------------------------------------------
# IDL 4 annotations

# Annotations that change what is sent, which GIOP has no form for.
NOT_ON_GIOP = {
    "optional": "GIOP has no form for an absent member",
    "external": "GIOP has no form for an external member",
    "value": "GIOP sends an enumerator as its position, not as a value",
}


def annotation(node, name):
    """The annotation ``@name`` of a node (the last one), or None."""
    find = getattr(node, "annotation_by_name", None)
    return find(name) if find is not None else None


def check_annotations(root):
    """Refuses the annotations of :data:`NOT_ON_GIOP` anywhere in ``root``.
    The others (``@key``, ``@id``, ``@final`` and the like, which are for
    DDS) are left to the backends, which ignore what they do not map.
    Code for DDS only does not call this (see :func:`load`)."""

    def check(node):
        for name, why in NOT_ON_GIOP.items():
            if annotation(node, name) is not None:
                where = node if getattr(node, "filepath", None) else getattr(node, "parent", None)
                raise GeneratorError("@%s is not supported: %s" % (name, why), where)

    def walk(n):
        check(n)
        for attr in ("definitions", "members", "values"):
            for c in getattr(n, attr, None) or []:
                if c is not n:
                    walk(c)
        for c in getattr(n, "cases", None) or []:
            walk(c)

    walk(root)


# ---------------------------------------------------------------------------
# DDS-XTypes annotations (DDS-XTypes 1.3 7.3.1.2.1, IDL 4.2 8.3)

EXTENSIBILITY_KINDS = ("final", "appendable", "mutable")


def _flag(node, name):
    """Whether the boolean annotation ``@name`` (whose value defaults to
    TRUE when it is written) is on ``node`` and true."""
    a = annotation(node, name)
    if a is None:
        return False
    v = a.value
    if v is None or str(v).strip() == "":
        return True
    v = str(v).strip().upper()
    if v in ("TRUE", "1"):
        return True
    if v in ("FALSE", "0"):
        return False
    raise GeneratorError("@%s(%s) is not a boolean" % (name, a.value), node)


def extensibility(node, default="appendable"):
    """The extensibility kind of a struct or union: ``"final"``,
    ``"appendable"`` or ``"mutable"``, from ``@final``, ``@appendable``,
    ``@mutable`` or ``@extensibility(KIND)``, else ``default`` (appendable
    by DDS-XTypes 7.3.1.2.1.8)."""
    found = [k for k in EXTENSIBILITY_KINDS if annotation(node, k) is not None]
    a = annotation(node, "extensibility")
    if a is not None:
        v = str(a.value or "").strip().lower()
        if v not in EXTENSIBILITY_KINDS:
            raise GeneratorError("@extensibility(%s) is not FINAL, APPENDABLE or MUTABLE" % a.value, node)
        found.append(v)
    if len(set(found)) > 1:
        raise GeneratorError("%s has more than one extensibility kind" % node.name, node)
    return found[0] if found else default


def is_key(member):
    return _flag(member, "key")


def is_optional(member):
    return _flag(member, "optional")


def is_external(member):
    return _flag(member, "external")


def must_understand(member):
    """``@must_understand``; key members must always be understood
    (DDS-XTypes 7.2.2.4.4.4.6)."""
    return _flag(member, "must_understand") or is_key(member)


def _string_arg(a):
    v = a.value
    if v is None:
        return ""
    v = str(v).strip()
    if len(v) >= 2 and v[0] == v[-1] == '"':
        v = v[1:-1]
    return v


def hashed_id(name):
    """A member ID computed from a string (DDS-XTypes 7.3.1.2.1.1): the
    first 4 octets of the MD5 of its UTF-8, as a little endian integer,
    with the top 4 bits cleared."""
    import hashlib

    d = hashlib.md5(name.encode("utf-8")).digest()
    return int.from_bytes(d[:4], "little") & 0x0FFFFFFF


def autoid(node):
    """``"sequential"`` or ``"hash"``: the ``@autoid`` of a struct or union,
    else of the innermost module around it that has one, else sequential
    (DDS-XTypes 7.3.1.2.1.1). ``@autoid`` without a value is HASH (IDL 4.2
    8.3.1.2)."""
    n = node
    while n is not None:
        a = annotation(n, "autoid")
        if a is not None:
            v = str(a.value or "HASH").strip().upper()
            if v not in ("SEQUENTIAL", "HASH"):
                raise GeneratorError("@autoid(%s) is not SEQUENTIAL or HASH" % a.value, n)
            return v.lower()
        n = getattr(n, "parent", None)
    return "sequential"


MAX_MEMBER_ID = 0x0FFFFFFF


def member_ids(node, members):
    """The member IDs of the ``members`` (nodes) of a struct or union
    ``node`` (DDS-XTypes 7.3.1.2.1.1): ``@id``, ``@hashid``, else by its
    ``@autoid``: the hash of the name, or the previous ID plus one (0 for
    the first member)."""
    mode = autoid(node)
    ids = []
    prev = None
    for m in members:
        a_id = annotation(m, "id")
        a_hash = annotation(m, "hashid")
        if a_id is not None and a_hash is not None:
            raise GeneratorError("%s has both @id and @hashid" % m.name, m)
        if a_id is not None:
            try:
                mid = int(str(a_id.value).strip(), 0)
            except (TypeError, ValueError):
                raise GeneratorError("@id(%s) of %s is not an integer" % (a_id.value, m.name), m)
        elif a_hash is not None:
            mid = hashed_id(_string_arg(a_hash) or m.name)
        elif mode == "hash":
            mid = hashed_id(m.name)
        else:
            mid = 0 if prev is None else prev + 1
        if not 0 <= mid <= MAX_MEMBER_ID:
            raise GeneratorError("the member ID %#x of %s is out of range" % (mid, m.name), m)
        ids.append(mid)
        prev = mid
    seen = {}
    for m, mid in zip(members, ids):
        if mid in seen:
            raise GeneratorError(
                "%s and %s of %s have the same member ID %#x" % (seen[mid], m.name, node.name, mid),
                node,
            )
        seen[mid] = m.name
    return ids


def is_nested(node):
    """Whether a struct or union is a nested type, which is not a topic
    type: ``@topic`` says no, else ``@nested``, else the ``@default_nested``
    of the innermost module that has one, else no (DDS-XTypes 7.3.1.2.1.7,
    7.3.1.2.1.16)."""
    if annotation(node, "topic") is not None:
        return False
    if annotation(node, "nested") is not None:
        return _flag(node, "nested")
    n = getattr(node, "parent", None)
    while n is not None:
        if annotation(n, "default_nested") is not None:
            return _flag(n, "default_nested")
        n = getattr(n, "parent", None)
    return False


def bit_bound(node, default, lo, hi):
    """The ``@bit_bound`` of an enum or bitmask, checked to be in
    ``lo..hi``."""
    a = annotation(node, "bit_bound")
    if a is None:
        return default
    try:
        v = int(str(a.value).strip(), 0)
    except (TypeError, ValueError):
        v = None
    if v is None or not lo <= v <= hi:
        raise GeneratorError("@bit_bound(%s) of %s is not in %d..%d" % (a.value, node.name, lo, hi), node)
    return v

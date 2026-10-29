"""Type stubs (``__init__.pyi``) for the packages of the Python backend.

The stub of a package has one section for each IDL file that defines names
in the module, so that the files of a module can be generated one by one
into the same directory.

The stubs describe the classes for type checkers rather than exactly as
they are at run time: the operations of an interface are declared on the
interface class (``Module.Iface``), since ``obj._narrow(Module.Iface)`` is
typed as returning it.
"""

import os

from . import frontend as fe
from .names import py_name

INT_KINDS = ("short", "long", "longlong", "ushort", "ulong", "ulonglong", "octet")


def _begin(idl_name):
    return "# rustorb-idl: begin %s" % idl_name


def _end(idl_name):
    return "# rustorb-idl: end %s" % idl_name


class StubWriter(object):
    def __init__(self, backend, modules, poa):
        self.b = backend
        self.modules = modules
        self.poa = poa
        self.pkg = backend.package_name(modules, poa)
        self.imports = set()
        self.lines = []
        self.level = 0

    def __call__(self, text=""):
        self.lines.append(("    " * self.level + text) if text else "")

    # -- type hints ------------------------------------------------------

    def name_of(self, node):
        """A type hint expression naming the Python class of a definition."""
        mods = fe.enclosing_modules(node)
        target = self.b.py_module(mods)
        local = self.b.qualname(node)
        if target == self.b.py_module(self.modules):
            if self.poa:
                self.imports.add(target)
                return "%s.%s" % (target, local)
            return local
        self.imports.add(target)
        return "%s.%s" % (target, local)

    def hint(self, t):
        t = t.resolved()
        k = t.kind
        if k in INT_KINDS:
            return "int"
        if k in ("float", "double", "longdouble"):
            return "float"
        if k == "boolean":
            return "bool"
        if k in ("char", "wchar", "string", "wstring"):
            return "str"
        if k == "any":
            return "CORBA.Any"
        if k == "TypeCode":
            return "CORBA.TypeCode"
        if k == "Object":
            return "Optional[CORBA.Object]"
        if k == "Principal":
            return "bytes"
        if k == "fixed":
            return "CORBA.fixed"
        if k in ("sequence", "array"):
            c = t.content.resolved()
            if c.kind == "octet":
                return "bytes"
            if c.kind == "char":
                return "str"
            return "List[%s]" % self.hint(c)
        if k == "named":
            n = t.node
            if fe.is_enum(n):
                return "_EnumItem"
            if fe.is_interface(n):
                return "Optional[%s]" % self.name_of(n)
            if fe.is_valuetype(n):
                if n.is_boxed:
                    return "Optional[%s]" % self.hint(fe.type_of(n.boxed_type, n))
                return "Optional[%s]" % self.name_of(n)
            if fe.is_native(n):
                return "Any"
            return self.name_of(n)
        if k == "void":
            return "None"
        return "Any"

    # -- definitions -----------------------------------------------------

    def items(self):
        for it in fe.definitions(self.b.spec):
            if it.modules == self.modules:
                yield it.node

    def write(self):
        for node in self.items():
            if self.poa:
                if fe.is_interface(node) and not fe.is_forward(node):
                    self.skeleton(node)
            else:
                self.definition(node)

    def definition(self, node):
        w = self
        if fe.is_const(node):
            w("%s: %s" % (py_name(node.name), self.hint(fe.type_of(node.type, node))))
        elif fe.is_enum(node):
            w("%s: _Enum" % py_name(node.name))
            for v in node.values:
                w("%s: _EnumItem" % py_name(v.name))
            w("_tc_%s: CORBA.TypeCode" % node.name)
        elif fe.is_typedef(node):
            t = fe.type_of(node.type, node)
            r = t.resolved()
            if r.kind == "named" and (fe.is_struct(r.node) or fe.is_union(r.node)):
                w("class %s(%s): ..." % (py_name(node.name), self.name_of(t.node)))
            else:
                w("class %s:" % py_name(node.name))
                w("    _NP_RepositoryId: str")
            w("_tc_%s: CORBA.TypeCode" % node.name)
        elif (fe.is_struct(node) or fe.is_exception(node)) and not fe.is_forward(node):
            base = "CORBA.UserException" if fe.is_exception(node) else "_Struct"
            w("class %s(%s):" % (py_name(node.name), base))
            self.level += 1
            w("_NP_RepositoryId: str")
            params = []
            for m in node.members:
                h = self.hint(fe.type_of(m.type, m))
                w("%s: %s" % (py_name(m.name), h))
                params.append("%s: %s" % (py_name(m.name), h))
            w("def __init__(%s) -> None: ..." % ", ".join(["self"] + params))
            self.nested(node)
            self.level -= 1
            w("_tc_%s: CORBA.TypeCode" % node.name)
        elif fe.is_union(node) and not fe.is_forward(node):
            w("class %s(_Union):" % py_name(node.name))
            self.level += 1
            w("_NP_RepositoryId: str")
            w("_d: Any")
            w("_v: Any")
            seen = []
            for m in node.members:
                if m.name in seen:
                    continue
                seen.append(m.name)
                w("%s: %s" % (py_name(m.name), self.hint(fe.type_of(m.type, m))))
            w("def __init__(self, *args: Any, **kw: Any) -> None: ...")
            self.nested(node)
            self.level -= 1
            w("_tc_%s: CORBA.TypeCode" % node.name)
        elif fe.is_interface(node):
            if fe.is_forward(node):
                if fe.type_of(node).node is node:  # no definition follows
                    w("_tc_%s: CORBA.TypeCode" % node.name)
                return
            self.interface(node)
        elif fe.is_valuetype(node):
            if fe.is_forward(node):
                if fe.full_definition(node) is None:
                    w("_tc_%s: CORBA.TypeCode" % node.name)
            elif node.is_boxed:
                w("class %s:" % py_name(node.name))
                w("    _NP_RepositoryId: str")
                w("_tc_%s: CORBA.TypeCode" % node.name)
            else:
                bases = [self.name_of(b) for b in fe.value_bases(node)] or ["CORBA.ValueBase"]
                w("class %s(%s):" % (py_name(node.name), ", ".join(bases)))
                self.level += 1
                w("_NP_RepositoryId: str")
                for m in node.state_members:
                    w("%s: %s" % (py_name(m.name), self.hint(fe.type_of(m.type, m))))
                w("def __init__(self, *args: Any, **kw: Any) -> None: ...")
                self.nested(node)
                self.level -= 1
                w("_tc_%s: CORBA.TypeCode" % node.name)
        elif fe.is_native(node):
            w("_tc_%s: CORBA.TypeCode" % node.name)
        elif fe.is_struct(node) or fe.is_union(node):
            w("_tc_%s: CORBA.TypeCode" % node.name)

    def nested(self, scope):
        for d in fe.members_of(scope):
            if fe.is_operation(d) or fe.is_attribute(d) or fe.is_interface(d):
                continue
            self.definition(d)

    def signatures(self, node):
        """``(name, parameters, return hint)`` for each operation and
        attribute accessor of an interface."""
        out = []
        for d in fe.members_of(node):
            if fe.is_operation(d):
                params, results = [], []
                ret = fe.type_of(d.returns, d)
                if ret.kind != "void":
                    results.append(self.hint(ret))
                for a in d.arguments:
                    h = self.hint(fe.type_of(a.type, a))
                    if a.direction in ("in", "inout"):
                        params.append("%s: %s" % (py_name(a.name), h))
                    if a.direction in ("out", "inout"):
                        results.append(h)
                if not results:
                    r = "None"
                elif len(results) == 1:
                    r = results[0]
                else:
                    r = "Tuple[%s]" % ", ".join(results)
                out.append((py_name(d.name), params, r))
            elif fe.is_attribute(d):
                h = self.hint(fe.type_of(d.type, d))
                out.append(("_get_" + d.name, [], h))
                if not d.readonly:
                    out.append(("_set_" + d.name, ["value: %s" % h], "None"))
        return out

    def interface(self, node):
        w = self
        bases = [self.name_of(b) for b in self.b.bases(node)]
        w(
            "class %s(%s):" % (py_name(node.name), ", ".join(bases))
            if bases
            else "class %s:" % py_name(node.name)
        )
        self.level += 1
        w("_NP_RepositoryId: str")
        w("_nil: None")
        self.nested(node)
        for name, params, r in self.signatures(node):
            w("def %s(%s) -> %s: ..." % (name, ", ".join(["self"] + params), r))
        for d in fe.members_of(node):
            if fe.is_attribute(d):
                w("%s: %s" % (py_name(d.name), self.hint(fe.type_of(d.type, d))))
        self.level -= 1
        objref_bases = (
            ["%s" % py_name(node.name)]
            + [
                "%s._objref_%s" % (self.b.py_module(fe.enclosing_modules(b)), b.name)
                if self.b.py_module(fe.enclosing_modules(b)) != self.b.py_module(self.modules)
                else "_objref_%s" % b.name
                for b in self.b.bases(node)
            ]
            + ["CORBA.Object"]
        )
        for b in self.b.bases(node):
            if self.b.py_module(fe.enclosing_modules(b)) != self.b.py_module(self.modules):
                self.imports.add(self.b.py_module(fe.enclosing_modules(b)))
        w("class _objref_%s(%s): ..." % (node.name, ", ".join(objref_bases)))
        w("_tc_%s: CORBA.TypeCode" % node.name)

    def skeleton(self, node):
        w = self
        bases = []
        for b in self.b.bases(node):
            pm = self.b.poa_module(fe.enclosing_modules(b))
            if pm == self.pkg:
                bases.append(py_name(b.name))
            else:
                self.imports.add(pm)
                bases.append("%s.%s" % (pm, py_name(b.name)))
        w("class %s(%s):" % (py_name(node.name), ", ".join(bases or ["PortableServer.Servant"])))
        self.level += 1
        w("_NP_RepositoryId: str")
        sigs = self.signatures(node)
        for name, params, r in sigs:
            w("def %s(%s) -> %s: ..." % (name, ", ".join(["self"] + params), r))
        if not sigs:
            w("...")
        self.level -= 1

    def section(self):
        self.write()
        head = [
            "from typing import Any, List, Optional, Tuple",
            "",
            "from rustorb import CORBA, PortableServer",
            "from rustorb._types import Enum as _Enum, EnumItem as _EnumItem, "
            "Struct as _Struct, Union as _Union",
        ]
        for m in sorted(self.imports):
            if m != self.pkg:
                head.append("import %s" % m)
        return head + [""] + self.lines


def package_stub(backend, modules, poa, path):
    idl = backend.idl_name
    sw = StubWriter(backend, modules, poa)
    section = [_begin(idl)] + sw.section() + [_end(idl)]

    kept = []
    if os.path.exists(path):
        with open(path) as f:
            old = f.read().splitlines()
        skip = False
        for line in old:
            if line.startswith("# rustorb-idl: begin "):
                skip = line == _begin(idl)
                if not skip:
                    kept.append(line)
                continue
            if line.startswith("# rustorb-idl: end "):
                if not skip:
                    kept.append(line)
                skip = False
                continue
            if not skip and kept is not None:
                kept.append(line)
        # drop the header of the old file; it is written again
        while kept and (
            kept[0].startswith("#")
            and not kept[0].startswith("# rustorb-idl: begin")
            or not kept[0].strip()
        ):
            kept.pop(0)

    header = [
        "# Generated by rustorb-idl. Do not edit.",
        "#",
        "# This file is not covered by the license of rustorb-idl.",
        "",
    ]
    while kept and not kept[-1].strip():
        kept.pop()
    body = kept + ([""] if kept else []) + section
    return "\n".join(header + body) + "\n"

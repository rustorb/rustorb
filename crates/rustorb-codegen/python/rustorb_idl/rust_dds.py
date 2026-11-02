"""DDS type support in the Rust backend: ``-Wbdds`` and ``-Wbdds_only``.

Structs, unions, enums and bitmasks get an impl of
``rustorb_typesupport::TypeSupport`` (XCDR1 and XCDR2, DDS-XTypes 1.3
clause 7.4), and the structs and unions that are not nested (DDS-XTypes
7.3.1.2.1.7, 7.3.1.2.1.16) an impl of ``rustorb_typesupport::TopicType``.
The code refers to the runtime only by the absolute path of
``-Wbtypesupport`` (``::rustorb_typesupport`` by default).

With ``-Wbdds`` the same Rust types also have the CORBA impls; with
``-Wbdds_only`` they do not, and the annotations GIOP has no form for
(``@optional``, ``@external``, ``@value``) and the types only DDS has
(``int8``, ``bitmask``) can be used.

The annotations read here are ``@key``, ``@id``, ``@autoid``, ``@hashid``,
``@optional``, ``@external``, ``@must_understand``, ``@final``,
``@appendable``, ``@mutable``, ``@extensibility``, ``@nested``,
``@default_nested``, ``@topic``, ``@bit_bound``, ``@value``,
``@position`` and ``@default_literal`` (see frontend.py).
"""

from . import frontend as fe

# basic IDL type -> descriptor (a TypeSupport) of rustorb-typesupport, or a
# Rust type that is its own descriptor.
DDS_BASIC = {
    "boolean": "bool",
    "octet": "u8",
    "int8": "i8",
    "short": "i16",
    "ushort": "u16",
    "long": "i32",
    "ulong": "u32",
    "longlong": "i64",
    "ulonglong": "u64",
    "float": "f32",
    "double": "f64",
}

# Kinds that have no form in DDS.
NO_DDS = {
    "any": "any",
    "Object": "Object",
    "TypeCode": "TypeCode",
    "Principal": "Principal",
    "fixed": "fixed",
    "bitset": "bitset",
    "void": "void",
}


# Member annotations that change what is written or read, which the DDS
# code does not implement yet: refused rather than ignored.
NOT_YET = ("non_serialized", "default", "try_construct")


def enum_holder(bits):
    """The integer type an enum of ``bits`` bits is written as
    (DDS-XTypes Table 32)."""
    if bits <= 8:
        return "i8"
    if bits <= 16:
        return "i16"
    return "i32"


def bitmask_holder(bits):
    """The integer type a bitmask of ``bits`` bits is written as
    (DDS-XTypes Table 33)."""
    if bits <= 8:
        return "u8"
    if bits <= 16:
        return "u16"
    if bits <= 32:
        return "u32"
    return "u64"


class DdsMember(object):
    """A member of a struct, as the DDS code sees it."""

    def __init__(self, node, field, t, mid):
        self.node = node
        self.field = field
        self.type = t
        self.id = mid
        self.key = fe.is_key(node)
        self.optional = fe.is_optional(node)
        self.external = fe.is_external(node)
        self.must_understand = fe.must_understand(node)
        if self.key and self.optional:
            raise fe.GeneratorError("%s cannot be both @key and @optional" % node.name, node)
        for name in NOT_YET:
            if fe.annotation(node, name) is not None:
                raise fe.GeneratorError(
                    "@%s (on %s) is not supported for DDS yet" % (name, node.name), node
                )


class DdsMixin(object):
    """The DDS part of :class:`rust_backend.RustBackend`."""

    @property
    def TS(self):
        return self.opt.typesupport

    @property
    def TST(self):
        return self.TS + "::TypeSupport"

    def default_extensibility(self):
        return self.opt.extensibility

    def check_dds(self, t, where):
        if t.kind in NO_DDS:
            raise fe.GeneratorError("%s has no form in DDS" % NO_DDS[t.kind], where)

    def dds_desc(self, t, cur, where=None):
        """The TypeSupport descriptor of ``t``."""
        k = t.kind
        TS = self.TS
        self.check_dds(t, where)
        if k in DDS_BASIC:
            return DDS_BASIC[k]
        if k == "char":
            return TS + "::Char8"
        if k == "wchar":
            return TS + "::Char16"
        if k == "longdouble":
            return TS + "::LongDouble"
        if k == "string":
            return "%s::BoundedStr<%d>" % (TS, t.bound) if t.bound else TS + "::Str"
        if k == "wstring":
            return "%s::BoundedWStr<%d>" % (TS, t.bound) if t.bound else TS + "::WStr"
        if k == "sequence":
            inner = self.dds_desc(t.content, cur, where)
            if t.bound:
                return "%s::BoundedSeq<%s, %d>" % (TS, inner, t.bound)
            return "%s::Seq<%s>" % (TS, inner)
        if k == "array":
            return "%s::Array<%s, %d>" % (TS, self.dds_desc(t.content, cur, where), t.length)
        if k == "bitmask":
            return self.ref(t.node, cur)
        if k == "named":
            n = self.full(t.node)
            if fe.is_typedef(n):
                return self.dds_desc(fe.type_of(n.type, n), cur, where)
            if fe.is_struct(n) or fe.is_union(n) or fe.is_enum(n) or fe.is_bitmask(n):
                return self.ref(n, cur)
            raise fe.GeneratorError(
                "the %s %s has no form in DDS" % (fe.kind_name(n), n.name), where
            )
        raise fe.GeneratorError("no DDS type for %r" % t, where)

    def dds_call(self, t, cur, fn, where=None, external=False):
        d = self.dds_desc(t, cur, where)
        if external:
            d = "%s::External<%s>" % (self.TS, d)
        return "<%s as %s>::%s" % (d, self.TST, fn)

    def dds_member_desc(self, m, cur, where):
        d = self.dds_desc(m.type, cur, where)
        if m.external:
            d = "%s::External<%s>" % (self.TS, d)
        return d

    def dds_field_type(self, node, t, cur):
        """The Rust type of a struct member, with ``@external`` (a Box) and
        ``@optional`` (an Option)."""
        rt = self.rtype(t, cur, node)
        if self.corba:
            return rt
        if fe.is_external(node):
            rt = "Box<%s>" % rt
        if fe.is_optional(node):
            rt = "Option<%s>" % rt
        return rt

    def dds_field_props(self, node, t, seen):
        p = self.props(t, seen)
        if self.corba:
            return p
        copy, peq, eq, dflt = p
        if fe.is_external(node):
            copy = False
        if fe.is_optional(node):
            dflt = True
        return (copy, peq, eq, dflt)

    def extensibility_expr(self, kind):
        return "%s::Extensibility::%s" % (self.TS, kind.capitalize())

    # -- structs ---------------------------------------------------------

    def dds_members(self, node, fields):
        ms = list(node.members)
        ids = fe.member_ids(node, ms)
        return [
            DdsMember(m, f, fe.type_of(m.type, m), mid) for m, f, mid in zip(ms, fields, ids)
        ]

    def dds_struct(self, w, node, cur, name, fields):
        members = self.dds_members(node, fields)
        kind = fe.extensibility(node, self.default_extensibility())
        TS, TST = self.TS, self.TST
        agg = TS + "::aggregate"
        keys = [m for m in members if m.key]
        holder = sorted(keys or members, key=lambda m: m.id)
        mv = "v" if members else "_v"

        w("impl %s for %s {" % (TST, name))
        w.indent()
        w("type Rust = %s;" % name)
        w()
        # write
        w("fn write(%s: &%s, enc: &mut %s::Encoder) -> %s::Result<()> {" % (mv, name, TS, TS))
        w.indent()
        w("let s = %s::begin(enc, %s);" % (agg, self.extensibility_expr(kind)))
        for m in members:
            d = self.dds_member_desc(m, cur, node)
            mu = "true" if m.must_understand else "false"
            if kind == "mutable":
                if m.optional:
                    w("if let Some(x) = &v.%s {" % m.field)
                    w("    %s::write_member::<%s>(x, %d, %s, enc)?;" % (agg, d, m.id, mu))
                    w("}")
                else:
                    w("%s::write_member::<%s>(&v.%s, %d, %s, enc)?;" % (agg, d, m.field, m.id, mu))
            elif m.optional:
                w("%s::write_optional::<%s>(v.%s.as_ref(), %d, %s, enc)?;" % (agg, d, m.field, m.id, mu))
            else:
                w("<%s as %s>::write(&v.%s, enc)?;" % (d, TST, m.field))
        w("%s::end(enc, s)" % agg)
        w.dedent()
        w("}")
        w()
        # read
        w("fn read(dec: &mut %s::Decoder<'_>) -> %s::Result<%s> {" % (TS, TS, name))
        w.indent()
        w("let s = %s::begin_read(dec, %s)?;" % (agg, self.extensibility_expr(kind)))
        if kind == "mutable":
            for m in members:
                w("let mut m_%s = None;" % m.field.replace("r#", ""))
            w("while let Some(m) = %s::next_member(dec, &s)? {" % agg)
            w.indent()
            w("match m.id() {")
            w.indent()
            for m in members:
                d = self.dds_member_desc(m, cur, node)
                w("%d => m_%s = Some(<%s as %s>::read(dec)?)," % (m.id, m.field.replace("r#", ""), d, TST))
            w("_ => m.unknown()?,")
            w.dedent()
            w("}")
            w("%s::end_member(dec, m)?;" % agg)
            w.dedent()
            w("}")
            w("%s::end_read(dec, s)?;" % agg)
            w("Ok(%s {" % name)
            for m in members:
                local = "m_" + m.field.replace("r#", "")
                p = self.dds_field_props(m.node, m.type, (self.full(node),))
                if m.optional:
                    w("    %s: %s," % (m.field, local))
                elif p[3] and not m.key:
                    w("    %s: %s.unwrap_or_default()," % (m.field, local))
                else:
                    w("    %s: %s.ok_or(%s::Error::MissingMember(%d))?," % (m.field, local, TS, m.id))
            w("})")
        else:
            # The members an older version of an appendable type did not
            # have take their default values.
            w("let v = %s {" % name)
            for m in members:
                d = self.dds_member_desc(m, cur, node)
                if m.optional:
                    read = "%s::read_optional::<%s>(dec)?" % (agg, d)
                    absent = "None"
                else:
                    read = "<%s as %s>::read(dec)?" % (d, TST)
                    p = self.dds_field_props(m.node, m.type, (self.full(node),))
                    absent = "Default::default()" if p[3] and not m.key else None
                if kind == "appendable" and absent is not None:
                    w("    %s: if %s::has_more(dec, &s) {" % (m.field, agg))
                    w("        %s" % read)
                    w("    } else {")
                    w("        %s" % absent)
                    w("    },")
                else:
                    w("    %s: %s," % (m.field, read))
            w("};")
            w("%s::end_read(dec, s)?;" % agg)
            w("Ok(v)")
        w.dedent()
        w("}")
        w()
        # key holder
        hv = "v" if holder else "_v"
        w("fn write_key(%s: &%s, %senc: &mut %s::Encoder) -> %s::Result<()> {" % (hv, name, "" if holder else "_", TS, TS))
        w.indent()
        for m in holder:
            d = self.dds_member_desc(m, cur, node)
            if m.optional:
                w("%s::write_optional_key::<%s>(v.%s.as_ref(), enc)?;" % (agg, d, m.field))
            else:
                w("<%s as %s>::write_key(&v.%s, enc)?;" % (d, TST, m.field))
        w("Ok(())")
        w.dedent()
        w("}")
        w()
        w("fn max_key_end(offset: usize) -> Option<usize> {")
        w.indent()
        for m in holder:
            d = self.dds_member_desc(m, cur, node)
            if m.optional:
                w("let offset = <bool as %s>::max_key_end(offset)?;" % TST)
            w("let offset = <%s as %s>::max_key_end(offset)?;" % (d, TST))
        w("Some(offset)")
        w.dedent()
        w("}")
        w.dedent()
        w("}")
        w()
        self.dds_topic(w, node, name, kind, bool(keys))

    def dds_topic(self, w, node, name, kind, keyed):
        if fe.is_nested(node):
            return
        w("impl %s::TopicType for %s {" % (self.TS, name))
        w('    const TYPE_NAME: &\'static str = "%s";' % "::".join(fe.scope_path(node)))
        w("    const EXTENSIBILITY: %s::Extensibility = %s;" % (self.TS, self.extensibility_expr(kind)))
        w("    const KEYED: bool = %s;" % ("true" if keyed else "false"))
        w("}")
        w()

    def check_dds_struct_members(self, node):
        if self.corba:
            return
        for m in node.members:
            if fe.annotation(m, "value") is not None:
                raise fe.GeneratorError("@value applies to enumerators, not to %s" % m.name, m)

    # -- unions ----------------------------------------------------------

    def dds_union(self, w, node, cur, name, disc, cases, default_index, variants, carries,
                  has_implicit, lit, covered):
        kind = fe.extensibility(node, self.default_extensibility())
        if kind == "mutable":
            raise fe.GeneratorError(
                "mutable unions are not supported for DDS yet (union %s)" % node.name, node
            )
        for m in node.members:
            if fe.is_optional(m) or fe.is_external(m) or fe.is_key(m):
                raise fe.GeneratorError(
                    "@optional, @external and @key are not supported on union members (%s)" % m.name,
                    m,
                )
        TS, TST = self.TS, self.TST
        agg = TS + "::aggregate"
        dd = self.dds_desc(disc, cur, node)
        labels = [l for ls, _, _ in cases for l in ls]
        all_labels = [lit(l) for l in labels]

        def body(w, fn):
            w("match v {")
            w.indent()
            for i, (ls, m, t) in enumerate(cases):
                md = self.dds_desc(t, cur, node)
                if carries[i]:
                    w("%s::%s(d, m) => {" % (name, variants[i]))
                    w.indent()
                    if i == default_index:
                        if all_labels:
                            w("if matches!(*d, %s) {" % " | ".join(all_labels))
                            w("    return Err(%s::Error::InvalidDiscriminator);" % TS)
                            w("}")
                    else:
                        w("if !matches!(*d, %s) {" % " | ".join(lit(l) for l in ls))
                        w("    return Err(%s::Error::InvalidDiscriminator);" % TS)
                        w("}")
                    w("<%s as %s>::write(d, enc)?;" % (dd, TST))
                    w("<%s as %s>::%s(m, enc)?;" % (md, TST, fn))
                    w.dedent()
                    w("}")
                else:
                    w("%s::%s(m) => {" % (name, variants[i]))
                    w("    <%s as %s>::write(&%s, enc)?;" % (dd, TST, lit(ls[0])))
                    w("    <%s as %s>::%s(m, enc)?;" % (md, TST, fn))
                    w("}")
            if has_implicit:
                w("%s::Default(d) => {" % name)
                w.indent()
                if all_labels:
                    w("if matches!(*d, %s) {" % " | ".join(all_labels))
                    w("    return Err(%s::Error::InvalidDiscriminator);" % TS)
                    w("}")
                w("<%s as %s>::write(d, enc)?;" % (dd, TST))
                w.dedent()
                w("}")
            w.dedent()
            w("}")

        w("#[allow(clippy::manual_range_patterns)]")
        w("impl %s for %s {" % (TST, name))
        w.indent()
        w("type Rust = %s;" % name)
        w()
        w("fn write(v: &%s, enc: &mut %s::Encoder) -> %s::Result<()> {" % (name, TS, TS))
        w.indent()
        w("let s = %s::begin(enc, %s);" % (agg, self.extensibility_expr(kind)))
        body(w, "write")
        w("%s::end(enc, s)" % agg)
        w.dedent()
        w("}")
        w()
        w("fn read(dec: &mut %s::Decoder<'_>) -> %s::Result<%s> {" % (TS, TS, name))
        w.indent()
        w("let s = %s::begin_read(dec, %s)?;" % (agg, self.extensibility_expr(kind)))
        w("let d = <%s as %s>::read(dec)?;" % (dd, TST))
        w("let v = match d {")
        w.indent()
        for i, (ls, m, t) in enumerate(cases):
            if i == default_index:
                continue
            mr = "<%s as %s>::read(dec)?" % (self.dds_desc(t, cur, node), TST)
            pat = " | ".join(lit(l) for l in ls)
            if carries[i]:
                w("%s => %s::%s(d, %s)," % (pat, name, variants[i], mr))
            else:
                w("%s => %s::%s(%s)," % (pat, name, variants[i], mr))
        if default_index >= 0:
            t = cases[default_index][2]
            w("_ => %s::%s(d, <%s as %s>::read(dec)?)," % (name, variants[default_index], self.dds_desc(t, cur, node), TST))
        elif has_implicit:
            w("_ => %s::Default(d)," % name)
        elif not covered:
            w("_ => return Err(%s::Error::InvalidDiscriminator)," % TS)
        w.dedent()
        w("};")
        w("%s::end_read(dec, s)?;" % agg)
        w("Ok(v)")
        w.dedent()
        w("}")
        w()
        w("fn write_key(v: &%s, enc: &mut %s::Encoder) -> %s::Result<()> {" % (name, TS, TS))
        w.indent()
        body(w, "write_key")
        w("Ok(())")
        w.dedent()
        w("}")
        w()
        w("fn max_key_end(offset: usize) -> Option<usize> {")
        w.indent()
        w("let offset = <%s as %s>::max_key_end(offset)?;" % (dd, TST))
        w("let end = offset;")
        for _, _, t in cases:
            w("let end = end.max(<%s as %s>::max_key_end(offset)?);" % (self.dds_desc(t, cur, node), TST))
        w("Some(end)")
        w.dedent()
        w("}")
        w.dedent()
        w("}")
        w()
        self.dds_topic(w, node, name, kind, False)

    # -- enums -----------------------------------------------------------

    def enum_values(self, node):
        """``(name, value)`` of the enumerators: their ``@value``, else the
        previous value plus one (DDS-XTypes 7.3.1.2.1.5); idl_parser
        computes these."""
        return [(v.name, v.value) for v in node.values]

    def dds_enum_methods(self, w, node, name, values):
        """``value`` and ``from_value``, in the inherent impl of the enum."""
        pairs = [(variant, value) for variant, (_, value) in zip(values, self.enum_values(node))]
        w()
        w("/// The value of the enumerator (its `@value`).")
        w("pub fn value(self) -> i32 {")
        w("    match self {")
        for variant, value in pairs:
            w("        %s::%s => %d," % (name, variant, value))
        w("    }")
        w("}")
        w()
        w("/// The enumerator of value `v`.")
        w("pub fn from_value(v: i32) -> Option<%s> {" % name)
        w("    match v {")
        for variant, value in pairs:
            w("        %d => Some(%s::%s)," % (value, name, variant))
        w("        _ => None,")
        w("    }")
        w("}")

    def dds_enum(self, w, node, name):
        bits = fe.bit_bound(node, 32, 1, 32)
        holder = enum_holder(bits)
        lo, hi = -(1 << (int(holder[1:]) - 1)), (1 << (int(holder[1:]) - 1)) - 1
        for vname, value in self.enum_values(node):
            if not lo <= value <= hi:
                raise fe.GeneratorError(
                    "the value %d of %s does not fit in @bit_bound(%d)" % (value, vname, bits), node
                )
        TS, TST = self.TS, self.TST
        w("impl %s for %s {" % (TST, name))
        w.indent()
        w("type Rust = %s;" % name)
        w()
        value = "v.value()" if holder == "i32" else "(v.value() as %s)" % holder
        widened = "v" if holder == "i32" else "i32::from(v)"
        w("fn write(v: &%s, enc: &mut %s::Encoder) -> %s::Result<()> {" % (name, TS, TS))
        w("    <%s as %s>::write(&%s, enc)" % (holder, TST, value))
        w("}")
        w()
        w("fn read(dec: &mut %s::Decoder<'_>) -> %s::Result<%s> {" % (TS, TS, name))
        w("    let v = <%s as %s>::read(dec)?;" % (holder, TST))
        w("    %s::from_value(%s).ok_or(%s::Error::InvalidEnumValue(i64::from(v)))" % (name, widened, TS))
        w("}")
        w()
        w("fn max_key_end(offset: usize) -> Option<usize> {")
        w("    <%s as %s>::max_key_end(offset)" % (holder, TST))
        w("}")
        w.dedent()
        w("}")
        w()

    # -- bitmasks --------------------------------------------------------

    def bitmask(self, w, node, cur):
        if not self.dds:
            raise fe.GeneratorError(
                "bitmask %s is mapped to Rust only for DDS (-Wbdds or -Wbdds_only)" % node.name, node
            )
        name = self.item_name(node)
        self.claim(cur, name, "bitmask %s" % node.name, node)
        bits = node.bit_bound
        holder = bitmask_holder(bits)
        from .rust_backend import ident, screaming

        flags = [(ident(screaming(v.name)), v.position, v.name) for v in node.values]
        if len(set(f for f, _, _ in flags)) != len(flags):
            raise fe.GeneratorError("values of %s map to the same Rust name" % node.name, node)
        for _, pos, vname in flags:
            if pos >= bits:
                raise fe.GeneratorError(
                    "the position %d of %s is not below @bit_bound(%d)" % (pos, vname, bits), node
                )
        TS, TST = self.TS, self.TST
        self.doc(w, node, "IDL bitmask")
        w("#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]")
        w("pub struct %s(pub %s);" % (name, holder))
        w()
        w("impl %s {" % name)
        w.indent()
        for f, pos, vname in flags:
            w("/// `%s` (bit %d)." % (vname, pos))
            w("pub const %s: %s = %s(1 << %d);" % (f, name, name, pos))
        w("/// No flag.")
        w("pub const EMPTY: %s = %s(0);" % (name, name))
        w()
        w("/// The bits.")
        w("pub const fn bits(self) -> %s {" % holder)
        w("    self.0")
        w("}")
        w()
        w("/// Whether every flag of `other` is set in `self`.")
        w("pub const fn contains(self, other: %s) -> bool {" % name)
        w("    self.0 & other.0 == other.0")
        w("}")
        w.dedent()
        w("}")
        w()
        for trait, op in (("BitOr", "|"), ("BitAnd", "&"), ("BitXor", "^")):
            w("impl ::std::ops::%s for %s {" % (trait, name))
            w("    type Output = %s;" % name)
            w("    fn %s(self, rhs: %s) -> %s {" % (_snake_op(trait), name, name))
            w("        %s(self.0 %s rhs.0)" % (name, op))
            w("    }")
            w("}")
            w()
            w("impl ::std::ops::%sAssign for %s {" % (trait, name))
            w("    fn %s_assign(&mut self, rhs: %s) {" % (_snake_op(trait), name))
            w("        self.0 %s= rhs.0;" % op)
            w("    }")
            w("}")
            w()
        w("impl %s for %s {" % (TST, name))
        w.indent()
        w("type Rust = %s;" % name)
        w()
        w("fn write(v: &%s, enc: &mut %s::Encoder) -> %s::Result<()> {" % (name, TS, TS))
        w("    <%s as %s>::write(&v.0, enc)" % (holder, TST))
        w("}")
        w()
        w("fn read(dec: &mut %s::Decoder<'_>) -> %s::Result<%s> {" % (TS, TS, name))
        w("    <%s as %s>::read(dec).map(%s)" % (holder, TST, name))
        w("}")
        w()
        w("fn max_key_end(offset: usize) -> Option<usize> {")
        w("    <%s as %s>::max_key_end(offset)" % (holder, TST))
        w("}")
        w.dedent()
        w("}")
        w()


def _snake_op(trait):
    return {"BitOr": "bitor", "BitAnd": "bitand", "BitXor": "bitxor"}[trait]

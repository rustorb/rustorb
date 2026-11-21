#!/usr/bin/env python3
"""Writes dotnet/src/Rustorb.Corba/Native/Ffi.g.cs: a [LibraryImport]
declaration for each function of the C ABI
(crates/rustorb-ffi/include/rustorb/rustorb.h).

    dotnet/tools/gen-pinvoke.py          # rewrite Ffi.g.cs
    dotnet/tools/gen-pinvoke.py --check  # fail if Ffi.g.cs is not up to date

Handles (pointers to the opaque structs of the ABI) are nint; pointers to
the structs the binding fills in (rorb_exception, rorb_bytes,
rorb_invocation, rorb_policy) are pointers to their C# layouts; other
pointers are unmanaged pointers but void* (contexts), which is nint, and
function pointers nint (a delegate* unmanaged cast). Short functions that neither call back nor
block are marked [SuppressGCTransition].

Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.
"""

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
HEADER = ROOT / "crates/rustorb-ffi/include/rustorb/rustorb.h"
OUTPUT = ROOT / "dotnet/src/Rustorb.Corba/Native/Ffi.g.cs"

NO_TRANSITION = {
    "rorb_out_write_boolean", "rorb_out_write_octet", "rorb_out_write_char",
    "rorb_out_write_wchar", "rorb_out_write_short", "rorb_out_write_ushort",
    "rorb_out_write_long", "rorb_out_write_ulong", "rorb_out_write_longlong",
    "rorb_out_write_ulonglong", "rorb_out_write_float", "rorb_out_write_double",
    "rorb_out_write_longdouble", "rorb_out_write_string", "rorb_out_write_wstring",
    "rorb_out_write_array", "rorb_out_failed",
    "rorb_in_read_boolean", "rorb_in_read_octet", "rorb_in_read_char",
    "rorb_in_read_wchar", "rorb_in_read_short", "rorb_in_read_ushort",
    "rorb_in_read_long", "rorb_in_read_ulong", "rorb_in_read_longlong",
    "rorb_in_read_ulonglong", "rorb_in_read_float", "rorb_in_read_double",
    "rorb_in_read_longdouble", "rorb_in_read_string", "rorb_in_read_wstring",
    "rorb_in_read_array", "rorb_in_read_length", "rorb_in_fail", "rorb_in_failed",
    "rorb_longdouble_from_double", "rorb_longdouble_to_double",
    "rorb_server_reply_output",
}

SCALARS = {
    "int32_t": "int", "uint32_t": "uint", "int": "int", "rorb_status": "int",
    "int64_t": "long", "uint64_t": "ulong", "size_t": "nuint",
    "int16_t": "short", "uint16_t": "ushort", "uint8_t": "byte", "int8_t": "sbyte",
    "char": "byte", "float": "float", "double": "double", "void": "void",
}
STRUCTS = {
    "rorb_exception": "RorbException", "rorb_bytes": "RorbBytes",
    "rorb_invocation": "RorbInvocation", "rorb_policy": "RorbPolicy",
}
KEYWORDS = {"in", "out", "ref", "string", "object", "base", "params", "event", "fixed", "lock", "operator"}


def declarations(text):
    text = re.sub(r"/\*.*?\*/", "", text, flags=re.S)
    text = re.sub(r"//[^\n]*", "", text)
    text = re.sub(r"^\s*#.*$", "", text, flags=re.M)
    text = re.sub(r"typedef\s+struct\s+\w+\s*\{.*?\}\s*\w+\s*;", "", text, flags=re.S)
    text = re.sub(r"typedef[^;]*;", "", text)
    text = text.replace('extern "C" {', "").replace("}", "")
    for stmt in text.split(";"):
        s = " ".join(stmt.split())
        m = re.match(r"(.*?)\b(rorb_\w+)\s*\((.*)\)$", s)
        if m:
            yield m.group(1).strip(), m.group(2), m.group(3)


def split_args(args):
    parts, depth, cur = [], 0, ""
    for ch in args:
        if ch == "," and depth == 0:
            parts.append(cur.strip())
            cur = ""
            continue
        depth += ch == "("
        depth -= ch == ")"
        cur += ch
    if cur.strip():
        parts.append(cur.strip())
    return [] if parts == ["void"] else parts


def cs_type(fn, decl):
    decl = re.sub(r"\bconst\b", "", decl).strip()
    if "(" in decl:
        return "nint"
    base, stars = decl, 0
    while base.endswith("*"):
        base, stars = base[:-1].strip(), stars + 1
    if base.startswith("rorb_each_") or base == "rorb_sendc_done_fn":
        return "nint" + "*" * stars
    if base in STRUCTS:
        return STRUCTS[base] + "*" * stars
    if base == "void" and stars == 1:
        return "nint"
    if base in SCALARS:
        return SCALARS[base] + "*" * stars
    if base.startswith("rorb_"):
        if base.endswith("_vtable") or base == "rorb_orb_initializer":
            return "void" + "*" * stars
        if stars == 0:
            raise SystemExit(f"{fn}: opaque type by value {decl!r}")
        return "nint" + "*" * (stars - 1)
    raise SystemExit(f"{fn}: unknown type {decl!r}")


def param(fn, decl):
    decl = decl.strip()
    m = re.match(r"(.*?)\(\s*\*\s*(\w+)\s*\)\s*\(.*\)$", decl)
    if m:
        return "nint", m.group(2)
    m = re.match(r"(.*?)(\w+)\s*\[\s*\w*\s*\]$", decl)
    if m:
        return cs_type(fn, m.group(1) + "*"), m.group(2)
    m = re.match(r"(.*?[\s\*])(\w+)$", decl)
    if not m:
        raise SystemExit(f"{fn}: cannot read parameter {decl!r}")
    return cs_type(fn, m.group(1)), m.group(2)


def generate():
    lines = []
    for ret, name, args in sorted(declarations(HEADER.read_text()), key=lambda d: d[1]):
        ps = []
        for a in split_args(args):
            t, n = param(name, a)
            if n in KEYWORDS:
                n = "@" + n
            ps.append(f"{t} {n}")
        r = cs_type(name, ret)
        attrs = "    [LibraryImport(Lib)]\n"
        if name in NO_TRANSITION:
            attrs += "    [SuppressGCTransition]\n"
        lines.append(f"{attrs}    internal static partial {r} {name}({', '.join(ps)});")
    return TEMPLATE.replace("@DECLS@", "\n\n".join(lines))


TEMPLATE = """\
// Generated by dotnet/tools/gen-pinvoke.py from
// crates/rustorb-ffi/include/rustorb/rustorb.h: do not edit.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

#nullable disable
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;

namespace Rustorb.Native;

internal static unsafe partial class Ffi
{
    internal const string Lib = "rustorb_ffi";

@DECLS@
}
"""


def main():
    text = generate()
    if "--check" in sys.argv:
        if not OUTPUT.exists() or OUTPUT.read_text() != text:
            raise SystemExit(f"{OUTPUT.relative_to(ROOT)} is out of date: run dotnet/tools/gen-pinvoke.py")
        return
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT.write_text(text)


if __name__ == "__main__":
    main()

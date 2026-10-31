#!/usr/bin/env python3
"""Writes java/src/main/java/org/rustorb/Ffi.java: a downcall handle for
each function of the C ABI (crates/rustorb-ffi/include/rustorb/rustorb.h)
that org.rustorb.Native uses (the names Ffi.rorb_... it mentions).

    java/tools/gen-ffi.py          # rewrite Ffi.java
    java/tools/gen-ffi.py --check  # fail if Ffi.java is not up to date

Pointers are passed as Java longs (JAVA_LONG), which all the 64-bit
targets of rustorb pass like pointers; the handles of the binding are
longs too. The parameters listed in HEAP are ADDRESS instead, for heap
segments (Java arrays) passed to the critical functions below. rorb_bytes
by value is its struct layout.

Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.
"""

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
HEADER = ROOT / "crates/rustorb-ffi/include/rustorb/rustorb.h"
NATIVE = ROOT / "java/src/main/java/org/rustorb/Native.java"
OUTPUT = ROOT / "java/src/main/java/org/rustorb/Ffi.java"

# Short functions that neither call back into Java nor block: called
# without the thread state transition (Linker.Option.critical). Those
# with parameters in HEAP may get Java arrays (critical(true)).
CRITICAL = {
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
    "rorb_server_reply_output", "rorb_object_is_equivalent",
}
HEAP = {
    ("rorb_out_write_string", "s"),
    ("rorb_out_write_wstring", "s"),
    ("rorb_out_write_array", "data"),
    ("rorb_in_read_array", "data"),
}

SCALARS = {
    "int32_t": ("JAVA_INT", "int"), "uint32_t": ("JAVA_INT", "int"),
    "int": ("JAVA_INT", "int"), "rorb_status": ("JAVA_INT", "int"),
    "int64_t": ("JAVA_LONG", "long"), "uint64_t": ("JAVA_LONG", "long"),
    "size_t": ("JAVA_LONG", "long"),
    "int16_t": ("JAVA_SHORT", "short"), "uint16_t": ("JAVA_SHORT", "short"),
    "uint8_t": ("JAVA_BYTE", "byte"), "int8_t": ("JAVA_BYTE", "byte"),
    "char": ("JAVA_BYTE", "byte"),
    "float": ("JAVA_FLOAT", "float"), "double": ("JAVA_DOUBLE", "double"),
    "rorb_bytes": ("BYTES", "MemorySegment"),
    "rorb_each_policy": ("JAVA_LONG", "long"),
}


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


def layout(fn, decl, ret=False):
    """The value layout of a parameter or return type."""
    decl = decl.replace("const ", "").strip()
    if "(" in decl or "[" in decl:  # a function pointer or an array
        return "JAVA_LONG"
    name = None
    if not ret:
        m = re.match(r"(.*?)(\w+)$", decl)
        if m and m.group(1).strip():
            decl, name = m.group(1).strip(), m.group(2)
    if "*" in decl:
        return "ADDRESS" if (fn, name) in HEAP else "JAVA_LONG"
    if decl not in SCALARS:
        raise SystemExit(f"{fn}: unknown type {decl!r}")
    return SCALARS[decl][0]


def generate():
    used = set(re.findall(r"\bFfi\.(rorb_\w+)", NATIVE.read_text()))
    found = {}
    for ret, name, args in declarations(HEADER.read_text()):
        if name not in used:
            continue
        params = [layout(name, a) for a in split_args(args)]
        r = None if ret == "void" else layout(name, ret, ret=True)
        found[name] = (r, params)
    missing = used - set(found)
    if missing:
        raise SystemExit("not in rustorb.h: " + ", ".join(sorted(missing)))
    lines = []
    for name in sorted(found):
        r, params = found[name]
        ps = ", ".join(params)
        fd = f"FunctionDescriptor.ofVoid({ps})" if r is None else \
            f"FunctionDescriptor.of({r}{', ' if ps else ''}{ps})"
        opt = ""
        if name in CRITICAL:
            heap = any(n == name for n, _ in HEAP)
            opt = ", CRITICAL_HEAP" if heap else ", CRITICAL"
        lines.append(f"    static final MethodHandle {name} = fn(\"{name}\",\n            {fd}{opt});")
    return TEMPLATE.replace("@HANDLES@", "\n".join(lines))


TEMPLATE = """\
// Generated by java/tools/gen-ffi.py from crates/rustorb-ffi/include/rustorb/rustorb.h:
// do not edit. The downcall handles of the functions of the C ABI that
// org.rustorb.Native uses. Pointers are Java longs (see gen-ffi.py).
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.rustorb;

import static java.lang.foreign.ValueLayout.ADDRESS;
import static java.lang.foreign.ValueLayout.JAVA_BYTE;
import static java.lang.foreign.ValueLayout.JAVA_DOUBLE;
import static java.lang.foreign.ValueLayout.JAVA_FLOAT;
import static java.lang.foreign.ValueLayout.JAVA_INT;
import static java.lang.foreign.ValueLayout.JAVA_LONG;
import static java.lang.foreign.ValueLayout.JAVA_SHORT;
import static org.rustorb.Library.BYTES;
import static org.rustorb.Library.CRITICAL;
import static org.rustorb.Library.CRITICAL_HEAP;
import static org.rustorb.Library.fn;

import java.lang.foreign.FunctionDescriptor;
import java.lang.invoke.MethodHandle;

@SuppressWarnings("unused")
final class Ffi {
    private Ffi() {}

@HANDLES@
}
"""


def main():
    text = generate()
    if "--check" in sys.argv:
        if not OUTPUT.exists() or OUTPUT.read_text() != text:
            raise SystemExit(f"{OUTPUT.relative_to(ROOT)} is out of date: run java/tools/gen-ffi.py")
        return
    OUTPUT.write_text(text)


if __name__ == "__main__":
    main()

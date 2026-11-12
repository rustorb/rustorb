"""omniORBpy's ``omniORB.URI``: stringified CosNaming names and
``corbaname:`` URIs, without a naming service.

The rules are those of the INS string form (``NamingContextExt``), with
the details measured on omniORBpy 4.3: the last component may end in
``.`` (``a.`` is the id ``a`` with an empty kind), ``\\`` escapes only ``/``,
``.`` and ``\\``, :func:`addrAndNameToURI` only checks that the address
is not empty, and it escapes every character but letters, digits and
``-_.~/`` as ``%XX`` of its UTF-8 bytes. They differ from the naming
service's ``to_name`` / ``to_url`` (rustorb's and omniNames'), so they
are implemented here rather than with the core's conversion.

Malformed names raise ``CosNaming.NamingContext.InvalidName`` and an
empty address ``CosNaming.NamingContextExt.InvalidAddress``. Arguments
of the wrong Python type raise ``CORBA.BAD_PARAM`` (omniORBpy raises a
``NameError`` there).
"""

from rustorb import CORBA

import CosNaming

__all__ = ["stringToName", "nameToString", "addrAndNameToURI"]

_ESCAPED = "/.\\"
_URI_SAFE = frozenset(
    b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_.~/"
)


def _bad_param():
    return CORBA.BAD_PARAM(0, CORBA.COMPLETED_NO)


def _component(text, last):
    """One component (the text between unescaped ``/``) as a
    NameComponent. A ``.`` before an empty kind (``a.``) is allowed in
    the last component only, as in omniORBpy."""
    if not text:
        raise CosNaming.NamingContext.InvalidName()
    parts = [[]]
    chars = iter(text)
    for c in chars:
        if c == "\\":
            c = next(chars, None)
            if c is None or c not in _ESCAPED:
                raise CosNaming.NamingContext.InvalidName()
            parts[-1].append(c)
        elif c == ".":
            if len(parts) == 2:
                raise CosNaming.NamingContext.InvalidName()
            parts.append([])
        else:
            parts[-1].append(c)
    id_ = "".join(parts[0])
    kind = "".join(parts[1]) if len(parts) == 2 else ""
    if len(parts) == 2 and id_ and not kind and not last:
        raise CosNaming.NamingContext.InvalidName()
    return CosNaming.NameComponent(id_, kind)


def _split(sname):
    """The components of ``sname``: split at the ``/`` that are not
    escaped, keeping the escapes."""
    components = []
    current = []
    escaped = False
    for c in sname:
        if escaped:
            current.append(c)
            escaped = False
        elif c == "\\":
            current.append(c)
            escaped = True
        elif c == "/":
            components.append("".join(current))
            current = []
        else:
            current.append(c)
    components.append("".join(current))
    return components


def stringToName(sname):
    """stringToName(string) -> CosNaming.Name

    Convert a stringified name to a CosNaming.Name (a list of
    ``CosNaming.NameComponent``). ``a.b/c`` is ``[("a", "b"), ("c",
    "")]``; ``\\`` escapes ``/``, ``.`` and ``\\``; ``.`` alone is a
    component with an empty id and kind, and the last component may end
    in ``.`` (``a.`` is ``("a", "")``). An empty string, an empty
    component, a ``.`` before an empty kind elsewhere, a second unescaped ``.`` in a component or any other
    escape is ``CosNaming.NamingContext.InvalidName``."""
    if not isinstance(sname, str):
        raise _bad_param()
    if not sname:
        raise CosNaming.NamingContext.InvalidName()
    texts = _split(sname)
    return [_component(text, i == len(texts) - 1) for i, text in enumerate(texts)]


def _escape(part):
    if not isinstance(part, str):
        raise _bad_param()
    return "".join("\\" + c if c in _ESCAPED else c for c in part)


def nameToString(name):
    """nameToString(CosNaming.Name) -> string

    Convert a CosNaming.Name into its stringified form, the form
    :func:`stringToName` reads. An empty name is
    ``CosNaming.NamingContext.InvalidName``."""
    if not isinstance(name, (list, tuple)):
        raise _bad_param()
    if not name:
        raise CosNaming.NamingContext.InvalidName()
    out = []
    for nc in name:
        if not isinstance(nc, CosNaming.NameComponent):
            raise _bad_param()
        if nc.kind == "" and nc.id == "":
            out.append(".")
        elif nc.kind == "":
            out.append(_escape(nc.id))
        else:
            out.append(_escape(nc.id) + "." + _escape(nc.kind))
    return "/".join(out)


def addrAndNameToURI(addr, sname):
    """addrAndNameToURI(addr, sname) -> URI

    Create a ``corbaname:`` URI from an address string (such as
    ``:host:2809`` or ``iiop:1.2@host``) and a stringified name:
    ``addrAndNameToURI(":host", "a/b c")`` is ``corbaname::host#a/b%20c``.
    An empty name gives ``corbaname:<addr>``. An empty address is
    ``CosNaming.NamingContextExt.InvalidAddress`` (the address is not
    checked further); a malformed name is
    ``CosNaming.NamingContext.InvalidName``."""
    if not isinstance(addr, str) or not isinstance(sname, str):
        raise _bad_param()
    if not addr:
        raise CosNaming.NamingContextExt.InvalidAddress()
    if not sname:
        return "corbaname:" + addr
    stringToName(sname)
    escaped = "".join(
        chr(b) if b in _URI_SAFE else "%%%02X" % b for b in sname.encode("utf-8")
    )
    return "corbaname:%s#%s" % (addr, escaped)

"""Python names for IDL names."""

import keyword
import re


def py_name(name):
    """An IDL identifier as a Python identifier: Python keywords get a
    leading ``_`` (``def`` -> ``_def``), as the IDL to Python mapping says."""
    if keyword.iskeyword(name):
        return "_" + name
    return name


def module_of_file(path):
    """The Python module that holds the definitions of an IDL file:
    ``InteropTest.idl`` -> ``InteropTest_idl``."""
    import os

    base = os.path.basename(path)
    if base.lower().endswith(".idl"):
        base = base[:-4]
    base = re.sub(r"[^0-9A-Za-z_]", "_", base)
    if base[:1].isdigit():
        base = "_" + base
    return base + "_idl"

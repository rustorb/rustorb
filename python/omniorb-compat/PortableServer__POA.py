"""``PortableServer__POA``, as omniORBpy installs it at the top level: the module of
rustorb itself (``rustorb.PortableServer__POA``)."""

import sys

import rustorb.PortableServer__POA

sys.modules[__name__] = rustorb.PortableServer__POA

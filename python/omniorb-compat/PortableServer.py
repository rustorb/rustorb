"""``PortableServer``, as omniORBpy installs it at the top level: the module of
rustorb itself (``rustorb.PortableServer``)."""

import sys

import rustorb.PortableServer

sys.modules[__name__] = rustorb.PortableServer

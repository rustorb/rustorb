"""``CORBA``, as omniORBpy installs it at the top level: the module of
rustorb itself (``rustorb.CORBA``)."""

import sys

import rustorb.CORBA

sys.modules[__name__] = rustorb.CORBA

"""The PortableServer module of the IDL to Python mapping.

This first version has the base class of servants, which the skeleton
classes generated from IDL derive from. The POA itself comes with the
native module.
"""

from . import CORBA


class Servant(object):
    """Base of servants. Skeleton classes (``Module__POA.Interface``) derive
    from it, and implementations derive from a skeleton class."""

    _NP_RepositoryId = "IDL:omg.org/CORBA/Object:1.0"

    def _this(self):
        raise CORBA.NO_IMPLEMENT(0, CORBA.COMPLETED_NO)

    def _default_POA(self):
        raise CORBA.NO_IMPLEMENT(0, CORBA.COMPLETED_NO)

    def _non_existent(self):
        return False

    def _is_a(self, repoId):
        from . import _rt

        return _rt.servant_is_a(self, repoId)

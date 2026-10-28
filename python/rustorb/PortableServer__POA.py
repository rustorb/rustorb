"""Skeletons of the servant manager and adapter activator interfaces of
``PortableServer``: implementations derive from these and are given to
``POA.set_servant_manager`` and ``POA.the_activator``.
"""

from . import PortableServer


class ServantManager(PortableServer.Servant):
    _NP_RepositoryId = "IDL:omg.org/PortableServer/ServantManager:2.3"


class ServantActivator(ServantManager):
    """``incarnate(oid, adapter)`` returns a servant (or raises
    ``PortableServer.ForwardRequest``); ``etherealize(oid, adapter,
    servant, cleanup_in_progress, remaining_activations)`` is called when
    the object is deactivated."""

    _NP_RepositoryId = "IDL:omg.org/PortableServer/ServantActivator:2.3"

    def etherealize(self, oid, adapter, serv, cleanup_in_progress, remaining_activations):
        pass


class ServantLocator(ServantManager):
    """``preinvoke(oid, adapter, operation)`` returns ``(servant, cookie)``
    for each request; ``postinvoke(oid, adapter, operation, cookie,
    servant)`` follows it."""

    _NP_RepositoryId = "IDL:omg.org/PortableServer/ServantLocator:2.3"

    def postinvoke(self, oid, adapter, operation, the_cookie, the_servant):
        pass


class AdapterActivator(PortableServer.Servant):
    """``unknown_adapter(parent, name)`` creates the child POA ``name`` and
    returns True, or returns False."""

    _NP_RepositoryId = "IDL:omg.org/PortableServer/AdapterActivator:2.3"

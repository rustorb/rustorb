"""Skeletons of the Messaging module: ``ReplyHandler``, the base of the
``AMI_<I>Handler`` skeletons of callback AMI (see ``rustorb.Messaging``).
"""

from . import Messaging
from . import PortableServer
from . import _rt


class ReplyHandler(PortableServer.Servant):
    _NP_RepositoryId = Messaging.ReplyHandler._NP_RepositoryId
    _NP_operations = Messaging.ReplyHandler._NP_operations


_rt.register_skeleton(ReplyHandler)

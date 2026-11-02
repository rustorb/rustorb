"""The BiDirPolicy module of the IDL to Python mapping: the
``BidirectionalPolicy`` of bidirectional GIOP (docs/bidir-giop.md).

Make the policy with ``orb.create_policy(BIDIRECTIONAL_POLICY_TYPE,
CORBA.Any(BiDirPolicy._tc_BidirectionalPolicyValue, BiDirPolicy.BOTH))``.
In ``create_POA``, ``BOTH`` lets the POA's objects take requests that come
over the connections this ORB opened and offered (callbacks); set on the
ORB's ``ORBPolicyManager``, it makes the ORB offer and accept such
connections, like ``-ORBofferBiDirectionalGIOP 1`` and
``-ORBacceptBiDirectionalGIOP 1``.
"""

from . import CORBA

#: ``BiDirPolicy::BidirectionalPolicyValue``, an unsigned short.
NORMAL = 0
BOTH = 1

_tc_BidirectionalPolicyValue = CORBA.TypeCode(
    CORBA.tk_alias,
    "IDL:omg.org/BiDirPolicy/BidirectionalPolicyValue:1.0",
    "BidirectionalPolicyValue",
    content=CORBA.TC_ushort,
)

BIDIRECTIONAL_POLICY_TYPE = 37


class BidirectionalPolicy(CORBA.Policy):
    """Whether bidirectional GIOP is used: ``value`` is ``NORMAL`` or
    ``BOTH``."""

    _NP_RepositoryId = "IDL:omg.org/BiDirPolicy/BidirectionalPolicy:1.0"
    policy_type = BIDIRECTIONAL_POLICY_TYPE


#: The name omniORBpy gives the TypeCode, for applications written for it.
_tc_BidrectionalPolicyValue = _tc_BidirectionalPolicyValue

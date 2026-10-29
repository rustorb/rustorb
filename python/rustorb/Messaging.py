"""The Messaging module of the IDL to Python mapping, as far as rustorb
has it: ``RelativeRoundtripTimeoutPolicy``, the standard form of the
client call timeouts (see ``rustorb.timeouts``).

Make the policy with ``orb.create_policy(RELATIVE_RT_TIMEOUT_POLICY_TYPE,
CORBA.Any(TimeBase._tc_TimeT, t))``, ``t`` in units of 100 ns, and apply
it with ``obj._set_policy_overrides``, the ORB's ``ORBPolicyManager`` or
the thread's ``PolicyCurrent``.
"""

from . import CORBA

RELATIVE_RT_TIMEOUT_POLICY_TYPE = 32


class RelativeRoundtripTimeoutPolicy(CORBA.Policy):
    """How long a call may take: ``relative_expiry``, a ``TimeBase::TimeT``
    (units of 100 ns)."""

    _NP_RepositoryId = "IDL:omg.org/Messaging/RelativeRoundtripTimeoutPolicy:1.0"
    policy_type = RELATIVE_RT_TIMEOUT_POLICY_TYPE

    def _get_relative_expiry(self):
        return self.value

    relative_expiry = property(_get_relative_expiry)

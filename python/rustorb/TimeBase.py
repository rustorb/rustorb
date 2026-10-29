"""The TimeBase module of the IDL to Python mapping, as far as rustorb
needs it: ``TimeT`` for ``Messaging.RelativeRoundtripTimeoutPolicy``."""

from . import CORBA

#: ``TimeBase::TimeT``: an unsigned long long, in units of 100 ns.
_tc_TimeT = CORBA.TypeCode(
    CORBA.tk_alias, "IDL:omg.org/TimeBase/TimeT:1.0", "TimeT", content=CORBA.TC_ulonglong
)

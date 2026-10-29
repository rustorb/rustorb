//! Client call timeouts: the defaults of the ORB, of the calling thread and
//! of an object reference, and which one a call uses.

use std::cell::Cell;
use std::time::Duration;

/// A stored timeout that is not set, so the next default applies.
pub(crate) const UNSET: u64 = u64::MAX;

/// Stores a timeout in an atomic: 0 for no limit, else units of 100
/// nanoseconds (at least 1), the unit of `TimeBase::TimeT`, so that a
/// `RelativeRoundtripTimeoutPolicy` reads back the value it was set to.
pub(crate) fn encode(timeout: Option<Duration>) -> u64 {
    match timeout {
        None => 0,
        Some(t) => u64::try_from(t.as_nanos() / 100)
            .unwrap_or(UNSET - 1)
            .clamp(1, UNSET - 1),
    }
}

/// The timeout `encode` stored (not [`UNSET`]).
pub(crate) fn decode(v: u64) -> Option<Duration> {
    (v != 0).then(|| time_t_to_duration(v))
}

/// A `TimeBase::TimeT` (units of 100 nanoseconds) as a duration.
pub(crate) fn time_t_to_duration(t: u64) -> Duration {
    Duration::new(t / 10_000_000, (t % 10_000_000) as u32 * 100)
}

/// A duration as a `TimeBase::TimeT`, rounded down (saturating).
pub(crate) fn duration_to_time_t(d: Duration) -> u64 {
    u64::try_from(d.as_nanos() / 100).unwrap_or(u64::MAX)
}

thread_local! {
    static THREAD_CALL_TIMEOUT: Cell<Option<Option<Duration>>> = const { Cell::new(None) };
}

/// Sets the call timeout of the calling thread, for every ORB: `Some(t)`
/// (where `t` is `None` for no limit) overrides the ORB's
/// [`call_timeout`](crate::Orb::call_timeout), but not the object
/// reference's ([`ObjectRef::set_call_timeout`](crate::ObjectRef::set_call_timeout));
/// `None` unsets it. Like omniORB's `setClientThreadCallTimeout`, which
/// omniORB honours only with `-ORBsupportPerThreadTimeOut 1`; rustorb
/// always does. Returns the previous setting.
pub fn set_thread_call_timeout(timeout: Option<Option<Duration>>) -> Option<Option<Duration>> {
    THREAD_CALL_TIMEOUT.with(|c| c.replace(timeout))
}

/// The setting of [`set_thread_call_timeout`] on the calling thread.
pub fn thread_call_timeout() -> Option<Option<Duration>> {
    THREAD_CALL_TIMEOUT.with(Cell::get)
}

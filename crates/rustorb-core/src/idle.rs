//! When connections were last used, for closing the idle ones
//! (`-ORBoutConScanPeriod`, `-ORBinConScanPeriod`; docs/connections.md).

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

/// The time base of [`Stamp`].
#[cfg(not(target_os = "linux"))]
fn epoch() -> std::time::Instant {
    use std::sync::OnceLock;
    use std::time::Instant;
    static EPOCH: OnceLock<Instant> = OnceLock::new();
    *EPOCH.get_or_init(Instant::now)
}

/// Now, in microseconds. On Linux from the coarse monotonic clock, which
/// is read without asking the hardware (a stamp is taken several times
/// a call, and idle timeouts are 10 ms and longer); its few milliseconds
/// of resolution do not matter here.
#[cfg(target_os = "linux")]
fn now_micros() -> u64 {
    let mut ts = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: one valid timespec. The clock is always there on Linux.
    unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC_COARSE, &mut ts) };
    let secs = u64::try_from(ts.tv_sec).unwrap_or(0);
    let nanos = u64::try_from(ts.tv_nsec).unwrap_or(0);
    secs.saturating_mul(1_000_000).saturating_add(nanos / 1000)
}

#[cfg(not(target_os = "linux"))]
fn now_micros() -> u64 {
    u64::try_from(epoch().elapsed().as_micros()).unwrap_or(u64::MAX)
}

/// The last time a connection was used, kept without a lock.
#[derive(Debug)]
pub(crate) struct Stamp(AtomicU64);

impl Stamp {
    /// A stamp of now.
    pub(crate) fn new() -> Stamp {
        Stamp(AtomicU64::new(now_micros()))
    }

    /// Records a use now.
    pub(crate) fn touch(&self) {
        self.0.store(now_micros(), Ordering::Relaxed);
    }

    /// How long ago the last use was.
    pub(crate) fn idle_for(&self) -> Duration {
        Duration::from_micros(now_micros().saturating_sub(self.0.load(Ordering::Relaxed)))
    }
}

/// How often the connections are looked at for an idle timeout of
/// `timeout`: a connection is closed between `timeout` and a quarter more.
pub(crate) fn scan_period(timeout: Duration) -> Duration {
    (timeout / 4).clamp(Duration::from_millis(10), Duration::from_secs(5))
}

/// How long a server waits, after `CloseConnection`, for the client to
/// close its side before it stops reading the connection.
pub(crate) const CLOSE_LINGER: Duration = Duration::from_secs(10);

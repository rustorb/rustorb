//! When connections were last used, for closing the idle ones
//! (`-ORBoutConScanPeriod`, `-ORBinConScanPeriod`; docs/connections.md).

use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// The time base of [`Stamp`].
fn epoch() -> Instant {
    static EPOCH: OnceLock<Instant> = OnceLock::new();
    *EPOCH.get_or_init(Instant::now)
}

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

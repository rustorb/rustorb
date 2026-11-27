//! Counters of what the ORB does inside, for measuring speed work
//! (docs/perf-plan.md): how often a thread read a connection itself and how
//! often the I/O task did, how often a read found nothing, how requests got
//! to the upcall threads, and how replies were written.
//!
//! The counters are process-wide and off at first; while off, counting is
//! one relaxed load. Turn them on with [`set_enabled`] (or
//! `-ORBstatistics 1`), take a [`snapshot`] before and after what you
//! measure, and subtract ([`Snapshot::since`]).
//!
//! ```
//! use rustorb_core::stats;
//!
//! stats::set_enabled(true);
//! let before = stats::snapshot();
//! // ... calls ...
//! let used = stats::snapshot().since(&before);
//! for (name, n) in used.iter() {
//!     println!("{name}: {n}");
//! }
//! ```
//!
//! The names and their meaning may change between releases: they follow
//! the code they count.

use std::fmt;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

macro_rules! counters {
    ($($(#[doc = $doc:literal])* $name:ident,)*) => {
        /// One counter.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        #[allow(non_camel_case_types)]
        #[non_exhaustive]
        pub enum Counter {
            $($(#[doc = $doc])* $name,)*
        }

        impl Counter {
            /// Every counter, in the order of [`Snapshot::iter`].
            pub const ALL: &'static [Counter] = &[$(Counter::$name,)*];

            /// The counter's name, as in [`Snapshot::iter`].
            pub const fn name(self) -> &'static str {
                match self {
                    $(Counter::$name => stringify!($name),)*
                }
            }
        }
    };
}

counters! {
    /// The I/O task of a connection was woken to read (readiness or a
    /// thread handing the reading back).
    task_wakeups,
    /// The I/O task was woken while a thread held the reading: a wake-up
    /// for nothing (the thread reads what arrived).
    task_wakeups_thread_reads,
    /// The I/O task was woken while an upcall thread handled a request of
    /// the connection, and found nothing more to read.
    task_wakeups_busy_empty,
    /// Messages the I/O task read.
    task_messages,
    /// Socket reads (`recv`), by the task or a thread.
    recv_calls,
    /// Socket reads that found nothing (`EAGAIN`).
    recv_would_block,
    /// A thread waited for the socket (`poll`).
    thread_polls,
    /// A thread took the reading of a connection.
    claims,
    /// A thread wanted the reading and the I/O task kept it.
    claim_failures,
    /// The reading of a connection was registered with the runtime's
    /// poller again, as a thread gave it back (Unix).
    watches,
    /// The reading of a connection was taken off the runtime's poller, as
    /// a thread took it (Unix).
    unwatches,
    /// Messages a thread read itself.
    thread_messages,
    /// A thread waited for a message until its time ran out.
    thread_read_timeouts,
    /// A thread gave the reading back to the I/O task with something
    /// for it to read (the task is woken).
    hand_backs,
    /// An upcall thread started following a connection after a request
    /// (server).
    follows,
    /// Requests an upcall thread read while following (server).
    follow_requests,
    /// A follow ended because jobs waited in the pool (server).
    follow_stops_queued,
    /// A follow ended after `-ORBfollowLimit` requests (server).
    follow_stops_limit,
    /// The I/O task left the reading to the upcall thread it gave a
    /// request to, which follows the connection after it (server).
    follow_pauses,
    /// An upcall thread held the reading longer than `-ORBfollowLinger`
    /// while it handled a request: the I/O task got the reading back, so
    /// that requests behind it do not wait (server).
    follow_overdue,
    /// A caller read its reply itself (client).
    replies_read_here,
    /// A caller waited for the I/O task to hand it its reply (client).
    replies_from_task,
    /// Jobs given to the upcall pool.
    pool_jobs,
    /// A job woke an idle upcall thread.
    pool_wakes,
    /// A job started a new upcall thread.
    pool_spawns,
    /// A job waited in the queue: no thread idle, and the pool at its
    /// maximum.
    pool_queued,
    /// An upcall thread went to sleep for want of jobs.
    pool_sleeps,
    /// Server messages written at once by the thread that made them.
    outbox_direct,
    /// Server messages (or their rest) left to the connection's writer
    /// task.
    outbox_queued,
}

const N: usize = Counter::ALL.len();

static ENABLED: AtomicBool = AtomicBool::new(false);
#[allow(clippy::declare_interior_mutable_const)]
const ZERO: AtomicU64 = AtomicU64::new(0);
static COUNTS: [AtomicU64; N] = [ZERO; N];

/// Turns counting on or off for the whole process (`-ORBstatistics 0|1`).
/// The counts are kept.
pub fn set_enabled(on: bool) {
    ENABLED.store(on, Ordering::Relaxed);
}

/// Whether the counters count.
pub fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

/// Adds one to `counter`, if counting is on.
#[inline]
pub(crate) fn count(counter: Counter) {
    if ENABLED.load(Ordering::Relaxed) {
        COUNTS[counter as usize].fetch_add(1, Ordering::Relaxed);
    }
}

/// The counts now.
pub fn snapshot() -> Snapshot {
    Snapshot {
        counts: std::array::from_fn(|i| COUNTS[i].load(Ordering::Relaxed)),
    }
}

/// Sets every count to 0.
pub fn reset() {
    for c in &COUNTS {
        c.store(0, Ordering::Relaxed);
    }
}

/// The counts at one moment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    counts: [u64; N],
}

impl Snapshot {
    /// The count of `counter`.
    pub fn get(&self, counter: Counter) -> u64 {
        self.counts[counter as usize]
    }

    /// What was counted from `earlier` to this one.
    pub fn since(&self, earlier: &Snapshot) -> Snapshot {
        Snapshot {
            counts: std::array::from_fn(|i| self.counts[i].saturating_sub(earlier.counts[i])),
        }
    }

    /// Every counter's name and count.
    pub fn iter(&self) -> impl Iterator<Item = (&'static str, u64)> + '_ {
        Counter::ALL.iter().map(|&c| (c.name(), self.get(c)))
    }
}

impl fmt::Display for Snapshot {
    /// One JSON object: `{"name":count,...}`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("{")?;
        for (i, (name, n)) in self.iter().enumerate() {
            if i > 0 {
                f.write_str(",")?;
            }
            write!(f, "\"{name}\":{n}")?;
        }
        f.write_str("}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_when_enabled() {
        // Other tests in the process may count too: compare counts of a
        // counter only this test moves while it runs is not possible, so
        // check only the lower bound and the JSON form.
        set_enabled(true);
        let before = snapshot();
        count(Counter::pool_sleeps);
        count(Counter::pool_sleeps);
        let used = snapshot().since(&before);
        assert!(used.get(Counter::pool_sleeps) >= 2);
        let json = used.to_string();
        assert!(json.starts_with("{\"task_wakeups\":"));
        assert!(json.contains("\"pool_sleeps\":"));
        assert_eq!(used.iter().count(), Counter::ALL.len());
    }
}

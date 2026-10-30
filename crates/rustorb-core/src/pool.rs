//! The upcall thread pool.
//!
//! Servants run on long-lived OS threads owned by the ORB, never on the
//! tokio workers. `min` threads are started with the pool and stay; more
//! are started while every thread is busy, up to `max`, and those end
//! after `idle_timeout` without work. Jobs beyond `max` wait in a queue.
//!
//! A job goes to the thread that became idle last, so that calls made one
//! after another run on the same thread (as they do with omniORB, which
//! serves a connection on one thread while it is not busy).

use std::collections::VecDeque;
use std::panic::{self, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::Duration;

pub(crate) type Job = Box<dyn FnOnce() + Send>;

#[derive(Debug)]
pub(crate) struct Pool {
    shared: Arc<Shared>,
}

#[derive(Debug)]
struct Shared {
    state: Mutex<State>,
    min: usize,
    max: usize,
    idle_timeout: Duration,
}

struct State {
    queue: VecDeque<Job>,
    /// Threads waiting for a job, the one idle the shortest last.
    idle: Vec<Arc<Waiter>>,
    /// Threads running, idle or not.
    threads: usize,
    stopping: bool,
}

impl std::fmt::Debug for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("State")
            .field("queued", &self.queue.len())
            .field("idle", &self.idle.len())
            .field("threads", &self.threads)
            .field("stopping", &self.stopping)
            .finish()
    }
}

/// How an idle thread is woken: each has its own condition variable (used
/// with the state's mutex), so that the pool chooses which one runs a job.
#[derive(Debug, Default)]
struct Waiter {
    cond: Condvar,
    woken: AtomicBool,
}

impl Waiter {
    fn wake(&self) {
        self.woken.store(true, Ordering::Relaxed);
        self.cond.notify_one();
    }
}

impl Pool {
    pub(crate) fn new(min: usize, max: usize, idle_timeout: Duration) -> Pool {
        let max = max.max(1);
        let pool = Pool {
            shared: Arc::new(Shared {
                state: Mutex::new(State {
                    queue: VecDeque::new(),
                    idle: Vec::new(),
                    threads: 0,
                    stopping: false,
                }),
                min: min.min(max),
                max,
                idle_timeout,
            }),
        };
        let mut state = pool.shared.state.lock().unwrap();
        for _ in 0..pool.shared.min {
            spawn(&pool.shared, &mut state);
        }
        drop(state);
        pool
    }

    /// Queues `job` for the thread that became idle last, starting a
    /// thread if none is idle and the pool is below its maximum.
    pub(crate) fn submit(&self, job: Job) {
        let mut state = self.shared.state.lock().unwrap();
        if state.stopping {
            return;
        }
        state.queue.push_back(job);
        // Threads woken before take the jobs queued before.
        if let Some(waiter) = state.idle.pop() {
            waiter.woken.store(true, Ordering::Relaxed);
            // Notified after the lock is released, so that the thread does
            // not wake only to wait for it.
            drop(state);
            waiter.cond.notify_one();
        } else if state.threads < self.shared.max {
            spawn(&self.shared, &mut state);
        }
    }

    /// Threads running, idle or not.
    #[cfg(test)]
    pub(crate) fn threads(&self) -> usize {
        self.shared.state.lock().unwrap().threads
    }
}

impl Drop for Pool {
    /// Lets every thread finish its current job and end. Queued jobs are
    /// dropped.
    fn drop(&mut self) {
        let mut state = self.shared.state.lock().unwrap();
        state.stopping = true;
        let queued = std::mem::take(&mut state.queue);
        for waiter in state.idle.drain(..) {
            waiter.wake();
        }
        drop(state);
        drop(queued);
    }
}

fn spawn(shared: &Arc<Shared>, state: &mut State) {
    let worker = Arc::clone(shared);
    let started = thread::Builder::new()
        .name("rustorb-upcall".into())
        .spawn(move || work(&worker));
    if started.is_ok() {
        state.threads += 1;
    }
}

fn work(shared: &Shared) {
    let me = Arc::new(Waiter::default());
    let mut state = shared.state.lock().unwrap();
    loop {
        if let Some(job) = state.queue.pop_front() {
            drop(state);
            // Jobs catch panics of user code themselves; this keeps the
            // thread alive if anything else panics.
            let _ = panic::catch_unwind(AssertUnwindSafe(job));
            state = shared.state.lock().unwrap();
            continue;
        }
        if state.stopping {
            break;
        }
        me.woken.store(false, Ordering::Relaxed);
        state.idle.push(Arc::clone(&me));
        let mut timed_out = false;
        while !me.woken.load(Ordering::Relaxed) && !timed_out {
            let (next, timeout) = me.cond.wait_timeout(state, shared.idle_timeout).unwrap();
            state = next;
            timed_out = timeout.timed_out();
        }
        if !me.woken.load(Ordering::Relaxed) {
            state.idle.retain(|w| !Arc::ptr_eq(w, &me));
            if state.queue.is_empty() && state.threads > shared.min {
                break;
            }
        }
    }
    state.threads -= 1;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Barrier;
    use std::sync::mpsc;

    #[test]
    fn grows_while_busy_and_shrinks_when_idle() {
        let pool = Pool::new(1, 8, Duration::from_millis(100));
        assert_eq!(pool.threads(), 1);
        // Eight jobs that only finish together need eight threads.
        let barrier = Arc::new(Barrier::new(8));
        let (tx, rx) = mpsc::channel();
        for _ in 0..8 {
            let barrier = Arc::clone(&barrier);
            let tx = tx.clone();
            pool.submit(Box::new(move || {
                barrier.wait();
                tx.send(thread::current().id()).unwrap();
            }));
        }
        let mut ids: Vec<_> = (0..8)
            .map(|_| rx.recv_timeout(Duration::from_secs(5)).unwrap())
            .collect();
        ids.sort_by_key(|id| format!("{id:?}"));
        ids.dedup();
        assert_eq!(ids.len(), 8);
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while pool.threads() > 1 && std::time::Instant::now() < deadline {
            thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(pool.threads(), 1);
    }

    #[test]
    fn jobs_beyond_the_maximum_wait() {
        let pool = Pool::new(0, 2, Duration::from_secs(60));
        let (tx, rx) = mpsc::channel();
        for i in 0..10 {
            let tx = tx.clone();
            pool.submit(Box::new(move || {
                thread::sleep(Duration::from_millis(5));
                tx.send(i).unwrap();
            }));
        }
        let mut got: Vec<i32> = (0..10)
            .map(|_| rx.recv_timeout(Duration::from_secs(5)).unwrap())
            .collect();
        got.sort_unstable();
        assert_eq!(got, (0..10).collect::<Vec<_>>());
        assert!(pool.threads() <= 2);
    }

    #[test]
    fn jobs_one_after_another_run_on_the_same_thread() {
        let pool = Pool::new(4, 8, Duration::from_secs(60));
        // Let the threads start and go idle.
        thread::sleep(Duration::from_millis(50));
        let (tx, rx) = mpsc::channel();
        let mut ids = Vec::new();
        for _ in 0..6 {
            let tx = tx.clone();
            pool.submit(Box::new(move || tx.send(thread::current().id()).unwrap()));
            ids.push(rx.recv_timeout(Duration::from_secs(5)).unwrap());
            // Let the thread go idle again.
            thread::sleep(Duration::from_millis(10));
        }
        ids.dedup();
        assert_eq!(ids.len(), 1);
    }

    #[test]
    fn a_panicking_job_does_not_end_the_thread() {
        let pool = Pool::new(1, 1, Duration::from_secs(60));
        pool.submit(Box::new(|| panic!("job panics")));
        let (tx, rx) = mpsc::channel();
        pool.submit(Box::new(move || tx.send(()).unwrap()));
        rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(pool.threads(), 1);
    }
}

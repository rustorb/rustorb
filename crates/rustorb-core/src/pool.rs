//! The upcall thread pool.
//!
//! Servants run on long-lived OS threads owned by the ORB, never on the
//! tokio workers. `min` threads are started with the pool and stay; more
//! are started while every thread is busy, up to `max`, and those end
//! after `idle_timeout` without work. Jobs beyond `max` wait in a queue.

use std::collections::VecDeque;
use std::panic::{self, AssertUnwindSafe};
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
    cond: Condvar,
    min: usize,
    max: usize,
    idle_timeout: Duration,
}

struct State {
    queue: VecDeque<Job>,
    /// Threads waiting for a job.
    idle: usize,
    /// Threads running, idle or not.
    threads: usize,
    stopping: bool,
}

impl std::fmt::Debug for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("State")
            .field("queued", &self.queue.len())
            .field("idle", &self.idle)
            .field("threads", &self.threads)
            .field("stopping", &self.stopping)
            .finish()
    }
}

impl Pool {
    pub(crate) fn new(min: usize, max: usize, idle_timeout: Duration) -> Pool {
        let max = max.max(1);
        let pool = Pool {
            shared: Arc::new(Shared {
                state: Mutex::new(State {
                    queue: VecDeque::new(),
                    idle: 0,
                    threads: 0,
                    stopping: false,
                }),
                cond: Condvar::new(),
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

    /// Queues `job` for the next free thread, starting a thread if none is
    /// free and the pool is below its maximum.
    pub(crate) fn submit(&self, job: Job) {
        let mut state = self.shared.state.lock().unwrap();
        if state.stopping {
            return;
        }
        state.queue.push_back(job);
        if state.idle >= state.queue.len() {
            self.shared.cond.notify_one();
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
        drop(state);
        self.shared.cond.notify_all();
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
        state.idle += 1;
        let (next, timeout) = shared
            .cond
            .wait_timeout(state, shared.idle_timeout)
            .unwrap();
        state = next;
        state.idle -= 1;
        if timeout.timed_out() && state.queue.is_empty() && state.threads > shared.min {
            break;
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
    fn a_panicking_job_does_not_end_the_thread() {
        let pool = Pool::new(1, 1, Duration::from_secs(60));
        pool.submit(Box::new(|| panic!("job panics")));
        let (tx, rx) = mpsc::channel();
        pool.submit(Box::new(move || tx.send(()).unwrap()));
        rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(pool.threads(), 1);
    }
}

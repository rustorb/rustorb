//! Runs a callback of the caller off the threads of an ORB's runtime.
//!
//! The arguments of a request of rorb_object_sendc are written when the
//! request goes out, on a thread of the ORB's I/O runtime. The callbacks
//! that write them are the caller's code (the JVM, for the Java mapping),
//! which should not run there: it would hold up the I/O of every connection
//! of the ORB.
//! They run on a thread of this module instead, while the runtime thread
//! waits (in tokio's block_in_place, so that its other tasks move on).

use std::any::Any;
use std::panic::{self, AssertUnwindSafe};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use tokio::runtime::{Handle, RuntimeFlavor};

type Job = Box<dyn FnOnce() + Send + 'static>;

/// The threads waiting for a job, with their ids.
static IDLE: Mutex<Vec<(u64, mpsc::Sender<Job>)>> = Mutex::new(Vec::new());
static NEXT_ID: AtomicU64 = AtomicU64::new(0);
/// An idle thread ends after this long.
const IDLE_TIMEOUT: Duration = Duration::from_secs(60);

/// Runs `f` and returns what it returns: on a thread of this module when
/// called on a thread of a multi-threaded tokio runtime, else here.
pub(crate) fn run<R: Send>(f: impl FnOnce() -> R + Send) -> R {
    match Handle::try_current() {
        Ok(h) if h.runtime_flavor() == RuntimeFlavor::MultiThread => {
            tokio::task::block_in_place(|| elsewhere(f))
        }
        _ => f(),
    }
}

/// The result of a job, and its condition.
struct Done<R> {
    result: Mutex<Option<Result<R, Box<dyn Any + Send>>>>,
    cond: Condvar,
}

fn elsewhere<R: Send>(f: impl FnOnce() -> R + Send) -> R {
    let done = Done::<R> {
        result: Mutex::new(None),
        cond: Condvar::new(),
    };
    let done_ref: &Done<R> = &done;
    let job: Box<dyn FnOnce() + Send + '_> = Box::new(move || {
        let r = panic::catch_unwind(AssertUnwindSafe(f));
        // Notified under the lock: once the waiter sees the result it
        // returns, and done is gone.
        let mut result = done_ref.result.lock().unwrap();
        *result = Some(r);
        done_ref.cond.notify_one();
        drop(result);
    });
    // SAFETY: this function does not return before the job has run (it
    // waits for its result below), so what the job borrows outlives it.
    let job: Job = unsafe { std::mem::transmute::<Box<dyn FnOnce() + Send + '_>, Job>(job) };
    if let Err(job) = submit(job) {
        job();
    }
    let mut result = done.result.lock().unwrap();
    loop {
        if let Some(r) = result.take() {
            return match r {
                Ok(v) => v,
                Err(p) => panic::resume_unwind(p),
            };
        }
        result = done.cond.wait(result).unwrap();
    }
}

/// Hands `job` to an idle thread or a new one; gives it back if no thread
/// could be started.
fn submit(mut job: Job) -> Result<(), Job> {
    loop {
        let idle = IDLE.lock().unwrap().pop();
        let Some((_, sender)) = idle else { break };
        match sender.send(job) {
            Ok(()) => return Ok(()),
            // That thread has ended.
            Err(mpsc::SendError(j)) => job = j,
        }
    }
    let (sender, receiver) = mpsc::channel::<Job>();
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    // Shared, so that it comes back if the thread does not start.
    let first = Arc::new(Mutex::new(Some(job)));
    let theirs = Arc::clone(&first);
    let spawned = std::thread::Builder::new()
        .name("rustorb-caller".into())
        .spawn(move || {
            let job = theirs.lock().unwrap().take();
            work(id, sender, receiver, job)
        });
    match spawned {
        Ok(_) => Ok(()),
        Err(_) => match first.lock().unwrap().take() {
            Some(job) => Err(job),
            None => Ok(()),
        },
    }
}

fn work(id: u64, sender: mpsc::Sender<Job>, receiver: mpsc::Receiver<Job>, first: Option<Job>) {
    if let Some(job) = first {
        job();
    }
    loop {
        IDLE.lock().unwrap().push((id, sender.clone()));
        let job = loop {
            match receiver.recv_timeout(IDLE_TIMEOUT) {
                Ok(job) => break job,
                Err(RecvTimeoutError::Timeout) => {
                    let mut idle = IDLE.lock().unwrap();
                    if let Some(i) = idle.iter().position(|(i, _)| *i == id) {
                        // Nobody can send to this thread any more.
                        idle.remove(i);
                        return;
                    }
                    // Taken from IDLE: a job is on its way.
                }
                Err(RecvTimeoutError::Disconnected) => return,
            }
        };
        job();
    }
}

//! Trace output to Python's `logging` (docs/tracing.md).
//!
//! [`_set_log_sink`] installs a global `tracing` subscriber that takes the
//! core's events the trace level (`-ORBtraceLevel`) selects. The threads
//! that report them (I/O threads among them) only queue them: a thread of
//! its own hands them to the sink, a Python function of `rustorb._log`,
//! with the interpreter attached. A thread reporting an event therefore
//! never waits for the GIL, which a Python thread may hold while it waits
//! for that thread.

use std::collections::VecDeque;
use std::fmt;
use std::sync::{Condvar, Mutex, OnceLock};

use pyo3::prelude::*;
use rustorb_core::trace;
use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Id, Record};
use tracing::subscriber::Interest;
use tracing::{Event, Level, Metadata, Subscriber};

/// One event: level name, target, message, and the line standard error
/// would get.
type Entry = (&'static str, &'static str, String, String);

struct Queue {
    entries: Mutex<VecDeque<Entry>>,
    ready: Condvar,
}

static QUEUE: Queue = Queue {
    entries: Mutex::new(VecDeque::new()),
    ready: Condvar::new(),
};

static SINK: OnceLock<Py<PyAny>> = OnceLock::new();

/// Takes the core's events into the queue.
struct ToLogging;

fn is_rustorb(meta: &Metadata<'_>) -> bool {
    meta.target().starts_with("rustorb")
}

impl Subscriber for ToLogging {
    fn register_callsite(&self, meta: &'static Metadata<'static>) -> Interest {
        if is_rustorb(meta) && meta.is_event() {
            // The trace level may change: asked each time.
            Interest::sometimes()
        } else {
            Interest::never()
        }
    }

    fn enabled(&self, meta: &Metadata<'_>) -> bool {
        is_rustorb(meta) && trace::selects(*meta.level())
    }

    fn new_span(&self, _: &Attributes<'_>) -> Id {
        Id::from_u64(1)
    }

    fn record(&self, _: &Id, _: &Record<'_>) {}

    fn record_follows_from(&self, _: &Id, _: &Id) {}

    fn event(&self, event: &Event<'_>) {
        let meta = event.metadata();
        let mut message = Message(String::new());
        event.record(&mut message);
        let level = *meta.level();
        let line = trace::stderr_line(level, &message.0);
        let entry = (trace::level_name(level), meta.target(), message.0, line);
        QUEUE.entries.lock().unwrap().push_back(entry);
        QUEUE.ready.notify_one();
    }

    fn enter(&self, _: &Id) {}

    fn exit(&self, _: &Id) {}
}

struct Message(String);

impl Visit for Message {
    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        use fmt::Write;
        if field.name() == "message" {
            let _ = write!(self.0, "{value:?}");
        } else {
            let _ = write!(self.0, " {}={value:?}", field.name());
        }
    }
}

fn take_all() -> Vec<Entry> {
    QUEUE.entries.lock().unwrap().drain(..).collect()
}

fn deliver(py: Python<'_>, entries: Vec<Entry>) {
    if entries.is_empty() {
        return;
    }
    if let Some(sink) = SINK.get() {
        // A failing sink loses its entries; there is nowhere to report it.
        let _ = sink.call1(py, (entries,));
    }
}

/// Hands queued entries to the sink as they come.
fn drain() {
    loop {
        let entries = {
            let mut queue = QUEUE.entries.lock().unwrap();
            while queue.is_empty() {
                queue = QUEUE.ready.wait(queue).unwrap();
            }
            queue.drain(..).collect::<Vec<_>>()
        };
        // Not while the interpreter shuts down: the entries are dropped.
        let _ = Python::try_attach(|py| deliver(py, entries));
    }
}

/// Sends the core's trace events to `sink`, called with a list of
/// `(level, target, message, line)` tuples. Returns false when another
/// global `tracing` subscriber was set first (which then gets them).
#[pyfunction]
pub(crate) fn _set_log_sink(sink: Py<PyAny>) -> bool {
    if SINK.set(sink).is_err() {
        return true;
    }
    if tracing::subscriber::set_global_default(ToLogging).is_err() {
        return false;
    }
    let _ = std::thread::Builder::new()
        .name("rustorb-log".into())
        .spawn(drain);
    true
}

/// Hands what is queued to the sink now (at exit, say).
#[pyfunction]
pub(crate) fn _flush_log(py: Python<'_>) {
    deliver(py, take_all());
}

/// Sets the trace level of the process, like `-ORBtraceLevel`.
#[pyfunction]
pub(crate) fn set_trace_level(level: u32) {
    trace::set_trace_level(level);
}

/// The trace level of the process.
#[pyfunction]
pub(crate) fn trace_level() -> u32 {
    trace::trace_level()
}

/// The lowest trace level that selects events of `level` ("warning",
/// "info", "debug", "trace").
#[pyfunction]
pub(crate) fn trace_level_of(level: &str) -> Option<u32> {
    let level = match level {
        "error" => Level::ERROR,
        "warning" => Level::WARN,
        "info" => Level::INFO,
        "debug" => Level::DEBUG,
        "trace" => Level::TRACE,
        _ => return None,
    };
    Some(trace::trace_level_of(level))
}

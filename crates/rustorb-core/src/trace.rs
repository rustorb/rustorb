//! Trace output: what the ORB does, for finding interoperability and
//! configuration problems (docs/tracing.md).
//!
//! The core reports its events through the [`tracing`] crate, with the
//! module of each event as its target (`rustorb_core::connection`,
//! `rustorb_core::server`, ...). An application that has set a global
//! `tracing` subscriber receives every event and filters them itself.
//! Without one, the events the [trace level](set_trace_level) selects are
//! written to standard error, one line each, like omniORB's
//! `-ORBtraceLevel`:
//!
//! | Level | `tracing` level | Events |
//! | --- | --- | --- |
//! | 0 | | none |
//! | 1 (default) | `WARN` | servant and servant manager panics, failures the application should know of |
//! | 5 | `INFO` | connections opened, accepted and closed; endpoints; `LOCATION_FORWARD` followed; POAs created and destroyed |
//! | 10 | `DEBUG` | each request and its reply or system exception, client and server; objects activated and deactivated |
//! | 25 | `TRACE` | each GIOP message sent and received: type, size and version |
//!
//! A level includes the events of the levels below it.

use std::fmt;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

pub use tracing::Level;

/// The trace level of a `tracing` level: the lowest
/// [trace level](set_trace_level) that selects it.
pub const fn trace_level_of(level: Level) -> u32 {
    match level {
        Level::ERROR | Level::WARN => 1,
        Level::INFO => 5,
        Level::DEBUG => 10,
        Level::TRACE => 25,
    }
}

static LEVEL: AtomicU32 = AtomicU32::new(1);
static TIME: AtomicBool = AtomicBool::new(false);

/// Sets the trace level for the whole process (`-ORBtraceLevel N`; see
/// the [module](self) for what each level selects). It decides what goes
/// to standard error when no global `tracing` subscriber is set, and what
/// the Python binding hands to `logging`; a subscriber of the application
/// filters with its own settings.
pub fn set_trace_level(level: u32) {
    LEVEL.store(level, Ordering::Relaxed);
}

/// The trace level [`set_trace_level`] set; 1 at first.
pub fn trace_level() -> u32 {
    LEVEL.load(Ordering::Relaxed)
}

/// Whether trace lines on standard error start with the time, in UTC
/// (`-ORBtraceTime 0|1`).
pub fn set_trace_time(on: bool) {
    TIME.store(on, Ordering::Relaxed);
}

/// Whether the trace level selects events of `level`.
pub fn selects(level: Level) -> bool {
    trace_level() >= trace_level_of(level)
}

/// Whether an event of `level` goes to standard error: the trace level
/// selects it and no global subscriber is set.
#[inline]
pub(crate) fn to_stderr(level: Level) -> bool {
    selects(level) && !tracing::dispatcher::has_been_set()
}

/// Writes one trace line to standard error.
#[cold]
pub(crate) fn write_stderr(level: Level, args: fmt::Arguments<'_>) {
    eprintln!("{}", line(level, args, TIME.load(Ordering::Relaxed)));
}

/// The line for `message` that standard error would get:
/// `rustorb: <level>: <message>`, after the time with
/// [`set_trace_time`]. For bindings that write trace lines themselves.
pub fn stderr_line(level: Level, message: &str) -> String {
    line(
        level,
        format_args!("{message}"),
        TIME.load(Ordering::Relaxed),
    )
}

/// The text of a trace line: `rustorb: <level>: <message>`, after the time
/// when `time` is set.
fn line(level: Level, args: fmt::Arguments<'_>, time: bool) -> String {
    let name = level_name(level);
    if time {
        format!("{} rustorb: {name}: {args}", utc_now())
    } else {
        format!("rustorb: {name}: {args}")
    }
}

/// The name of `level` in trace lines (and in the Python binding).
pub fn level_name(level: Level) -> &'static str {
    match level {
        Level::ERROR => "error",
        Level::WARN => "warning",
        Level::INFO => "info",
        Level::DEBUG => "debug",
        Level::TRACE => "trace",
    }
}

/// The current time as `YYYY-MM-DD hh:mm:ss.uuuuuu`, UTC.
fn utc_now() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    format_utc(now.as_secs(), now.subsec_micros())
}

fn format_utc(secs: u64, micros: u32) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let (y, m, d) = civil_from_days(days);
    format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02}.{micros:06}",
        rem / 3600,
        rem / 60 % 60,
        rem % 60
    )
}

/// The proleptic Gregorian date of a day count since 1970-01-01.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

/// Reports an event: to the `tracing` subscriber, or to standard error
/// when there is none and the trace level selects it. The arguments are
/// evaluated only when the event goes somewhere.
macro_rules! event {
    ($level:ident, $($arg:tt)+) => {
        if $crate::trace::to_stderr(::tracing::Level::$level) {
            $crate::trace::write_stderr(::tracing::Level::$level, format_args!($($arg)+));
        } else {
            ::tracing::event!(::tracing::Level::$level, $($arg)+);
        }
    };
}

macro_rules! warning {
    ($($arg:tt)+) => { $crate::trace::event!(WARN, $($arg)+) };
}

macro_rules! info {
    ($($arg:tt)+) => { $crate::trace::event!(INFO, $($arg)+) };
}

macro_rules! debug {
    ($($arg:tt)+) => { $crate::trace::event!(DEBUG, $($arg)+) };
}

macro_rules! trace {
    ($($arg:tt)+) => { $crate::trace::event!(TRACE, $($arg)+) };
}

pub(crate) use {debug, event, info, trace, warning};

/// The name of a GIOP message type in trace lines, from the type octet of
/// its header.
pub(crate) fn message_type_name(octet: u8) -> &'static str {
    match octet {
        0 => "Request",
        1 => "Reply",
        2 => "CancelRequest",
        3 => "LocateRequest",
        4 => "LocateReply",
        5 => "CloseConnection",
        6 => "MessageError",
        7 => "Fragment",
        _ => "unknown",
    }
}

/// The type, version and size of a whole message (header included), for
/// trace lines: `Request (GIOP 1.2, 120 bytes)`.
pub(crate) struct MessageSummary<'a>(pub(crate) &'a [u8], pub(crate) usize);

impl fmt::Display for MessageSummary<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let MessageSummary(head, len) = *self;
        match head {
            [b'G', b'I', b'O', b'P', major, minor, _, kind, ..] => write!(
                f,
                "{} (GIOP {major}.{minor}, {len} bytes)",
                message_type_name(*kind)
            ),
            _ => write!(f, "a message of {len} bytes"),
        }
    }
}

/// An object reference in trace lines: its type and first IIOP address,
/// `IDL:Echo:1.0 at host:port`.
pub(crate) struct IorSummary<'a>(pub(crate) &'a rustorb_giop::Ior);

impl fmt::Display for IorSummary<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let ior = self.0;
        if ior.is_nil() {
            return f.write_str("a nil reference");
        }
        let id = if ior.type_id.is_empty() {
            "an object"
        } else {
            &ior.type_id
        };
        let first = ior
            .profiles
            .iter()
            .filter(|p| p.tag == rustorb_giop::TAG_INTERNET_IOP)
            .find_map(|p| p.iiop().ok());
        match first {
            Some(iiop) => write!(f, "{id} at {}:{}", iiop.host, iiop.port),
            None => write!(f, "{id} (no IIOP profile)"),
        }
    }
}

/// An ObjectId in trace lines: as text when it is printable ASCII, else
/// in hex.
pub(crate) struct IdSummary<'a>(pub(crate) &'a [u8]);

impl fmt::Display for IdSummary<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let id = self.0;
        if !id.is_empty() && id.iter().all(|b| b.is_ascii_graphic() || *b == b' ') {
            write!(f, "{:?}", String::from_utf8_lossy(id))
        } else {
            f.write_str("0x")?;
            id.iter().try_for_each(|b| write!(f, "{b:02x}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utc_dates() {
        assert_eq!(format_utc(0, 0), "1970-01-01 00:00:00.000000");
        assert_eq!(format_utc(951_782_400, 5), "2000-02-29 00:00:00.000005");
        assert_eq!(
            format_utc(1_793_569_356, 123_456),
            "2026-11-01 21:42:36.123456"
        );
    }

    #[test]
    fn lines() {
        assert_eq!(
            line(Level::WARN, format_args!("x {}", 1), false),
            "rustorb: warning: x 1"
        );
        assert!(line(Level::INFO, format_args!("y"), true).ends_with(" rustorb: info: y"));
    }

    #[test]
    fn levels() {
        assert_eq!(trace_level_of(Level::WARN), 1);
        assert_eq!(trace_level_of(Level::TRACE), 25);
        let msg = b"GIOP\x01\x02\x01\x00\x00\x00\x00\x00";
        assert_eq!(IdSummary(b"Echo").to_string(), "\"Echo\"");
        assert_eq!(IdSummary(&[0, 0xab]).to_string(), "0x00ab");
        assert_eq!(
            MessageSummary(msg, 12).to_string(),
            "Request (GIOP 1.2, 12 bytes)"
        );
    }
}

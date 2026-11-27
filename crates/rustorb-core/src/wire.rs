//! Reading GIOP messages off a stream, shared by client and server
//! connections.
//!
//! A connection is read either by its reader task on the runtime or, for a
//! while, by a thread outside it: on the server, the upcall thread that
//! just answered a request of the connection reads the next one (leader /
//! follower); on the client, the caller waiting for the only reply
//! outstanding reads it. [`Inbound`] hands the reading between the two so
//! that only one of them reads at a time; see docs/read-ownership.md.

use std::io;
use std::mem::MaybeUninit;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

use rustorb_giop::{FragmentAssembler, Message, MessageHeader, Version};
use tokio::sync::Notify;

use crate::sock::ReadHalf;
use crate::stats::{self, Counter};
use crate::tls::Tls;
use crate::trace;

/// A peer address in trace lines.
pub(crate) struct PeerAddr(pub(crate) Option<SocketAddr>);

impl std::fmt::Display for PeerAddr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            Some(addr) => addr.fmt(f),
            None => f.write_str("an unknown peer"),
        }
    }
}

/// How much a connection reads ahead: a small message and the header of
/// the next one usually come in with one read. Bodies larger than this
/// are read straight into their own buffer.
const READ_AHEAD: usize = 16 * 1024;

/// While a large message comes in, the socket counts as readable only
/// once this much more of it (or all the rest, if less) has arrived
/// ([`ReadHalf::set_recv_low_water`]): the reader then wakes once for
/// each such piece instead of for each segment. Linux lowers it to half
/// the receive buffer.
const LARGE_LOW_WATER: usize = 256 * 1024;

/// Why no message could be read.
#[derive(Debug)]
pub(crate) enum ReadError {
    /// The stream ended, or failed, before a message header.
    Closed(io::Error),
    /// The header is not a GIOP header.
    BadHeader(String),
    /// The message is longer than the limit; carries its version.
    TooLarge(usize, Version),
    /// The stream ended, or failed, inside a message.
    Truncated(io::Error),
    /// A whole message that could not be joined or decoded.
    Malformed(Version, String),
}

/// Reads one byte source into spare capacity; `WouldBlock` when nothing is
/// there now.
type ReadFn<'a> = dyn FnMut(&mut [MaybeUninit<u8>]) -> io::Result<usize> + 'a;

/// Cuts a stream into whole messages without waiting: what has arrived is
/// kept until the rest of the message does.
#[derive(Debug)]
struct MessageReader {
    /// Read-ahead; the bytes not taken yet are `buf[start..]`.
    buf: Vec<u8>,
    start: usize,
    /// A message larger than the read-ahead, read straight into its own
    /// buffer: its header and the bytes so far.
    large: Option<(MessageHeader, Vec<u8>)>,
    max_message_size: usize,
}

impl MessageReader {
    fn new(max_message_size: usize) -> MessageReader {
        MessageReader {
            buf: Vec::with_capacity(READ_AHEAD),
            start: 0,
            large: None,
            max_message_size,
        }
    }

    /// Whether the next message is larger than the read-ahead.
    fn starts_large(&self) -> bool {
        let buffered = &self.buf[self.start..];
        self.large.is_some()
            || buffered.len() >= MessageHeader::SIZE
                && MessageHeader::parse(&buffered[..MessageHeader::SIZE])
                    .is_ok_and(|h| h.total_len() > READ_AHEAD)
    }

    /// How many bytes the socket should have before it counts as
    /// readable: the rest of the large message being read, up to
    /// [`LARGE_LOW_WATER`]; else 1.
    fn low_water(&self) -> usize {
        match &self.large {
            Some((header, msg)) => (header.total_len() - msg.len()).clamp(1, LARGE_LOW_WATER),
            None => 1,
        }
    }

    /// Whether part of a message (or more) was read and not taken.
    fn has_buffered(&self) -> bool {
        self.start < self.buf.len() || self.large.is_some()
    }

    /// The next whole message, header included, in a buffer of its own
    /// (not zeroed first). `Ok(None)` when `read` has nothing more now.
    /// With `defer_large`, a message larger than the read-ahead is left
    /// for later ([`starts_large`](Self::starts_large) tells).
    fn next(
        &mut self,
        read: &mut ReadFn<'_>,
        defer_large: bool,
    ) -> Result<Option<(MessageHeader, Vec<u8>)>, ReadError> {
        loop {
            if let Some((header, msg)) = &mut self.large {
                let total = header.total_len();
                while msg.len() < total {
                    let left = total - msg.len();
                    match read(&mut msg.spare_capacity_mut()[..left]) {
                        Ok(0) => {
                            return Err(ReadError::Truncated(io::ErrorKind::UnexpectedEof.into()));
                        }
                        Ok(n) => {
                            // SAFETY: `read` initialised the first `n`
                            // bytes of the spare capacity.
                            unsafe { msg.set_len(msg.len() + n) };
                        }
                        Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                        Err(e) if e.kind() == io::ErrorKind::WouldBlock => return Ok(None),
                        Err(e) => return Err(ReadError::Truncated(e)),
                    }
                }
                return Ok(self.large.take());
            }
            let buffered = &self.buf[self.start..];
            if buffered.len() >= MessageHeader::SIZE {
                let header = MessageHeader::parse(&buffered[..MessageHeader::SIZE])
                    .map_err(|e| ReadError::BadHeader(e.to_string()))?;
                let total = header.total_len();
                if total > self.max_message_size {
                    return Err(ReadError::TooLarge(total, header.version));
                }
                if buffered.len() >= total {
                    let msg = buffered[..total].to_vec();
                    self.start += total;
                    return Ok(Some((header, msg)));
                }
                if total > READ_AHEAD {
                    if defer_large {
                        return Ok(None);
                    }
                    let mut msg = Vec::with_capacity(total);
                    msg.extend_from_slice(buffered);
                    self.start = self.buf.len();
                    self.large = Some((header, msg));
                    continue;
                }
            }
            match self.fill(read) {
                Ok(true) => {}
                Ok(false) => return Ok(None),
                Err(e) => return Err(e),
            }
        }
    }

    /// Reads what `read` has into the read-ahead. `Ok(false)` when it had
    /// nothing.
    fn fill(&mut self, read: &mut ReadFn<'_>) -> Result<bool, ReadError> {
        if self.start == self.buf.len() {
            self.buf.clear();
            self.start = 0;
        } else if self.buf.len() == self.buf.capacity() {
            self.buf.drain(..self.start);
            self.start = 0;
        }
        let at_boundary = self.start == self.buf.len();
        loop {
            match read(self.buf.spare_capacity_mut()) {
                Ok(0) => {
                    let eof = io::Error::from(io::ErrorKind::UnexpectedEof);
                    return Err(if at_boundary {
                        ReadError::Closed(eof)
                    } else {
                        ReadError::Truncated(eof)
                    });
                }
                Ok(n) => {
                    // SAFETY: `read` initialised the first `n` bytes of the
                    // spare capacity.
                    unsafe { self.buf.set_len(self.buf.len() + n) };
                    return Ok(true);
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => return Ok(false),
                Err(e) if at_boundary => return Err(ReadError::Closed(e)),
                Err(e) => return Err(ReadError::Truncated(e)),
            }
        }
    }

    /// Reads into the read-ahead until `read` has nothing more or it is
    /// full, without taking a message. Returns whether there is anything
    /// for [`next`](Self::next): something buffered, or a failure, which
    /// `read` reports again.
    fn drain(&mut self, read: &mut ReadFn<'_>) -> bool {
        if self.large.is_none() {
            while self.buf.len() < self.buf.capacity() || self.start > 0 {
                match self.fill(read) {
                    Ok(true) => {}
                    Ok(false) => break,
                    Err(_) => return true,
                }
            }
        }
        self.has_buffered()
    }
}

/// The read side of a connection: what was read and not taken, and the
/// fragments of messages not complete yet.
#[derive(Debug)]
pub(crate) struct ReadSide {
    reader: MessageReader,
    pub(crate) assembler: FragmentAssembler,
    /// A message or failure that a thread read and left for the reader
    /// task.
    held: Option<Result<(Version, Message), ReadError>>,
    /// The address of the peer, for trace lines.
    peer: Option<SocketAddr>,
    /// The last message a thread read had to be waited for
    /// ([`FirstRead::AsBefore`]).
    waited: bool,
    /// The low-water mark set on the socket (1, the default, until a
    /// large message comes); `None` when it is left alone (TLS: what the
    /// socket has is not what the reader still needs).
    low_water: Option<usize>,
}

impl ReadSide {
    /// The next whole message, fragments joined. `Ok(None)` when `read`
    /// has nothing more now.
    fn next(
        &mut self,
        stream: &ReadHalf,
        read: &mut ReadFn<'_>,
        defer_large: bool,
    ) -> Result<Option<(Version, Message)>, ReadError> {
        if let Some(held) = self.held.take() {
            return held.map(Some);
        }
        loop {
            let next = self.reader.next(read, defer_large);
            self.update_low_water(stream);
            let Some((header, msg)) = next? else {
                return Ok(None);
            };
            trace::trace!(
                "received {} from {}",
                trace::MessageSummary(&msg, msg.len()),
                PeerAddr(self.peer)
            );
            let version = header.version;
            let malformed = |e: rustorb_giop::Error| ReadError::Malformed(version, e.to_string());
            let Some(whole) = self.assembler.push_owned(msg).map_err(malformed)? else {
                continue;
            };
            let (_, message) = whole.into_decoded().map_err(malformed)?;
            return Ok(Some((version, message)));
        }
    }
}

impl ReadSide {
    /// Sets the socket's low-water mark to what the reader needs now
    /// ([`MessageReader::low_water`]), if that changed.
    fn update_low_water(&mut self, stream: &ReadHalf) {
        let Some(current) = &mut self.low_water else {
            return;
        };
        let want = self.reader.low_water();
        if want != *current {
            stream.set_recv_low_water(want);
            *current = want;
            stats::count(Counter::low_water_sets);
        }
    }
}

/// Who reads the connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Holder {
    /// The reader task; `active` while it reads or handles what it read.
    Task { active: bool },
    /// A thread reads, or waits for the stream to become readable.
    Thread,
    /// The thread that read last (or, after [`Inbound::pause`], the
    /// thread the task gave a request to) handles what it read and means
    /// to read again. With `watched` the reading is on the runtime's
    /// poller and the reader task reads if more arrives meanwhile; without
    /// it nobody reads until the thread is back, or until the thread has
    /// been busy too long ([`Inbound::overdue`]).
    Busy { watched: bool },
}

/// Whether [`Inbound::read_here`] waits for the socket before it first
/// reads, when nothing is buffered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FirstRead {
    /// It waits: what it reads has seldom arrived yet (a reply to a
    /// request just sent).
    Wait,
    /// It waits if the last message it read on the connection had to be
    /// waited for: when messages come one by one, the next has not
    /// arrived yet either; when they stream in, it has.
    AsBefore,
}

/// What [`Inbound::read_here`] got.
#[derive(Debug)]
pub(crate) enum Here {
    /// A whole message; the thread still holds the reading.
    Message(Version, Message),
    /// Nothing came before the deadline; the reading went back to the
    /// task.
    TimedOut,
    /// The stream ended or failed; that is left for the task, which has
    /// the reading back.
    HandedBack,
}

/// The read half of a connection, read by its reader task or, for a while,
/// by one thread.
///
/// The task reads through the runtime's readiness (so that a `WouldBlock`
/// clears it); a thread reads the socket directly and waits with `poll`.
/// A thread may take the reading only from an idle task. While a thread
/// reads, the reading is off the runtime's poller (Unix, `unwatch`), so
/// data arriving does not wake the task; should an event seen before
/// still wake it, it forgets that readiness. Whenever the reading goes back to the task, the thread first
/// reads what has arrived (under the holder lock, so nothing arrives
/// unseen in between) and wakes the task if anything is buffered. The lock
/// order is `holder`, then `side`, then the stream's registration.
#[derive(Debug)]
pub(crate) struct Inbound {
    stream: ReadHalf,
    /// TLS on the stream: every read goes through it.
    tls: Option<Arc<Tls>>,
    holder: Mutex<Holder>,
    side: Mutex<ReadSide>,
    /// Wakes the task when a thread hands the reading back with something
    /// buffered.
    back: Notify,
    /// Set with `back`: the task's next read goes to the socket even if
    /// the runtime has seen no readiness (the thread's reads bypass it,
    /// and the rest of a large message may have arrived already).
    handed_back: AtomicBool,
    /// When the busy turn off the poller began (nanoseconds after
    /// [`epoch`], 0 when not in one), for [`Inbound::overdue`].
    busy_at: AtomicU64,
    /// Busy turns off the poller begun: a [`Lag`] keeps watching the
    /// connection while this moves.
    turns: AtomicU64,
    /// On the list of a [`Lag`].
    listed: AtomicBool,
    peer: Option<SocketAddr>,
}

/// The time [`Inbound::busy_at`] counts from.
fn epoch() -> Instant {
    static EPOCH: OnceLock<Instant> = OnceLock::new();
    *EPOCH.get_or_init(Instant::now)
}

/// Now, in nanoseconds after [`epoch`]; never 0.
fn now_ns() -> u64 {
    u64::try_from(epoch().elapsed().as_nanos())
        .unwrap_or(u64::MAX)
        .max(1)
}

impl Inbound {
    pub(crate) fn new(stream: ReadHalf, tls: Option<Arc<Tls>>, max_message_size: usize) -> Inbound {
        let peer = stream.peer_addr();
        let tls_free = tls.is_none();
        Inbound {
            stream,
            tls,
            holder: Mutex::new(Holder::Task { active: true }),
            side: Mutex::new(ReadSide {
                reader: MessageReader::new(max_message_size),
                assembler: FragmentAssembler::new(max_message_size),
                held: None,
                peer,
                waited: false,
                low_water: tls_free.then_some(1),
            }),
            back: Notify::new(),
            handed_back: AtomicBool::new(false),
            busy_at: AtomicU64::new(0),
            turns: AtomicU64::new(0),
            listed: AtomicBool::new(false),
            peer,
        }
    }

    /// Reads `buf` with `raw`, through TLS if the stream has it.
    fn read(&self, raw: &mut ReadFn<'_>, buf: &mut [MaybeUninit<u8>]) -> io::Result<usize> {
        let mut counted = |buf: &mut [MaybeUninit<u8>]| {
            let result = raw(buf);
            stats::count(Counter::recv_calls);
            if matches!(&result, Err(e) if e.kind() == io::ErrorKind::WouldBlock) {
                stats::count(Counter::recv_would_block);
            }
            result
        };
        match &self.tls {
            Some(tls) => tls.read(buf, &mut counted),
            None => counted(buf),
        }
    }

    pub(crate) fn side(&self) -> MutexGuard<'_, ReadSide> {
        self.side.lock().unwrap()
    }

    /// The address of the peer.
    pub(crate) fn peer(&self) -> PeerAddr {
        PeerAddr(self.peer)
    }

    /// Whether part of a message, or a message not handled yet, was read,
    /// or a thread is reading: the connection is not idle.
    pub(crate) fn mid_message(&self) -> bool {
        match self.side.try_lock() {
            Ok(side) => {
                side.reader.has_buffered() || side.assembler.has_pending() || side.held.is_some()
            }
            Err(_) => true,
        }
    }

    /// Shuts the read direction down, which ends a thread's wait and the
    /// task's reading.
    pub(crate) fn shutdown(&self) {
        self.stream.shutdown_read();
        // A thread the reading was left to may never come back (the pool
        // stops): the task reads the end.
        self.give_back_busy(stats::Counter::follow_overdue);
    }

    // --- the reader task ---------------------------------------------------

    /// The next message for the reader task, waiting for it as needed.
    /// The task counts as active from here until it waits again.
    pub(crate) async fn next(&self) -> Result<(Version, Message), ReadError> {
        loop {
            if self.task_may_read() {
                let stream = &self.stream;
                let direct = self.handed_back.swap(false, Ordering::AcqRel);
                let mut raw = |buf: &mut [MaybeUninit<u8>]| {
                    if direct {
                        stream.recv(buf)
                    } else {
                        stream.try_recv(buf)
                    }
                };
                let mut read = |buf: &mut [MaybeUninit<u8>]| self.read(&mut raw, buf);
                if let Some(msg) = self.side().next(stream, &mut read, false)? {
                    stats::count(Counter::task_messages);
                    return Ok(msg);
                }
            }
            self.task_wait().await.map_err(ReadError::Closed)?;
        }
    }

    /// Whether the task holds the reading, taking it from a busy thread if
    /// more arrived.
    fn task_may_read(&self) -> bool {
        let mut holder = self.holder.lock().unwrap();
        match *holder {
            Holder::Task { .. } => {
                *holder = Holder::Task { active: true };
                true
            }
            Holder::Busy { watched: false } => {
                // Woken by an event from before the reading went off the
                // poller: the thread reads what arrived.
                stats::count(Counter::task_wakeups_thread_reads);
                self.stream.clear_readable();
                false
            }
            Holder::Busy { watched: true } => {
                // The thread handles a request; read what arrived since,
                // so that requests on one connection run side by side.
                let stream = &self.stream;
                let mut side = self.side.lock().unwrap();
                let mut raw = |buf: &mut [MaybeUninit<u8>]| stream.try_recv(buf);
                let more = side.reader.drain(&mut |buf| self.read(&mut raw, buf));
                if more {
                    *holder = Holder::Task { active: true };
                    true
                } else {
                    stats::count(Counter::task_wakeups_busy_empty);
                    false
                }
            }
            Holder::Thread => {
                // The thread reads what woke the task.
                stats::count(Counter::task_wakeups_thread_reads);
                self.stream.clear_readable();
                false
            }
        }
    }

    /// Waits until the stream is readable or a thread hands the reading
    /// back. The task is idle meanwhile, so a thread may take the reading.
    /// Fails if the reading could not be put back on the runtime's poller.
    async fn task_wait(&self) -> io::Result<()> {
        {
            let mut holder = self.holder.lock().unwrap();
            if let Holder::Task { .. } = *holder {
                *holder = Holder::Task { active: false };
            }
        }
        let mut back = std::pin::pin!(self.back.notified());
        let result = std::future::poll_fn(|cx| {
            if back.as_mut().poll(cx).is_ready() {
                return std::task::Poll::Ready(Ok(()));
            }
            self.stream.poll_readable(cx)
        })
        .await;
        stats::count(Counter::task_wakeups);
        result
    }

    // --- a thread ----------------------------------------------------------

    /// Takes the reading for the calling thread, from an idle task or from
    /// the thread's own busy turn. Only where a thread can wait for the
    /// socket (`poll`).
    pub(crate) fn claim(&self) -> bool {
        if !cfg!(unix) {
            return false;
        }
        let mut holder = self.holder.lock().unwrap();
        match *holder {
            Holder::Task { active: false } | Holder::Busy { .. } => {
                *holder = Holder::Thread;
                self.busy_at.store(0, Ordering::Relaxed);
                // Data arriving now wakes only this thread.
                self.stream.unwatch();
                stats::count(Counter::claims);
                true
            }
            _ => {
                stats::count(Counter::claim_failures);
                false
            }
        }
    }

    /// Reads the next message on the calling thread, which holds the
    /// reading, waiting for it until `until` (for ever with `None`).
    ///
    /// `first` says whether the thread waits for the socket before its
    /// first read when nothing is buffered, which saves a read that finds
    /// nothing when the message has seldom arrived yet.
    pub(crate) fn read_here(&self, until: Option<Instant>, first: FirstRead) -> Here {
        let mut side = self.side();
        let stream = &self.stream;
        let mut raw = |buf: &mut [MaybeUninit<u8>]| stream.recv(buf);
        let mut read = |buf: &mut [MaybeUninit<u8>]| self.read(&mut raw, buf);
        let wait_first = match first {
            FirstRead::Wait => true,
            FirstRead::AsBefore => side.waited,
        };
        let mut wait = wait_first && side.held.is_none() && !side.reader.has_buffered();
        let mut waited = false;
        loop {
            if wait {
                if !wait_readable(stream, until) {
                    stats::count(Counter::thread_read_timeouts);
                    drop(side);
                    self.release();
                    return Here::TimedOut;
                }
                waited = true;
            }
            wait = true;
            let next = side.next(stream, &mut read, true);
            if matches!(next, Ok(Some(_))) {
                side.waited = waited;
            }
            match next {
                Ok(Some((version, msg))) => {
                    stats::count(Counter::thread_messages);
                    return Here::Message(version, msg);
                }
                // A large message comes in many pieces: the task reads
                // them as they come, which costs fewer wake-ups than a
                // thread waiting for each, and allocates its buffer where
                // it did before.
                Ok(None) if side.reader.starts_large() => {
                    drop(side);
                    self.release();
                    return Here::HandedBack;
                }
                Ok(None) => {}
                Err(e) => {
                    side.held = Some(Err(e));
                    drop(side);
                    self.release();
                    return Here::HandedBack;
                }
            }
        }
    }

    /// Leaves a message the thread read, and does not handle, for the
    /// task, and hands the reading back.
    pub(crate) fn give_back(&self, version: Version, msg: Message) {
        self.side().held = Some(Ok((version, msg)));
        self.release();
    }

    /// The thread goes on to handle the message it read. Returns whether
    /// it keeps the reading for afterwards ([`claim`] then takes it
    /// back).
    ///
    /// With `off`, the reading stays off the poller meanwhile: the thread
    /// reads what arrives once it is back, which costs no hand-over and no
    /// wake-up; a [`Lag`] gives the reading to the task should the thread
    /// stay busy too long ([`note_busy`](Self::note_busy) tells it). The
    /// caller then hands the connection to its `Lag`. Without `off` the
    /// task reads at once what arrives meanwhile (and gets the reading now
    /// if more is buffered already).
    pub(crate) fn busy(&self, off: bool) -> bool {
        let mut holder = self.holder.lock().unwrap();
        if off {
            *holder = Holder::Busy { watched: false };
            self.note_busy();
            return true;
        }
        let stream = &self.stream;
        let mut side = self.side.lock().unwrap();
        let mut raw = |buf: &mut [MaybeUninit<u8>]| stream.recv(buf);
        let more = side.reader.drain(&mut |buf| self.read(&mut raw, buf));
        // Either way the task may read now: what arrives next is its to
        // read.
        self.stream.watch();
        if more || side.held.is_some() {
            *holder = Holder::Task { active: false };
            drop(side);
            drop(holder);
            self.hand_back();
            false
        } else {
            *holder = Holder::Busy { watched: true };
            true
        }
    }

    /// The reader task, which just gave a request to a thread that will
    /// follow the connection after it, leaves the reading to that thread:
    /// the connection is busy off the poller ([`busy`](Self::busy) with
    /// `off`) as if the thread had read the request itself. Returns
    /// whether it did; the caller then hands the connection to its
    /// [`Lag`]. Only where a thread can read (Unix), and only by the task
    /// while it reads.
    pub(crate) fn pause(&self) -> bool {
        if !cfg!(unix) {
            return false;
        }
        let mut holder = self.holder.lock().unwrap();
        if *holder != (Holder::Task { active: true }) {
            return false;
        }
        *holder = Holder::Busy { watched: false };
        self.stream.unwatch();
        self.note_busy();
        stats::count(Counter::follow_pauses);
        true
    }

    /// A busy turn off the poller begins; under the holder lock.
    fn note_busy(&self) {
        self.busy_at.store(now_ns(), Ordering::Relaxed);
        self.turns.fetch_add(1, Ordering::SeqCst);
    }

    /// If the connection has been busy off the poller for `limit` or
    /// longer, gives the reading to the task, so that what arrives (and
    /// what is buffered already) is read while the thread is busy: as
    /// [`busy`](Self::busy) without `off` would have. The thread may still
    /// take the reading back afterwards.
    pub(crate) fn overdue(&self, now: u64, limit: Duration) {
        let at = self.busy_at.load(Ordering::Relaxed);
        let limit = u64::try_from(limit.as_nanos()).unwrap_or(u64::MAX);
        if at == 0 || now.saturating_sub(at) < limit {
            return;
        }
        self.give_back_busy(Counter::follow_overdue);
    }

    /// Ends a busy turn off the poller, if one runs, the way `overdue`
    /// does, counting `counter`.
    fn give_back_busy(&self, counter: Counter) {
        let mut holder = self.holder.lock().unwrap();
        if *holder != (Holder::Busy { watched: false }) {
            return;
        }
        self.busy_at.store(0, Ordering::Relaxed);
        stats::count(counter);
        let stream = &self.stream;
        let mut side = self.side.lock().unwrap();
        let mut raw = |buf: &mut [MaybeUninit<u8>]| stream.recv(buf);
        let more = side.reader.drain(&mut |buf| self.read(&mut raw, buf));
        self.stream.watch();
        if more || side.held.is_some() {
            *holder = Holder::Task { active: false };
            drop(side);
            drop(holder);
            self.hand_back();
        } else {
            *holder = Holder::Busy { watched: true };
        }
    }

    /// Gives the reading to the task if a client caller kept it after its
    /// reply (a busy turn off the poller): another caller waits for the
    /// task to read its reply.
    pub(crate) fn resume(&self) {
        self.give_back_busy(Counter::keep_resumes);
    }

    /// Whether the connection is in a busy turn off the poller.
    fn busy_off(&self) -> bool {
        self.busy_at.load(Ordering::Relaxed) != 0
    }

    /// Wakes the task to read what a thread left.
    fn hand_back(&self) {
        stats::count(Counter::hand_backs);
        self.handed_back.store(true, Ordering::Release);
        self.back.notify_one();
    }

    /// Gives the reading back to the task; by the thread holding it, or
    /// whose busy turn it is (a thread that will not read again).
    pub(crate) fn release(&self) {
        let mut holder = self.holder.lock().unwrap();
        match *holder {
            Holder::Thread | Holder::Busy { watched: false } => {
                self.busy_at.store(0, Ordering::Relaxed);
                let stream = &self.stream;
                let mut side = self.side.lock().unwrap();
                let mut raw = |buf: &mut [MaybeUninit<u8>]| stream.recv(buf);
                let more = side.reader.drain(&mut |buf| self.read(&mut raw, buf));
                *holder = Holder::Task { active: false };
                self.stream.watch();
                if more || side.held.is_some() {
                    drop(side);
                    drop(holder);
                    self.hand_back();
                }
            }
            // Whatever arrived while busy woke the task, which read it
            // (the reading is on the poller while busy).
            Holder::Busy { watched: true } => *holder = Holder::Task { active: false },
            Holder::Task { .. } => {}
        }
    }
}

/// Waits until `stream` has something to read, or `until`. Returns
/// whether it may have.
#[cfg(unix)]
fn wait_readable(stream: &ReadHalf, until: Option<Instant>) -> bool {
    loop {
        let timeout = match until {
            None => -1,
            Some(at) => {
                let left = at.saturating_duration_since(Instant::now());
                if left.is_zero() {
                    return false;
                }
                // Rounded up, so that the wait does not end early.
                i32::try_from(left.as_micros().div_ceil(1000)).unwrap_or(i32::MAX)
            }
        };
        let mut fd = libc::pollfd {
            fd: stream.raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        stats::count(Counter::thread_polls);
        // SAFETY: one valid pollfd.
        let n = unsafe { libc::poll(&mut fd, 1, timeout) };
        if n > 0 {
            return true;
        }
        if n < 0 && io::Error::last_os_error().kind() != io::ErrorKind::Interrupted {
            // Let the read find the error.
            return true;
        }
    }
}

#[cfg(not(unix))]
fn wait_readable(_: &ReadHalf, _: Option<Instant>) -> bool {
    // A thread never claims the reading here.
    false
}

/// Watches the connections whose reading a busy thread keeps off the
/// poller ([`Inbound::busy`] with `off`, [`Inbound::pause`]): one whose
/// thread has been busy for `limit` gets its reading back to the task
/// ([`Inbound::overdue`]), so that a long upcall does not hold up the
/// requests behind it on the same connection for more than
/// `2 * limit`. One thread per server, started with the first
/// connection, that looks every `limit` while any connection is on its
/// list, and sleeps otherwise. A connection leaves the list once it has begun no
/// busy turn for a while.
#[derive(Debug)]
pub(crate) struct Lag {
    shared: Arc<LagShared>,
}

#[derive(Debug)]
struct LagShared {
    state: Mutex<LagState>,
    cond: std::sync::Condvar,
    limit: Duration,
}

#[derive(Debug, Default)]
struct LagState {
    /// The connections, the busy turns each had when last looked at, and
    /// how many looks since saw no new turn.
    conns: Vec<(std::sync::Weak<Inbound>, u64, u32)>,
    started: bool,
    /// The thread waits for a connection, with no time limit.
    asleep: bool,
    stopping: bool,
}

/// Looks with no new busy turn after which a connection leaves the list.
const LAG_IDLE_LOOKS: u32 = 64;

impl Lag {
    pub(crate) fn new(limit: Duration) -> Lag {
        Lag {
            shared: Arc::new(LagShared {
                state: Mutex::new(LagState::default()),
                cond: std::sync::Condvar::new(),
                limit: limit.max(Duration::from_micros(100)),
            }),
        }
    }

    /// Watches `inbound` after it began a busy turn off the poller.
    pub(crate) fn watch(&self, inbound: &Arc<Inbound>) {
        // A turn began before this (SeqCst on both sides): either the
        // thread that drops the connection from the list sees the turn,
        // or this sees the connection unlisted.
        if inbound.listed.swap(true, Ordering::SeqCst) {
            return;
        }
        let mut state = self.shared.state.lock().unwrap();
        if state.stopping {
            return;
        }
        let turns = inbound.turns.load(Ordering::SeqCst);
        state.conns.push((Arc::downgrade(inbound), turns, 0));
        if !state.started {
            let shared = Arc::clone(&self.shared);
            let started = std::thread::Builder::new()
                .name("rustorb-lag".into())
                .spawn(move || shared.run());
            if started.is_err() {
                // Without the thread, nobody would give the reading back.
                drop(state);
                inbound.give_back_busy(Counter::follow_overdue);
                return;
            }
            state.started = true;
        } else if state.asleep {
            state.asleep = false;
            self.shared.cond.notify_one();
        }
    }
}

impl Drop for Lag {
    fn drop(&mut self) {
        let mut state = self.shared.state.lock().unwrap();
        state.stopping = true;
        state.conns.clear();
        self.shared.cond.notify_one();
    }
}

impl LagShared {
    fn run(&self) {
        let look = self.limit;
        let mut state = self.state.lock().unwrap();
        let mut conns = Vec::new();
        loop {
            if state.stopping {
                return;
            }
            if state.conns.is_empty() {
                state.asleep = true;
                while state.asleep && !state.stopping {
                    state = self.cond.wait(state).unwrap();
                }
                continue;
            }
            state = self.cond.wait_timeout(state, look).unwrap().0;
            if state.stopping {
                return;
            }
            // Looked at without the lock: giving the reading back takes
            // the connection's locks.
            std::mem::swap(&mut conns, &mut state.conns);
            drop(state);
            let now = now_ns();
            conns.retain_mut(|(weak, seen, idle)| {
                let Some(inbound) = weak.upgrade() else {
                    return false;
                };
                inbound.overdue(now, self.limit);
                let turns = inbound.turns.load(Ordering::SeqCst);
                if turns != *seen {
                    *seen = turns;
                    *idle = 0;
                    return true;
                }
                *idle += 1;
                if *idle < LAG_IDLE_LOOKS || inbound.busy_off() {
                    return true;
                }
                inbound.listed.store(false, Ordering::SeqCst);
                // A turn that began meanwhile may not have listed it again.
                if inbound.turns.load(Ordering::SeqCst) != *seen || inbound.busy_off() {
                    inbound.listed.store(true, Ordering::SeqCst);
                    return true;
                }
                false
            });
            state = self.state.lock().unwrap();
            // Connections listed meanwhile stay too.
            conns.append(&mut state.conns);
            std::mem::swap(&mut conns, &mut state.conns);
        }
    }
}

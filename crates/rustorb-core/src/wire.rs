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
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Instant;

use rustorb_giop::{FragmentAssembler, Message, MessageHeader, Version};
use socket2::SockRef;
use tokio::io::Interest;
use tokio::net::tcp::OwnedReadHalf;
use tokio::sync::Notify;

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
}

impl ReadSide {
    /// The next whole message, fragments joined. `Ok(None)` when `read`
    /// has nothing more now.
    fn next(
        &mut self,
        read: &mut ReadFn<'_>,
        defer_large: bool,
    ) -> Result<Option<(Version, Message)>, ReadError> {
        if let Some(held) = self.held.take() {
            return held.map(Some);
        }
        loop {
            let Some((header, msg)) = self.reader.next(read, defer_large)? else {
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

/// Who reads the connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Holder {
    /// The reader task; `active` while it reads or handles what it read.
    Task { active: bool },
    /// A thread reads, or waits for the stream to become readable.
    Thread,
    /// The thread that read last handles what it read and means to read
    /// again. The reader task reads if more arrives meanwhile.
    Busy,
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
/// A thread may take the reading only from an idle task. Data arriving
/// while a thread reads wakes the task too; it then forgets that
/// readiness. Whenever the reading goes back to the task, the thread first
/// reads what has arrived (under the holder lock, so nothing arrives
/// unseen in between) and wakes the task if anything is buffered. The lock
/// order is `holder`, then `side`.
#[derive(Debug)]
pub(crate) struct Inbound {
    stream: OwnedReadHalf,
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
    peer: Option<SocketAddr>,
}

impl Inbound {
    pub(crate) fn new(
        stream: OwnedReadHalf,
        tls: Option<Arc<Tls>>,
        max_message_size: usize,
    ) -> Inbound {
        let peer = stream.peer_addr().ok();
        Inbound {
            stream,
            tls,
            holder: Mutex::new(Holder::Task { active: true }),
            side: Mutex::new(ReadSide {
                reader: MessageReader::new(max_message_size),
                assembler: FragmentAssembler::new(max_message_size),
                held: None,
                peer,
            }),
            back: Notify::new(),
            handed_back: AtomicBool::new(false),
            peer,
        }
    }

    /// Reads `buf` with `raw`, through TLS if the stream has it.
    fn read(&self, raw: &mut ReadFn<'_>, buf: &mut [MaybeUninit<u8>]) -> io::Result<usize> {
        match &self.tls {
            Some(tls) => tls.read(buf, raw),
            None => raw(buf),
        }
    }

    pub(crate) fn side(&self) -> MutexGuard<'_, ReadSide> {
        self.side.lock().unwrap()
    }

    /// Shuts the read direction down, which ends a thread's wait.
    /// The address of the peer.
    pub(crate) fn peer(&self) -> PeerAddr {
        PeerAddr(self.peer)
    }

    pub(crate) fn shutdown(&self) {
        let _ = SockRef::from(self.stream.as_ref()).shutdown(std::net::Shutdown::Read);
    }

    // --- the reader task ---------------------------------------------------

    /// The next message for the reader task, waiting for it as needed.
    /// The task counts as active from here until it waits again.
    pub(crate) async fn next(&self) -> Result<(Version, Message), ReadError> {
        loop {
            if self.task_may_read() {
                let stream = self.stream.as_ref();
                let direct = self.handed_back.swap(false, Ordering::AcqRel);
                let mut raw = |buf: &mut [MaybeUninit<u8>]| {
                    if direct {
                        SockRef::from(stream).recv(buf)
                    } else {
                        stream.try_io(Interest::READABLE, || SockRef::from(stream).recv(buf))
                    }
                };
                let mut read = |buf: &mut [MaybeUninit<u8>]| self.read(&mut raw, buf);
                if let Some(msg) = self.side().next(&mut read, false)? {
                    return Ok(msg);
                }
            }
            self.task_wait().await;
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
            Holder::Busy => {
                // The thread handles a request; read what arrived since,
                // so that requests on one connection run side by side.
                let stream = self.stream.as_ref();
                let mut side = self.side.lock().unwrap();
                let mut raw = |buf: &mut [MaybeUninit<u8>]| {
                    stream.try_io(Interest::READABLE, || SockRef::from(stream).recv(buf))
                };
                let more = side.reader.drain(&mut |buf| self.read(&mut raw, buf));
                if more {
                    *holder = Holder::Task { active: true };
                    true
                } else {
                    false
                }
            }
            Holder::Thread => {
                // The thread reads what woke the task.
                let _ = self.stream.as_ref().try_io(Interest::READABLE, || {
                    Err::<(), _>(io::ErrorKind::WouldBlock.into())
                });
                false
            }
        }
    }

    /// Waits until the stream is readable or a thread hands the reading
    /// back. The task is idle meanwhile, so a thread may take the reading.
    async fn task_wait(&self) {
        {
            let mut holder = self.holder.lock().unwrap();
            if let Holder::Task { .. } = *holder {
                *holder = Holder::Task { active: false };
            }
        }
        let mut back = std::pin::pin!(self.back.notified());
        let mut readable = std::pin::pin!(self.stream.readable());
        std::future::poll_fn(|cx| {
            if back.as_mut().poll(cx).is_ready() || readable.as_mut().poll(cx).is_ready() {
                std::task::Poll::Ready(())
            } else {
                std::task::Poll::Pending
            }
        })
        .await;
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
            Holder::Task { active: false } | Holder::Busy => {
                *holder = Holder::Thread;
                true
            }
            _ => false,
        }
    }

    /// Reads the next message on the calling thread, which holds the
    /// reading, waiting for it until `until` (for ever with `None`).
    pub(crate) fn read_here(&self, until: Option<Instant>) -> Here {
        let mut side = self.side();
        let stream = self.stream.as_ref();
        let mut raw = |buf: &mut [MaybeUninit<u8>]| SockRef::from(stream).recv(buf);
        let mut read = |buf: &mut [MaybeUninit<u8>]| self.read(&mut raw, buf);
        loop {
            match side.next(&mut read, true) {
                Ok(Some((version, msg))) => return Here::Message(version, msg),
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
            if !wait_readable(stream, until) {
                drop(side);
                self.release();
                return Here::TimedOut;
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
    /// back); if more is buffered already, the task gets the reading now.
    pub(crate) fn busy(&self) -> bool {
        let mut holder = self.holder.lock().unwrap();
        let stream = self.stream.as_ref();
        let mut side = self.side.lock().unwrap();
        let mut raw = |buf: &mut [MaybeUninit<u8>]| SockRef::from(stream).recv(buf);
        let more = side.reader.drain(&mut |buf| self.read(&mut raw, buf));
        if more || side.held.is_some() {
            *holder = Holder::Task { active: false };
            drop(side);
            drop(holder);
            self.hand_back();
            false
        } else {
            *holder = Holder::Busy;
            true
        }
    }

    /// Wakes the task to read what a thread left.
    fn hand_back(&self) {
        self.handed_back.store(true, Ordering::Release);
        self.back.notify_one();
    }

    /// Ends the busy turn of a thread that will not read again.
    pub(crate) fn unbusy(&self) {
        let mut holder = self.holder.lock().unwrap();
        if *holder == Holder::Busy {
            *holder = Holder::Task { active: false };
        }
    }

    /// Gives the reading back to the task; by the thread holding it.
    pub(crate) fn release(&self) {
        let mut holder = self.holder.lock().unwrap();
        match *holder {
            Holder::Thread => {
                let stream = self.stream.as_ref();
                let mut side = self.side.lock().unwrap();
                let mut raw = |buf: &mut [MaybeUninit<u8>]| SockRef::from(stream).recv(buf);
                let more = side.reader.drain(&mut |buf| self.read(&mut raw, buf));
                *holder = Holder::Task { active: false };
                if more || side.held.is_some() {
                    drop(side);
                    drop(holder);
                    self.hand_back();
                }
            }
            // Whatever arrived while busy woke the task, which read it.
            Holder::Busy => *holder = Holder::Task { active: false },
            Holder::Task { .. } => {}
        }
    }
}

/// Waits until `stream` has something to read, or `until`. Returns
/// whether it may have.
#[cfg(unix)]
fn wait_readable(stream: &tokio::net::TcpStream, until: Option<Instant>) -> bool {
    use std::os::fd::AsRawFd;
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
            fd: stream.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
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
fn wait_readable(_: &tokio::net::TcpStream, _: Option<Instant>) -> bool {
    // A thread never claims the reading here.
    false
}

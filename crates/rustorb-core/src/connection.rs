//! Client side GIOP connections.
//!
//! A [`Connection`] owns one TCP stream. Requests are written under a lock,
//! and a reader task takes every message off the stream, joins fragments
//! and hands each `Reply` or `LocateReply` to the caller waiting for its
//! request id. A caller that is the only one waiting reads its reply
//! itself instead ([`Connection::read_reply_here`]). A connection to a
//! TLS endpoint does both through [`Tls`].
//!
//! Bidirectional GIOP (docs/bidir-giop.md) adds two cases. A connection
//! this ORB opened and offered for bidirectional use also takes requests
//! from the server: the reader task hands them to the server side through
//! a [`ServerConn`] over the same stream. And a connection a client
//! offered is used for calls to that client's objects: the [`Connection`]
//! then writes through the server connection's [`Outbox`], and the server
//! connection's reader hands it the replies.

use std::collections::HashMap;
use std::io::{self, IoSlice};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, OnceLock, Weak};

use std::time::{Duration, Instant};

use rustorb_cdr::ByteOrder;
use rustorb_giop::{Message, MessageEncoder, OutMessage, ServiceContext, Version};
use tokio::io::AsyncWriteExt;
use tokio::net::TcpSocket;
use tokio::net::tcp::OwnedWriteHalf;
use tokio::sync::oneshot;

use crate::idle::Stamp;
use crate::server::{Outbox, Server, ServerConn};
use crate::tls::{self, Tls};
use crate::trace;
use crate::wire::{Here, Inbound, ReadError};
use crate::{CodeSets, OrbOptions};

/// Why a connection stopped delivering replies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Closed {
    /// The server sent `CloseConnection`: requests without a reply were
    /// not processed and may be sent again.
    Orderly,
    /// The stream failed, or a message could not be read.
    Failed(String),
}

/// At most this many pieces go into one vectored write.
const MAX_SLICES: usize = 64;

/// How long [`Connection::close`] waits to write `CloseConnection`.
const CLOSE_TIME: Duration = Duration::from_secs(2);

/// Why [`Connection::send`] did not write a message. Either way the
/// server cannot have read the whole of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SendError {
    /// The deadline passed first.
    TimedOut,
    /// The stream failed; the connection is closed.
    Failed,
    /// The connection is being closed and took nothing: the message may
    /// go on a new connection.
    Closing,
}

/// What the ORB pools connections by: host, port, and whether the
/// connection uses TLS.
pub(crate) type EndpointKey = (String, u16, bool);

/// The ORB's client connections, one per endpoint.
pub(crate) type ConnectionPool = Mutex<HashMap<EndpointKey, Arc<Connection>>>;

/// What a waiting caller receives.
pub(crate) type Delivery = Result<Message, Closed>;

#[derive(Debug)]
struct Pending {
    waiters: HashMap<u32, oneshot::Sender<Delivery>>,
    closed: Option<Closed>,
}

/// Where a connection writes its requests.
enum Writer {
    /// A stream of its own (a connection this ORB opened).
    Stream(tokio::sync::Mutex<OwnedWriteHalf>),
    /// The outbox of a server connection a client offered for
    /// bidirectional GIOP.
    Outbox(Arc<Outbox>),
}

impl std::fmt::Debug for Writer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Writer::Stream(_) => "Stream",
            Writer::Outbox(_) => "Outbox",
        })
    }
}

/// The server side of a connection this ORB opened and offered for
/// bidirectional GIOP.
pub(crate) struct Serving {
    pub(crate) server: Weak<Server>,
    pub(crate) conn: Arc<ServerConn>,
    /// The `BI_DIR_IIOP` service context with the ORB's listen points.
    context: ServiceContext,
}

impl std::fmt::Debug for Serving {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Serving").finish_non_exhaustive()
    }
}

/// One client connection to a server endpoint.
#[derive(Debug)]
pub(crate) struct Connection {
    endpoint: EndpointKey,
    inbound: Arc<Inbound>,
    writer: Writer,
    tls: Option<Arc<Tls>>,
    pending: Mutex<Pending>,
    next_request_id: AtomicU32,
    /// The code sets of the first request with a code set context; `None`
    /// until one was sent.
    code_sets: Mutex<Option<CodeSets>>,
    /// Set under the `pending` lock (by [`close`](Self::close) too), so
    /// that an idle connection takes no new request once chosen to close.
    closing: AtomicBool,
    /// When a request or reply last went over the connection.
    last_used: Stamp,
    /// The pool the connection leaves when it ends.
    pool: Weak<ConnectionPool>,
    /// Set when the connection is offered for bidirectional GIOP.
    serving: OnceLock<Serving>,
    /// Whether a request with the `BI_DIR_IIOP` context went out.
    offered: AtomicBool,
}

impl Connection {
    /// Connects to `host:port`, with TLS when `tls` is given, and starts
    /// the reader task on the current runtime. The socket buffers are set
    /// before connecting, so that the window scale fits them.
    pub(crate) async fn connect(
        host: &str,
        port: u16,
        tls: Option<Arc<rustls::ClientConfig>>,
        options: &OrbOptions,
        pool: Weak<ConnectionPool>,
    ) -> io::Result<Arc<Connection>> {
        let max_message_size = options.max_message_size;
        let mut last = io::Error::new(io::ErrorKind::NotFound, "no address");
        let mut connected = None;
        for addr in tokio::net::lookup_host((host, port)).await? {
            let socket = if addr.is_ipv4() {
                TcpSocket::new_v4()?
            } else {
                TcpSocket::new_v6()?
            };
            if let Some(size) = options.socket_send_buffer {
                socket.set_send_buffer_size(u32::try_from(size).unwrap_or(u32::MAX))?;
            }
            if let Some(size) = options.socket_receive_buffer {
                socket.set_recv_buffer_size(u32::try_from(size).unwrap_or(u32::MAX))?;
            }
            match socket.connect(addr).await {
                Ok(stream) => {
                    connected = Some(stream);
                    break;
                }
                Err(e) => {
                    trace::info!("could not connect to {addr} ({host}:{port}): {e}");
                    last = e;
                }
            }
        }
        let stream = connected.ok_or(last)?;
        stream.set_nodelay(true)?;
        let session = match tls {
            Some(config) => match tls::connect(&stream, config, host).await {
                Ok(session) => Some(session),
                Err(e) => {
                    trace::info!("TLS handshake with {host}:{port} failed: {e}");
                    return Err(e);
                }
            },
            None => None,
        };
        trace::info!(
            "connected to {host}:{port}{}",
            if session.is_some() { " with TLS" } else { "" }
        );
        let (reader, writer) = stream.into_split();
        let conn = Arc::new(Connection {
            endpoint: (host.to_owned(), port, session.is_some()),
            inbound: Arc::new(Inbound::new(reader, session.clone(), max_message_size)),
            writer: Writer::Stream(tokio::sync::Mutex::new(writer)),
            tls: session,
            pending: Mutex::new(Pending {
                waiters: HashMap::new(),
                closed: None,
            }),
            // The side that opened a connection uses even request ids,
            // the other side odd ones (GIOP 1.2, bidirectional GIOP).
            next_request_id: AtomicU32::new(0),
            code_sets: Mutex::new(None),
            closing: AtomicBool::new(false),
            last_used: Stamp::new(),
            pool,
            serving: OnceLock::new(),
            offered: AtomicBool::new(false),
        });
        tokio::spawn(read_loop(Arc::clone(&conn)));
        Ok(conn)
    }

    /// A connection for calls over a server connection that a client
    /// offered for bidirectional GIOP. It has no reader task: the server
    /// connection's reader hands it the replies ([`deliver`](Self::deliver)),
    /// and ends it ([`fail`](Self::fail)).
    pub(crate) fn accepted(
        endpoint: EndpointKey,
        inbound: Arc<Inbound>,
        out: Arc<Outbox>,
        pool: Weak<ConnectionPool>,
    ) -> Arc<Connection> {
        Arc::new(Connection {
            endpoint,
            inbound,
            writer: Writer::Outbox(out),
            tls: None,
            pending: Mutex::new(Pending {
                waiters: HashMap::new(),
                closed: None,
            }),
            next_request_id: AtomicU32::new(1),
            code_sets: Mutex::new(None),
            closing: AtomicBool::new(false),
            last_used: Stamp::new(),
            pool,
            serving: OnceLock::new(),
            offered: AtomicBool::new(false),
        })
    }

    /// Whether this is a connection a client offered for bidirectional
    /// GIOP ([`accepted`](Self::accepted)).
    pub(crate) fn is_accepted(&self) -> bool {
        matches!(self.writer, Writer::Outbox(_))
    }

    /// Offers the connection for bidirectional GIOP: `context` (the
    /// `BI_DIR_IIOP` context) goes with the next GIOP 1.2 request, and
    /// requests from the server go to `server` through `conn`.
    pub(crate) fn serve(
        &self,
        server: Weak<Server>,
        conn: Arc<ServerConn>,
        context: ServiceContext,
    ) {
        let _ = self.serving.set(Serving {
            server,
            conn,
            context,
        });
    }

    /// The `BI_DIR_IIOP` context for a GIOP 1.2 request, until one has
    /// gone out ([`offered`](Self::offered)).
    pub(crate) fn bidir_context(&self) -> Option<&ServiceContext> {
        let serving = self.serving.get()?;
        (!self.offered.load(Ordering::Relaxed)).then_some(&serving.context)
    }

    /// Records that a request with the `BI_DIR_IIOP` context went out.
    pub(crate) fn offered(&self) {
        if !self.offered.swap(true, Ordering::Relaxed) {
            trace::info!(
                "offered the connection to {} for bidirectional GIOP",
                self.peer()
            );
        }
    }

    /// The reading side of the stream.
    pub(crate) fn inbound(&self) -> Arc<Inbound> {
        Arc::clone(&self.inbound)
    }

    pub(crate) fn endpoint(&self) -> &EndpointKey {
        &self.endpoint
    }

    /// Whether the connection can take new requests.
    pub(crate) fn is_usable(&self) -> bool {
        !self.closing.load(Ordering::Acquire) && self.pending.lock().unwrap().closed.is_none()
    }

    pub(crate) fn next_request_id(&self) -> u32 {
        self.next_request_id.fetch_add(2, Ordering::Relaxed)
    }

    /// The code sets already announced on this connection, if any.
    pub(crate) fn code_sets(&self) -> Option<CodeSets> {
        *self.code_sets.lock().unwrap()
    }

    /// Fixes the code sets of the connection to `proposed` unless an
    /// earlier request fixed them, and returns the ones in force.
    pub(crate) fn claim_code_sets(&self, proposed: CodeSets) -> CodeSets {
        *self.code_sets.lock().unwrap().get_or_insert(proposed)
    }

    /// Registers a caller waiting for the reply to `request_id`.
    pub(crate) fn expect_reply(
        &self,
        request_id: u32,
    ) -> Result<oneshot::Receiver<Delivery>, Closed> {
        let mut pending = self.pending.lock().unwrap();
        if let Some(closed) = &pending.closed {
            return Err(closed.clone());
        }
        if self.closing.load(Ordering::Acquire) {
            // Closed by this side with nothing outstanding: the request
            // was not sent, and goes on a new connection.
            return Err(Closed::Orderly);
        }
        self.last_used.touch();
        let (tx, rx) = oneshot::channel();
        pending.waiters.insert(request_id, tx);
        Ok(rx)
    }

    /// Reads the reply to `request_id` on the calling thread, if it is the
    /// only reply waited for: one thread hand-over less than through the
    /// reader task. Replies to requests sent meanwhile are handed to their
    /// callers. `None` when the caller is to wait for the reader task
    /// after all (others wait too, the task is reading, or the connection
    /// ended); `Some(None)` when `deadline` passed.
    ///
    /// Blocks the thread; the caller is a thread of its own (the sync API
    /// runs the call with `block_on` outside the runtime).
    pub(crate) fn read_reply_here(
        &self,
        request_id: u32,
        deadline: Option<Instant>,
    ) -> Option<Option<Message>> {
        if self.is_accepted() {
            // The server connection's reader reads.
            return None;
        }
        {
            let pending = self.pending.lock().unwrap();
            if pending.closed.is_some()
                || pending.waiters.len() != 1
                || !pending.waiters.contains_key(&request_id)
            {
                return None;
            }
        }
        if !self.inbound.claim() {
            return None;
        }
        // The reader task may have delivered the reply between the check
        // above and the claim; it reads no more now.
        if !self
            .pending
            .lock()
            .unwrap()
            .waiters
            .contains_key(&request_id)
        {
            self.inbound.release();
            return None;
        }
        loop {
            let (version, message) = match self.inbound.read_here(deadline) {
                Here::Message(version, message) => (version, message),
                Here::TimedOut => return Some(None),
                Here::HandedBack => return None,
            };
            let id = match &message {
                Message::Reply { header, .. } => header.request_id,
                Message::LocateReply { header, .. } => header.request_id,
                _ => {
                    self.inbound.give_back(version, message);
                    return None;
                }
            };
            if id == request_id {
                self.inbound.release();
                self.forget(request_id);
                return Some(Some(message));
            }
            self.deliver(id, message);
        }
    }

    /// Stops waiting for `request_id`, after a timeout.
    pub(crate) fn forget(&self, request_id: u32) {
        self.pending.lock().unwrap().waiters.remove(&request_id);
        self.last_used.touch();
    }

    /// Chooses the connection to close if no reply is awaited, nothing
    /// was sent or received for `timeout`, and no reply is being read.
    /// From then on it takes no request ([`expect_reply`] and [`send`]
    /// refuse them, to go on a new connection); the caller closes it
    /// with [`close`](Self::close).
    ///
    /// [`expect_reply`]: Self::expect_reply
    /// [`send`]: Self::send
    ///
    /// A connection offered for bidirectional GIOP is chosen only when no
    /// request from the server runs either, nor came in for `timeout`;
    /// from then on the requests from the server that expect a reply are
    /// not processed (the server sends them again after the
    /// `CloseConnection` that [`close_idle`](Self::close_idle) sends). A
    /// connection a client offered ([`accepted`](Self::accepted)) is never
    /// chosen: the server side closes it.
    pub(crate) fn choose_idle(&self, timeout: Duration) -> bool {
        if self.is_accepted() {
            return false;
        }
        let pending = self.pending.lock().unwrap();
        if pending.closed.is_some()
            || !pending.waiters.is_empty()
            || self.closing.load(Ordering::Acquire)
            || self.last_used.idle_for() < timeout
            || self.inbound.mid_message()
        {
            return false;
        }
        if let Some(serving) = self.serving.get() {
            if !serving.conn.choose_idle(Some(timeout)) {
                return false;
            }
        }
        self.closing.store(true, Ordering::Release);
        true
    }

    /// For a connection a client offered: chooses it to close if no reply
    /// is awaited and (with `timeout`) none was sent or received for that
    /// long. Called by the server connection under its own lock.
    pub(crate) fn choose_close(&self, timeout: Option<Duration>) -> bool {
        let pending = self.pending.lock().unwrap();
        if pending.closed.is_some() || self.closing.load(Ordering::Acquire) {
            return true;
        }
        if !pending.waiters.is_empty() || timeout.is_some_and(|t| self.last_used.idle_for() < t) {
            return false;
        }
        self.closing.store(true, Ordering::Release);
        true
    }

    /// Whether no reply is awaited, for a connection a client offered.
    pub(crate) fn awaits_nothing(&self) -> bool {
        self.pending.lock().unwrap().waiters.is_empty()
    }

    /// When a request or reply last went over the connection.
    pub(crate) fn idle_for(&self) -> Duration {
        self.last_used.idle_for()
    }

    /// Closes a connection [`choose_idle`](Self::choose_idle) chose: one
    /// offered for bidirectional GIOP with `CloseConnection` (GIOP 1.2,
    /// where either side may send it), so that the server sends again
    /// the requests it sent meanwhile; any other without.
    pub(crate) async fn close_idle(&self, order: ByteOrder) {
        let close_connection = self
            .offered
            .load(Ordering::Relaxed)
            .then_some((Version::V1_2, order));
        self.close(close_connection).await;
    }

    /// Writes one whole message, giving up at `deadline`. A message cut
    /// short by the deadline leaves the stream unusable, so the connection
    /// is then closed; one not started leaves it as it was.
    pub(crate) async fn send(
        &self,
        msg: &OutMessage,
        deadline: Option<Instant>,
    ) -> Result<(), SendError> {
        let writer = match &self.writer {
            Writer::Stream(writer) => writer,
            Writer::Outbox(out) => return self.send_outbox(out, msg),
        };
        let mut writer = match deadline {
            Some(at) => match tokio::time::timeout_at(at.into(), writer.lock()).await {
                Ok(writer) => writer,
                Err(_) => return Err(SendError::TimedOut),
            },
            None => writer.lock().await,
        };
        if self.closing.load(Ordering::Acquire) {
            return Err(SendError::Closing);
        }
        self.last_used.touch();
        trace::trace!("sent {} to {}", summary(msg), self.peer());
        if let Some(tls) = &self.tls {
            return self.send_tls(tls, &writer, msg, deadline).await;
        }
        let mut written = 0;
        while written < msg.len() {
            let (one, many);
            let slices: &[IoSlice<'_>] = match msg.as_single() {
                Some(bytes) => {
                    one = [IoSlice::new(&bytes[written..])];
                    &one
                }
                None => {
                    many = msg.slices_from(written, MAX_SLICES);
                    &many
                }
            };
            let write = writer.write_vectored(slices);
            let result = match deadline {
                Some(at) => match tokio::time::timeout_at(at.into(), write).await {
                    Ok(result) => result,
                    Err(_) if written == 0 => return Err(SendError::TimedOut),
                    Err(_) => {
                        let _ = writer.shutdown().await;
                        drop(writer);
                        self.fail(Closed::Failed("write timed out".into()));
                        return Err(SendError::TimedOut);
                    }
                },
                None => write.await,
            };
            match result {
                Ok(0) => {
                    drop(writer);
                    self.fail(Closed::Failed("write failed: stream closed".into()));
                    return Err(SendError::Failed);
                }
                Ok(n) => written += n,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => {
                    drop(writer);
                    self.fail(Closed::Failed(format!("write failed: {e}")));
                    return Err(SendError::Failed);
                }
            }
        }
        Ok(())
    }

    /// [`send`](Self::send) through a server connection's outbox, which
    /// writes it, or queues it for its writer task, without waiting.
    fn send_outbox(&self, out: &Outbox, msg: &OutMessage) -> Result<(), SendError> {
        if self.closing.load(Ordering::Acquire) {
            return Err(SendError::Closing);
        }
        self.last_used.touch();
        trace::trace!("sent {} to {}", summary(msg), self.peer());
        if out.send(msg.clone()) {
            Ok(())
        } else {
            self.fail(Closed::Failed("the connection is closed".into()));
            Err(SendError::Failed)
        }
    }

    /// [`send`](Self::send) over TLS. The whole message is encrypted at
    /// once and cannot be taken back, so a deadline passing before it is
    /// written closes the connection: the server never gets the rest of
    /// its last record.
    async fn send_tls(
        &self,
        tls: &Tls,
        writer: &OwnedWriteHalf,
        msg: &OutMessage,
        deadline: Option<Instant>,
    ) -> Result<(), SendError> {
        tls.encrypt(msg, 0);
        loop {
            match tls.flush(|bytes| writer.try_write(bytes)) {
                Ok(true) => return Ok(()),
                Ok(false) => {}
                Err(e) => {
                    self.fail(Closed::Failed(format!("write failed: {e}")));
                    return Err(SendError::Failed);
                }
            }
            let writable = match deadline {
                Some(at) => match tokio::time::timeout_at(at.into(), writer.writable()).await {
                    Ok(result) => result,
                    Err(_) => {
                        self.fail(Closed::Failed("write timed out".into()));
                        return Err(SendError::TimedOut);
                    }
                },
                None => writer.writable().await,
            };
            if let Err(e) = writable {
                self.fail(Closed::Failed(format!("write failed: {e}")));
                return Err(SendError::Failed);
            }
        }
    }

    /// Sends `CloseConnection` (when `version` is given) and shuts the
    /// stream down. Requests issued afterwards use a new connection.
    /// A peer that stopped reading gets [`CLOSE_TIME`] before the stream
    /// is dropped without it.
    ///
    /// An idle connection is closed without `CloseConnection`, which a
    /// client sends only from GIOP 1.2 on: the server sees the stream end
    /// with nothing outstanding, the orderly close of a client.
    ///
    /// A connection a client offered only takes no more requests: the
    /// server side closes the stream.
    pub(crate) async fn close(&self, close_connection: Option<(Version, ByteOrder)>) {
        self.closing.store(true, Ordering::Release);
        let writer = match &self.writer {
            Writer::Stream(writer) => writer,
            Writer::Outbox(_) => return,
        };
        trace::info!("closing the connection to {}", self.peer());
        let msg = match close_connection {
            Some((version, order)) => match MessageEncoder::close_connection(version, order) {
                Ok(msg) => Some(msg),
                Err(_) => return,
            },
            None => None,
        };
        let _ = tokio::time::timeout(CLOSE_TIME, async {
            let mut writer = writer.lock().await;
            match &self.tls {
                Some(tls) => {
                    if let Some(msg) = msg {
                        tls.encrypt(&msg.into(), 0);
                    }
                    tls.close_notify();
                    while let Ok(false) = tls.flush(|bytes| writer.try_write(bytes)) {
                        if writer.writable().await.is_err() {
                            break;
                        }
                    }
                }
                None => {
                    if let Some(msg) = msg {
                        let _ = writer.write_all(&msg).await;
                    }
                }
            }
            let _ = writer.shutdown().await;
        })
        .await;
    }

    /// Marks the connection closed and wakes every waiting caller.
    pub(crate) fn fail(&self, why: Closed) {
        let waiters = {
            let mut pending = self.pending.lock().unwrap();
            if pending.closed.is_none() {
                pending.closed = Some(why.clone());
                match &why {
                    _ if self.closing.load(Ordering::Acquire) => {
                        trace::info!("connection to {} closed", self.peer());
                    }
                    Closed::Orderly => {
                        trace::info!("{} closed the connection (CloseConnection)", self.peer())
                    }
                    Closed::Failed(e) => trace::info!("connection to {} lost: {e}", self.peer()),
                }
            }
            std::mem::take(&mut pending.waiters)
        };
        // Ends the wait of a caller reading its reply. The stream of a
        // connection a client offered belongs to the server connection.
        if !self.is_accepted() {
            self.inbound.shutdown();
        }
        for (_, tx) in waiters {
            let _ = tx.send(Err(why.clone()));
        }
    }

    /// `host:port` of the server, for trace lines.
    fn peer(&self) -> Peer<'_> {
        Peer(&self.endpoint)
    }

    pub(crate) fn deliver(&self, request_id: u32, msg: Message) {
        self.last_used.touch();
        let tx = self.pending.lock().unwrap().waiters.remove(&request_id);
        if let Some(tx) = tx {
            let _ = tx.send(Ok(msg));
        }
    }
}

/// The server of a connection in trace lines.
struct Peer<'a>(&'a EndpointKey);

impl std::fmt::Display for Peer<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (host, port, _) = self.0;
        write!(f, "{host}:{port}")
    }
}

/// The type, version and size of an outgoing message, for trace lines.
pub(crate) fn summary(msg: &OutMessage) -> trace::MessageSummary<'_> {
    let head = msg.pieces().next().unwrap_or(&[]);
    trace::MessageSummary(head, msg.len())
}

async fn read_loop(conn: Arc<Connection>) {
    let why = match read_messages(&conn).await {
        Ok(why) => why,
        Err(e) => Closed::Failed(e),
    };
    conn.fail(why);
    if let Writer::Stream(writer) = &conn.writer {
        let _ = writer.lock().await.shutdown().await;
    }
    // The socket closes once the last caller holding the connection is
    // done with it.
    conn.leave_pool();
}

impl Connection {
    /// Takes the connection out of its pool, under every key it has there
    /// (a connection a client offered has one per listen point).
    pub(crate) fn leave_pool(self: &Arc<Self>) {
        if let Some(pool) = self.pool.upgrade() {
            pool.lock().unwrap().retain(|_, c| !Arc::ptr_eq(c, self));
        }
    }
}

/// Reads messages until the stream ends. Returns how it ended.
async fn read_messages(conn: &Connection) -> Result<Closed, String> {
    loop {
        let (version, message) = match conn.inbound.next().await {
            Ok(read) => read,
            Err(ReadError::Closed(e)) if e.kind() == io::ErrorKind::UnexpectedEof => {
                return Err("connection closed by the server".into());
            }
            Err(ReadError::Closed(e) | ReadError::Truncated(e)) => {
                return Err(format!("read failed: {e}"));
            }
            Err(ReadError::BadHeader(e) | ReadError::Malformed(_, e)) => return Err(e),
            Err(ReadError::TooLarge(total, _)) => {
                return Err(format!("message of {total} bytes exceeds the limit"));
            }
        };
        match message {
            Message::Reply { ref header, .. } => conn.deliver(header.request_id, message),
            Message::LocateReply { ref header, .. } => conn.deliver(header.request_id, message),
            Message::CloseConnection => return Ok(Closed::Orderly),
            Message::MessageError => return Err("the server sent MessageError".into()),
            Message::Request { header, body } => {
                // Without an offer of bidirectional GIOP, requests from
                // the server are not expected and are dropped.
                if let Some(serving) = conn.serving.get() {
                    if let Some(server) = serving.server.upgrade() {
                        server.on_request_detached(&serving.conn, version, header, body);
                    }
                }
            }
            Message::LocateRequest(header) => {
                if let Some(serving) = conn.serving.get() {
                    if let Some(server) = serving.server.upgrade() {
                        server.on_locate_request(&serving.conn, version, &header);
                    }
                }
            }
            Message::CancelRequest(header) => {
                if let Some(serving) = conn.serving.get() {
                    serving.conn.cancel_request(header.request_id);
                }
            }
            Message::Fragment { .. } => {}
        }
    }
}

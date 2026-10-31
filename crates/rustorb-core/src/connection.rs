//! Client side GIOP connections.
//!
//! A [`Connection`] owns one TCP stream. Requests are written under a lock,
//! and a reader task takes every message off the stream, joins fragments
//! and hands each `Reply` or `LocateReply` to the caller waiting for its
//! request id. A caller that is the only one waiting reads its reply
//! itself instead ([`Connection::read_reply_here`]). A connection to a
//! TLS endpoint does both through [`Tls`].

use std::collections::HashMap;
use std::io::{self, IoSlice};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use rustorb_cdr::ByteOrder;
use rustorb_giop::{Message, MessageEncoder, OutMessage, Version};
use tokio::io::AsyncWriteExt;
use tokio::net::TcpSocket;
use tokio::net::tcp::OwnedWriteHalf;
use tokio::sync::oneshot;

use crate::tls::{self, Tls};
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
}

/// What the ORB pools connections by: host, port, and whether the
/// connection uses TLS.
pub(crate) type EndpointKey = (String, u16, bool);

/// What a waiting caller receives.
pub(crate) type Delivery = Result<Message, Closed>;

#[derive(Debug)]
struct Pending {
    waiters: HashMap<u32, oneshot::Sender<Delivery>>,
    closed: Option<Closed>,
}

/// One client connection to a server endpoint.
#[derive(Debug)]
pub(crate) struct Connection {
    endpoint: EndpointKey,
    inbound: Inbound,
    writer: tokio::sync::Mutex<OwnedWriteHalf>,
    tls: Option<Arc<Tls>>,
    pending: Mutex<Pending>,
    next_request_id: AtomicU32,
    /// The code sets of the first request with a code set context; `None`
    /// until one was sent.
    code_sets: Mutex<Option<CodeSets>>,
    closing: AtomicBool,
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
                Err(e) => last = e,
            }
        }
        let stream = connected.ok_or(last)?;
        stream.set_nodelay(true)?;
        let session = match tls {
            Some(config) => Some(tls::connect(&stream, config, host).await?),
            None => None,
        };
        let (reader, writer) = stream.into_split();
        let conn = Arc::new(Connection {
            endpoint: (host.to_owned(), port, session.is_some()),
            inbound: Inbound::new(reader, session.clone(), max_message_size),
            writer: tokio::sync::Mutex::new(writer),
            tls: session,
            pending: Mutex::new(Pending {
                waiters: HashMap::new(),
                closed: None,
            }),
            next_request_id: AtomicU32::new(0),
            code_sets: Mutex::new(None),
            closing: AtomicBool::new(false),
        });
        tokio::spawn(read_loop(Arc::clone(&conn)));
        Ok(conn)
    }

    pub(crate) fn endpoint(&self) -> &EndpointKey {
        &self.endpoint
    }

    /// Whether the connection can take new requests.
    pub(crate) fn is_usable(&self) -> bool {
        !self.closing.load(Ordering::Acquire) && self.pending.lock().unwrap().closed.is_none()
    }

    pub(crate) fn next_request_id(&self) -> u32 {
        self.next_request_id.fetch_add(1, Ordering::Relaxed)
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
    }

    /// Writes one whole message, giving up at `deadline`. A message cut
    /// short by the deadline leaves the stream unusable, so the connection
    /// is then closed; one not started leaves it as it was.
    pub(crate) async fn send(
        &self,
        msg: &OutMessage,
        deadline: Option<Instant>,
    ) -> Result<(), SendError> {
        let mut writer = match deadline {
            Some(at) => match tokio::time::timeout_at(at.into(), self.writer.lock()).await {
                Ok(writer) => writer,
                Err(_) => return Err(SendError::TimedOut),
            },
            None => self.writer.lock().await,
        };
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

    /// Sends `CloseConnection` if nothing is outstanding and shuts the
    /// stream down. Requests issued afterwards use a new connection.
    /// A peer that stopped reading gets [`CLOSE_TIME`] before the stream
    /// is dropped without it.
    pub(crate) async fn close(&self, version: Version, order: ByteOrder) {
        self.closing.store(true, Ordering::Release);
        if let Ok(msg) = MessageEncoder::close_connection(version, order) {
            let _ = tokio::time::timeout(CLOSE_TIME, async {
                let mut writer = self.writer.lock().await;
                match &self.tls {
                    Some(tls) => {
                        tls.encrypt(&msg.into(), 0);
                        tls.close_notify();
                        while let Ok(false) = tls.flush(|bytes| writer.try_write(bytes)) {
                            if writer.writable().await.is_err() {
                                break;
                            }
                        }
                    }
                    None => {
                        let _ = writer.write_all(&msg).await;
                    }
                }
                let _ = writer.shutdown().await;
            })
            .await;
        }
    }

    /// Marks the connection closed and wakes every waiting caller.
    fn fail(&self, why: Closed) {
        let waiters = {
            let mut pending = self.pending.lock().unwrap();
            if pending.closed.is_none() {
                pending.closed = Some(why.clone());
            }
            std::mem::take(&mut pending.waiters)
        };
        // Ends the wait of a caller reading its reply.
        self.inbound.shutdown();
        for (_, tx) in waiters {
            let _ = tx.send(Err(why.clone()));
        }
    }

    fn deliver(&self, request_id: u32, msg: Message) {
        let tx = self.pending.lock().unwrap().waiters.remove(&request_id);
        if let Some(tx) = tx {
            let _ = tx.send(Ok(msg));
        }
    }
}

async fn read_loop(conn: Arc<Connection>) {
    let why = match read_messages(&conn).await {
        Ok(why) => why,
        Err(e) => Closed::Failed(e),
    };
    conn.fail(why);
    let _ = conn.writer.lock().await.shutdown().await;
}

/// Reads messages until the stream ends. Returns how it ended.
async fn read_messages(conn: &Connection) -> Result<Closed, String> {
    loop {
        let (_, message) = match conn.inbound.next().await {
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
            // A client connection is not bidirectional: requests from the
            // server are not expected and are dropped.
            Message::Request { .. }
            | Message::LocateRequest(_)
            | Message::CancelRequest(_)
            | Message::Fragment { .. } => {}
        }
    }
}

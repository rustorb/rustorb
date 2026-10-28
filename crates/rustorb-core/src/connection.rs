//! Client side GIOP connections.
//!
//! A [`Connection`] owns one TCP stream. Requests are written under a lock,
//! and a reader task takes every message off the stream, joins fragments
//! and hands each `Reply` or `LocateReply` to the caller waiting for its
//! request id.

use std::collections::HashMap;
use std::io;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use rustorb_cdr::ByteOrder;
use rustorb_giop::{
    AssembledMessage, FragmentAssembler, Message, MessageEncoder, MessageHeader, Version,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::sync::oneshot;

use crate::CodeSets;

/// Why a connection stopped delivering replies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Closed {
    /// The server sent `CloseConnection`: requests without a reply were
    /// not processed and may be sent again.
    Orderly,
    /// The stream failed, or a message could not be read.
    Failed(String),
}

/// What a waiting caller receives.
pub(crate) type Delivery = Result<AssembledMessage, Closed>;

#[derive(Debug)]
struct Pending {
    waiters: HashMap<u32, oneshot::Sender<Delivery>>,
    closed: Option<Closed>,
}

/// One client connection to a server endpoint.
#[derive(Debug)]
pub(crate) struct Connection {
    endpoint: (String, u16),
    writer: tokio::sync::Mutex<OwnedWriteHalf>,
    pending: Mutex<Pending>,
    next_request_id: AtomicU32,
    /// The code sets of the first request with a code set context; `None`
    /// until one was sent.
    code_sets: Mutex<Option<CodeSets>>,
    closing: AtomicBool,
}

impl Connection {
    /// Connects to `host:port` and starts the reader task on the current
    /// runtime.
    pub(crate) async fn connect(
        host: &str,
        port: u16,
        max_message_size: usize,
    ) -> io::Result<Arc<Connection>> {
        let stream = TcpStream::connect((host, port)).await?;
        stream.set_nodelay(true)?;
        let (reader, writer) = stream.into_split();
        let conn = Arc::new(Connection {
            endpoint: (host.to_owned(), port),
            writer: tokio::sync::Mutex::new(writer),
            pending: Mutex::new(Pending {
                waiters: HashMap::new(),
                closed: None,
            }),
            next_request_id: AtomicU32::new(0),
            code_sets: Mutex::new(None),
            closing: AtomicBool::new(false),
        });
        tokio::spawn(read_loop(Arc::clone(&conn), reader, max_message_size));
        Ok(conn)
    }

    pub(crate) fn endpoint(&self) -> &(String, u16) {
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

    /// Stops waiting for `request_id`, after a timeout.
    pub(crate) fn forget(&self, request_id: u32) {
        self.pending.lock().unwrap().waiters.remove(&request_id);
    }

    /// Writes one whole message.
    pub(crate) async fn send(&self, msg: &[u8]) -> io::Result<()> {
        let mut writer = self.writer.lock().await;
        let result = writer.write_all(msg).await;
        if let Err(e) = &result {
            drop(writer);
            self.fail(Closed::Failed(format!("write failed: {e}")));
        }
        result
    }

    /// Sends `CloseConnection` if nothing is outstanding and shuts the
    /// stream down. Requests issued afterwards use a new connection.
    pub(crate) async fn close(&self, version: Version, order: ByteOrder) {
        self.closing.store(true, Ordering::Release);
        if let Ok(msg) = MessageEncoder::close_connection(version, order) {
            let mut writer = self.writer.lock().await;
            let _ = writer.write_all(&msg).await;
            let _ = writer.shutdown().await;
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
        for (_, tx) in waiters {
            let _ = tx.send(Err(why.clone()));
        }
    }

    fn deliver(&self, request_id: u32, msg: AssembledMessage) {
        let tx = self.pending.lock().unwrap().waiters.remove(&request_id);
        if let Some(tx) = tx {
            let _ = tx.send(Ok(msg));
        }
    }
}

async fn read_loop(conn: Arc<Connection>, mut reader: OwnedReadHalf, max_message_size: usize) {
    let why = match read_messages(&conn, &mut reader, max_message_size).await {
        Ok(why) => why,
        Err(e) => Closed::Failed(e),
    };
    conn.fail(why);
    let _ = conn.writer.lock().await.shutdown().await;
}

/// Reads messages until the stream ends. Returns how it ended.
async fn read_messages(
    conn: &Connection,
    reader: &mut OwnedReadHalf,
    max_message_size: usize,
) -> Result<Closed, String> {
    let mut assembler = FragmentAssembler::new(max_message_size);
    loop {
        let mut header = [0u8; MessageHeader::SIZE];
        match reader.read_exact(&mut header).await {
            Ok(_) => {}
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => {
                return Err("connection closed by the server".into());
            }
            Err(e) => return Err(format!("read failed: {e}")),
        }
        let parsed = MessageHeader::parse(&header).map_err(|e| e.to_string())?;
        let total = parsed.total_len();
        if total > max_message_size {
            return Err(format!("message of {total} bytes exceeds the limit"));
        }
        let mut msg = Vec::with_capacity(total);
        msg.extend_from_slice(&header);
        msg.resize(total, 0);
        reader
            .read_exact(&mut msg[MessageHeader::SIZE..])
            .await
            .map_err(|e| format!("read failed: {e}"))?;

        let Some(whole) = assembler.push(&msg).map_err(|e| e.to_string())? else {
            continue;
        };
        let (_, message) = whole.decode().map_err(|e| e.to_string())?;
        match message {
            Message::Reply { header, .. } => conn.deliver(header.request_id, whole),
            Message::LocateReply { header, .. } => conn.deliver(header.request_id, whole),
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

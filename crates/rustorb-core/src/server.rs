//! The server side: listening endpoints, server connections and the
//! dispatch of requests to POAs.
//!
//! The I/O tasks only read and write GIOP messages, parse object keys and
//! find the target POA; servants always run on the upcall pool. Replies go
//! to a writer task per connection through a channel and are written in
//! the order they complete, which need not be the order of the requests
//! (omniORB does the same).

use std::collections::hash_map::RandomState;
use std::collections::{HashMap, HashSet};
use std::hash::{BuildHasher, Hasher};
use std::io;
use std::panic::{self, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock, Weak};
use std::time::{Duration, SystemTime};

use rustorb_cdr::ByteOrder;
use rustorb_giop::{
    AlternateIiopAddress, Body, CodeSetComponent, CodeSetComponentInfo, CodeSetContext,
    FragmentAssembler, IiopProfile, Ior, LocateReplyHeader, LocateRequestHeader, LocateStatus,
    Message, MessageEncoder, MessageHeader, RequestHeader, ServiceContext, TargetAddress, Version,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::{TcpListener, TcpStream};
use tokio::runtime::Handle;
use tokio::sync::{OwnedSemaphorePermit, Semaphore, mpsc};
use tokio::task::JoinHandle;

use crate::codeset::{CONVERSION_CHAR, NATIVE_CHAR, NATIVE_WCHAR};
use crate::orb::OrbInner;
use crate::poa::{
    BOOT_ID_LEN, Dispatch, Disposition, ManagerInner, ParsedKey, PoaInner, PoaManager, UpcallGuard,
    log, no_such_object, parse_key,
};
use crate::pool::{Job, Pool};
use crate::servant::{OBJECT_ID, ServerReply, system_exception_reply};
use crate::stream::InStream;
use crate::{
    CodeSets, CompletionStatus, EndpointAddr, Lifespan, Orb, Servant, SystemException,
    SystemExceptionKind,
};

/// The server side of an ORB, started with the RootPOA.
pub(crate) struct Server {
    orb: Weak<OrbInner>,
    boot_id: [u8; BOOT_ID_LEN],
    /// Host and port of each endpoint as published in IORs.
    published: Vec<(String, u16)>,
    fixed_port: bool,
    order: ByteOrder,
    max_message_size: usize,
    max_requests: usize,
    root: OnceLock<Arc<PoaInner>>,
    pool: Mutex<Option<Pool>>,
    managers: Mutex<Vec<Weak<ManagerInner>>>,
    connections: Mutex<HashMap<u64, Arc<ServerConn>>>,
    next_connection: AtomicU64,
    listeners: Mutex<Vec<JoinHandle<()>>>,
    shutting_down: AtomicBool,
    in_flight: Mutex<usize>,
    in_flight_done: Condvar,
}

impl std::fmt::Debug for Server {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Server")
            .field("published", &self.published)
            .finish_non_exhaustive()
    }
}

/// The code sets rustorb publishes in its IORs: UTF-8 for `char`, which
/// it converts to ISO 8859-1, and UTF-16 for `wchar`.
fn code_set_info() -> CodeSetComponentInfo {
    CodeSetComponentInfo {
        for_char_data: CodeSetComponent {
            native_code_set: NATIVE_CHAR,
            conversion_code_sets: CONVERSION_CHAR.to_vec(),
        },
        for_wchar_data: CodeSetComponent {
            native_code_set: NATIVE_WCHAR,
            conversion_code_sets: Vec::new(),
        },
    }
}

/// A random id for this start of the ORB. `RandomState` is seeded from the
/// operating system's random source.
fn new_boot_id() -> [u8; BOOT_ID_LEN] {
    let mut h = RandomState::new().build_hasher();
    if let Ok(t) = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
        h.write_u128(t.as_nanos());
    }
    h.write_u32(std::process::id());
    h.finish().to_be_bytes()
}

/// The host name of the machine, published when an endpoint names no host.
fn host_name() -> String {
    let candidates = [
        std::fs::read_to_string("/proc/sys/kernel/hostname").ok(),
        std::env::var("HOSTNAME").ok(),
    ];
    candidates
        .into_iter()
        .flatten()
        .map(|h| h.trim().to_owned())
        .find(|h| !h.is_empty())
        .or_else(|| {
            let out = std::process::Command::new("hostname").output().ok()?;
            let name = String::from_utf8(out.stdout).ok()?.trim().to_owned();
            (!name.is_empty()).then_some(name)
        })
        .unwrap_or_else(|| "localhost".into())
}

fn initialize() -> SystemException {
    SystemException::new(SystemExceptionKind::INITIALIZE, 0, CompletionStatus::No)
}

impl Server {
    /// Opens the endpoints of `orb` and creates the RootPOA.
    pub(crate) fn start(orb: &Orb) -> Result<Arc<Server>, SystemException> {
        let options = &orb.inner.options;
        let endpoints = if options.endpoints.is_empty() {
            vec![EndpointAddr {
                host: String::new(),
                port: 0,
            }]
        } else {
            options.endpoints.clone()
        };
        let mut listeners = Vec::new();
        let mut published = Vec::new();
        for (i, ep) in endpoints.iter().enumerate() {
            let bind_host = if ep.host.is_empty() {
                "0.0.0.0"
            } else {
                &ep.host
            };
            let listener =
                std::net::TcpListener::bind((bind_host, ep.port)).map_err(|_| initialize())?;
            listener.set_nonblocking(true).map_err(|_| initialize())?;
            let port = listener.local_addr().map_err(|_| initialize())?.port();
            let publish = options.endpoint_publish.as_ref().filter(|_| i == 0);
            let host = match publish {
                Some(p) if !p.host.is_empty() => p.host.clone(),
                _ if !ep.host.is_empty() => ep.host.clone(),
                _ => host_name(),
            };
            let port = match publish {
                Some(p) if p.port != 0 => p.port,
                _ => port,
            };
            listeners.push(listener);
            published.push((host, port));
        }
        let server = Arc::new(Server {
            orb: Arc::downgrade(&orb.inner),
            boot_id: new_boot_id(),
            published,
            fixed_port: endpoints.iter().any(|ep| ep.port != 0),
            order: options.byte_order,
            max_message_size: options.max_message_size,
            max_requests: options.max_requests_per_connection.max(1),
            root: OnceLock::new(),
            pool: Mutex::new(Some(Pool::new(
                options.min_server_threads,
                options.max_server_threads,
                options.server_thread_idle_timeout,
            ))),
            managers: Mutex::new(Vec::new()),
            connections: Mutex::new(HashMap::new()),
            next_connection: AtomicU64::new(0),
            listeners: Mutex::new(Vec::new()),
            shutting_down: AtomicBool::new(false),
            in_flight: Mutex::new(0),
            in_flight_done: Condvar::new(),
        });
        let manager = PoaManager::new(&server);
        let _ = server.root.set(PoaInner::new_root(manager, server.boot_id));

        let handle = orb.inner.handle.clone();
        let _entered = handle.enter();
        let mut tasks = Vec::new();
        for listener in listeners {
            let listener = TcpListener::from_std(listener).map_err(|_| initialize())?;
            tasks.push(handle.spawn(accept_loop(Arc::downgrade(&server), listener)));
        }
        *server.listeners.lock().unwrap() = tasks;
        Ok(server)
    }

    pub(crate) fn root(&self) -> &Arc<PoaInner> {
        self.root
            .get()
            .expect("the RootPOA is created with the server")
    }

    /// Whether some endpoint listens on a port given in the options.
    pub(crate) fn has_fixed_port(&self) -> bool {
        self.fixed_port
    }

    /// Host and port of each endpoint, as published in IORs.
    pub(crate) fn published(&self) -> &[(String, u16)] {
        &self.published
    }

    pub(crate) fn register_manager(&self, manager: &Arc<ManagerInner>) {
        let mut managers = self.managers.lock().unwrap();
        managers.retain(|m| m.strong_count() > 0);
        managers.push(Arc::downgrade(manager));
    }

    /// Runs `job` on the upcall pool.
    pub(crate) fn submit(&self, job: Job) {
        if let Some(pool) = &*self.pool.lock().unwrap() {
            pool.submit(job);
        }
    }

    /// An IOR for `key` with one IIOP 1.2 profile: the first endpoint, the
    /// code sets, and the other endpoints as alternate addresses.
    pub(crate) fn ior(&self, type_id: &str, key: Vec<u8>) -> Result<Ior, SystemException> {
        let marshal =
            |_| SystemException::new(SystemExceptionKind::MARSHAL, 0, CompletionStatus::No);
        let (host, port) = &self.published[0];
        let mut components = vec![code_set_info().to_component(self.order).map_err(marshal)?];
        for (host, port) in &self.published[1..] {
            let alternate = AlternateIiopAddress {
                host: host.clone(),
                port: *port,
            };
            components.push(alternate.to_component(self.order).map_err(marshal)?);
        }
        let profile = IiopProfile {
            version: Version::V1_2,
            host: host.clone(),
            port: *port,
            object_key: key,
            components,
        };
        Ok(Ior {
            type_id: type_id.to_owned(),
            profiles: vec![profile.to_tagged_profile(self.order).map_err(marshal)?],
        })
    }

    fn orb(&self) -> Option<Orb> {
        self.orb.upgrade().map(|inner| Orb { inner })
    }

    fn is_shutting_down(&self) -> bool {
        self.shutting_down.load(Ordering::Acquire)
    }

    /// The POA and ObjectId a key denotes.
    fn find(&self, key: &[u8]) -> Result<(Arc<PoaInner>, Vec<u8>), SystemException> {
        let ParsedKey::Poa { path, boot_id, id } = parse_key(key) else {
            return Err(no_such_object());
        };
        let mut poa = Arc::clone(self.root());
        for name in path {
            poa = poa.child(name).ok_or_else(no_such_object)?;
        }
        let lifespan_matches = match (poa.policies().lifespan, boot_id) {
            (Lifespan::Persistent, None) => true,
            (Lifespan::Transient, Some(boot_id)) => boot_id == self.boot_id,
            _ => false,
        };
        if !lifespan_matches || poa.is_destroyed() {
            return Err(no_such_object());
        }
        Ok((poa, id.to_vec()))
    }

    fn ticket(self: &Arc<Self>, permit: OwnedSemaphorePermit) -> Ticket {
        *self.in_flight.lock().unwrap() += 1;
        Ticket {
            server: Arc::clone(self),
            _permit: permit,
        }
    }

    async fn on_request(
        self: &Arc<Self>,
        conn: &Arc<ServerConn>,
        version: Version,
        header: RequestHeader,
        body: Body,
    ) {
        if self.is_shutting_down() {
            // Not processed: the CloseConnection that follows tells the
            // client it may send the request again.
            return;
        }
        let code_sets = conn.code_sets_for(version, &header.service_context);
        conn.saw_version(version);
        let Ok(permit) = Arc::clone(&conn.permits).acquire_owned().await else {
            return;
        };
        let ticket = self.ticket(permit);
        let reply = ReplyTo::new(
            conn,
            version,
            self.order,
            header.request_id,
            header.response_expected(),
        );
        let Some(key) = target_key(&header.target) else {
            reply.system(&no_such_object());
            return;
        };
        let (poa, id) = match self.find(&key) {
            Ok(found) => found,
            Err(e) => {
                reply.system(&e);
                return;
            }
        };
        let server = Arc::clone(self);
        let target = Arc::clone(&poa);
        let operation = header.operation;
        let dispatch: Dispatch = Box::new(move |disposition| match disposition {
            Disposition::Run => {
                server.upcall(&target, &id, &operation, &body, code_sets, reply);
                drop(ticket);
            }
            Disposition::Reject(e) => {
                reply.system(&e);
                drop(ticket);
            }
        });
        poa.manager.inner.dispatch(self, dispatch);
    }

    /// Calls the servant, on an upcall thread.
    fn upcall(
        &self,
        poa: &Arc<PoaInner>,
        id: &[u8],
        operation: &str,
        body: &Body,
        code_sets: CodeSets,
        reply: ReplyTo,
    ) {
        if reply.is_cancelled() {
            return;
        }
        let Some(orb) = self.orb() else { return };
        let Some(servant) = poa.servant(id) else {
            reply.system(&no_such_object());
            return;
        };
        let _poa_activity = poa.activity().enter();
        let _manager_activity = poa.manager.inner.activity().enter();
        let mut out = match ServerReply::new(reply.version, self.order, reply.request_id, code_sets)
        {
            Ok(out) => out,
            Err(e) => {
                reply.system(&e);
                return;
            }
        };
        let mut input = InStream {
            dec: body.decoder(),
            code_sets,
            orb: &orb,
            completed: CompletionStatus::No,
            wchar_needs_code_set: true,
        };
        let result = {
            let _upcall = UpcallGuard::enter();
            panic::catch_unwind(AssertUnwindSafe(|| {
                invoke(&*servant, operation, &mut input, &mut out)
            }))
        };
        match result {
            Ok(Ok(())) => match out.finish() {
                Ok(msg) => reply.send(msg),
                Err(e) => reply.system(&e),
            },
            Ok(Err(e)) => reply.system(&e),
            Err(_) => {
                log::warn(&format!("servant panicked in {operation:?}"));
                reply.system(&SystemException::new(
                    SystemExceptionKind::UNKNOWN,
                    0,
                    CompletionStatus::Maybe,
                ));
            }
        }
    }

    fn on_locate(&self, conn: &ServerConn, version: Version, header: &LocateRequestHeader) {
        if self.is_shutting_down() {
            return;
        }
        conn.saw_version(version);
        let here = target_key(&header.target)
            .and_then(|key| self.find(&key).ok())
            .is_some_and(|(poa, id)| {
                !poa.manager.inner.is_inactive() && poa.servant(&id).is_some()
            });
        let status = if here {
            LocateStatus::ObjectHere
        } else {
            LocateStatus::UnknownObject
        };
        let reply = LocateReplyHeader {
            request_id: header.request_id,
            locate_status: status,
        };
        if let Ok(msg) = MessageEncoder::locate_reply(version, self.order, &reply)
            .and_then(MessageEncoder::finish)
        {
            conn.send(Outgoing::Message(msg));
        }
    }

    fn add_connection(&self, conn: &Arc<ServerConn>) -> u64 {
        let id = self.next_connection.fetch_add(1, Ordering::Relaxed);
        self.connections
            .lock()
            .unwrap()
            .insert(id, Arc::clone(conn));
        id
    }

    fn remove_connection(&self, id: u64) {
        self.connections.lock().unwrap().remove(&id);
    }

    /// Stops the server: no new connections or requests, held requests
    /// dropped, the running ones finished and answered, every POA
    /// destroyed, and then `CloseConnection` on every connection.
    ///
    /// Blocks; must not run on a runtime thread or in a servant.
    pub(crate) fn shutdown(&self, handle: &Handle) {
        if self.shutting_down.swap(true, Ordering::AcqRel) {
            return;
        }
        for task in self.listeners.lock().unwrap().drain(..) {
            task.abort();
        }
        let managers: Vec<_> = self
            .managers
            .lock()
            .unwrap()
            .iter()
            .filter_map(Weak::upgrade)
            .collect();
        for manager in managers {
            manager.shut_down();
        }
        let mut in_flight = self.in_flight.lock().unwrap();
        while *in_flight > 0 {
            in_flight = self.in_flight_done.wait(in_flight).unwrap();
        }
        drop(in_flight);
        self.root().destroy_tree();

        let conns: Vec<_> = self
            .connections
            .lock()
            .unwrap()
            .drain()
            .map(|(_, c)| c)
            .collect();
        let mut writers = Vec::new();
        for conn in conns {
            let version = *conn.version.lock().unwrap();
            if let Ok(msg) = MessageEncoder::close_connection(version, self.order) {
                conn.send(Outgoing::Close(msg));
            }
            writers.extend(conn.writer.lock().unwrap().take());
        }
        handle.block_on(async move {
            for writer in writers {
                let _ = tokio::time::timeout(Duration::from_secs(5), writer).await;
            }
        });
    }

    /// Ends the upcall threads once they finish their current job.
    pub(crate) fn stop_pool(&self) {
        let pool = self.pool.lock().unwrap().take();
        drop(pool);
    }
}

/// Runs the ORB's part of a request: the operations every object has, and
/// the servant for the rest.
fn invoke(
    servant: &dyn Servant,
    operation: &str,
    input: &mut InStream<'_>,
    reply: &mut ServerReply,
) -> Result<(), SystemException> {
    match operation {
        "_is_a" => {
            let id = input.read_string()?;
            let is_a = id == OBJECT_ID || servant.is_a(&id);
            reply.output().write_bool(is_a);
            Ok(())
        }
        "_non_existent" => {
            reply.output().write_bool(false);
            Ok(())
        }
        "_interface" => Err(SystemException::new(
            SystemExceptionKind::INTF_REPOS,
            0,
            CompletionStatus::No,
        )),
        "_repository_id" => reply.output().write_string(servant.repository_id()),
        _ => servant.dispatch(operation, input, reply),
    }
}

/// The object key of a request's target.
fn target_key(target: &TargetAddress) -> Option<Vec<u8>> {
    match target {
        TargetAddress::Key(key) => Some(key.clone()),
        TargetAddress::Profile(profile) => profile.iiop().ok().map(|p| p.object_key),
        TargetAddress::Reference {
            selected_profile_index,
            ior,
        } => ior
            .profiles
            .get(usize::try_from(*selected_profile_index).ok()?)?
            .iiop()
            .ok()
            .map(|p| p.object_key),
    }
}

/// Counts a request from when it is read until its reply is queued, and
/// holds its place among the connection's requests.
struct Ticket {
    server: Arc<Server>,
    _permit: OwnedSemaphorePermit,
}

impl Drop for Ticket {
    fn drop(&mut self) {
        let mut in_flight = self.server.in_flight.lock().unwrap();
        *in_flight -= 1;
        if *in_flight == 0 {
            self.server.in_flight_done.notify_all();
        }
    }
}

/// What the writer task writes.
enum Outgoing {
    Message(Vec<u8>),
    /// A last message (`CloseConnection` or `MessageError`), after which
    /// the connection is shut down.
    Close(Vec<u8>),
}

/// One connection accepted by the server.
struct ServerConn {
    tx: mpsc::UnboundedSender<Outgoing>,
    /// The code sets of the first request with a code set context.
    code_sets: Mutex<Option<CodeSets>>,
    requests: Mutex<Requests>,
    /// The GIOP version of the last message received, used for
    /// `CloseConnection`.
    version: Mutex<Version>,
    permits: Arc<Semaphore>,
    writer: Mutex<Option<JoinHandle<()>>>,
}

#[derive(Default)]
struct Requests {
    pending: HashSet<u32>,
    cancelled: HashSet<u32>,
}

impl ServerConn {
    fn send(&self, msg: Outgoing) {
        let _ = self.tx.send(msg);
    }

    fn saw_version(&self, version: Version) {
        *self.version.lock().unwrap() = version;
    }

    /// The code sets for a request: fixed by the first code set context of
    /// the connection (GIOP 1.1 and later), the defaults before that.
    fn code_sets_for(&self, version: Version, contexts: &[ServiceContext]) -> CodeSets {
        if version < Version::V1_1 {
            return CodeSets::DEFAULT;
        }
        let mut fixed = self.code_sets.lock().unwrap();
        if fixed.is_none() {
            if let Some(Ok(cs)) = CodeSetContext::find(contexts) {
                *fixed = Some(CodeSets {
                    char_data: cs.char_data,
                    wchar_data: (cs.wchar_data != 0).then_some(cs.wchar_data),
                });
            }
        }
        fixed.unwrap_or(CodeSets::DEFAULT)
    }

    fn begin(&self, request_id: u32) {
        self.requests.lock().unwrap().pending.insert(request_id);
    }

    fn cancel(&self, request_id: u32) {
        let mut requests = self.requests.lock().unwrap();
        if requests.pending.contains(&request_id) {
            requests.cancelled.insert(request_id);
        }
    }

    fn is_cancelled(&self, request_id: u32) -> bool {
        self.requests
            .lock()
            .unwrap()
            .cancelled
            .contains(&request_id)
    }

    /// Forgets the request; returns whether it was cancelled.
    fn end(&self, request_id: u32) -> bool {
        let mut requests = self.requests.lock().unwrap();
        requests.pending.remove(&request_id);
        requests.cancelled.remove(&request_id)
    }
}

/// Where the reply to one request goes. Dropping it without a reply
/// forgets the request.
struct ReplyTo {
    conn: Option<Arc<ServerConn>>,
    version: Version,
    order: ByteOrder,
    request_id: u32,
    response: bool,
}

impl ReplyTo {
    fn new(
        conn: &Arc<ServerConn>,
        version: Version,
        order: ByteOrder,
        request_id: u32,
        response: bool,
    ) -> ReplyTo {
        conn.begin(request_id);
        ReplyTo {
            conn: Some(Arc::clone(conn)),
            version,
            order,
            request_id,
            response,
        }
    }

    fn is_cancelled(&self) -> bool {
        self.conn
            .as_ref()
            .is_some_and(|c| c.is_cancelled(self.request_id))
    }

    /// Queues the reply, unless the request was oneway or cancelled.
    fn send(mut self, msg: Vec<u8>) {
        if let Some(conn) = self.conn.take() {
            let cancelled = conn.end(self.request_id);
            if self.response && !cancelled {
                conn.send(Outgoing::Message(msg));
            }
        }
    }

    fn system(self, e: &SystemException) {
        if !self.response {
            return;
        }
        match system_exception_reply(self.version, self.order, self.request_id, e) {
            Some(msg) => self.send(msg),
            None => drop(self),
        }
    }
}

impl Drop for ReplyTo {
    fn drop(&mut self) {
        if let Some(conn) = self.conn.take() {
            conn.end(self.request_id);
        }
    }
}

async fn accept_loop(server: Weak<Server>, listener: TcpListener) {
    loop {
        let Ok((stream, _)) = listener.accept().await else {
            // Out of file descriptors and the like: try again shortly.
            tokio::time::sleep(Duration::from_millis(100)).await;
            continue;
        };
        let Some(server) = server.upgrade() else {
            return;
        };
        if server.is_shutting_down() {
            return;
        }
        tokio::spawn(serve(server, stream));
    }
}

async fn serve(server: Arc<Server>, stream: TcpStream) {
    let _ = stream.set_nodelay(true);
    let (mut reader, writer) = stream.into_split();
    let (tx, rx) = mpsc::unbounded_channel();
    let conn = Arc::new(ServerConn {
        tx,
        code_sets: Mutex::new(None),
        requests: Mutex::default(),
        version: Mutex::new(Version::V1_0),
        permits: Arc::new(Semaphore::new(server.max_requests)),
        writer: Mutex::new(None),
    });
    *conn.writer.lock().unwrap() = Some(tokio::spawn(write_loop(writer, rx)));
    let id = server.add_connection(&conn);
    let ended = read_loop(&server, &conn, &mut reader).await;
    server.remove_connection(id);
    match ended {
        Ok(Ended::Peer) => {}
        Ok(Ended::CloseConnection) => conn.send(Outgoing::Close(Vec::new())),
        Err(version) => {
            if let Ok(msg) = MessageEncoder::message_error(version, server.order) {
                conn.send(Outgoing::Close(msg));
            }
        }
    }
    // The writer ends once the requests still running have queued their
    // replies and dropped the connection.
}

async fn write_loop(mut writer: OwnedWriteHalf, mut rx: mpsc::UnboundedReceiver<Outgoing>) {
    while let Some(out) = rx.recv().await {
        match out {
            Outgoing::Message(msg) => {
                if writer.write_all(&msg).await.is_err() {
                    return;
                }
            }
            Outgoing::Close(msg) => {
                let _ = writer.write_all(&msg).await;
                break;
            }
        }
    }
    let _ = writer.shutdown().await;
}

/// How a connection's read loop ended normally.
enum Ended {
    /// The client closed the stream.
    Peer,
    /// The client sent `CloseConnection` or `MessageError`.
    CloseConnection,
}

/// Reads requests until the connection ends. `Err` carries the GIOP
/// version for the `MessageError` sent back for a malformed message.
async fn read_loop(
    server: &Arc<Server>,
    conn: &Arc<ServerConn>,
    reader: &mut OwnedReadHalf,
) -> Result<Ended, Version> {
    let max = server.max_message_size;
    let mut assembler = FragmentAssembler::new(max);
    loop {
        let mut header = [0u8; MessageHeader::SIZE];
        match reader.read_exact(&mut header).await {
            Ok(_) => {}
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(Ended::Peer),
            Err(_) => return Ok(Ended::Peer),
        }
        let parsed = MessageHeader::parse(&header).map_err(|_| Version::V1_0)?;
        let version = parsed.version;
        let total = parsed.total_len();
        if total > max {
            return Err(version);
        }
        let mut msg = Vec::with_capacity(total);
        msg.extend_from_slice(&header);
        msg.resize(total, 0);
        if reader
            .read_exact(&mut msg[MessageHeader::SIZE..])
            .await
            .is_err()
        {
            return Ok(Ended::Peer);
        }
        let Some(whole) = assembler.push(&msg).map_err(|_| version)? else {
            continue;
        };
        let (_, message) = whole.decode().map_err(|_| version)?;
        match message {
            Message::Request { header, body } => {
                server.on_request(conn, version, header, body).await;
            }
            Message::LocateRequest(header) => server.on_locate(conn, version, &header),
            Message::CancelRequest(header) => {
                assembler.cancel(header.request_id);
                conn.cancel(header.request_id);
            }
            Message::CloseConnection | Message::MessageError => {
                return Ok(Ended::CloseConnection);
            }
            // Not bidirectional: replies have no request to answer.
            Message::Reply { .. } | Message::LocateReply { .. } | Message::Fragment { .. } => {
                return Err(version);
            }
        }
    }
}

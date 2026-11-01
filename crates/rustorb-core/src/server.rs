//! The server side: listening endpoints, server connections and the
//! dispatch of requests to POAs.
//!
//! The I/O tasks only read and write GIOP messages, parse object keys and
//! find the target POA; user code (servants, servant managers, adapter
//! activators) always runs on the upcall pool, or on the main thread for
//! MAIN_THREAD_MODEL. Replies are written in the order they complete,
//! which need not be the order of the requests (omniORB does the same):
//! by the thread that made them when the socket takes them at once,
//! otherwise by a writer task per connection ([`Outbox`]).
//!
//! A TLS endpoint (SSLIOP, docs/ssliop.md) runs the handshake before the
//! connection is served, and then reads and writes through [`Tls`].
//!
//! Calls from the ORB to its own objects do not go through the network:
//! the request is marshalled as usual and dispatched on the calling thread
//! ([`Server::call_local`]).

use std::collections::hash_map::RandomState;
use std::collections::{HashMap, HashSet, VecDeque};
use std::hash::{BuildHasher, Hasher};
use std::io;
use std::net::SocketAddr;
use std::panic::{self, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock, Weak, mpsc as std_mpsc};
use std::time::{Duration, Instant, SystemTime};

use rustorb_cdr::ByteOrder;
use rustorb_giop::{
    AlternateIiopAddress, Body, CodeSetComponent, CodeSetComponentInfo, CodeSetContext,
    IiopProfile, Ior, LocateReplyHeader, LocateRequestHeader, LocateStatus, Message,
    MessageEncoder, OutMessage, RequestHeader, ServiceContext, SslSecTrans, TargetAddress, Version,
    association_options, component_tag,
};
use tokio::io::AsyncWriteExt;
use tokio::net::tcp::OwnedWriteHalf;
use tokio::net::{TcpListener, TcpStream};
use tokio::runtime::Handle;
use tokio::sync::{Notify, OwnedSemaphorePermit, Semaphore};
use tokio::task::JoinHandle;

use crate::codeset::{CONVERSION_CHAR, NATIVE_CHAR, NATIVE_WCHAR};
use crate::connection::summary;
use crate::idle::{self, Stamp};
use crate::orb::OrbInner;
use crate::poa::{
    BOOT_ID_LEN, CurrentGuard, Dispatch, Disposition, Etherealization, ManagerInner, ParsedKey,
    PoaInner, PoaManager, SerialEntry, UpcallGuard, etherealize_all, is_main_thread,
    no_such_object, parse_key,
};
use crate::pool::{Job, Pool};
use crate::servant::{OBJECT_ID, ServerReply, location_forward_reply, system_exception_reply};
use crate::stream::InStream;
use crate::tls::{self, Tls};
use crate::trace;
use crate::wire::{Here, Inbound, PeerAddr, ReadError};
use crate::{
    CodeSets, CompletionStatus, EndpointAddr, Lifespan, Orb, Poa, Servant, ServantManagerError,
    SystemException, SystemExceptionKind, ThreadPolicy,
};

/// The server side of an ORB, started with the RootPOA.
pub(crate) struct Server {
    orb: Weak<OrbInner>,
    boot_id: [u8; BOOT_ID_LEN],
    /// Host and port of each endpoint as published in IORs.
    published: Vec<(String, u16)>,
    /// The ports of the endpoints that listen on a loopback address or on
    /// every interface.
    loopback_ports: Vec<u16>,
    /// The same for the TLS endpoints.
    published_tls: Vec<(String, u16)>,
    loopback_tls_ports: Vec<u16>,
    /// Whether TLS clients must send a certificate.
    client_auth_required: bool,
    fixed_port: bool,
    order: ByteOrder,
    max_message_size: usize,
    fragment_size: usize,
    max_requests: usize,
    /// [`crate::OrbOptions::follow_linger`] and [`crate::OrbOptions::follow_limit`].
    follow_linger: Duration,
    follow_limit: usize,
    /// [`crate::OrbOptions::server_idle_timeout`].
    idle_timeout: Option<Duration>,
    root: OnceLock<Arc<PoaInner>>,
    /// Simple object keys registered with `Orb::register_object_key`, and
    /// the POA keys they stand for.
    simple_keys: Mutex<HashMap<Vec<u8>, Vec<u8>>>,
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
        let endpoints = if options.endpoints.is_empty() && options.tls_endpoints.is_empty() {
            vec![EndpointAddr {
                host: String::new(),
                port: 0,
            }]
        } else {
            options.endpoints.clone()
        };
        let tls_config = if options.tls_endpoints.is_empty() {
            None
        } else {
            Some(tls::server_config(&options.tls).map_err(|_| initialize())?)
        };
        let mut listeners = Vec::new();
        let mut published = Vec::new();
        let mut loopback_ports = Vec::new();
        let mut published_tls = Vec::new();
        let mut loopback_tls_ports = Vec::new();
        let plain = endpoints
            .iter()
            .map(|ep| (ep, false, options.endpoint_publish.as_ref()));
        let secure = options
            .tls_endpoints
            .iter()
            .map(|ep| (ep, true, options.tls_endpoint_publish.as_ref()));
        let mut first = [true, true];
        for (ep, is_tls, publish) in plain.chain(secure) {
            let bind_host = if ep.host.is_empty() {
                "0.0.0.0"
            } else {
                &ep.host
            };
            let listener =
                std::net::TcpListener::bind((bind_host, ep.port)).map_err(|_| initialize())?;
            listener.set_nonblocking(true).map_err(|_| initialize())?;
            // Accepted connections take the listener's buffer sizes.
            let socket = socket2::SockRef::from(&listener);
            if let Some(size) = options.socket_send_buffer {
                socket
                    .set_send_buffer_size(size)
                    .map_err(|_| initialize())?;
            }
            if let Some(size) = options.socket_receive_buffer {
                socket
                    .set_recv_buffer_size(size)
                    .map_err(|_| initialize())?;
            }
            let port = listener.local_addr().map_err(|_| initialize())?.port();
            let (published, loopback_ports) = if is_tls {
                (&mut published_tls, &mut loopback_tls_ports)
            } else {
                (&mut published, &mut loopback_ports)
            };
            if ep.host.is_empty() || is_loopback(&ep.host) {
                loopback_ports.push(port);
            }
            // The publish address applies to the first endpoint of its kind.
            let publish = publish.filter(|_| std::mem::take(&mut first[usize::from(is_tls)]));
            let host = match publish {
                Some(p) if !p.host.is_empty() => p.host.clone(),
                _ if !ep.host.is_empty() => ep.host.clone(),
                _ => host_name(),
            };
            let port = match publish {
                Some(p) if p.port != 0 => p.port,
                _ => port,
            };
            trace::info!(
                "listening on {}{}, published as {host}:{port}",
                listener
                    .local_addr()
                    .map_or_else(|_| format!("{bind_host}:{}", ep.port), |a| a.to_string()),
                if is_tls { " (TLS)" } else { "" }
            );
            listeners.push((listener, is_tls));
            published.push((host, port));
        }
        let server = Arc::new(Server {
            orb: Arc::downgrade(&orb.inner),
            boot_id: new_boot_id(),
            published,
            loopback_ports,
            published_tls,
            loopback_tls_ports,
            client_auth_required: options.tls.client_auth == crate::ClientAuth::Required,
            fixed_port: endpoints
                .iter()
                .chain(&options.tls_endpoints)
                .any(|ep| ep.port != 0),
            order: options.byte_order,
            max_message_size: options.max_message_size,
            fragment_size: options.fragment_size,
            max_requests: options.max_requests_per_connection.max(1),
            follow_linger: options.follow_linger,
            follow_limit: if options.follow_linger.is_zero() {
                0
            } else {
                options.follow_limit
            },
            idle_timeout: options.server_idle_timeout,
            root: OnceLock::new(),
            simple_keys: Mutex::new(HashMap::new()),
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
        let limit = (options.max_server_connections > 0)
            .then(|| Arc::new(Semaphore::new(options.max_server_connections)));
        for (listener, is_tls) in listeners {
            let listener = TcpListener::from_std(listener).map_err(|_| initialize())?;
            let tls = tls_config.clone().filter(|_| is_tls);
            tasks.push(handle.spawn(accept_loop(
                Arc::downgrade(&server),
                listener,
                tls,
                limit.clone(),
            )));
        }
        if let Some(timeout) = server.idle_timeout {
            tasks.push(handle.spawn(close_idle(Arc::downgrade(&server), timeout)));
        }
        *server.listeners.lock().unwrap() = tasks;
        Ok(server)
    }

    pub(crate) fn root(&self) -> &Arc<PoaInner> {
        self.root
            .get()
            .expect("the RootPOA is created with the server")
    }

    /// Makes requests with the object key `simple` go to the object with
    /// the POA key `target` (`None` removes the entry).
    pub(crate) fn set_simple_key(&self, simple: &[u8], target: Option<Vec<u8>>) {
        let mut keys = self.simple_keys.lock().unwrap();
        match target {
            Some(target) => keys.insert(simple.to_vec(), target),
            None => keys.remove(simple),
        };
    }

    /// Whether some endpoint listens on a port given in the options.
    pub(crate) fn has_fixed_port(&self) -> bool {
        self.fixed_port
    }

    /// Host and port of each endpoint, as published in IORs.
    pub(crate) fn published(&self) -> &[(String, u16)] {
        &self.published
    }

    /// Host and port of each TLS endpoint, as published in IORs.
    pub(crate) fn published_tls(&self) -> &[(String, u16)] {
        &self.published_tls
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
    /// code sets, the first TLS endpoint (`TAG_SSL_SEC_TRANS`) and the
    /// other endpoints as alternate addresses. With TLS endpoints only,
    /// the profile has the host of the first one and port 0. A TLS
    /// endpoint the first profile cannot carry (another host, or not the
    /// first) gets a profile of its own.
    pub(crate) fn ior(&self, type_id: &str, key: Vec<u8>) -> Result<Ior, SystemException> {
        let marshal =
            |_| SystemException::new(SystemExceptionKind::MARSHAL, 0, CompletionStatus::No);
        let (host, port) = match self.published.first() {
            Some((host, port)) => (host.clone(), *port),
            None => (self.published_tls[0].0.clone(), 0),
        };
        let mut components = vec![code_set_info().to_component(self.order).map_err(marshal)?];
        let mut tls = self.published_tls.iter();
        let mut rest_tls = Vec::new();
        if let Some((tls_host, tls_port)) = tls.next() {
            if tls_host.eq_ignore_ascii_case(&host) {
                components.push(self.ssl_component(*tls_port).map_err(marshal)?);
            } else {
                rest_tls.push((tls_host, *tls_port));
            }
        }
        rest_tls.extend(tls.map(|(h, p)| (h, *p)));
        for (host, port) in self.published.iter().skip(1) {
            let alternate = AlternateIiopAddress {
                host: host.clone(),
                port: *port,
            };
            components.push(alternate.to_component(self.order).map_err(marshal)?);
        }
        let mut profiles = vec![
            IiopProfile {
                version: Version::V1_2,
                host,
                port,
                object_key: key.clone(),
                components,
            }
            .to_tagged_profile(self.order)
            .map_err(marshal)?,
        ];
        for (host, port) in rest_tls {
            let components = vec![
                code_set_info().to_component(self.order).map_err(marshal)?,
                self.ssl_component(port).map_err(marshal)?,
            ];
            let profile = IiopProfile {
                version: Version::V1_2,
                host: host.clone(),
                port: 0,
                object_key: key.clone(),
                components,
            };
            profiles.push(profile.to_tagged_profile(self.order).map_err(marshal)?);
        }
        Ok(Ior {
            type_id: type_id.to_owned(),
            profiles,
        })
    }

    /// The `TAG_SSL_SEC_TRANS` component of the TLS endpoint at `port`:
    /// protection, and trust in the target and (optionally) the client,
    /// the same options omniORB publishes; unprotected IIOP is supported
    /// too when there are plain endpoints.
    fn ssl_component(&self, port: u16) -> rustorb_giop::Result<rustorb_giop::TaggedComponent> {
        use association_options::*;
        let mut requires = INTEGRITY | CONFIDENTIALITY | ESTABLISH_TRUST_IN_TARGET;
        let mut supports = requires | ESTABLISH_TRUST_IN_CLIENT;
        if !self.published.is_empty() {
            supports |= NO_PROTECTION;
        }
        if self.client_auth_required {
            requires |= ESTABLISH_TRUST_IN_CLIENT;
        }
        SslSecTrans {
            target_supports: supports,
            target_requires: requires,
            port,
        }
        .to_component(self.order)
    }

    fn orb(&self) -> Option<Orb> {
        self.orb.upgrade().map(|inner| Orb { inner })
    }

    fn is_shutting_down(&self) -> bool {
        self.shutting_down.load(Ordering::Acquire)
    }

    /// The POA and ObjectId a key denotes. Without `orb`, a POA that only
    /// an adapter activator could create is [`Lookup::NeedsActivator`];
    /// with it, the activators are called (user code).
    fn find(&self, key: &[u8], orb: Option<&Orb>) -> Result<(Arc<PoaInner>, Vec<u8>), Lookup> {
        let alias;
        let key = match parse_key(key) {
            ParsedKey::Simple(simple) => {
                alias = self
                    .simple_keys
                    .lock()
                    .unwrap()
                    .get(simple)
                    .cloned()
                    .ok_or(Lookup::Missing)?;
                &alias[..]
            }
            _ => key,
        };
        let ParsedKey::Poa { path, boot_id, id } = parse_key(key) else {
            return Err(Lookup::Missing);
        };
        let mut poa = Arc::clone(self.root());
        for name in path {
            poa = match orb {
                Some(orb) => poa
                    .find_or_activate_child(orb, name)
                    .ok_or(Lookup::Missing)?,
                None => poa.child(name).map_err(|activator| {
                    if activator {
                        Lookup::NeedsActivator
                    } else {
                        Lookup::Missing
                    }
                })?,
            };
        }
        let lifespan_matches = match (poa.policies().lifespan, boot_id) {
            (Lifespan::Persistent, None) => true,
            (Lifespan::Transient, Some(boot_id)) => boot_id == self.boot_id,
            _ => false,
        };
        if !lifespan_matches || poa.is_destroyed() {
            return Err(Lookup::Missing);
        }
        Ok((poa, id.to_vec()))
    }

    fn ticket(self: &Arc<Self>, permit: Option<OwnedSemaphorePermit>) -> Ticket {
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
        let Ok(permit) = Arc::clone(&conn.permits).acquire_owned().await else {
            return;
        };
        if let Some((key, request)) = self.prepare(conn, version, header, body, permit) {
            self.route(key, request);
        }
    }

    /// Makes a request read off `conn` ready to go to its POA. `None` when
    /// it has been answered already, or is dropped because the server is
    /// shutting down.
    fn prepare(
        self: &Arc<Self>,
        conn: &Arc<ServerConn>,
        version: Version,
        header: RequestHeader,
        body: Body,
        permit: OwnedSemaphorePermit,
    ) -> Option<(Vec<u8>, Incoming)> {
        let code_sets = conn.code_sets_for(version, &header.service_context);
        conn.saw_version(version);
        trace::debug!(
            "request {:?} (id {}{}) from {}, GIOP {}.{}",
            header.operation,
            header.request_id,
            if header.response_expected() {
                ""
            } else {
                ", oneway"
            },
            conn.inbound.peer(),
            version.major,
            version.minor
        );
        let ticket = self.ticket(Some(permit));
        if self.is_shutting_down() {
            // Checked again under the ticket: a shutdown that began after
            // the check in `on_request` may have destroyed the POAs, and
            // one that begins from here on waits for this request.
            return None;
        }
        if !conn.begin(header.request_id, header.response_expected()) {
            trace::debug!(
                "request {:?} (id {}) not processed: CloseConnection sent",
                header.operation,
                header.request_id
            );
            return None;
        }
        let reply = ReplyTo::new(
            conn,
            version,
            self.order,
            header.request_id,
            header.response_expected(),
        );
        let Some(key) = into_target_key(header.target) else {
            reply.system(&no_such_object());
            return None;
        };
        let request = Incoming {
            operation: header.operation,
            body,
            code_sets,
            reply,
            _ticket: ticket,
        };
        Some((key, request))
    }

    /// Finds the POA of a request, on an I/O task, and hands the request
    /// to it.
    fn route(self: &Arc<Self>, key: Vec<u8>, request: Incoming) {
        match self.find(&key, None) {
            Ok((poa, id)) => self.admit(&poa, id, request),
            Err(Lookup::Missing) => request.reply.system(&no_such_object()),
            Err(Lookup::NeedsActivator) => {
                // Adapter activators are user code: find the POA on an
                // upcall thread.
                let server = Arc::clone(self);
                self.submit(Box::new(move || {
                    let found = server
                        .orb()
                        .and_then(|orb| server.find(&key, Some(&orb)).ok());
                    match found {
                        Some((poa, id)) => server.admit(&poa, id, request),
                        None => request.reply.system(&no_such_object()),
                    }
                }));
            }
        }
    }

    /// Hands a request to the manager of its POA, which runs, holds or
    /// refuses it.
    fn admit(self: &Arc<Self>, poa: &Arc<PoaInner>, id: Vec<u8>, request: Incoming) {
        let server = Arc::clone(self);
        let target = Arc::clone(poa);
        let dispatch: Dispatch = Box::new(move |disposition| match disposition {
            Disposition::Run => server.schedule_upcall(&target, id, request),
            Disposition::Reject(e) => request.reply.system(&e),
        });
        poa.manager.inner.dispatch(dispatch);
    }

    /// Runs a request on the thread the thread policy of `poa` asks for.
    /// An upcall thread that answered a request of a connection goes on to
    /// read the connection's next request ([`follow`](Self::follow)).
    fn schedule_upcall(self: &Arc<Self>, poa: &Arc<PoaInner>, id: Vec<u8>, request: Incoming) {
        if !matches!(poa.policies().thread, ThreadPolicy::OrbCtrl) {
            let job = self.upcall_job(poa, id, request);
            self.schedule(poa, job);
            return;
        }
        let server = Arc::clone(self);
        let poa = Arc::clone(poa);
        let conn = request.reply.conn();
        self.submit(Box::new(move || {
            server.upcall(&poa, &id, request);
            if let Some(conn) = conn {
                server.follow(&conn);
            }
        }));
    }

    /// Leader / follower: an upcall thread that answered a request of
    /// `conn` reads the connection's next request itself and runs it, so
    /// that the request needs no hand-over from the I/O task to the pool.
    /// The thread waits [`crate::OrbOptions::follow_linger`] at most for a
    /// request, and stops when the pool has queued work (which other
    /// connections wait for), after [`crate::OrbOptions::follow_limit`] requests,
    /// or when the I/O task reads the connection. See
    /// docs/read-ownership.md.
    fn follow(self: &Arc<Self>, conn: &Arc<ServerConn>) {
        for _ in 0..self.follow_limit {
            if self.is_shutting_down() || self.pool_has_queued() {
                break;
            }
            let Ok(permit) = Arc::clone(&conn.permits).try_acquire_owned() else {
                break;
            };
            if !conn.inbound.claim() {
                return;
            }
            let (version, message) = match conn
                .inbound
                .read_here(Some(Instant::now() + self.follow_linger))
            {
                Here::Message(version, message) => (version, message),
                Here::TimedOut | Here::HandedBack => return,
            };
            conn.last_used.touch();
            let keep = match message {
                Message::Request { header, body } => {
                    let keep = conn.inbound.busy();
                    if let Some((key, request)) = self.prepare(conn, version, header, body, permit)
                    {
                        self.route_here(key, request);
                    }
                    keep
                }
                Message::LocateRequest(header) => {
                    let keep = conn.inbound.busy();
                    self.on_locate(conn, version, &header);
                    keep
                }
                Message::CancelRequest(header) => {
                    let keep = conn.inbound.busy();
                    conn.cancel_request(header.request_id);
                    keep
                }
                other => {
                    conn.inbound.give_back(version, other);
                    return;
                }
            };
            if !keep {
                return;
            }
        }
        conn.inbound.unbusy();
    }

    /// Like [`route`](Self::route), on an upcall thread: runs the request
    /// on this thread when its POA is ORB_CTRL_MODEL and its manager lets
    /// it run at once.
    fn route_here(self: &Arc<Self>, key: Vec<u8>, request: Incoming) {
        let found = match self.find(&key, None) {
            Err(Lookup::NeedsActivator) => match self.orb() {
                Some(orb) => self.find(&key, Some(&orb)),
                None => Err(Lookup::Missing),
            },
            found => found,
        };
        let Ok((poa, id)) = found else {
            request.reply.system(&no_such_object());
            return;
        };
        if !matches!(poa.policies().thread, ThreadPolicy::OrbCtrl) {
            self.admit(&poa, id, request);
            return;
        }
        // The verdict comes at once, or later from another thread (a
        // holding manager): then the request is scheduled as usual.
        let here = Arc::new(Mutex::new(RunHere {
            waiting: true,
            request: None,
        }));
        let slot = Arc::clone(&here);
        let server = Arc::clone(self);
        let target = Arc::clone(&poa);
        poa.manager
            .inner
            .dispatch(Box::new(move |disposition| match disposition {
                Disposition::Run => {
                    let mut slot = slot.lock().unwrap();
                    if slot.waiting {
                        slot.request = Some((id, request));
                    } else {
                        drop(slot);
                        server.schedule_upcall(&target, id, request);
                    }
                }
                Disposition::Reject(e) => request.reply.system(&e),
            }));
        let run = {
            let mut here = here.lock().unwrap();
            here.waiting = false;
            here.request.take()
        };
        if let Some((id, request)) = run {
            self.upcall(&poa, &id, request);
        }
    }

    fn pool_has_queued(&self) -> bool {
        self.pool
            .lock()
            .unwrap()
            .as_ref()
            .is_some_and(Pool::has_queued)
    }

    fn upcall_job(self: &Arc<Self>, poa: &Arc<PoaInner>, id: Vec<u8>, request: Incoming) -> Job {
        let server = Arc::clone(self);
        let poa = Arc::clone(poa);
        Box::new(move || server.upcall(&poa, &id, request))
    }

    /// Runs `job` on the thread the thread policy of `poa` asks for.
    fn schedule(self: &Arc<Self>, poa: &Arc<PoaInner>, job: Job) {
        match poa.policies().thread {
            ThreadPolicy::OrbCtrl => self.submit(job),
            ThreadPolicy::SingleThread => poa.run_serially(self, job),
            ThreadPolicy::MainThread => {
                if let Some(orb) = self.orb.upgrade() {
                    orb.push_main(job);
                }
            }
        }
    }

    /// Finds the servant and calls it, on the thread the request runs on.
    fn upcall(self: &Arc<Self>, poa: &Arc<PoaInner>, id: &[u8], request: Incoming) {
        let Incoming {
            operation,
            body,
            code_sets,
            reply,
            _ticket,
        } = request;
        if reply.is_cancelled() {
            return;
        }
        let Some(orb) = self.orb() else { return };
        let adapter = Poa {
            orb: orb.clone(),
            inner: Arc::clone(poa),
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
        let resolved = match poa.resolve(&adapter, self, id, &operation) {
            Ok(resolved) => resolved,
            Err(ServantManagerError::ForwardRequest(to)) => {
                reply.forward(to.ior());
                return;
            }
            Err(ServantManagerError::System(e)) => {
                reply.system(&e);
                return;
            }
        };
        let mut input = InStream {
            dec: body.decoder(),
            code_sets,
            orb: Some(&orb),
            completed: CompletionStatus::No,
            wchar_needs_code_set: true,
            values: Vec::new(),
            typed_values: Vec::new(),
            custom_items: None,
            body: Some(&body),
        };
        let result = {
            let _upcall = UpcallGuard::enter();
            let _current = CurrentGuard::enter(&adapter, id, &resolved.servant);
            panic::catch_unwind(AssertUnwindSafe(|| {
                invoke(&*resolved.servant, &operation, &mut input, &mut out)
            }))
        };
        resolved.finish(&adapter, id, &operation);
        match result {
            Ok(Ok(())) => match out.finish() {
                Ok(msg) => {
                    trace::debug!("request {operation:?} (id {}) done", reply.request_id);
                    reply.send(msg)
                }
                Err(e) => reply.system(&e),
            },
            Ok(Err(e)) => reply.system(&e),
            Err(_) => {
                trace::warning!("servant panicked in {operation:?}");
                reply.system(&SystemException::new(
                    SystemExceptionKind::UNKNOWN,
                    0,
                    CompletionStatus::Maybe,
                ));
            }
        }
    }

    fn on_locate(
        self: &Arc<Self>,
        conn: &Arc<ServerConn>,
        version: Version,
        header: &LocateRequestHeader,
    ) {
        if self.is_shutting_down() {
            return;
        }
        conn.saw_version(version);
        trace::debug!(
            "LocateRequest (id {}) from {}",
            header.request_id,
            conn.inbound.peer()
        );
        let order = self.order;
        let request_id = header.request_id;
        let conn = Arc::clone(conn);
        let answer = move |here: bool| {
            let status = if here {
                LocateStatus::ObjectHere
            } else {
                LocateStatus::UnknownObject
            };
            let reply = LocateReplyHeader {
                request_id,
                locate_status: status,
            };
            if let Ok(msg) = MessageEncoder::locate_reply(version, order, &reply)
                .and_then(MessageEncoder::finish)
            {
                conn.send(msg.into());
            }
        };
        let Some(key) = target_key(&header.target) else {
            answer(false);
            return;
        };
        match self.find(&key, None) {
            Ok((poa, id)) => answer(located(&poa, &id)),
            Err(Lookup::Missing) => answer(false),
            Err(Lookup::NeedsActivator) => {
                let server = Arc::clone(self);
                self.submit(Box::new(move || {
                    let found = server
                        .orb()
                        .and_then(|orb| server.find(&key, Some(&orb)).ok());
                    answer(found.is_some_and(|(poa, id)| located(&poa, &id)));
                }));
            }
        }
    }

    // --- calls from this ORB ---------------------------------------------------

    /// Whether `host:port` is an endpoint of this server (a TLS one if
    /// `tls`), so that a call to it can be made without the network.
    pub(crate) fn is_local(&self, host: &str, port: u16, tls: bool) -> bool {
        if self.is_shutting_down() {
            return false;
        }
        let (published, loopback_ports) = if tls {
            (&self.published_tls, &self.loopback_tls_ports)
        } else {
            (&self.published, &self.loopback_ports)
        };
        published
            .iter()
            .any(|(h, p)| *p == port && h.eq_ignore_ascii_case(host))
            || (is_loopback(host) && loopback_ports.contains(&port))
    }

    /// Whether `profile` names an endpoint of this server: its own address
    /// or its TLS endpoint.
    pub(crate) fn is_own_profile(&self, profile: &IiopProfile) -> bool {
        self.is_local(&profile.host, profile.port, false)
            || profile
                .components
                .iter()
                .filter(|c| c.tag == component_tag::SSL_SEC_TRANS)
                .filter_map(|c| SslSecTrans::from_component(c).ok())
                .any(|ssl| self.is_local(&profile.host, ssl.port, true))
    }

    /// Runs a request of this ORB to one of its own objects, on the calling
    /// thread, and returns the whole reply message (`None` for a oneway
    /// request). The request goes through the POA manager like a remote
    /// one. It is queued and waited for only when the thread policy asks
    /// for another thread: MAIN_THREAD_MODEL called from a thread that is
    /// not running the main loop, or SINGLE_THREAD_MODEL while another
    /// thread runs a request of the POA.
    pub(crate) fn call_local(
        self: &Arc<Self>,
        version: Version,
        header: RequestHeader,
        body: Body,
        code_sets: CodeSets,
        timeout: Option<Duration>,
    ) -> Result<Option<Vec<u8>>, SystemException> {
        if self.is_shutting_down() {
            return Err(transient());
        }
        let Some(orb) = self.orb() else {
            return Err(transient());
        };
        let response = header.response_expected();
        let (tx, rx) = std_mpsc::channel();
        let reply = ReplyTo::local(tx, version, self.order, header.request_id, response);
        let request = Incoming {
            operation: header.operation,
            body,
            code_sets,
            reply,
            _ticket: self.ticket(None),
        };
        let found = target_key(&header.target).and_then(|key| self.find(&key, Some(&orb)).ok());
        match found {
            None => request.reply.system(&no_such_object()),
            Some((poa, id)) => {
                // The manager's verdict comes at once, or when a holding
                // manager changes its state.
                let (dtx, drx) = std_mpsc::channel();
                poa.manager.inner.dispatch(Box::new(move |disposition| {
                    let _ = dtx.send(disposition);
                }));
                match wait(&drx, timeout, CompletionStatus::No)? {
                    Disposition::Reject(e) => request.reply.system(&e),
                    Disposition::Run => self.run_local(&poa, id, request),
                }
            }
        }
        if !response {
            return Ok(None);
        }
        wait(&rx, timeout, CompletionStatus::Maybe).map(Some)
    }

    fn run_local(self: &Arc<Self>, poa: &Arc<PoaInner>, id: Vec<u8>, request: Incoming) {
        match poa.policies().thread {
            ThreadPolicy::OrbCtrl => self.upcall(poa, &id, request),
            ThreadPolicy::SingleThread => match poa.enter_serially(self) {
                SerialEntry::Reentrant => self.upcall(poa, &id, request),
                SerialEntry::Acquired(_serial) => self.upcall(poa, &id, request),
                SerialEntry::Busy => {
                    let job = self.upcall_job(poa, id, request);
                    poa.run_serially(self, job);
                }
            },
            ThreadPolicy::MainThread if is_main_thread() => self.upcall(poa, &id, request),
            ThreadPolicy::MainThread => {
                let job = self.upcall_job(poa, id, request);
                self.schedule(poa, job);
            }
        }
    }

    /// Answers a `LocateRequest` of this ORB to one of its own objects.
    pub(crate) fn locate_local(&self, target: &TargetAddress) -> Result<bool, SystemException> {
        if self.is_shutting_down() {
            return Err(transient());
        }
        let Some(orb) = self.orb() else {
            return Err(transient());
        };
        Ok(target_key(target)
            .and_then(|key| self.find(&key, Some(&orb)).ok())
            .is_some_and(|(poa, id)| located(&poa, &id)))
    }

    // --- etherealization -------------------------------------------------------

    /// Etherealizes `work` once the requests running in its POAs have
    /// finished: before returning with `wait`, else on an upcall thread.
    pub(crate) fn etherealize(&self, work: Vec<Etherealization>, wait: bool) {
        if work.is_empty() {
            return;
        }
        let Some(orb) = self.orb() else { return };
        let run = move || {
            let mut waited: Vec<&Arc<PoaInner>> = Vec::new();
            for e in &work {
                if !waited.iter().any(|p| Arc::ptr_eq(p, e.poa())) {
                    e.poa().activity().wait_idle();
                    waited.push(e.poa());
                }
            }
            etherealize_all(&orb, work);
        };
        if wait {
            run();
        } else {
            self.submit(Box::new(run));
        }
    }

    /// Etherealizes one deactivated object on an upcall thread.
    pub(crate) fn etherealize_later(&self, work: Etherealization) {
        let Some(orb) = self.orb() else { return };
        self.submit(Box::new(move || work.run(&orb, None)));
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

    /// Sends `CloseConnection` on `conn`, chosen to close by
    /// [`ServerConn::choose_idle`], and stops reading it if the client
    /// has not closed its side after [`idle::CLOSE_LINGER`]. Requests the
    /// client sent meanwhile are not processed (oneways are: the client
    /// does not send them again), so the client sends them again on a new
    /// connection.
    fn close_conn(&self, conn: &Arc<ServerConn>) {
        let version = *conn.version.lock().unwrap();
        if let Ok(msg) = MessageEncoder::close_connection(version, self.order) {
            conn.close(msg);
        }
        let conn = Arc::downgrade(conn);
        tokio::spawn(async move {
            tokio::time::sleep(idle::CLOSE_LINGER).await;
            if let Some(conn) = conn.upgrade() {
                conn.inbound.shutdown();
            }
        });
    }

    /// Closes the connections idle for `timeout`.
    fn close_idle_connections(&self, timeout: Duration) {
        let idle: Vec<_> = self
            .connections
            .lock()
            .unwrap()
            .values()
            .filter(|c| c.choose_idle(Some(timeout)))
            .cloned()
            .collect();
        for conn in idle {
            trace::info!(
                "closing the connection from {}: idle for {timeout:?}",
                conn.inbound.peer()
            );
            self.close_conn(&conn);
        }
    }

    /// Closes the connection used least recently of those with no request
    /// running, to make room for a new one. Returns whether there was one.
    fn close_least_used(&self) -> bool {
        let candidate = self
            .connections
            .lock()
            .unwrap()
            .values()
            .filter(|c| c.may_close())
            .max_by_key(|c| c.last_used.idle_for())
            .cloned();
        match candidate {
            Some(conn) if conn.choose_idle(None) => {
                trace::info!(
                    "closing the connection from {}: at -ORBmaxServerConnections",
                    conn.inbound.peer()
                );
                self.close_conn(&conn);
                true
            }
            _ => false,
        }
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
        // MAIN_THREAD_MODEL requests not yet started are dropped like held
        // ones: nobody may be running the main loop to start them.
        if let Some(orb) = self.orb.upgrade() {
            orb.close_main();
        }
        let mut in_flight = self.in_flight.lock().unwrap();
        while *in_flight > 0 {
            in_flight = self.in_flight_done.wait(in_flight).unwrap();
        }
        drop(in_flight);
        let work = self.root().destroy_tree(true);
        if let Some(orb) = self.orb() {
            etherealize_all(&orb, work);
        }

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
                conn.close(msg);
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

/// At most this many pieces go into one vectored write (the usual
/// `IOV_MAX` is 1024).
const MAX_SLICES: usize = 64;

/// A request that [`Server::route_here`] runs on its own thread if the
/// manager lets it run before `dispatch` returns.
struct RunHere {
    waiting: bool,
    request: Option<(Vec<u8>, Incoming)>,
}

/// Why a key names no POA.
enum Lookup {
    /// There is no such POA, or the key is not one of this server's.
    Missing,
    /// A POA on the path is missing, but its parent has an adapter
    /// activator that may create it.
    NeedsActivator,
}

/// A request on its way to the servant.
struct Incoming {
    operation: String,
    body: Body,
    code_sets: CodeSets,
    reply: ReplyTo,
    _ticket: Ticket,
}

/// Whether a `LocateRequest` gets `OBJECT_HERE`. The servant manager is not
/// asked, and a holding or discarding manager does not matter (omniORB
/// answers the same).
fn located(poa: &PoaInner, id: &[u8]) -> bool {
    !poa.manager.inner.is_inactive() && poa.has_object(id)
}

fn transient() -> SystemException {
    SystemException::new(SystemExceptionKind::TRANSIENT, 0, CompletionStatus::No)
}

fn is_loopback(host: &str) -> bool {
    let host = host.trim_start_matches('[').trim_end_matches(']');
    host.eq_ignore_ascii_case("localhost")
        || host
            .parse::<std::net::IpAddr>()
            .is_ok_and(|a| a.is_loopback())
}

/// Waits for a colocated call: `TIMEOUT` after `timeout`, and `TRANSIENT`
/// if the request was dropped (at shutdown).
fn wait<T>(
    rx: &std_mpsc::Receiver<T>,
    timeout: Option<Duration>,
    completed: CompletionStatus,
) -> Result<T, SystemException> {
    match timeout {
        Some(limit) => rx.recv_timeout(limit).map_err(|e| match e {
            std_mpsc::RecvTimeoutError::Timeout => {
                SystemException::new(SystemExceptionKind::TIMEOUT, 0, completed)
            }
            std_mpsc::RecvTimeoutError::Disconnected => transient(),
        }),
        None => rx.recv().map_err(|_| transient()),
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

/// The object key of a request's target, which it takes.
fn into_target_key(target: TargetAddress) -> Option<Vec<u8>> {
    match target {
        TargetAddress::Key(key) => Some(key),
        other => target_key(&other),
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
    /// The connection's permit; none for a colocated call.
    _permit: Option<OwnedSemaphorePermit>,
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

/// The write side of a server connection.
///
/// A reply is written by the thread that made it, straight to the socket,
/// while nothing is queued before it and the socket takes all of it, so
/// that a reply needs no hand-over to the runtime. What the socket does not
/// take is queued for the writer task, and so is everything after it until
/// the queue is empty again. Writes happen under the lock or by the writer
/// task while it holds the stream, so messages never interleave.
struct Outbox {
    state: Mutex<OutState>,
    /// TLS on the stream: messages are encrypted, and the records go out
    /// in the order rustls made them.
    tls: Option<Arc<Tls>>,
    /// Wakes the writer task when something is queued or the connection
    /// closes.
    ready: Notify,
    /// The address of the client, for trace lines.
    peer: Option<SocketAddr>,
}

struct OutState {
    /// The stream; `None` while the writer task writes, and once closed.
    stream: Option<OwnedWriteHalf>,
    /// Messages for the writer task, each with how much of it was
    /// written already.
    queue: VecDeque<(OutMessage, usize)>,
    /// No more messages are taken: the writer task writes the queue,
    /// shuts the stream down and ends.
    closing: bool,
    /// Writing failed: everything is dropped.
    failed: bool,
    /// TLS records wait for the writer task (the socket did not take them).
    flush: bool,
}

impl Outbox {
    fn new(stream: OwnedWriteHalf, tls: Option<Arc<Tls>>, peer: Option<SocketAddr>) -> Outbox {
        Outbox {
            tls,
            peer,
            state: Mutex::new(OutState {
                stream: Some(stream),
                queue: VecDeque::new(),
                closing: false,
                failed: false,
                flush: false,
            }),
            ready: Notify::new(),
        }
    }

    /// Writes `msg`, or queues what the socket does not take now.
    fn send(&self, msg: OutMessage) {
        let mut state = self.state.lock().unwrap();
        if state.closing || state.failed {
            return;
        }
        trace::trace!("sent {} to {}", summary(&msg), PeerAddr(self.peer));
        if let Some(tls) = &self.tls {
            if let (true, Some(stream)) = (state.queue.is_empty(), &state.stream) {
                tls.encrypt(&msg, 0);
                if let Ok(true) = tls.flush(|bytes| stream.try_write(bytes)) {
                    return;
                }
                // Not writable now, or failed: the writer task waits for
                // the socket, or finds the error again.
                state.flush = true;
            } else {
                state.queue.push_back((msg, 0));
            }
            drop(state);
            self.ready.notify_one();
            return;
        }
        let mut written = 0;
        if let (true, Some(stream)) = (state.queue.is_empty(), &state.stream) {
            while written < msg.len() {
                let result = match msg.as_single() {
                    Some(bytes) => stream.try_write(&bytes[written..]),
                    None => stream.try_write_vectored(&msg.slices_from(written, MAX_SLICES)),
                };
                match result {
                    Ok(0) => break,
                    Ok(n) => written += n,
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                    // Not writable now, or failed: the writer task waits
                    // for the socket, or finds the error again.
                    Err(_) => break,
                }
            }
            if written == msg.len() {
                return;
            }
        }
        state.queue.push_back((msg, written));
        drop(state);
        self.ready.notify_one();
    }

    /// Queues a last message (none if empty) and has the writer task shut
    /// the stream down after it. Later messages are dropped.
    fn close(&self, last: Vec<u8>) {
        let mut state = self.state.lock().unwrap();
        if state.closing || state.failed {
            return;
        }
        if !last.is_empty() {
            state.queue.push_back((last.into(), 0));
        }
        state.closing = true;
        drop(state);
        self.ready.notify_one();
    }
}

/// One connection accepted by the server.
struct ServerConn {
    inbound: Inbound,
    out: Arc<Outbox>,
    /// The code sets of the first request with a code set context.
    code_sets: Mutex<Option<CodeSets>>,
    requests: Mutex<Requests>,
    /// The GIOP version of the last message received, used for
    /// `CloseConnection`.
    version: Mutex<Version>,
    permits: Arc<Semaphore>,
    writer: Mutex<Option<JoinHandle<()>>>,
    /// Largest reply sent (`OrbOptions::max_message_size`).
    max_message_size: usize,
    /// `OrbOptions::fragment_size`.
    fragment_size: usize,
    /// When a message last came in or a request ended.
    last_used: Stamp,
}

impl Drop for ServerConn {
    /// The requests still running hold the connection: once the last one
    /// replied, the stream is shut down.
    fn drop(&mut self) {
        self.out.close(Vec::new());
    }
}

#[derive(Default)]
struct Requests {
    pending: HashSet<u32>,
    cancelled: HashSet<u32>,
    /// `CloseConnection` is sent or about to be: no more requests that
    /// expect a reply are processed.
    closing: bool,
}

impl ServerConn {
    fn send(&self, msg: OutMessage) {
        self.out.send(msg);
    }

    /// Sends `last` (if not empty) after what is queued, then shuts the
    /// stream down.
    fn close(&self, last: Vec<u8>) {
        self.out.close(last);
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

    /// Admits a request, unless the connection is closing and the client
    /// waits for a reply (it sends the request again after
    /// `CloseConnection`). Returns whether it was admitted.
    fn begin(&self, request_id: u32, response: bool) -> bool {
        let mut requests = self.requests.lock().unwrap();
        if requests.closing && response {
            return false;
        }
        requests.pending.insert(request_id);
        true
    }

    /// Whether the connection could be closed now: no request running,
    /// none being read, and not closing already.
    fn may_close(&self) -> bool {
        let requests = self.requests.lock().unwrap();
        !requests.closing && requests.pending.is_empty() && !self.inbound.mid_message()
    }

    /// Chooses the connection to close if [`may_close`](Self::may_close)
    /// holds and it was not used for `timeout` (any time with `None`).
    /// From then on requests that expect a reply are not processed; the
    /// caller sends `CloseConnection`.
    fn choose_idle(&self, timeout: Option<Duration>) -> bool {
        let mut requests = self.requests.lock().unwrap();
        if requests.closing
            || !requests.pending.is_empty()
            || timeout.is_some_and(|t| self.last_used.idle_for() < t)
            || self.inbound.mid_message()
        {
            return false;
        }
        requests.closing = true;
        true
    }

    /// `CancelRequest`: drops the fragments of the request, and its reply.
    fn cancel_request(&self, request_id: u32) {
        self.inbound.side().assembler.cancel(request_id);
        self.cancel(request_id);
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
        self.last_used.touch();
        let mut requests = self.requests.lock().unwrap();
        requests.pending.remove(&request_id);
        requests.cancelled.remove(&request_id)
    }
}

/// Where the reply to one request goes. Dropping it without a reply
/// forgets the request.
struct ReplyTo {
    sink: Option<Sink>,
    version: Version,
    order: ByteOrder,
    request_id: u32,
    response: bool,
}

enum Sink {
    /// A server connection.
    Conn(Arc<ServerConn>),
    /// The thread waiting for a colocated call.
    Local(std_mpsc::Sender<Vec<u8>>),
}

impl ReplyTo {
    fn new(
        conn: &Arc<ServerConn>,
        version: Version,
        order: ByteOrder,
        request_id: u32,
        response: bool,
    ) -> ReplyTo {
        ReplyTo {
            sink: Some(Sink::Conn(Arc::clone(conn))),
            version,
            order,
            request_id,
            response,
        }
    }

    fn local(
        tx: std_mpsc::Sender<Vec<u8>>,
        version: Version,
        order: ByteOrder,
        request_id: u32,
        response: bool,
    ) -> ReplyTo {
        ReplyTo {
            sink: Some(Sink::Local(tx)),
            version,
            order,
            request_id,
            response,
        }
    }

    /// The connection the request came from.
    fn conn(&self) -> Option<Arc<ServerConn>> {
        match &self.sink {
            Some(Sink::Conn(c)) => Some(Arc::clone(c)),
            _ => None,
        }
    }

    fn is_cancelled(&self) -> bool {
        match &self.sink {
            Some(Sink::Conn(c)) => c.is_cancelled(self.request_id),
            _ => false,
        }
    }

    /// Queues the reply, unless the request was oneway or cancelled. A
    /// reply over the largest message is replaced by `MARSHAL`, completed
    /// yes.
    fn send(mut self, msg: OutMessage) {
        match self.sink.take() {
            Some(Sink::Conn(conn)) => {
                let cancelled = conn.end(self.request_id);
                if !self.response || cancelled {
                    return;
                }
                if msg.len() <= conn.max_message_size {
                    if conn.fragment_size > 0 {
                        conn.send(msg.fragment(conn.fragment_size));
                    } else {
                        conn.send(msg);
                    }
                    return;
                }
                trace::warning!(
                    "reply to request id {} of {} bytes is over the largest GIOP message \
                     ({} bytes, -ORBgiopMaxMsgSize): replying MARSHAL",
                    self.request_id,
                    msg.len(),
                    conn.max_message_size
                );
                let e =
                    SystemException::new(SystemExceptionKind::MARSHAL, 0, CompletionStatus::Yes);
                if let Some(msg) =
                    system_exception_reply(self.version, self.order, self.request_id, &e)
                {
                    conn.send(msg.into());
                }
            }
            Some(Sink::Local(tx)) if self.response => {
                let _ = tx.send(msg.into_vec());
            }
            Some(Sink::Local(_)) | None => {}
        }
    }

    /// Sends the client to `ior` with `LOCATION_FORWARD`.
    fn forward(self, ior: &Ior) {
        trace::debug!(
            "request id {}: replying LOCATION_FORWARD to {}",
            self.request_id,
            crate::trace::IorSummary(ior)
        );
        if !self.response {
            return;
        }
        match location_forward_reply(self.version, self.order, self.request_id, ior) {
            Some(msg) => self.send(msg.into()),
            None => drop(self),
        }
    }

    fn system(self, e: &SystemException) {
        trace::debug!("request id {}: replying {e}", self.request_id);
        if !self.response {
            return;
        }
        match system_exception_reply(self.version, self.order, self.request_id, e) {
            Some(msg) => self.send(msg.into()),
            None => drop(self),
        }
    }
}

impl Drop for ReplyTo {
    fn drop(&mut self) {
        if let Some(Sink::Conn(conn)) = self.sink.take() {
            conn.end(self.request_id);
        }
    }
}

/// How long a client has for the TLS handshake.
const HANDSHAKE_TIME: Duration = Duration::from_secs(30);

async fn accept_loop(
    server: Weak<Server>,
    listener: TcpListener,
    tls: Option<Arc<rustls::ServerConfig>>,
    limit: Option<Arc<Semaphore>>,
) {
    loop {
        // At the connection limit, new connections wait in the backlog.
        let permit = match &limit {
            Some(limit) => Some(connection_permit(&server, limit).await),
            None => None,
        };
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
        tokio::spawn(serve(server, stream, tls.clone(), permit));
    }
}

/// A place for one more connection under `-ORBmaxServerConnections`. When
/// all are taken, the connection used least recently with no request
/// running is closed, and so on each second until a place is free.
async fn connection_permit(server: &Weak<Server>, limit: &Arc<Semaphore>) -> OwnedSemaphorePermit {
    loop {
        if let Ok(permit) = Arc::clone(limit).try_acquire_owned() {
            return permit;
        }
        if let Some(server) = server.upgrade() {
            server.close_least_used();
        }
        let wait = Arc::clone(limit).acquire_owned();
        if let Ok(Ok(permit)) = tokio::time::timeout(Duration::from_secs(1), wait).await {
            return permit;
        }
    }
}

/// Closes the server connections idle for `timeout`, until the server is
/// gone or shut down (which aborts the task).
async fn close_idle(server: Weak<Server>, timeout: Duration) {
    let period = idle::scan_period(timeout);
    loop {
        tokio::time::sleep(period).await;
        let Some(server) = server.upgrade() else {
            return;
        };
        if server.is_shutting_down() {
            return;
        }
        server.close_idle_connections(timeout);
    }
}

/// Serves one connection. `_permit` is its place under
/// `-ORBmaxServerConnections`, given back when the client has closed.
async fn serve(
    server: Arc<Server>,
    stream: TcpStream,
    tls: Option<Arc<rustls::ServerConfig>>,
    _permit: Option<OwnedSemaphorePermit>,
) {
    let _ = stream.set_nodelay(true);
    let peer = PeerAddr(stream.peer_addr().ok());
    let session = match tls {
        Some(config) => {
            match tokio::time::timeout(HANDSHAKE_TIME, tls::accept(&stream, config)).await {
                Ok(Ok(session)) => Some(session),
                // A failed handshake (a certificate refused, say), or none.
                Ok(Err(e)) => {
                    trace::info!("TLS handshake with {peer} failed: {e}");
                    return;
                }
                Err(_) => {
                    trace::info!("TLS handshake with {peer} timed out");
                    return;
                }
            }
        }
        None => None,
    };
    trace::info!(
        "accepted a connection from {peer}{}",
        if session.is_some() { " with TLS" } else { "" }
    );
    let (reader, writer) = stream.into_split();
    let out = Arc::new(Outbox::new(writer, session.clone(), peer.0));
    let conn = Arc::new(ServerConn {
        inbound: Inbound::new(reader, session, server.max_message_size),
        out: Arc::clone(&out),
        code_sets: Mutex::new(None),
        requests: Mutex::default(),
        version: Mutex::new(Version::V1_0),
        permits: Arc::new(Semaphore::new(server.max_requests)),
        writer: Mutex::new(None),
        max_message_size: server.max_message_size,
        fragment_size: server.fragment_size,
        last_used: Stamp::new(),
    });
    *conn.writer.lock().unwrap() = Some(tokio::spawn(write_loop(out)));
    let id = server.add_connection(&conn);
    let ended = read_loop(&server, &conn).await;
    server.remove_connection(id);
    match ended {
        Ok(Ended::Peer) => trace::info!("connection from {peer} closed by the client"),
        Ok(Ended::CloseConnection) => {
            trace::info!("connection from {peer} closed (CloseConnection or MessageError)");
            conn.close(Vec::new());
        }
        Err(version) => {
            trace::info!("malformed message from {peer}: sending MessageError and closing");
            if let Ok(msg) = MessageEncoder::message_error(version, server.order) {
                conn.close(msg);
            }
        }
    }
    // The writer ends once the requests still running have queued their
    // replies and dropped the connection.
}

/// Writes what [`Outbox::send`] queued, and shuts the stream down once the
/// outbox is closed (by `close`, or when the connection is dropped).
async fn write_loop(out: Arc<Outbox>) {
    enum Step {
        Write(OwnedWriteHalf, VecDeque<(OutMessage, usize)>),
        Wait,
        Shutdown(Option<OwnedWriteHalf>),
        End,
    }
    loop {
        let step = {
            let mut state = out.state.lock().unwrap();
            if state.failed {
                Step::End
            } else if !state.queue.is_empty() || state.flush {
                state.flush = false;
                match state.stream.take() {
                    Some(stream) => Step::Write(stream, std::mem::take(&mut state.queue)),
                    None => Step::End,
                }
            } else if state.closing {
                Step::Shutdown(state.stream.take())
            } else {
                Step::Wait
            }
        };
        match step {
            Step::Write(stream, batch) if out.tls.is_some() => {
                let tls = out.tls.as_deref().unwrap();
                for (msg, written) in &batch {
                    tls.encrypt(msg, *written);
                }
                if flush_tls(tls, &stream).await.is_err() {
                    let mut state = out.state.lock().unwrap();
                    state.failed = true;
                    state.queue.clear();
                    return;
                }
                out.state.lock().unwrap().stream = Some(stream);
            }
            Step::Write(mut stream, mut batch) => {
                // Everything queued, in as few writes as the socket takes.
                while !batch.is_empty() {
                    let result = {
                        let mut slices = Vec::new();
                        for (msg, written) in &batch {
                            if slices.len() == MAX_SLICES {
                                break;
                            }
                            slices.extend(msg.slices_from(*written, MAX_SLICES - slices.len()));
                        }
                        stream.write_vectored(&slices).await
                    };
                    match result {
                        Ok(n) if n > 0 => advance(&mut batch, n),
                        Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                        _ => {
                            let mut state = out.state.lock().unwrap();
                            state.failed = true;
                            state.queue.clear();
                            return;
                        }
                    }
                }
                out.state.lock().unwrap().stream = Some(stream);
            }
            Step::Wait => out.ready.notified().await,
            Step::Shutdown(stream) => {
                if let Some(mut stream) = stream {
                    if let Some(tls) = &out.tls {
                        tls.close_notify();
                        let _ = flush_tls(tls, &stream).await;
                    }
                    let _ = stream.shutdown().await;
                }
                return;
            }
            Step::End => return,
        }
    }
}

/// Writes the TLS records waiting, waiting for the socket as needed.
async fn flush_tls(tls: &Tls, stream: &OwnedWriteHalf) -> io::Result<()> {
    while !tls.flush(|bytes| stream.try_write(bytes))? {
        stream.writable().await?;
    }
    Ok(())
}

/// Drops the first `n` bytes of the queued messages.
fn advance(batch: &mut VecDeque<(OutMessage, usize)>, mut n: usize) {
    while n > 0 {
        let Some((msg, written)) = batch.front_mut() else {
            return;
        };
        let left = msg.len() - *written;
        if n < left {
            *written += n;
            return;
        }
        n -= left;
        batch.pop_front();
    }
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
async fn read_loop(server: &Arc<Server>, conn: &Arc<ServerConn>) -> Result<Ended, Version> {
    loop {
        let (version, message) = match conn.inbound.next().await {
            Ok(read) => read,
            Err(ReadError::Closed(_) | ReadError::Truncated(_)) => return Ok(Ended::Peer),
            Err(ReadError::BadHeader(_)) => return Err(Version::V1_0),
            Err(ReadError::TooLarge(_, version) | ReadError::Malformed(version, _)) => {
                return Err(version);
            }
        };
        conn.last_used.touch();
        match message {
            Message::Request { header, body } => {
                server.on_request(conn, version, header, body).await;
            }
            Message::LocateRequest(header) => server.on_locate(conn, version, &header),
            Message::CancelRequest(header) => conn.cancel_request(header.request_id),
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

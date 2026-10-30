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
//! Calls from the ORB to its own objects do not go through the network:
//! the request is marshalled as usual and dispatched on the calling thread
//! ([`Server::call_local`]).

use std::collections::hash_map::RandomState;
use std::collections::{HashMap, HashSet, VecDeque};
use std::hash::{BuildHasher, Hasher};
use std::io;
use std::panic::{self, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock, Weak, mpsc as std_mpsc};
use std::time::{Duration, SystemTime};

use rustorb_cdr::ByteOrder;
use rustorb_giop::{
    AlternateIiopAddress, Body, CodeSetComponent, CodeSetComponentInfo, CodeSetContext,
    FragmentAssembler, IiopProfile, Ior, LocateReplyHeader, LocateRequestHeader, LocateStatus,
    Message, MessageEncoder, RequestHeader, ServiceContext, TargetAddress, Version,
};
use tokio::io::{AsyncWriteExt, BufReader};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::{TcpListener, TcpStream};
use tokio::runtime::Handle;
use tokio::sync::{Notify, OwnedSemaphorePermit, Semaphore};
use tokio::task::JoinHandle;

use crate::codeset::{CONVERSION_CHAR, NATIVE_CHAR, NATIVE_WCHAR};
use crate::orb::OrbInner;
use crate::poa::{
    BOOT_ID_LEN, CurrentGuard, Dispatch, Disposition, Etherealization, ManagerInner, ParsedKey,
    PoaInner, PoaManager, SerialEntry, UpcallGuard, etherealize_all, is_main_thread, log,
    no_such_object, parse_key,
};
use crate::pool::{Job, Pool};
use crate::servant::{OBJECT_ID, ServerReply, location_forward_reply, system_exception_reply};
use crate::stream::InStream;
use crate::wire::{self, ReadError};
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
    fixed_port: bool,
    order: ByteOrder,
    max_message_size: usize,
    max_requests: usize,
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
        let mut loopback_ports = Vec::new();
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
            if ep.host.is_empty() || is_loopback(&ep.host) {
                loopback_ports.push(port);
            }
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
            loopback_ports,
            fixed_port: endpoints.iter().any(|ep| ep.port != 0),
            order: options.byte_order,
            max_message_size: options.max_message_size,
            max_requests: options.max_requests_per_connection.max(1),
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
        let code_sets = conn.code_sets_for(version, &header.service_context);
        conn.saw_version(version);
        let Ok(permit) = Arc::clone(&conn.permits).acquire_owned().await else {
            return;
        };
        let ticket = self.ticket(Some(permit));
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
        let request = Incoming {
            operation: header.operation,
            body,
            code_sets,
            reply,
            _ticket: ticket,
        };
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
            Disposition::Run => {
                let job = server.upcall_job(&target, id, request);
                server.schedule(&target, job);
            }
            Disposition::Reject(e) => request.reply.system(&e),
        });
        poa.manager.inner.dispatch(dispatch);
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
                conn.send(msg);
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

    /// Whether `host:port` is an endpoint of this server, so that a call to
    /// it can be made without the network.
    pub(crate) fn is_local(&self, host: &str, port: u16) -> bool {
        if self.is_shutting_down() {
            return false;
        }
        self.published
            .iter()
            .any(|(h, p)| *p == port && h.eq_ignore_ascii_case(host))
            || (is_loopback(host) && self.loopback_ports.contains(&port))
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
    /// Wakes the writer task when something is queued or the connection
    /// closes.
    ready: Notify,
}

struct OutState {
    /// The stream; `None` while the writer task writes, and once closed.
    stream: Option<OwnedWriteHalf>,
    /// Messages for the writer task, each with how much of it was
    /// written already.
    queue: VecDeque<(Vec<u8>, usize)>,
    /// No more messages are taken: the writer task writes the queue,
    /// shuts the stream down and ends.
    closing: bool,
    /// Writing failed: everything is dropped.
    failed: bool,
}

impl Outbox {
    fn new(stream: OwnedWriteHalf) -> Outbox {
        Outbox {
            state: Mutex::new(OutState {
                stream: Some(stream),
                queue: VecDeque::new(),
                closing: false,
                failed: false,
            }),
            ready: Notify::new(),
        }
    }

    /// Writes `msg`, or queues what the socket does not take now.
    fn send(&self, msg: Vec<u8>) {
        let mut state = self.state.lock().unwrap();
        if state.closing || state.failed {
            return;
        }
        let mut written = 0;
        if let (true, Some(stream)) = (state.queue.is_empty(), &state.stream) {
            while written < msg.len() {
                match stream.try_write(&msg[written..]) {
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
            state.queue.push_back((last, 0));
        }
        state.closing = true;
        drop(state);
        self.ready.notify_one();
    }
}

/// One connection accepted by the server.
struct ServerConn {
    out: Arc<Outbox>,
    /// The code sets of the first request with a code set context.
    code_sets: Mutex<Option<CodeSets>>,
    requests: Mutex<Requests>,
    /// The GIOP version of the last message received, used for
    /// `CloseConnection`.
    version: Mutex<Version>,
    permits: Arc<Semaphore>,
    writer: Mutex<Option<JoinHandle<()>>>,
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
}

impl ServerConn {
    fn send(&self, msg: Vec<u8>) {
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
        conn.begin(request_id);
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

    fn is_cancelled(&self) -> bool {
        match &self.sink {
            Some(Sink::Conn(c)) => c.is_cancelled(self.request_id),
            _ => false,
        }
    }

    /// Queues the reply, unless the request was oneway or cancelled.
    fn send(mut self, msg: Vec<u8>) {
        match self.sink.take() {
            Some(Sink::Conn(conn)) => {
                let cancelled = conn.end(self.request_id);
                if self.response && !cancelled {
                    conn.send(msg);
                }
            }
            Some(Sink::Local(tx)) if self.response => {
                let _ = tx.send(msg);
            }
            Some(Sink::Local(_)) | None => {}
        }
    }

    /// Sends the client to `ior` with `LOCATION_FORWARD`.
    fn forward(self, ior: &Ior) {
        if !self.response {
            return;
        }
        match location_forward_reply(self.version, self.order, self.request_id, ior) {
            Some(msg) => self.send(msg),
            None => drop(self),
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
        if let Some(Sink::Conn(conn)) = self.sink.take() {
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
    let (reader, writer) = stream.into_split();
    let mut reader = wire::buffered(reader);
    let out = Arc::new(Outbox::new(writer));
    let conn = Arc::new(ServerConn {
        out: Arc::clone(&out),
        code_sets: Mutex::new(None),
        requests: Mutex::default(),
        version: Mutex::new(Version::V1_0),
        permits: Arc::new(Semaphore::new(server.max_requests)),
        writer: Mutex::new(None),
    });
    *conn.writer.lock().unwrap() = Some(tokio::spawn(write_loop(out)));
    let id = server.add_connection(&conn);
    let ended = read_loop(&server, &conn, &mut reader).await;
    server.remove_connection(id);
    match ended {
        Ok(Ended::Peer) => {}
        Ok(Ended::CloseConnection) => conn.close(Vec::new()),
        Err(version) => {
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
        Write(OwnedWriteHalf, VecDeque<(Vec<u8>, usize)>),
        Wait,
        Shutdown(Option<OwnedWriteHalf>),
        End,
    }
    loop {
        let step = {
            let mut state = out.state.lock().unwrap();
            if state.failed {
                Step::End
            } else if !state.queue.is_empty() {
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
            Step::Write(mut stream, batch) => {
                for (msg, written) in batch {
                    if stream.write_all(&msg[written..]).await.is_err() {
                        let mut state = out.state.lock().unwrap();
                        state.failed = true;
                        state.queue.clear();
                        return;
                    }
                }
                out.state.lock().unwrap().stream = Some(stream);
            }
            Step::Wait => out.ready.notified().await,
            Step::Shutdown(stream) => {
                if let Some(mut stream) = stream {
                    let _ = stream.shutdown().await;
                }
                return;
            }
            Step::End => return,
        }
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
async fn read_loop(
    server: &Arc<Server>,
    conn: &Arc<ServerConn>,
    reader: &mut BufReader<OwnedReadHalf>,
) -> Result<Ended, Version> {
    let max = server.max_message_size;
    let mut assembler = FragmentAssembler::new(max);
    loop {
        let (header, msg) = match wire::read_message(reader, max).await {
            Ok(read) => read,
            Err(ReadError::Closed(_) | ReadError::Truncated(_)) => return Ok(Ended::Peer),
            Err(ReadError::BadHeader(_)) => return Err(Version::V1_0),
            Err(ReadError::TooLarge(_, version)) => return Err(version),
        };
        let version = header.version;
        let Some(whole) = assembler.push_owned(msg).map_err(|_| version)? else {
            continue;
        };
        let (_, message) = whole.into_decoded().map_err(|_| version)?;
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

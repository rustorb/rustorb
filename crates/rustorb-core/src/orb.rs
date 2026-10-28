use std::collections::{HashMap, VecDeque};
use std::future::Future;
use std::panic::{self, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};
use std::{fmt, io};

use rustorb_cdr::ByteOrder;
use rustorb_giop::{CorbaName, Corbaloc, Ior, ObjectUrl, Version};
use tokio::runtime::{Handle, Runtime};

use crate::connection::Connection;
use crate::poa::{self, MainThreadGuard, Poa};
use crate::pool::Job;
use crate::server::Server;
use crate::{
    CompletionStatus, Exception, ObjectRef, SystemException, SystemExceptionKind, minor, name,
};

/// Settings of an [`Orb`].
#[derive(Debug, Clone)]
pub struct OrbOptions {
    /// The highest GIOP version used for requests. A request uses the
    /// lower of this and the version of the target's IIOP profile.
    pub max_giop_version: Version,
    /// Byte order of the messages rustorb sends.
    pub byte_order: ByteOrder,
    /// How long to wait for a TCP connection; `None` waits as long as the
    /// operating system does. Expiry raises `TRANSIENT`.
    pub connect_timeout: Option<Duration>,
    /// How long to wait for a reply; `None` waits forever. Expiry raises
    /// `TIMEOUT`, completed maybe.
    pub request_timeout: Option<Duration>,
    /// Largest message accepted from a server, fragments joined.
    pub max_message_size: usize,
    /// How many `LOCATION_FORWARD` replies one invocation follows before
    /// it gives up with `TRANSIENT`.
    pub max_forwards: u32,
    /// Initial references by name (`-ORBInitRef Name=URL`).
    pub init_refs: HashMap<String, String>,
    /// Base URL for initial references not in `init_refs`
    /// (`-ORBDefaultInitRef URL`); the name is appended after a `/`.
    pub default_init_ref: Option<String>,
    /// Number of tokio worker threads for network I/O.
    pub io_threads: usize,
    /// Addresses to listen on for requests (`-ORBendPoint giop:tcp:HOST:PORT`).
    /// Empty means one endpoint on every interface with a port the
    /// operating system chooses. The server side starts listening when the
    /// RootPOA is first used.
    pub endpoints: Vec<EndpointAddr>,
    /// Address published in IORs instead of the listening one
    /// (`-ORBendPointPublish giop:tcp:HOST:PORT`); a port of 0 keeps the
    /// listening port.
    pub endpoint_publish: Option<EndpointAddr>,
    /// Upcall threads kept running even when idle.
    pub min_server_threads: usize,
    /// Most upcall threads running at once (`-ORBmaxServerThreadPoolSize`).
    /// Requests beyond that wait in a queue.
    pub max_server_threads: usize,
    /// How long an upcall thread beyond `min_server_threads` waits for work
    /// before it ends.
    pub server_thread_idle_timeout: Duration,
    /// Requests of one connection that may wait or run at once. Beyond
    /// that the connection is not read until one completes.
    pub max_requests_per_connection: usize,
}

/// A TCP address of an endpoint, as in `giop:tcp:HOST:PORT`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EndpointAddr {
    /// Host name or address. Empty means every interface when listening,
    /// and the host name of the machine when published.
    pub host: String,
    /// TCP port; 0 lets the operating system choose.
    pub port: u16,
}

impl EndpointAddr {
    /// Parses `giop:tcp:HOST:PORT`; the host and the port may be empty, and
    /// an IPv6 address is written in brackets.
    pub fn parse(s: &str) -> Option<EndpointAddr> {
        let rest = s.strip_prefix("giop:tcp:")?;
        let (host, port) = rest.rsplit_once(':')?;
        let port = if port.is_empty() {
            0
        } else {
            port.parse().ok()?
        };
        let host = match host.strip_prefix('[') {
            Some(h) => h.strip_suffix(']')?,
            None if host.contains(':') => return None,
            None => host,
        };
        Some(EndpointAddr {
            host: host.to_owned(),
            port,
        })
    }
}

impl Default for OrbOptions {
    fn default() -> OrbOptions {
        OrbOptions {
            max_giop_version: Version::V1_2,
            byte_order: ByteOrder::NATIVE,
            connect_timeout: None,
            request_timeout: None,
            max_message_size: 64 * 1024 * 1024,
            max_forwards: 16,
            init_refs: HashMap::new(),
            default_init_ref: None,
            io_threads: 2,
            endpoints: Vec::new(),
            endpoint_publish: None,
            min_server_threads: 4,
            max_server_threads: 100,
            server_thread_idle_timeout: Duration::from_secs(60),
            max_requests_per_connection: 256,
        }
    }
}

impl OrbOptions {
    /// Reads ORB options from command line arguments and removes them,
    /// like `CORBA::ORB_init`. Other arguments are left in place.
    ///
    /// | Option | Setting |
    /// | --- | --- |
    /// | `-ORBInitRef Name=URL` | [`init_refs`](OrbOptions::init_refs) |
    /// | `-ORBDefaultInitRef URL` | [`default_init_ref`](OrbOptions::default_init_ref) |
    /// | `-ORBmaxGIOPVersion 1.1` | [`max_giop_version`](OrbOptions::max_giop_version) |
    /// | `-ORBclientConnectTimeOutPeriod MS` | [`connect_timeout`](OrbOptions::connect_timeout) (0 for none) |
    /// | `-ORBclientCallTimeOutPeriod MS` | [`request_timeout`](OrbOptions::request_timeout) (0 for none) |
    /// | `-ORBendPoint giop:tcp:HOST:PORT` | [`endpoints`](OrbOptions::endpoints) (repeatable) |
    /// | `-ORBendPointPublish giop:tcp:HOST:PORT` | [`endpoint_publish`](OrbOptions::endpoint_publish) |
    /// | `-ORBmaxServerThreadPoolSize N` | [`max_server_threads`](OrbOptions::max_server_threads) |
    ///
    /// The value may also follow the option name after `=`, as in
    /// `-ORBmaxGIOPVersion=1.1`.
    pub fn parse_args(&mut self, args: &mut Vec<String>) -> Result<(), SystemException> {
        let mut rest = Vec::with_capacity(args.len());
        let mut iter = std::mem::take(args).into_iter();
        while let Some(arg) = iter.next() {
            let Some(option) = arg.strip_prefix("-ORB") else {
                rest.push(arg);
                continue;
            };
            let (name, inline) = match option.split_once('=') {
                Some((name, value))
                    if matches!(
                        name,
                        "maxGIOPVersion"
                            | "clientConnectTimeOutPeriod"
                            | "clientCallTimeOutPeriod"
                            | "maxServerThreadPoolSize"
                    ) =>
                {
                    (name.to_owned(), Some(value.to_owned()))
                }
                _ => (option.to_owned(), None),
            };
            if !matches!(
                name.as_str(),
                "InitRef"
                    | "DefaultInitRef"
                    | "maxGIOPVersion"
                    | "clientConnectTimeOutPeriod"
                    | "clientCallTimeOutPeriod"
                    | "endPoint"
                    | "endPointPublish"
                    | "maxServerThreadPoolSize"
            ) {
                rest.push(arg);
                continue;
            }
            let value = match inline.or_else(|| iter.next()) {
                Some(value) => value,
                None => return Err(bad_option()),
            };
            match name.as_str() {
                "InitRef" => {
                    let (id, url) = value.split_once('=').ok_or_else(bad_option)?;
                    self.init_refs.insert(id.to_owned(), url.to_owned());
                }
                "DefaultInitRef" => self.default_init_ref = Some(value),
                "maxGIOPVersion" => {
                    let (major, minor) = value.split_once('.').ok_or_else(bad_option)?;
                    let version = Version {
                        major: major.parse().map_err(|_| bad_option())?,
                        minor: minor.parse().map_err(|_| bad_option())?,
                    };
                    if !version.is_supported() {
                        return Err(bad_option());
                    }
                    self.max_giop_version = version;
                }
                "clientConnectTimeOutPeriod" => self.connect_timeout = millis(&value)?,
                "clientCallTimeOutPeriod" => self.request_timeout = millis(&value)?,
                "endPoint" => self
                    .endpoints
                    .push(EndpointAddr::parse(&value).ok_or_else(bad_option)?),
                "endPointPublish" => {
                    self.endpoint_publish =
                        Some(EndpointAddr::parse(&value).ok_or_else(bad_option)?);
                }
                "maxServerThreadPoolSize" => {
                    let n: usize = value.parse().map_err(|_| bad_option())?;
                    if n == 0 {
                        return Err(bad_option());
                    }
                    self.max_server_threads = n;
                }
                _ => unreachable!(),
            }
        }
        *args = rest;
        Ok(())
    }
}

fn millis(value: &str) -> Result<Option<Duration>, SystemException> {
    let ms: u64 = value.parse().map_err(|_| bad_option())?;
    Ok((ms > 0).then(|| Duration::from_millis(ms)))
}

fn bad_param() -> SystemException {
    SystemException::new(SystemExceptionKind::BAD_PARAM, 0, CompletionStatus::No)
}

fn bad_option() -> SystemException {
    SystemException::new(SystemExceptionKind::BAD_PARAM, 0, CompletionStatus::No)
}

/// Why `resolve_initial_references` failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolveError {
    /// `CORBA::ORB::InvalidName`: no initial reference has this name.
    InvalidName(String),
    /// The configured URL could not be turned into a reference.
    System(SystemException),
}

impl fmt::Display for ResolveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ResolveError::InvalidName(name) => write!(f, "no initial reference named {name:?}"),
            ResolveError::System(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for ResolveError {}

impl From<SystemException> for ResolveError {
    fn from(e: SystemException) -> Self {
        ResolveError::System(e)
    }
}

/// An ORB: the client side, and the server side once the RootPOA is used.
///
/// Network I/O runs on a tokio runtime owned by the ORB; the methods here
/// and on [`ObjectRef`] block the calling thread. They must not be called
/// from inside a tokio runtime (they fail with `BAD_INV_ORDER`). Servants
/// run on a pool of upcall threads owned by the ORB.
///
/// `Orb` is a cheap handle: clones share the same ORB, which shuts down
/// when the last clone and the last [`ObjectRef`] are gone. Servants that
/// hold the ORB keep it alive until [`shutdown`](Orb::shutdown) or
/// [`destroy`](Orb::destroy) drops them.
#[derive(Clone)]
pub struct Orb {
    pub(crate) inner: Arc<OrbInner>,
}

pub(crate) struct OrbInner {
    runtime: Option<Runtime>,
    pub(crate) handle: Handle,
    pub(crate) options: OrbOptions,
    connections: Mutex<HashMap<(String, u16), Arc<Connection>>>,
    shut_down: AtomicBool,
    server: Mutex<Option<Arc<Server>>>,
    /// The MAIN_THREAD_MODEL requests, and whether shutdown has completed;
    /// `run` waits for both.
    main: Mutex<MainQueue>,
    main_cond: Condvar,
}

#[derive(Default)]
struct MainQueue {
    jobs: VecDeque<Job>,
    /// No more requests are queued (the server is shutting down).
    closed: bool,
    /// Shutdown has completed.
    stopped: bool,
}

impl OrbInner {
    /// Queues a MAIN_THREAD_MODEL request.
    pub(crate) fn push_main(&self, job: Job) {
        let mut main = self.main.lock().unwrap();
        if main.closed {
            return;
        }
        main.jobs.push_back(job);
        self.main_cond.notify_one();
    }

    /// Drops the queued MAIN_THREAD_MODEL requests and refuses new ones.
    pub(crate) fn close_main(&self) {
        let jobs = {
            let mut main = self.main.lock().unwrap();
            main.closed = true;
            std::mem::take(&mut main.jobs)
        };
        drop(jobs);
    }
}

impl Drop for OrbInner {
    fn drop(&mut self) {
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_background();
        }
    }
}

/// Clones of one ORB are equal.
impl PartialEq for Orb {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }
}

impl Eq for Orb {}

impl fmt::Debug for Orb {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Orb")
            .field("options", &self.inner.options)
            .finish_non_exhaustive()
    }
}

impl Orb {
    /// Initialises an ORB from command line arguments, removing the ORB
    /// options (see [`OrbOptions::parse_args`]).
    pub fn init(args: &mut Vec<String>) -> Result<Orb, SystemException> {
        let mut options = OrbOptions::default();
        options.parse_args(args)?;
        Orb::with_options(options)
    }

    /// Initialises an ORB with `options`.
    pub fn with_options(options: OrbOptions) -> Result<Orb, SystemException> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(options.io_threads.max(1))
            .thread_name("rustorb-io")
            .enable_io()
            .enable_time()
            .build()
            .map_err(|_| {
                SystemException::new(SystemExceptionKind::INITIALIZE, 0, CompletionStatus::No)
            })?;
        let handle = runtime.handle().clone();
        Ok(Orb {
            inner: Arc::new(OrbInner {
                runtime: Some(runtime),
                handle,
                options,
                connections: Mutex::new(HashMap::new()),
                shut_down: AtomicBool::new(false),
                server: Mutex::new(None),
                main: Mutex::default(),
                main_cond: Condvar::new(),
            }),
        })
    }

    /// The options the ORB was created with.
    pub fn options(&self) -> &OrbOptions {
        &self.inner.options
    }

    /// Turns a stringified IOR, a `corbaloc:` or a `corbaname:` URL into a
    /// reference. Returns `None` for a nil IOR, or a name bound to nil.
    ///
    /// `corbaloc:rir:` goes through
    /// [`resolve_initial_references`](Orb::resolve_initial_references).
    /// `corbaname:` resolves the name after `#` in the naming context the
    /// rest of the URL denotes (with no name, the context itself is
    /// returned). The name must be a valid stringified name
    /// (`BAD_PARAM`, minor `BAD_SCHEME_SPECIFIC_PART`); a name the naming
    /// service cannot resolve raises `BAD_PARAM` with minor
    /// `STRING_TO_OBJECT_OTHER`, and system exceptions from the naming
    /// service are passed on.
    pub fn string_to_object(&self, s: &str) -> Result<Option<ObjectRef>, SystemException> {
        self.check_running()?;
        let url = ObjectUrl::parse(s).map_err(|_| {
            let trimmed = s.trim_ascii_start();
            let known = ["IOR:", "corbaloc:", "corbaname:"].iter().any(|scheme| {
                trimmed
                    .get(..scheme.len())
                    .is_some_and(|head| head.eq_ignore_ascii_case(scheme))
            });
            let minor = if known {
                minor::BAD_SCHEME_SPECIFIC_PART
            } else {
                minor::BAD_SCHEME_NAME
            };
            SystemException::new(SystemExceptionKind::BAD_PARAM, minor, CompletionStatus::No)
        })?;
        match url {
            ObjectUrl::Ior(ior) if ior.is_nil() => Ok(None),
            ObjectUrl::Ior(ior) => Ok(Some(ObjectRef::new(self.clone(), ior))),
            ObjectUrl::Corbaloc(loc) => self.corbaloc_to_object(loc).map(Some),
            ObjectUrl::CorbaName(url) => self.resolve_corbaname(url),
        }
    }

    fn corbaloc_to_object(&self, loc: Corbaloc) -> Result<ObjectRef, SystemException> {
        if loc.is_rir() {
            let name = String::from_utf8(loc.key).map_err(|_| {
                SystemException::new(
                    SystemExceptionKind::BAD_PARAM,
                    minor::BAD_SCHEME_SPECIFIC_PART,
                    CompletionStatus::No,
                )
            })?;
            return match self.resolve_initial_references(&name) {
                Ok(obj) => Ok(obj),
                Err(ResolveError::System(e)) => Err(e),
                Err(ResolveError::InvalidName(_)) => Err(SystemException::new(
                    SystemExceptionKind::BAD_PARAM,
                    minor::STRING_TO_OBJECT_OTHER,
                    CompletionStatus::No,
                )),
            };
        }
        let ior = loc.to_ior(self.inner.options.byte_order).map_err(|_| {
            SystemException::new(
                SystemExceptionKind::BAD_PARAM,
                minor::BAD_ADDRESS,
                CompletionStatus::No,
            )
        })?;
        Ok(ObjectRef::new(self.clone(), ior))
    }

    /// Resolves a `corbaname:` URL: the `corbaloc` part gives the naming
    /// context, and the name after `#`, if any, is resolved in it with
    /// `CosNaming::NamingContext::resolve`.
    fn resolve_corbaname(&self, url: CorbaName) -> Result<Option<ObjectRef>, SystemException> {
        let context = self.corbaloc_to_object(url.context)?;
        if url.name.is_empty() {
            return Ok(Some(context));
        }
        let name = name::to_name(&url.name).map_err(|_| {
            SystemException::new(
                SystemExceptionKind::BAD_PARAM,
                minor::BAD_SCHEME_SPECIFIC_PART,
                CompletionStatus::No,
            )
        })?;
        let reply = context
            .request("resolve")
            .args(|out| name::write_name(out, &name))
            .invoke();
        match reply {
            Ok(reply) => reply.input().read_object(),
            Err(Exception::System(e)) => Err(e),
            // NotFound, CannotProceed or InvalidName.
            Err(Exception::User(_)) => Err(SystemException::new(
                SystemExceptionKind::BAD_PARAM,
                minor::STRING_TO_OBJECT_OTHER,
                CompletionStatus::No,
            )),
        }
    }

    /// Writes a reference as a stringified IOR; `None` gives the nil IOR.
    pub fn object_to_string(&self, obj: Option<&ObjectRef>) -> Result<String, SystemException> {
        let nil = Ior::nil();
        let ior = obj.map_or(&nil, |obj| obj.ior());
        ior.to_ior_string(self.inner.options.byte_order)
            .map_err(|_| {
                SystemException::new(SystemExceptionKind::MARSHAL, 0, CompletionStatus::No)
            })
    }

    /// Looks up an initial reference: first in
    /// [`init_refs`](OrbOptions::init_refs), then under
    /// [`default_init_ref`](OrbOptions::default_init_ref).
    pub fn resolve_initial_references(&self, name: &str) -> Result<ObjectRef, ResolveError> {
        let options = &self.inner.options;
        let url = match (options.init_refs.get(name), &options.default_init_ref) {
            (Some(url), _) => url.clone(),
            (None, Some(base)) => format!("{}/{name}", base.trim_end_matches('/')),
            (None, None) => return Err(ResolveError::InvalidName(name.to_owned())),
        };
        // An init ref pointing at rir: again would loop.
        if url
            .trim_ascii()
            .to_ascii_lowercase()
            .starts_with("corbaloc:rir:")
        {
            return Err(ResolveError::InvalidName(name.to_owned()));
        }
        self.string_to_object(&url)?
            .ok_or_else(|| ResolveError::InvalidName(name.to_owned()))
    }

    /// The names [`resolve_initial_references`](Orb::resolve_initial_references)
    /// knows without the default URL.
    pub fn list_initial_services(&self) -> Vec<String> {
        let mut names: Vec<_> = self.inner.options.init_refs.keys().cloned().collect();
        names.sort();
        names
    }

    /// The RootPOA. The first call starts the server side: it opens the
    /// [`endpoints`](OrbOptions::endpoints) (`INITIALIZE` if one cannot be
    /// opened) and the upcall thread pool.
    pub fn root_poa(&self) -> Result<Poa, SystemException> {
        self.check_running()?;
        let mut server = self.inner.server.lock().unwrap();
        let server = match &*server {
            Some(s) => Arc::clone(s),
            None => {
                let s = Server::start(self)?;
                *server = Some(Arc::clone(&s));
                s
            }
        };
        Ok(Poa {
            orb: self.clone(),
            inner: Arc::clone(server.root()),
        })
    }

    /// Makes `obj`, an object of this ORB's POAs, answer to the simple
    /// object key `key`, as in `corbaloc::host:port/NameService`, and
    /// returns a reference with that key. Requests with the key are
    /// dispatched to `obj` as if they had its own key: no
    /// `LOCATION_FORWARD` is involved, so the reference stays valid as
    /// long as the key is registered, also across restarts that register
    /// it again.
    ///
    /// `BAD_PARAM` if `key` is empty or `obj` is not an object of this
    /// ORB's POAs.
    pub fn register_object_key(
        &self,
        key: &str,
        obj: &ObjectRef,
    ) -> Result<ObjectRef, SystemException> {
        let server = self.server().ok_or_else(bad_param)?;
        if key.is_empty() {
            return Err(bad_param());
        }
        let target = obj
            .ior()
            .iiop_profiles()
            .filter_map(Result::ok)
            .find(|p| {
                server.is_local(&p.host, p.port)
                    && matches!(poa::parse_key(&p.object_key), poa::ParsedKey::Poa { .. })
            })
            .map(|p| p.object_key)
            .ok_or_else(bad_param)?;
        let ior = server.ior(obj.type_id(), key.as_bytes().to_vec())?;
        server.set_simple_key(key.as_bytes(), Some(target));
        Ok(ObjectRef::new(self.clone(), ior))
    }

    /// Removes a key registered with
    /// [`register_object_key`](Orb::register_object_key); requests with it
    /// then raise `OBJECT_NOT_EXIST`.
    pub fn unregister_object_key(&self, key: &str) {
        if let Some(server) = self.server() {
            server.set_simple_key(key.as_bytes(), None);
        }
    }

    /// The server side, if the RootPOA was used.
    pub(crate) fn server(&self) -> Option<Arc<Server>> {
        self.inner.server.lock().unwrap().clone()
    }

    /// The server side, if `host:port` is one of its endpoints.
    pub(crate) fn local_server(&self, host: &str, port: u16) -> Option<Arc<Server>> {
        self.server().filter(|s| s.is_local(host, port))
    }

    /// The addresses the server side publishes in IORs, as host and port;
    /// empty until the RootPOA is used.
    pub fn endpoints(&self) -> Vec<(String, u16)> {
        self.server()
            .map(|s| s.published().to_vec())
            .unwrap_or_default()
    }

    /// `CORBA::ORB::run`: blocks until the ORB has shut down, running the
    /// requests of MAIN_THREAD_MODEL POAs meanwhile. Other requests are
    /// processed on the upcall threads whether or not a thread runs this.
    /// Several threads may run it at once.
    pub fn run(&self) {
        while !self.run_timeout(None) {}
    }

    /// [`run`](Orb::run) for at most `timeout` (`None` for no limit).
    /// Returns true when the ORB has shut down, false when the time ran
    /// out. For a binding that must look at its own events, such as
    /// signals, while it runs the ORB.
    pub fn run_timeout(&self, timeout: Option<Duration>) -> bool {
        let deadline = timeout.map(|t| Instant::now() + t);
        let _main = MainThreadGuard::enter();
        let mut main = self.inner.main.lock().unwrap();
        loop {
            if let Some(job) = main.jobs.pop_front() {
                drop(main);
                run_job(job);
                main = self.inner.main.lock().unwrap();
                continue;
            }
            if main.stopped {
                return true;
            }
            main = match deadline {
                None => self.inner.main_cond.wait(main).unwrap(),
                Some(deadline) => {
                    let now = Instant::now();
                    if now >= deadline {
                        return false;
                    }
                    self.inner
                        .main_cond
                        .wait_timeout(main, deadline - now)
                        .unwrap()
                        .0
                }
            };
        }
    }

    /// `CORBA::ORB::work_pending`: whether a MAIN_THREAD_MODEL request
    /// waits for [`perform_work`](Orb::perform_work). `BAD_INV_ORDER` after
    /// shutdown.
    pub fn work_pending(&self) -> Result<bool, SystemException> {
        self.check_running()?;
        Ok(!self.inner.main.lock().unwrap().jobs.is_empty())
    }

    /// `CORBA::ORB::perform_work`: runs one waiting MAIN_THREAD_MODEL
    /// request on the calling thread, if there is one, and returns without
    /// waiting otherwise. For an application's own event loop.
    /// `BAD_INV_ORDER` after shutdown.
    pub fn perform_work(&self) -> Result<(), SystemException> {
        self.check_running()?;
        let job = self.inner.main.lock().unwrap().jobs.pop_front();
        if let Some(job) = job {
            let _main = MainThreadGuard::enter();
            run_job(job);
        }
        Ok(())
    }

    /// `CORBA::ORB::shutdown`. Refuses new invocations (`BAD_INV_ORDER`)
    /// and new requests, drops the requests the POA managers hold, lets the
    /// running ones finish and send their replies, destroys every POA, and
    /// then closes every connection, client and server side, with
    /// `CloseConnection`. Invocations still waiting for a reply fail with
    /// `COMM_FAILURE` or `TRANSIENT`.
    ///
    /// With `wait_for_completion`, returns when all that is done; called
    /// from a servant it fails with `BAD_INV_ORDER` (would deadlock).
    /// Without it, the shutdown runs on another thread and the call returns
    /// at once, so a servant can call it and still send its reply.
    pub fn shutdown(&self, wait_for_completion: bool) -> Result<(), SystemException> {
        if wait_for_completion && poa::in_upcall() {
            return Err(SystemException::new(
                SystemExceptionKind::BAD_INV_ORDER,
                minor::WOULD_DEADLOCK,
                CompletionStatus::No,
            ));
        }
        if self.inner.shut_down.swap(true, Ordering::AcqRel) {
            if wait_for_completion {
                self.run();
            }
            return Ok(());
        }
        if wait_for_completion {
            // The shutdown blocks on the runtime, which cannot be done from
            // inside a runtime (the caller's own, say).
            if Handle::try_current().is_ok() {
                std::thread::scope(|s| {
                    let _ = s.spawn(|| self.complete_shutdown()).join();
                });
            } else {
                self.complete_shutdown();
            }
        } else {
            let orb = self.clone();
            std::thread::Builder::new()
                .name("rustorb-shutdown".into())
                .spawn(move || orb.complete_shutdown())
                .map_err(|_| {
                    SystemException::new(SystemExceptionKind::NO_RESOURCES, 0, CompletionStatus::No)
                })?;
        }
        Ok(())
    }

    fn complete_shutdown(&self) {
        if let Some(server) = self.server() {
            server.shutdown(&self.inner.handle);
        }
        let conns: Vec<_> = self
            .inner
            .connections
            .lock()
            .unwrap()
            .drain()
            .map(|(_, c)| c)
            .collect();
        let version = self.inner.options.max_giop_version;
        let order = self.inner.options.byte_order;
        let _ = self.block_on(async move {
            for conn in conns {
                conn.close(version, order).await;
            }
        });
        self.inner.close_main();
        self.inner.main.lock().unwrap().stopped = true;
        self.inner.main_cond.notify_all();
    }

    /// `CORBA::ORB::destroy`: [`shutdown(true)`](Orb::shutdown), and then
    /// ends the upcall threads. Fails like `shutdown(true)` when called
    /// from a servant.
    pub fn destroy(&self) -> Result<(), SystemException> {
        self.shutdown(true)?;
        if let Some(server) = self.inner.server.lock().unwrap().take() {
            server.stop_pool();
        }
        Ok(())
    }

    pub(crate) fn check_running(&self) -> Result<(), SystemException> {
        if self.inner.shut_down.load(Ordering::Acquire) {
            return Err(SystemException::new(
                SystemExceptionKind::BAD_INV_ORDER,
                minor::ORB_SHUT_DOWN,
                CompletionStatus::No,
            ));
        }
        Ok(())
    }

    /// Runs `fut` on the ORB's runtime and waits for it.
    pub(crate) fn block_on<F: Future>(&self, fut: F) -> Result<F::Output, SystemException> {
        if Handle::try_current().is_ok() {
            return Err(SystemException::new(
                SystemExceptionKind::BAD_INV_ORDER,
                0,
                CompletionStatus::No,
            ));
        }
        Ok(self.inner.handle.block_on(fut))
    }

    /// A usable connection to `host:port`, opened if needed.
    pub(crate) async fn connection(
        &self,
        host: &str,
        port: u16,
    ) -> Result<Arc<Connection>, SystemException> {
        self.check_running()?;
        let key = (host.to_owned(), port);
        if let Some(conn) = self.inner.connections.lock().unwrap().get(&key) {
            if conn.is_usable() {
                return Ok(Arc::clone(conn));
            }
        }
        let max = self.inner.options.max_message_size;
        let connect = Connection::connect(host, port, max);
        let result = match self.inner.options.connect_timeout {
            Some(limit) => tokio::time::timeout(limit, connect)
                .await
                .unwrap_or_else(|_| Err(io::ErrorKind::TimedOut.into())),
            None => connect.await,
        };
        let conn = result.map_err(|_| {
            SystemException::new(SystemExceptionKind::TRANSIENT, 0, CompletionStatus::No)
        })?;
        let mut conns = self.inner.connections.lock().unwrap();
        match conns.get(&key) {
            // Another caller connected first: use its connection, and let
            // this one close when it is dropped.
            Some(existing) if existing.is_usable() => Ok(Arc::clone(existing)),
            _ => {
                conns.insert(key, Arc::clone(&conn));
                Ok(conn)
            }
        }
    }

    /// Drops `conn` from the pool, if it is still the pooled one.
    pub(crate) fn discard(&self, conn: &Arc<Connection>) {
        let mut conns = self.inner.connections.lock().unwrap();
        if conns
            .get(conn.endpoint())
            .is_some_and(|c| Arc::ptr_eq(c, conn))
        {
            conns.remove(conn.endpoint());
        }
    }
}

/// Runs a MAIN_THREAD_MODEL request. Requests catch the panics of user
/// code themselves; this keeps the loop going if anything else panics.
fn run_job(job: Job) {
    let _ = panic::catch_unwind(AssertUnwindSafe(job));
}

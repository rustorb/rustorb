use std::collections::{BTreeMap, HashMap, VecDeque};
use std::fmt;
use std::future::Future;
use std::panic::{self, AssertUnwindSafe};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};

use rustorb_cdr::ByteOrder;
use rustorb_giop::{CorbaName, Corbaloc, Ior, ObjectUrl, Version};
use tokio::runtime::{Handle, Runtime};

use crate::connection::{Connection, ConnectionPool};
use crate::poa::{self, MainThreadGuard, Poa};
use crate::pool::Job;
use crate::server::Server;
use crate::{
    CompletionStatus, Exception, ObjectRef, SystemException, SystemExceptionKind, minor, name,
    timeout,
};
use crate::{tls, trace};

/// Settings of an [`Orb`].
#[derive(Debug, Clone)]
pub struct OrbOptions {
    /// The highest GIOP version used for requests. A request uses the
    /// lower of this and the version of the target's IIOP profile.
    pub max_giop_version: Version,
    /// Byte order of the messages rustorb sends.
    pub byte_order: ByteOrder,
    /// How long to wait for a TCP connection; `None` waits until the call
    /// timeout expires, or as long as the operating system does. Expiry
    /// raises `TIMEOUT`, completed no. The initial value of
    /// [`Orb::set_connect_timeout`].
    pub connect_timeout: Option<Duration>,
    /// How long to wait for a reply; `None` waits forever. Expiry raises
    /// `TIMEOUT`, completed maybe. The initial value of
    /// [`Orb::set_call_timeout`]; see there for the timeouts that override
    /// it.
    pub request_timeout: Option<Duration>,
    /// Largest GIOP message, header included and fragments joined, that
    /// the ORB sends or accepts (`-ORBgiopMaxMsgSize N`; default 64 MiB).
    /// A request over it is not sent (`MARSHAL`, completed no), a reply
    /// over it is replaced by `MARSHAL`, completed yes; a message
    /// received over it closes the connection.
    pub max_message_size: usize,
    /// Splits GIOP 1.2 requests and replies longer than this many bytes
    /// into `Fragment` messages of at most this length
    /// (`-ORBgiopFragmentSize N`; at least 1024, rounded down to a
    /// multiple of 8). 0, the default, sends every message whole. GIOP 1.1
    /// messages are never split. See docs/message-size.md.
    pub fragment_size: usize,
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
    /// How long an upcall thread that answered a request waits for the
    /// next request of the same connection, to read and run it itself
    /// (`-ORBfollowLinger US`, in microseconds; docs/read-ownership.md).
    /// Zero turns this off: the I/O task reads every request.
    pub follow_linger: Duration,
    /// How many requests of one connection an upcall thread reads in a
    /// row before it lets the I/O task read again (`-ORBfollowLimit N`).
    /// Zero turns this off.
    pub follow_limit: usize,
    /// Requests of one connection that may wait or run at once. Beyond
    /// that the connection is not read until one completes.
    pub max_requests_per_connection: usize,
    /// Closes a client connection nothing was sent or received on for
    /// this long, with no reply awaited (`-ORBoutConScanPeriod S`, in
    /// seconds, 0 for never; default 120 s). The next call opens a new
    /// one. See docs/connections.md.
    pub client_idle_timeout: Option<Duration>,
    /// Closes a server connection with `CloseConnection` when no request
    /// came in for this long and none is running
    /// (`-ORBinConScanPeriod S`, in seconds, 0 for never; default 180 s).
    pub server_idle_timeout: Option<Duration>,
    /// Most connections the server side keeps open at once
    /// (`-ORBmaxServerConnections N`; 0, the default, for no limit). At
    /// the limit the connection used least recently, of those with no
    /// request running, is closed with `CloseConnection`, and new
    /// connections wait in the listen backlog until one ends.
    pub max_server_connections: usize,
    /// The socket send buffer of each connection (`SO_SNDBUF`), client and
    /// server; `None` leaves the operating system's default (which on
    /// Linux grows as needed).
    pub socket_send_buffer: Option<usize>,
    /// The socket receive buffer of each connection (`SO_RCVBUF`); `None`
    /// leaves the operating system's default.
    pub socket_receive_buffer: Option<usize>,
    /// Addresses to listen on for requests over TLS (SSLIOP,
    /// `-ORBendPoint giop:ssl:HOST:PORT`). Published in the
    /// `TAG_SSL_SEC_TRANS` component of IORs; with TLS endpoints and none
    /// in [`endpoints`](OrbOptions::endpoints), the ORB has no unprotected
    /// endpoint. Needs [`tls`](OrbOptions::tls) with a key file.
    pub tls_endpoints: Vec<EndpointAddr>,
    /// Address published for the first TLS endpoint instead of the
    /// listening one (`-ORBendPointPublish giop:ssl:HOST:PORT`).
    pub tls_endpoint_publish: Option<EndpointAddr>,
    /// Certificates, key and checks of TLS connections, client and server.
    pub tls: TlsOptions,
    /// The trace level of the process (`-ORBtraceLevel N`), set when the
    /// ORB is created; `None` leaves it as it is (1 at first). See
    /// [`trace`](crate::trace) and docs/tracing.md.
    pub trace_level: Option<u32>,
    /// Whether trace lines on standard error start with the time
    /// (`-ORBtraceTime 0|1`), set when the ORB is created; `None` leaves
    /// it as it is (off at first).
    pub trace_time: Option<bool>,
    /// The arguments the ORB was created with, for
    /// [`OrbInitInfo::arguments`](crate::pi::OrbInitInfo::arguments).
    /// [`Orb::init`] sets them to its arguments, ORB options included.
    pub arguments: Vec<String>,
    /// The ORB id (`-ORBid`), for the Portable Interceptors.
    pub orb_id: String,
    /// The server id (`-ORBServerId`), for the Portable Interceptors.
    pub server_id: String,
    /// The [`OrbInitializer`](crate::pi::OrbInitializer)s of this ORB,
    /// run after those of
    /// [`register_orb_initializer`](crate::pi::register_orb_initializer)
    /// (docs/portable-interceptors.md).
    pub initializers: Vec<Arc<dyn crate::pi::OrbInitializer>>,
}

/// The TLS settings of an ORB (docs/ssliop.md). Files are PEM.
///
/// A client connects with TLS to the objects whose IORs offer it when
/// [`ca_file`](TlsOptions::ca_file) is set; a server listens on the
/// [TLS endpoints](OrbOptions::tls_endpoints).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TlsOptions {
    /// The CA certificates trusted for the peer's certificate
    /// (`-ORBsslCAFile`). A client needs it to use TLS, a server to check
    /// client certificates.
    pub ca_file: Option<PathBuf>,
    /// This ORB's certificate, followed by its intermediate CAs
    /// (`-ORBsslCertFile`). Without it the certificates are read from
    /// [`key_file`](TlsOptions::key_file), which may hold both.
    pub cert_file: Option<PathBuf>,
    /// This ORB's private key, unencrypted (`-ORBsslKeyFile`). A server
    /// needs it; a client sends its certificate when it has one.
    pub key_file: Option<PathBuf>,
    /// Whether a server asks clients for a certificate
    /// (`-ORBsslVerifyClient none|optional|required`).
    pub client_auth: ClientAuth,
    /// Whether a client checks that the server's certificate is for the
    /// host it connects to, as named in the IOR (`-ORBsslVerifyHost 0|1`).
    /// The chain is checked either way.
    pub verify_host: bool,
}

impl Default for TlsOptions {
    fn default() -> TlsOptions {
        TlsOptions {
            ca_file: None,
            cert_file: None,
            key_file: None,
            client_auth: ClientAuth::None,
            verify_host: true,
        }
    }
}

/// Whether a TLS server asks clients for a certificate. A certificate a
/// client sends is checked against [`TlsOptions::ca_file`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ClientAuth {
    /// Clients are not asked.
    #[default]
    None,
    /// Clients are asked; one without a certificate is accepted.
    Optional,
    /// Clients without a valid certificate are refused.
    Required,
}

/// A TCP address of an endpoint, as in `giop:tcp:HOST:PORT` (or
/// `giop:ssl:HOST:PORT` for a TLS endpoint).
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
        EndpointAddr::parse_after(s.strip_prefix("giop:tcp:")?)
    }

    /// Parses `giop:ssl:HOST:PORT`, a TLS endpoint, like
    /// [`parse`](EndpointAddr::parse).
    pub fn parse_tls(s: &str) -> Option<EndpointAddr> {
        EndpointAddr::parse_after(s.strip_prefix("giop:ssl:")?)
    }

    fn parse_after(rest: &str) -> Option<EndpointAddr> {
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
            fragment_size: 0,
            max_forwards: 16,
            init_refs: HashMap::new(),
            default_init_ref: None,
            io_threads: 2,
            endpoints: Vec::new(),
            endpoint_publish: None,
            min_server_threads: 4,
            max_server_threads: 100,
            server_thread_idle_timeout: Duration::from_secs(60),
            follow_linger: Duration::from_millis(1),
            follow_limit: 64,
            max_requests_per_connection: 256,
            client_idle_timeout: Some(Duration::from_secs(120)),
            server_idle_timeout: Some(Duration::from_secs(180)),
            max_server_connections: 0,
            socket_send_buffer: None,
            socket_receive_buffer: None,
            tls_endpoints: Vec::new(),
            tls_endpoint_publish: None,
            tls: TlsOptions::default(),
            trace_level: None,
            trace_time: None,
            arguments: Vec::new(),
            orb_id: String::new(),
            server_id: String::new(),
            initializers: Vec::new(),
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
    /// | `-ORBid ID` | [`orb_id`](OrbOptions::orb_id) |
    /// | `-ORBServerId ID` | [`server_id`](OrbOptions::server_id) |
    /// | `-ORBDefaultInitRef URL` | [`default_init_ref`](OrbOptions::default_init_ref) |
    /// | `-ORBmaxGIOPVersion 1.1` | [`max_giop_version`](OrbOptions::max_giop_version) |
    /// | `-ORBgiopMaxMsgSize N` | [`max_message_size`](OrbOptions::max_message_size), in bytes (at least 1024) |
    /// | `-ORBgiopFragmentSize N` | [`fragment_size`](OrbOptions::fragment_size), in bytes (0, or at least 1024) |
    /// | `-ORBclientConnectTimeOutPeriod MS` | [`connect_timeout`](OrbOptions::connect_timeout) (0 for none) |
    /// | `-ORBclientCallTimeOutPeriod MS` | [`request_timeout`](OrbOptions::request_timeout) (0 for none) |
    /// | `-ORBsupportPerThreadTimeOut 0\|1` | accepted and ignored: per-thread timeouts always work |
    /// | `-ORBendPoint giop:tcp:HOST:PORT` | [`endpoints`](OrbOptions::endpoints) (repeatable) |
    /// | `-ORBendPointPublish giop:tcp:HOST:PORT` | [`endpoint_publish`](OrbOptions::endpoint_publish) (`all(addr)` for the default) |
    /// | `-ORBmaxServerThreadPoolSize N` | [`max_server_threads`](OrbOptions::max_server_threads) |
    /// | `-ORBfollowLinger US` | [`follow_linger`](OrbOptions::follow_linger), in microseconds |
    /// | `-ORBfollowLimit N` | [`follow_limit`](OrbOptions::follow_limit) |
    /// | `-ORBoutConScanPeriod S` | [`client_idle_timeout`](OrbOptions::client_idle_timeout), in seconds (0 for never) |
    /// | `-ORBinConScanPeriod S` | [`server_idle_timeout`](OrbOptions::server_idle_timeout), in seconds (0 for never) |
    /// | `-ORBmaxServerConnections N` | [`max_server_connections`](OrbOptions::max_server_connections) (0 for no limit) |
    /// | `-ORBendPoint giop:ssl:HOST:PORT` | [`tls_endpoints`](OrbOptions::tls_endpoints) (repeatable) |
    /// | `-ORBendPointPublish giop:ssl:HOST:PORT` | [`tls_endpoint_publish`](OrbOptions::tls_endpoint_publish) |
    /// | `-ORBsslCAFile PATH` | [`TlsOptions::ca_file`] |
    /// | `-ORBsslCertFile PATH` | [`TlsOptions::cert_file`] |
    /// | `-ORBsslKeyFile PATH` | [`TlsOptions::key_file`] |
    /// | `-ORBsslVerifyClient none\|optional\|required` | [`TlsOptions::client_auth`] |
    /// | `-ORBsslVerifyHost 0\|1` | [`TlsOptions::verify_host`] |
    /// | `-ORBtraceLevel N` | [`trace_level`](OrbOptions::trace_level) |
    /// | `-ORBtraceTime 0\|1` | [`trace_time`](OrbOptions::trace_time) |
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
                            | "giopMaxMsgSize"
                            | "giopFragmentSize"
                            | "clientConnectTimeOutPeriod"
                            | "clientCallTimeOutPeriod"
                            | "supportPerThreadTimeOut"
                            | "maxServerThreadPoolSize"
                            | "followLinger"
                            | "followLimit"
                            | "outConScanPeriod"
                            | "inConScanPeriod"
                            | "maxServerConnections"
                            | "sslCAFile"
                            | "sslCertFile"
                            | "sslKeyFile"
                            | "sslVerifyClient"
                            | "sslVerifyHost"
                            | "traceLevel"
                            | "traceTime"
                            | "id"
                            | "ServerId"
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
                    | "giopMaxMsgSize"
                    | "giopFragmentSize"
                    | "clientConnectTimeOutPeriod"
                    | "clientCallTimeOutPeriod"
                    | "supportPerThreadTimeOut"
                    | "endPoint"
                    | "endPointPublish"
                    | "maxServerThreadPoolSize"
                    | "followLinger"
                    | "followLimit"
                    | "outConScanPeriod"
                    | "inConScanPeriod"
                    | "maxServerConnections"
                    | "sslCAFile"
                    | "sslCertFile"
                    | "sslKeyFile"
                    | "sslVerifyClient"
                    | "sslVerifyHost"
                    | "traceLevel"
                    | "traceTime"
                    | "id"
                    | "ServerId"
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
                "giopMaxMsgSize" => {
                    let n: usize = value.parse().map_err(|_| bad_option())?;
                    if n < MIN_MESSAGE_SIZE {
                        return Err(bad_option());
                    }
                    self.max_message_size = n;
                }
                "giopFragmentSize" => {
                    let n: usize = value.parse().map_err(|_| bad_option())?;
                    if n != 0 && n < MIN_MESSAGE_SIZE {
                        return Err(bad_option());
                    }
                    self.fragment_size = n;
                }
                "clientConnectTimeOutPeriod" => self.connect_timeout = millis(&value)?,
                "clientCallTimeOutPeriod" => self.request_timeout = millis(&value)?,
                // rustorb always honours per-thread timeouts.
                "supportPerThreadTimeOut" => {
                    if !matches!(value.as_str(), "0" | "1") {
                        return Err(bad_option());
                    }
                }
                "endPoint" if value.starts_with("giop:ssl:") => self
                    .tls_endpoints
                    .push(EndpointAddr::parse_tls(&value).ok_or_else(bad_option)?),
                "endPoint" => self
                    .endpoints
                    .push(EndpointAddr::parse(&value).ok_or_else(bad_option)?),
                // `all(addr)` (every address of the host, omniORB's
                // spelling) is what the ORB publishes anyway.
                "endPointPublish" if value == "all(addr)" => self.endpoint_publish = None,
                "endPointPublish" if value.starts_with("giop:ssl:") => {
                    self.tls_endpoint_publish =
                        Some(EndpointAddr::parse_tls(&value).ok_or_else(bad_option)?);
                }
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
                "followLinger" => {
                    let us: u64 = value.parse().map_err(|_| bad_option())?;
                    self.follow_linger = Duration::from_micros(us);
                }
                "followLimit" => {
                    self.follow_limit = value.parse().map_err(|_| bad_option())?;
                }
                "outConScanPeriod" => self.client_idle_timeout = seconds(&value)?,
                "inConScanPeriod" => self.server_idle_timeout = seconds(&value)?,
                "maxServerConnections" => {
                    self.max_server_connections = value.parse().map_err(|_| bad_option())?;
                }
                "sslCAFile" => self.tls.ca_file = Some(value.into()),
                "sslCertFile" => self.tls.cert_file = Some(value.into()),
                "sslKeyFile" => self.tls.key_file = Some(value.into()),
                "sslVerifyClient" => {
                    self.tls.client_auth = match value.as_str() {
                        "none" => ClientAuth::None,
                        "optional" => ClientAuth::Optional,
                        "required" => ClientAuth::Required,
                        _ => return Err(bad_option()),
                    };
                }
                "sslVerifyHost" => {
                    self.tls.verify_host = match value.as_str() {
                        "0" => false,
                        "1" => true,
                        _ => return Err(bad_option()),
                    };
                }
                "traceLevel" => {
                    self.trace_level = Some(value.parse().map_err(|_| bad_option())?);
                }
                "id" => self.orb_id = value,
                "ServerId" => self.server_id = value,
                "traceTime" => {
                    self.trace_time = Some(match value.as_str() {
                        "0" => false,
                        "1" => true,
                        _ => return Err(bad_option()),
                    });
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

fn seconds(value: &str) -> Result<Option<Duration>, SystemException> {
    let s: u64 = value.parse().map_err(|_| bad_option())?;
    Ok((s > 0).then(|| Duration::from_secs(s)))
}

fn bad_param() -> SystemException {
    SystemException::new(SystemExceptionKind::BAD_PARAM, 0, CompletionStatus::No)
}

/// The smallest `-ORBgiopMaxMsgSize`: room for the headers of any request
/// and for a system exception reply.
const MIN_MESSAGE_SIZE: usize = 1024;

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
/// Network I/O runs on a tokio runtime owned by the ORB, or on one the
/// application shares with it ([`with_runtime`](Orb::with_runtime)); the
/// methods here and on [`ObjectRef`] block the calling thread. They must
/// not be called from inside a tokio runtime, the threads of
/// `spawn_blocking` included (they fail with `BAD_INV_ORDER`): async code
/// uses [`ObjectRef::request_async`] and [`run_async`](Orb::run_async)
/// instead. Servants run on a pool of upcall threads owned by the ORB.
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
    /// [`Orb::set_call_timeout`] and [`Orb::set_connect_timeout`], as
    /// [`timeout::encode`](crate::timeout::encode) stores them.
    call_timeout: AtomicU64,
    connect_timeout: AtomicU64,
    connections: Arc<ConnectionPool>,
    /// The task closing idle client connections, started with the first
    /// connection when [`OrbOptions::client_idle_timeout`] is set.
    idle_scan: OnceLock<tokio::task::JoinHandle<()>>,
    /// The configuration of client TLS connections, made when first used.
    tls_client: OnceLock<Result<Arc<rustls::ClientConfig>, String>>,
    /// [`Orb::register_initial_reference`] and
    /// [`Orb::register_local_initial_reference`]. IORs rather than
    /// references, which would hold the ORB.
    registered: Mutex<BTreeMap<String, Registered>>,
    shut_down: AtomicBool,
    server: Mutex<Option<Arc<Server>>>,
    /// The MAIN_THREAD_MODEL requests, and whether shutdown has completed;
    /// `run` waits for both.
    main: Mutex<MainQueue>,
    main_cond: Condvar,
    /// Wakes [`Orb::run_async`] when shutdown has completed.
    stopped: tokio::sync::Notify,
    /// A number no other ORB of the process has, for the Portable
    /// Interceptors' slots.
    pub(crate) serial: u64,
    /// Whether `pi` is set: what each request looks at first.
    pi_on: AtomicBool,
    /// The Portable Interceptors, until the ORB is destroyed.
    pi: Mutex<Option<Arc<crate::pi::Interceptors>>>,
}

/// An initial reference registered at run time.
enum Registered {
    Object(Ior),
    Local(Arc<dyn std::any::Any + Send + Sync>),
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
        let mut options = OrbOptions {
            arguments: args.clone(),
            ..OrbOptions::default()
        };
        options.parse_args(args)?;
        Orb::with_options(options)
    }

    /// Initialises an ORB with `options`. Fails with the exception of an
    /// [`OrbInitializer`](crate::pi::OrbInitializer) that failed.
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
        Orb::new(options, Some(runtime), handle)
    }

    /// Initialises an ORB with `options` whose network I/O runs on the
    /// application's runtime `handle` instead of a runtime of its own
    /// ([`OrbOptions::io_threads`] is not used). The runtime must have I/O
    /// and time enabled. A `current_thread` runtime does I/O only while a
    /// thread runs its `block_on`, so blocking calls (from servants, say)
    /// wait until one does; a multi-thread runtime has no such limit.
    ///
    /// Shutting the ORB down needs the runtime to run. On a
    /// `current_thread` runtime, a blocking [`shutdown(true)`](Orb::shutdown)
    /// or [`destroy`](Orb::destroy) outside its `block_on` waits for it, and
    /// inside it blocks the runtime: shut down from inside instead, with
    /// `shutdown(false)` and [`run_async`](Orb::run_async), and destroy
    /// after that.
    ///
    /// Destroy the ORB before the runtime shuts down. Destroying it closes
    /// its connections and stops its listening tasks; the runtime keeps no
    /// task of the ORB. Once the runtime is gone, calls fail with
    /// `COMM_FAILURE` or `TRANSIENT`.
    ///
    /// When an [`OrbInitializer`](crate::pi::OrbInitializer) fails, the
    /// ORB is returned shut down (calls fail with `BAD_INV_ORDER`); use
    /// [`try_with_runtime`](Orb::try_with_runtime) to get the exception.
    pub fn with_runtime(options: OrbOptions, handle: Handle) -> Orb {
        match Orb::build(options, None, handle) {
            (orb, Ok(())) => orb,
            (orb, Err(_)) => {
                orb.inner.shut_down.store(true, Ordering::Release);
                orb
            }
        }
    }

    /// [`with_runtime`](Orb::with_runtime), failing with the exception of
    /// an [`OrbInitializer`](crate::pi::OrbInitializer) that failed.
    pub fn try_with_runtime(options: OrbOptions, handle: Handle) -> Result<Orb, SystemException> {
        Orb::new(options, None, handle)
    }

    fn new(
        options: OrbOptions,
        runtime: Option<Runtime>,
        handle: Handle,
    ) -> Result<Orb, SystemException> {
        let (orb, result) = Orb::build(options, runtime, handle);
        result.map(|()| orb)
    }

    /// Makes the ORB and runs its initializers.
    fn build(
        options: OrbOptions,
        runtime: Option<Runtime>,
        handle: Handle,
    ) -> (Orb, Result<(), SystemException>) {
        if let Some(level) = options.trace_level {
            crate::trace::set_trace_level(level);
        }
        if let Some(on) = options.trace_time {
            crate::trace::set_trace_time(on);
        }
        let orb = Orb {
            inner: Arc::new(OrbInner {
                runtime,
                handle,
                call_timeout: AtomicU64::new(timeout::encode(options.request_timeout)),
                connect_timeout: AtomicU64::new(timeout::encode(options.connect_timeout)),
                options,
                connections: Arc::default(),
                idle_scan: OnceLock::new(),
                tls_client: OnceLock::new(),
                registered: Mutex::default(),
                shut_down: AtomicBool::new(false),
                server: Mutex::new(None),
                main: Mutex::default(),
                main_cond: Condvar::new(),
                stopped: tokio::sync::Notify::new(),
                serial: crate::pi::new_serial(),
                pi_on: AtomicBool::new(false),
                pi: Mutex::new(None),
            }),
        };
        let mut initializers = crate::pi::registered_initializers();
        initializers.extend(orb.inner.options.initializers.iter().cloned());
        match crate::pi::initialize(&orb, &initializers) {
            Ok(Some(pi)) => {
                *orb.inner.pi.lock().unwrap() = Some(pi);
                orb.inner.pi_on.store(true, Ordering::Release);
                (orb, Ok(()))
            }
            Ok(None) => (orb, Ok(())),
            Err(e) => (orb, Err(e)),
        }
    }

    /// The interceptors of the ORB, if it has any.
    pub(crate) fn interceptors(&self) -> Option<Arc<crate::pi::Interceptors>> {
        if !self.inner.pi_on.load(Ordering::Acquire) {
            return None;
        }
        self.inner.pi.lock().unwrap().clone()
    }

    /// `resolve_initial_references("PICurrent")`: the slots of the
    /// Portable Interceptors for the calling thread.
    pub fn pi_current(&self) -> crate::pi::PiCurrent {
        crate::pi::PiCurrent::new(self)
    }

    /// `resolve_initial_references("CodecFactory")`.
    pub fn codec_factory(&self) -> crate::iop::CodecFactory {
        crate::iop::CodecFactory::new(self)
    }

    /// The handle of the runtime the ORB's network I/O runs on: its own,
    /// or the one given to [`with_runtime`](Orb::with_runtime).
    pub fn runtime_handle(&self) -> &Handle {
        &self.inner.handle
    }

    /// The options the ORB was created with.
    pub fn options(&self) -> &OrbOptions {
        &self.inner.options
    }

    /// Sets how long calls through this ORB may take (`None`: no limit),
    /// like omniORB's `setClientCallTimeout(millisecs)`. The time covers
    /// connecting (unless a [connect timeout](Orb::set_connect_timeout)
    /// is set), following forwards and waiting for the reply. When it
    /// expires the call raises `TIMEOUT`: completed maybe once the request
    /// was sent, and then a `CancelRequest` goes to the server; completed
    /// no before.
    ///
    /// A timeout of the calling thread
    /// ([`set_thread_call_timeout`](crate::set_thread_call_timeout))
    /// overrides it, one of the object reference
    /// ([`ObjectRef::set_call_timeout`]) overrides both, and one given for
    /// the request ([`Request::timeout`](crate::Request::timeout)) all of
    /// them.
    pub fn set_call_timeout(&self, timeout: Option<Duration>) {
        self.inner
            .call_timeout
            .store(timeout::encode(timeout), Ordering::Relaxed);
    }

    /// The timeout [`set_call_timeout`](Orb::set_call_timeout) set, or
    /// [`OrbOptions::request_timeout`].
    pub fn call_timeout(&self) -> Option<Duration> {
        timeout::decode(self.inner.call_timeout.load(Ordering::Relaxed))
    }

    /// Sets how long to wait for a new TCP connection, like omniORB's
    /// `setClientConnectTimeout`: while connecting it replaces the call
    /// timeout. `None` connects until the call timeout expires, or as long
    /// as the operating system waits. Expiry raises `TIMEOUT`, completed
    /// no.
    pub fn set_connect_timeout(&self, timeout: Option<Duration>) {
        self.inner
            .connect_timeout
            .store(timeout::encode(timeout), Ordering::Relaxed);
    }

    /// The timeout [`set_connect_timeout`](Orb::set_connect_timeout) set,
    /// or [`OrbOptions::connect_timeout`].
    pub fn connect_timeout(&self) -> Option<Duration> {
        timeout::decode(self.inner.connect_timeout.load(Ordering::Relaxed))
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

    /// Looks up an initial reference: first among those
    /// [registered](Orb::register_initial_reference), then in
    /// [`init_refs`](OrbOptions::init_refs), then under
    /// [`default_init_ref`](OrbOptions::default_init_ref).
    pub fn resolve_initial_references(&self, name: &str) -> Result<ObjectRef, ResolveError> {
        match self.inner.registered.lock().unwrap().get(name) {
            Some(Registered::Object(ior)) => {
                return Ok(ObjectRef::new(self.clone(), ior.clone()));
            }
            Some(Registered::Local(_)) => {
                return Err(ResolveError::InvalidName(name.to_owned()));
            }
            None => {}
        }
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
    /// knows without the default URL, sorted.
    pub fn list_initial_services(&self) -> Vec<String> {
        let mut names: Vec<_> = self.inner.options.init_refs.keys().cloned().collect();
        names.extend(self.inner.registered.lock().unwrap().keys().cloned());
        names.sort();
        names.dedup();
        names
    }

    /// `CORBA::ORB::register_initial_reference`: makes
    /// [`resolve_initial_references`](Orb::resolve_initial_references)
    /// return `obj` for `name`. `InvalidName` if `name` is empty or
    /// already an initial reference (registered or given with
    /// `-ORBInitRef`), `BAD_PARAM` (minor
    /// [`NIL_INITIAL_REFERENCE`](crate::minor::NIL_INITIAL_REFERENCE)) for
    /// a nil `obj`. The names a binding resolves itself, such as
    /// `RootPOA`, are the binding's to refuse.
    pub fn register_initial_reference(
        &self,
        name: &str,
        obj: Option<&ObjectRef>,
    ) -> Result<(), ResolveError> {
        let obj = obj.ok_or_else(|| {
            SystemException::new(
                SystemExceptionKind::BAD_PARAM,
                minor::NIL_INITIAL_REFERENCE,
                CompletionStatus::No,
            )
        });
        self.register(name, || Ok(Registered::Object(obj?.ior().clone())))
    }

    /// [`register_initial_reference`](Orb::register_initial_reference) for
    /// an object of a binding that has no reference, such as a local
    /// object: the ORB keeps `obj` under `name` until it is destroyed, for
    /// [`resolve_local_initial_reference`](Orb::resolve_local_initial_reference).
    /// `resolve_initial_references` does not resolve it.
    pub fn register_local_initial_reference(
        &self,
        name: &str,
        obj: Arc<dyn std::any::Any + Send + Sync>,
    ) -> Result<(), ResolveError> {
        self.register(name, || Ok(Registered::Local(obj)))
    }

    /// An object registered with
    /// [`register_local_initial_reference`](Orb::register_local_initial_reference).
    pub fn resolve_local_initial_reference(
        &self,
        name: &str,
    ) -> Option<Arc<dyn std::any::Any + Send + Sync>> {
        match self.inner.registered.lock().unwrap().get(name) {
            Some(Registered::Local(obj)) => Some(Arc::clone(obj)),
            _ => None,
        }
    }

    fn register(
        &self,
        name: &str,
        entry: impl FnOnce() -> Result<Registered, SystemException>,
    ) -> Result<(), ResolveError> {
        let mut registered = self.inner.registered.lock().unwrap();
        if name.is_empty()
            || registered.contains_key(name)
            || self.inner.options.init_refs.contains_key(name)
        {
            return Err(ResolveError::InvalidName(name.to_owned()));
        }
        registered.insert(name.to_owned(), entry()?);
        Ok(())
    }

    /// The RootPOA. The first call starts the server side: it opens the
    /// [`endpoints`](OrbOptions::endpoints) (`INITIALIZE` if one cannot be
    /// opened) and the upcall thread pool.
    pub fn root_poa(&self) -> Result<Poa, SystemException> {
        self.check_running()?;
        // The IOR interceptors run without the lock (they may use the ORB).
        let root_components = match &*self.inner.server.lock().unwrap() {
            Some(_) => Vec::new(),
            None => crate::pi::establish_components(self, &poa::Policies::root(), &[]),
        };
        let mut server = self.inner.server.lock().unwrap();
        let server = match &*server {
            Some(s) => Arc::clone(s),
            None => {
                let s = Server::start(self, root_components)?;
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
                server.is_own_profile(p)
                    && matches!(poa::parse_key(&p.object_key), poa::ParsedKey::Poa { .. })
            })
            .map(|p| p.object_key)
            .ok_or_else(bad_param)?;
        let ior = server.ior(obj.type_id(), key.as_bytes().to_vec(), &[])?;
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

    /// The server side, if `host:port` is one of its endpoints (a TLS one
    /// if `tls`).
    pub(crate) fn local_server(&self, host: &str, port: u16, tls: bool) -> Option<Arc<Server>> {
        self.server().filter(|s| s.is_local(host, port, tls))
    }

    /// The addresses the server side publishes in IORs, as host and port;
    /// empty until the RootPOA is used.
    pub fn endpoints(&self) -> Vec<(String, u16)> {
        self.server()
            .map(|s| s.published().to_vec())
            .unwrap_or_default()
    }

    /// The TLS endpoints the server side publishes in IORs, as host and
    /// port; empty until the RootPOA is used.
    pub fn tls_endpoints(&self) -> Vec<(String, u16)> {
        self.server()
            .map(|s| s.published_tls().to_vec())
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

    /// [`run`](Orb::run) for async code: resolves when the ORB has shut
    /// down. It does not run MAIN_THREAD_MODEL requests, which need
    /// [`run`](Orb::run) or [`perform_work`](Orb::perform_work) on a
    /// thread. With [`shutdown(false)`](Orb::shutdown), the way to shut the
    /// ORB down from async code and wait for it.
    pub async fn run_async(&self) {
        loop {
            let notified = std::pin::pin!(self.inner.stopped.notified());
            let mut notified = notified;
            notified.as_mut().enable();
            let stopped = self.inner.main.lock().unwrap().stopped;
            if stopped {
                return;
            }
            notified.await;
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
        if let Some(task) = self.inner.idle_scan.get() {
            task.abort();
        }
        let version = self.inner.options.max_giop_version;
        let order = self.inner.options.byte_order;
        let _ = self.block_on(async move {
            for conn in conns {
                conn.close(Some((version, order))).await;
            }
        });
        self.inner.close_main();
        self.inner.main.lock().unwrap().stopped = true;
        self.inner.main_cond.notify_all();
        self.inner.stopped.notify_waiters();
    }

    /// `CORBA::ORB::destroy`: [`shutdown(true)`](Orb::shutdown), and then
    /// ends the upcall threads. Fails like `shutdown(true)` when called
    /// from a servant.
    pub fn destroy(&self) -> Result<(), SystemException> {
        self.shutdown(true)?;
        if let Some(server) = self.inner.server.lock().unwrap().take() {
            server.stop_pool();
        }
        // Local objects may hold the ORB.
        let registered = std::mem::take(&mut *self.inner.registered.lock().unwrap());
        drop(registered);
        // So may interceptors.
        self.inner.pi_on.store(false, Ordering::Release);
        let pi = self.inner.pi.lock().unwrap().take();
        if let Some(pi) = pi {
            pi.destroy();
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

    /// `fut`, polled inside the ORB's runtime: the sockets, timers and
    /// tasks it makes belong to that runtime whichever executor polls it.
    pub(crate) fn in_runtime<F: Future>(&self, fut: F) -> InRuntime<F> {
        InRuntime {
            handle: self.inner.handle.clone(),
            fut: Box::pin(fut),
        }
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

    /// Whether this ORB can make TLS connections: it has CA certificates
    /// to check servers with.
    pub(crate) fn has_tls_client(&self) -> bool {
        self.inner.options.tls.ca_file.is_some()
    }

    /// The configuration of client TLS connections. A configuration that
    /// cannot be loaded makes every TLS connection fail with `TRANSIENT`.
    fn tls_client(&self) -> Result<Arc<rustls::ClientConfig>, SystemException> {
        self.inner
            .tls_client
            .get_or_init(|| tls::client_config(&self.inner.options.tls))
            .clone()
            .map_err(|_| {
                SystemException::new(SystemExceptionKind::TRANSIENT, 0, CompletionStatus::No)
            })
    }

    /// A usable connection to `host:port`, with TLS if `tls`, opened if
    /// needed.
    ///
    /// Connecting stops at the connect timeout, else at `deadline` (the
    /// end of the call), with `TIMEOUT`, completed no, like omniORB. The
    /// TLS handshake counts as part of connecting.
    pub(crate) async fn connection(
        &self,
        host: &str,
        port: u16,
        tls: bool,
        deadline: Option<Instant>,
    ) -> Result<Arc<Connection>, SystemException> {
        self.check_running()?;
        let key = (host.to_owned(), port, tls);
        if let Some(conn) = self.inner.connections.lock().unwrap().get(&key) {
            if conn.is_usable() {
                return Ok(Arc::clone(conn));
            }
        }
        let config = if tls { Some(self.tls_client()?) } else { None };
        if let Some(timeout) = self.inner.options.client_idle_timeout {
            self.inner.idle_scan.get_or_init(|| {
                self.inner
                    .handle
                    .spawn(close_idle(Arc::downgrade(&self.inner.connections), timeout))
            });
        }
        let connect = Connection::connect(
            host,
            port,
            config,
            &self.inner.options,
            Arc::downgrade(&self.inner.connections),
        );
        let limit = match self.connect_timeout() {
            Some(t) => Some(Instant::now() + t),
            None => deadline,
        };
        let result = match limit {
            Some(at) => match tokio::time::timeout_at(at.into(), connect).await {
                Ok(result) => result,
                Err(_) => {
                    return Err(SystemException::new(
                        SystemExceptionKind::TIMEOUT,
                        0,
                        CompletionStatus::No,
                    ));
                }
            },
            None => connect.await,
        };
        let conn = result.map_err(|_| {
            SystemException::new(SystemExceptionKind::TRANSIENT, 0, CompletionStatus::No)
        })?;
        let mut conns = self.inner.connections.lock().unwrap();
        match conns.get(&key) {
            // Another caller connected first: use its connection, and
            // close this one (its reader task would keep it open).
            Some(existing) if existing.is_usable() => {
                tokio::spawn(async move { conn.close(None).await });
                Ok(Arc::clone(existing))
            }
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

/// Closes the client connections of `pool` idle for `timeout`, until the
/// ORB is gone or shut down (which aborts the task).
async fn close_idle(pool: std::sync::Weak<ConnectionPool>, timeout: Duration) {
    let period = crate::idle::scan_period(timeout);
    loop {
        tokio::time::sleep(period).await;
        let Some(pool) = pool.upgrade() else {
            return;
        };
        let idle: Vec<_> = {
            let mut conns = pool.lock().unwrap();
            let idle: Vec<_> = conns
                .iter()
                .filter(|(_, c)| c.choose_idle(timeout))
                .map(|(key, _)| key.clone())
                .collect();
            idle.iter().filter_map(|key| conns.remove(key)).collect()
        };
        drop(pool);
        for conn in idle {
            trace::info!(
                "closing the connection to {}:{}: idle for {timeout:?}",
                conn.endpoint().0,
                conn.endpoint().1
            );
            tokio::spawn(async move { conn.close(None).await });
        }
    }
}

/// Runs a MAIN_THREAD_MODEL request. Requests catch the panics of user
/// code themselves; this keeps the loop going if anything else panics.
fn run_job(job: Job) {
    let _ = panic::catch_unwind(AssertUnwindSafe(job));
}

/// See [`Orb::in_runtime`].
pub(crate) struct InRuntime<F> {
    handle: Handle,
    fut: std::pin::Pin<Box<F>>,
}

impl<F: Future> Future for InRuntime<F> {
    type Output = F::Output;

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<F::Output> {
        let this = &mut *self;
        let _runtime = this.handle.enter();
        this.fut.as_mut().poll(cx)
    }
}

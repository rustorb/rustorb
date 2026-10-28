use std::collections::HashMap;
use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::{fmt, io};

use rustorb_cdr::ByteOrder;
use rustorb_giop::{Ior, ObjectUrl, Version};
use tokio::runtime::{Handle, Runtime};

use crate::connection::Connection;
use crate::{CompletionStatus, ObjectRef, SystemException, SystemExceptionKind, minor};

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
                        "maxGIOPVersion" | "clientConnectTimeOutPeriod" | "clientCallTimeOutPeriod"
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

/// The client side of an ORB.
///
/// Network I/O runs on a tokio runtime owned by the ORB; the methods here
/// and on [`ObjectRef`] block the calling thread. They must not be called
/// from inside a tokio runtime (they fail with `BAD_INV_ORDER`).
///
/// `Orb` is a cheap handle: clones share the same ORB, which shuts down
/// when the last clone and the last [`ObjectRef`] are gone.
#[derive(Clone)]
pub struct Orb {
    pub(crate) inner: Arc<OrbInner>,
}

pub(crate) struct OrbInner {
    runtime: Option<Runtime>,
    handle: Handle,
    pub(crate) options: OrbOptions,
    connections: Mutex<HashMap<(String, u16), Arc<Connection>>>,
    shut_down: AtomicBool,
}

impl Drop for OrbInner {
    fn drop(&mut self) {
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_background();
        }
    }
}

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
            }),
        })
    }

    /// The options the ORB was created with.
    pub fn options(&self) -> &OrbOptions {
        &self.inner.options
    }

    /// Turns a stringified IOR or a `corbaloc:` URL into a reference.
    /// Returns `None` for a nil IOR.
    ///
    /// `corbaloc:rir:` goes through
    /// [`resolve_initial_references`](Orb::resolve_initial_references).
    /// `corbaname:` needs the naming service client and is not supported
    /// yet (`NO_IMPLEMENT`).
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
            ObjectUrl::Corbaloc(loc) if loc.is_rir() => {
                let name = String::from_utf8(loc.key).map_err(|_| {
                    SystemException::new(
                        SystemExceptionKind::BAD_PARAM,
                        minor::BAD_SCHEME_SPECIFIC_PART,
                        CompletionStatus::No,
                    )
                })?;
                match self.resolve_initial_references(&name) {
                    Ok(obj) => Ok(Some(obj)),
                    Err(ResolveError::System(e)) => Err(e),
                    Err(ResolveError::InvalidName(_)) => Err(SystemException::new(
                        SystemExceptionKind::BAD_PARAM,
                        minor::STRING_TO_OBJECT_OTHER,
                        CompletionStatus::No,
                    )),
                }
            }
            ObjectUrl::Corbaloc(loc) => {
                let ior = loc.to_ior(self.inner.options.byte_order).map_err(|_| {
                    SystemException::new(
                        SystemExceptionKind::BAD_PARAM,
                        minor::BAD_ADDRESS,
                        CompletionStatus::No,
                    )
                })?;
                Ok(Some(ObjectRef::new(self.clone(), ior)))
            }
            ObjectUrl::CorbaName(_) => Err(SystemException::new(
                SystemExceptionKind::NO_IMPLEMENT,
                0,
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

    /// Closes every connection with `CloseConnection` and refuses new
    /// invocations (`BAD_INV_ORDER`). Calls in progress fail with
    /// `COMM_FAILURE` or `TRANSIENT`.
    pub fn shutdown(&self) {
        if self.inner.shut_down.swap(true, Ordering::AcqRel) {
            return;
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

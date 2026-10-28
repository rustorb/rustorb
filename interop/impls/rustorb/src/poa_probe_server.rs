//! rustorb server for the POA and thread model probes (`PoaProbe.idl`).
//!
//! ```text
//! rustorb-poa-probe-server --host HOST --port N --ior-file FILE [ORB options]
//! ```
//!
//! Publishes Target objects in POAs with different policies, like the
//! omniORB probe servers (`impls/omniorbpy/poa_probe_server.py`), and
//! writes the IOR of the Control object to FILE. `scripts/poa-probe`
//! drives it; see `run-poa-probe.sh`. The skeletons are hand-written until
//! the code generator exists.

mod idl;

use std::collections::HashMap;
use std::process::ExitCode;
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::{self, ThreadId};
use std::time::{Duration, Instant};

use idl::{Get, Put, Sys};
use rustorb_core::{
    AdapterActivator, CompletionStatus, Cookie, Exception, IdAssignment, IdUniqueness, InStream,
    Lifespan, ObjectRef, Orb, OutStream, Poa, PoaCurrent, PoaError, PoaManager, Policy,
    RequestProcessing, Servant, ServantActivator, ServantLocator, ServantManager,
    ServantManagerError, ServantRetention, ServerReply, SystemException, SystemExceptionKind,
    ThreadPolicy, minor,
};

const TARGET_ID: &str = "IDL:rustorb.org/PoaProbe/Target:1.0";
const CONTROL_ID: &str = "IDL:rustorb.org/PoaProbe/Control:1.0";
const FAILED_ID: &str = "IDL:rustorb.org/PoaProbe/Failed:1.0";

static START: OnceLock<Instant> = OnceLock::new();
static MAIN_THREAD: OnceLock<ThreadId> = OnceLock::new();
static THREADS: Mutex<Vec<ThreadId>> = Mutex::new(Vec::new());
static EVENTS: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// Seconds since the server started.
fn now() -> f64 {
    START.get_or_init(Instant::now).elapsed().as_secs_f64()
}

/// A short, stable label for the current thread: "main" for the one that
/// runs `ORB::run`, else "T1", "T2", ... in the order they are first seen.
fn thread_label() -> String {
    let me = thread::current().id();
    if MAIN_THREAD.get() == Some(&me) {
        return "main".into();
    }
    let mut threads = THREADS.lock().unwrap();
    let index = match threads.iter().position(|t| *t == me) {
        Some(i) => i,
        None => {
            threads.push(me);
            threads.len() - 1
        }
    };
    format!("T{}", index + 1)
}

fn event(text: String) {
    EVENTS.lock().unwrap().push(text);
}

fn latin1(id: &[u8]) -> String {
    id.iter().map(|&b| char::from(b)).collect()
}

/// The POA names from below the RootPOA, joined with `/`.
fn poa_path(poa: &Poa) -> String {
    let mut names = Vec::new();
    let mut poa = poa.clone();
    while let Some(parent) = poa.the_parent() {
        names.push(poa.the_name().to_owned());
        poa = parent;
    }
    names.reverse();
    if names.is_empty() {
        "RootPOA".into()
    } else {
        names.join("/")
    }
}

fn sys(kind: SystemExceptionKind, minor: u32) -> SystemException {
    SystemException::new(kind, minor, CompletionStatus::No)
}

fn bad_operation() -> SystemException {
    sys(SystemExceptionKind::BAD_OPERATION, minor::UNKNOWN_OPERATION)
}

/// `PoaProbe::CallInfo`.
struct CallInfo {
    thread: String,
    object_id: String,
    poa: String,
    start: f64,
    end: f64,
}

impl Put for CallInfo {
    fn put(&self, out: &mut OutStream<'_>) -> Sys<()> {
        (&self.thread, &self.object_id, &self.poa).put(out)?;
        (self.start, self.end).put(out)
    }
}

impl Get for CallInfo {
    fn get(input: &mut InStream<'_>) -> Sys<Self> {
        let (thread, object_id, poa) = Get::get(input)?;
        let (start, end) = Get::get(input)?;
        Ok(CallInfo {
            thread,
            object_id,
            poa,
            start,
            end,
        })
    }
}

fn sleep_ms(ms: i32) {
    if ms > 0 {
        thread::sleep(Duration::from_millis(ms.unsigned_abs().into()));
    }
}

/// `PoaProbe::Target`.
struct Target;

impl Target {
    fn work(ms: i32) -> CallInfo {
        let start = now();
        let thread = thread_label();
        let object_id = PoaCurrent::get_object_id()
            .map(|id| latin1(&id))
            .unwrap_or_default();
        let poa = PoaCurrent::get_poa()
            .map(|p| poa_path(&p))
            .unwrap_or_default();
        sleep_ms(ms);
        CallInfo {
            thread,
            object_id,
            poa,
            start,
            end: now(),
        }
    }
}

impl Servant for Target {
    fn repository_id(&self) -> &str {
        TARGET_ID
    }

    fn dispatch(
        &self,
        operation: &str,
        input: &mut InStream<'_>,
        reply: &mut ServerReply,
    ) -> Result<(), SystemException> {
        match operation {
            "work" => {
                let (ms,): (i32,) = Get::get(input)?;
                Target::work(ms).put(&mut reply.output())
            }
            "relay" => {
                let (other, ms): (Option<ObjectRef>, i32) = Get::get(input)?;
                let other = other.ok_or_else(|| sys(SystemExceptionKind::BAD_PARAM, 0))?;
                let outer = thread_label();
                let inner = other
                    .request("work")
                    .args(|out| ms.put(out))
                    .timeout(Some(Duration::from_secs(3)))
                    .invoke()
                    .map_err(|e| match e {
                        Exception::System(e) => e,
                        Exception::User(_) => SystemException::new(
                            SystemExceptionKind::UNKNOWN,
                            minor::UNLISTED_USER_EXCEPTION,
                            CompletionStatus::Maybe,
                        ),
                    })?;
                let info = CallInfo::get(&mut inner.input())?;
                (info, outer).put(&mut reply.output())
            }
            "post" => {
                let (seq, ms): (i32, i32) = Get::get(input)?;
                sleep_ms(ms);
                event(format!("post {seq}"));
                Ok(())
            }
            _ => Err(bad_operation()),
        }
    }
}

/// Answers `incarnate` with the shared Target, except for the ObjectIds
/// "bad" (`OBJECT_NOT_EXIST`) and "fwd" (forwarded to `forward`).
struct Activator {
    servant: Arc<dyn Servant>,
    forward: ObjectRef,
}

impl ServantActivator for Activator {
    fn incarnate(
        &self,
        id: &[u8],
        _adapter: &Poa,
    ) -> Result<Arc<dyn Servant>, ServantManagerError> {
        let name = latin1(id);
        event(format!("incarnate {name}"));
        match name.as_str() {
            "bad" => Err(sys(SystemExceptionKind::OBJECT_NOT_EXIST, 0).into()),
            "fwd" => Err(ServantManagerError::ForwardRequest(self.forward.clone())),
            _ => Ok(Arc::clone(&self.servant)),
        }
    }

    fn etherealize(
        &self,
        id: &[u8],
        _adapter: &Poa,
        _servant: Arc<dyn Servant>,
        _cleanup_in_progress: bool,
        _remaining_activations: bool,
    ) {
        event(format!("etherealize {}", latin1(id)));
    }
}

/// Like [`Activator`], for every request.
struct Locator {
    servant: Arc<dyn Servant>,
    forward: ObjectRef,
}

impl ServantLocator for Locator {
    fn preinvoke(
        &self,
        id: &[u8],
        _adapter: &Poa,
        operation: &str,
    ) -> Result<(Arc<dyn Servant>, Cookie), ServantManagerError> {
        let name = latin1(id);
        event(format!("preinvoke {name} {operation}"));
        match name.as_str() {
            "bad" => Err(sys(SystemExceptionKind::OBJECT_NOT_EXIST, 0).into()),
            "fwd" => Err(ServantManagerError::ForwardRequest(self.forward.clone())),
            _ => Ok((Arc::clone(&self.servant), Box::new(()))),
        }
    }

    fn postinvoke(
        &self,
        id: &[u8],
        _adapter: &Poa,
        operation: &str,
        _cookie: Cookie,
        _servant: Arc<dyn Servant>,
    ) {
        event(format!("postinvoke {} {operation}", latin1(id)));
    }
}

/// Creates POAs named lazy* on demand, with a default servant.
struct Adapters {
    servant: Arc<dyn Servant>,
}

impl AdapterActivator for Adapters {
    fn unknown_adapter(&self, parent: &Poa, name: &str) -> bool {
        event(format!("unknown_adapter {name}"));
        if !name.starts_with("lazy") {
            return false;
        }
        let created = parent.create_poa(
            name,
            Some(&parent.the_poa_manager()),
            &[
                Policy::Lifespan(Lifespan::Persistent),
                Policy::IdAssignment(IdAssignment::User),
                Policy::ServantRetention(ServantRetention::NonRetain),
                Policy::RequestProcessing(RequestProcessing::UseDefaultServant),
                Policy::IdUniqueness(IdUniqueness::Multiple),
            ],
        );
        match created {
            Ok(poa) => poa.set_servant(Arc::clone(&self.servant)).is_ok(),
            Err(_) => false,
        }
    }
}

/// `PoaProbe::Control`.
struct Control {
    orb: Orb,
    root: Poa,
    managed: PoaManager,
    system_refs: HashMap<String, ObjectRef>,
}

/// `PoaProbe::Failed`.
fn failed(reply: &mut ServerReply, reason: &str) -> Sys<()> {
    let mut out = reply.user_exception(FAILED_ID)?;
    out.write_string(reason)
}

impl Control {
    fn poa(&self, path: &str) -> Result<Poa, String> {
        let mut poa = self.root.clone();
        for name in path.split('/').filter(|p| !p.is_empty() && *p != "RootPOA") {
            poa = poa
                .find_poa(name, true)
                .map_err(|_| format!("no POA {path}"))?;
        }
        Ok(poa)
    }

    fn reference(&self, poa: &str, oid: &str) -> Result<String, String> {
        let obj = match self.system_refs.get(poa) {
            Some(obj) => obj.clone(),
            None => self
                .poa(poa)?
                .create_reference_with_id(&oid_bytes(oid), TARGET_ID)
                .map_err(|e| e.to_string())?,
        };
        self.orb
            .object_to_string(Some(&obj))
            .map_err(|e| e.to_string())
    }

    fn set_state(&self, state: &str) -> Result<(), String> {
        let result = match state {
            "active" => self.managed.activate(),
            "holding" => self.managed.hold_requests(false),
            "discarding" => self.managed.discard_requests(false),
            "inactive" => self.managed.deactivate(false, false),
            _ => return Err(format!("unknown state {state}")),
        };
        result.map_err(|e| match e {
            PoaError::AdapterInactive => "AdapterInactive".to_owned(),
            e => e.to_string(),
        })
    }

    fn shutdown_later(&self, delay_ms: i32) {
        let orb = self.orb.clone();
        thread::spawn(move || {
            sleep_ms(delay_ms);
            let _ = orb.shutdown(false);
        });
    }
}

fn oid_bytes(oid: &str) -> Vec<u8> {
    oid.chars()
        .map(|c| u8::try_from(c).unwrap_or(b'?'))
        .collect()
}

/// Writes the result of an operation that raises `Failed`.
fn or_failed<T: Put>(reply: &mut ServerReply, result: Result<T, String>) -> Sys<()> {
    match result {
        Ok(v) => v.put(&mut reply.output()),
        Err(reason) => failed(reply, &reason),
    }
}

impl Servant for Control {
    fn repository_id(&self) -> &str {
        CONTROL_ID
    }

    fn dispatch(
        &self,
        operation: &str,
        input: &mut InStream<'_>,
        reply: &mut ServerReply,
    ) -> Result<(), SystemException> {
        match operation {
            "reference" => {
                let (poa, oid): (String, String) = Get::get(input)?;
                or_failed(reply, self.reference(&poa, &oid))
            }
            "set_state" => {
                let (state,): (String,) = Get::get(input)?;
                or_failed(reply, self.set_state(&state))
            }
            "deactivate_object" => {
                let (poa, oid): (String, String) = Get::get(input)?;
                let result = self.poa(&poa).and_then(|p| {
                    p.deactivate_object(&oid_bytes(&oid)).map_err(|e| match e {
                        PoaError::ObjectNotActive => "ObjectNotActive".to_owned(),
                        e => e.to_string(),
                    })
                });
                or_failed(reply, result)
            }
            "activate_object" => {
                let (poa, oid): (String, String) = Get::get(input)?;
                let result = self.poa(&poa).and_then(|p| {
                    p.activate_object_with_id(&oid_bytes(&oid), Arc::new(Target))
                        .map_err(|e| match e {
                            PoaError::ObjectAlreadyActive => "ObjectAlreadyActive".to_owned(),
                            PoaError::ServantAlreadyActive => "ServantAlreadyActive".to_owned(),
                            e => e.to_string(),
                        })
                });
                or_failed(reply, result)
            }
            "destroy_poa" => {
                let (poa,): (String,) = Get::get(input)?;
                let result = self
                    .poa(&poa)
                    .and_then(|p| p.destroy(true, false).map_err(|e| e.to_string()));
                or_failed(reply, result)
            }
            "events" => {
                let events = std::mem::take(&mut *EVENTS.lock().unwrap());
                events.put(&mut reply.output())
            }
            "shutdown_inside" => {
                let (wait,): (bool,) = Get::get(input)?;
                let result = match self.orb.shutdown(wait) {
                    Ok(()) => String::new(),
                    Err(e) => format!("{} minor 0x{:08x}", e.repository_id, e.minor),
                };
                result.put(&mut reply.output())
            }
            "shutdown_later" => {
                let (delay,): (i32,) = Get::get(input)?;
                self.shutdown_later(delay);
                Ok(())
            }
            _ => Err(bad_operation()),
        }
    }
}

struct Args {
    host: String,
    port: u16,
    ior_file: String,
}

fn parse_args(args: Vec<String>) -> Result<(Args, Vec<String>), String> {
    let mut host = String::new();
    let mut port = 0;
    let mut ior_file = None;
    let mut rest = Vec::new();
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        let mut value = |opt: &str| iter.next().ok_or_else(|| format!("{opt} needs a value"));
        match arg.as_str() {
            "--host" => host = value("--host")?,
            "--port" => port = value("--port")?.parse().map_err(|_| "bad --port")?,
            "--ior-file" => ior_file = Some(value("--ior-file")?),
            _ => rest.push(arg),
        }
    }
    let ior_file = ior_file.ok_or("--ior-file is required")?;
    Ok((
        Args {
            host,
            port,
            ior_file,
        },
        rest,
    ))
}

fn run() -> Result<(), String> {
    let _ = START.set(Instant::now());
    let _ = MAIN_THREAD.set(thread::current().id());
    let (args, mut orb_args) = parse_args(std::env::args().skip(1).collect())?;
    if args.port != 0 {
        orb_args.extend([
            "-ORBendPoint".to_owned(),
            format!("giop:tcp:{}:{}", args.host, args.port),
        ]);
    }
    let orb = Orb::init(&mut orb_args).map_err(|e| format!("ORB_init: {e}"))?;
    if !orb_args.is_empty() {
        return Err(format!("unknown arguments: {orb_args:?}"));
    }
    let e = |what: &str| {
        let what = what.to_owned();
        move |e: PoaError| format!("{what}: {e}")
    };
    let root = orb.root_poa().map_err(|e| format!("RootPOA: {e}"))?;
    let root_manager = root.the_poa_manager();
    let servant: Arc<dyn Servant> = Arc::new(Target);

    let persistent = |more: &[Policy]| {
        let mut p = vec![
            Policy::Lifespan(Lifespan::Persistent),
            Policy::IdAssignment(IdAssignment::User),
        ];
        p.extend_from_slice(more);
        p
    };
    let create = |parent: &Poa, name: &str, policies: &[Policy]| {
        parent
            .create_poa(name, Some(&root_manager), policies)
            .map_err(e(&format!("create_POA {name}")))
    };

    let mut system_refs = HashMap::new();
    // RootPOA: TRANSIENT, SYSTEM_ID, IMPLICIT_ACTIVATION.
    system_refs.insert(
        "RootPOA".to_owned(),
        root.servant_to_reference(&servant)
            .map_err(e("servant_to_reference"))?,
    );

    let persistent_user = create(&root, "persistent_user", &persistent(&[]))?;
    persistent_user
        .activate_object_with_id(b"obj", Arc::clone(&servant))
        .map_err(e("activate"))?;
    persistent_user
        .activate_object_with_id(b"victim", Arc::new(Target))
        .map_err(e("activate"))?;
    let nested = create(&persistent_user, "nested", &persistent(&[]))?;
    nested
        .activate_object_with_id(b"obj", Arc::clone(&servant))
        .map_err(e("activate"))?;

    let persistent_system = create(
        &root,
        "persistent_system",
        &[Policy::Lifespan(Lifespan::Persistent)],
    )?;
    let oid = persistent_system
        .activate_object(Arc::new(Target))
        .map_err(e("activate"))?;
    system_refs.insert(
        "persistent_system".to_owned(),
        persistent_system
            .id_to_reference(&oid)
            .map_err(e("id_to_reference"))?,
    );

    let transient_user = create(
        &root,
        "transient_user",
        &[Policy::IdAssignment(IdAssignment::User)],
    )?;
    transient_user
        .activate_object_with_id(b"obj", Arc::clone(&servant))
        .map_err(e("activate"))?;

    let transient_system = create(&root, "transient_system", &[])?;
    let oid = transient_system
        .activate_object(Arc::new(Target))
        .map_err(e("activate"))?;
    system_refs.insert(
        "transient_system".to_owned(),
        transient_system
            .id_to_reference(&oid)
            .map_err(e("id_to_reference"))?,
    );

    for name in ["single", "single2"] {
        let poa = create(
            &root,
            name,
            &persistent(&[Policy::Thread(ThreadPolicy::SingleThread)]),
        )?;
        poa.activate_object_with_id(b"a", Arc::clone(&servant))
            .map_err(e("activate"))?;
        poa.activate_object_with_id(b"b", Arc::new(Target))
            .map_err(e("activate"))?;
    }

    let main_poa = create(
        &root,
        "main",
        &persistent(&[Policy::Thread(ThreadPolicy::MainThread)]),
    )?;
    main_poa
        .activate_object_with_id(b"a", Arc::clone(&servant))
        .map_err(e("activate"))?;
    main_poa
        .activate_object_with_id(b"b", Arc::new(Target))
        .map_err(e("activate"))?;

    let managed = root
        .create_poa("managed", None, &persistent(&[]))
        .map_err(e("create_POA managed"))?;
    let managed_manager = managed.the_poa_manager();
    managed
        .activate_object_with_id(b"obj", Arc::clone(&servant))
        .map_err(e("activate"))?;

    let doomed = create(&root, "doomed", &persistent(&[]))?;
    doomed
        .activate_object_with_id(b"obj", Arc::clone(&servant))
        .map_err(e("activate"))?;

    let forward = persistent_user
        .id_to_reference(b"obj")
        .map_err(e("id_to_reference"))?;
    let activator = create(
        &root,
        "activator",
        &persistent(&[Policy::RequestProcessing(
            RequestProcessing::UseServantManager,
        )]),
    )?;
    activator
        .set_servant_manager(ServantManager::Activator(Arc::new(Activator {
            servant: Arc::clone(&servant),
            forward: forward.clone(),
        })))
        .map_err(e("set_servant_manager"))?;
    let locator = create(
        &root,
        "locator",
        &persistent(&[
            Policy::RequestProcessing(RequestProcessing::UseServantManager),
            Policy::ServantRetention(ServantRetention::NonRetain),
        ]),
    )?;
    locator
        .set_servant_manager(ServantManager::Locator(Arc::new(Locator {
            servant: Arc::clone(&servant),
            forward,
        })))
        .map_err(e("set_servant_manager"))?;

    let default = create(
        &root,
        "default",
        &persistent(&[
            Policy::RequestProcessing(RequestProcessing::UseDefaultServant),
            Policy::ServantRetention(ServantRetention::NonRetain),
            Policy::IdUniqueness(IdUniqueness::Multiple),
        ]),
    )?;
    default
        .set_servant(Arc::clone(&servant))
        .map_err(e("set_servant"))?;

    root.set_the_activator(Some(Arc::new(Adapters {
        servant: Arc::clone(&servant),
    })))
    .map_err(e("the_activator"))?;

    // The Control object has its own POAManager, so that it keeps working
    // whatever the probe does to the others.
    let control_poa = root
        .create_poa("control", None, &persistent(&[]))
        .map_err(e("create_POA control"))?;
    let control = Arc::new(Control {
        orb: orb.clone(),
        root: root.clone(),
        managed: managed_manager.clone(),
        system_refs,
    });
    control_poa
        .activate_object_with_id(b"Control", control)
        .map_err(e("activate"))?;
    control_poa
        .the_poa_manager()
        .activate()
        .map_err(e("activate"))?;
    managed_manager.activate().map_err(e("activate"))?;
    root_manager.activate().map_err(e("activate"))?;

    let control_ref = control_poa
        .id_to_reference(b"Control")
        .map_err(e("id_to_reference"))?;
    let ior = orb
        .object_to_string(Some(&control_ref))
        .map_err(|e| e.to_string())?;
    let tmp = format!("{}.tmp", args.ior_file);
    std::fs::write(&tmp, format!("{ior}\n")).map_err(|e| format!("{tmp}: {e}"))?;
    std::fs::rename(&tmp, &args.ior_file).map_err(|e| format!("{}: {e}", args.ior_file))?;
    println!("server ready");

    orb.run();
    println!("server stopped");
    orb.destroy().map_err(|e| format!("destroy: {e}"))?;
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

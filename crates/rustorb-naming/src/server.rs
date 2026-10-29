//! The naming service itself: servants for `NamingContextExt` and
//! `BindingIterator` on rustorb's POA, over a [`Store`].

use std::collections::VecDeque;
use std::collections::hash_map::RandomState;
use std::fmt;
use std::hash::{BuildHasher, Hasher};
use std::io;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use rustorb_core::name::{self, Name, NameComponent};
use rustorb_core::{
    CompletionStatus, Cookie, IdAssignment, Lifespan, ObjectRef, Orb, Poa, PoaCurrent, PoaError,
    Policy, RequestProcessing, Servant, ServantLocator, ServantManager, ServantManagerError,
    ServantRetention, SystemException, SystemExceptionKind, minor,
};

use crate::client::{Binding, BindingType, NamingContext, from_idl_name, to_idl_name};
use crate::idl::cos_naming::{self as idl, naming_context as nc, naming_context_ext as nce};
use crate::repository_id::NAMING_CONTEXT_EXT;
use crate::store::{Change, Snapshot, Store, StoredBinding};
use crate::{NamingError, NotFoundReason};

/// The object key of the root context, as in
/// `corbaloc::host:2809/NameService`, and its id in the store.
pub const ROOT_KEY: &str = "NameService";

/// The POA of the naming contexts, PERSISTENT with the context ids as
/// ObjectIds, so that references to contexts survive a restart.
const CONTEXT_POA: &str = "rustorb-naming";
/// The POA of the binding iterators, which do not survive a restart.
const ITERATOR_POA: &str = "rustorb-naming-iterators";
/// Binding iterators kept at most; beyond that the oldest is destroyed.
const MAX_ITERATORS: usize = 1000;

/// Why a [`NamingService`] could not start.
#[derive(Debug)]
pub enum StartError {
    /// The store could not be read.
    Store(io::Error),
    /// The POAs could not be created.
    Poa(PoaError),
}

impl fmt::Display for StartError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StartError::Store(e) => write!(f, "cannot load the naming data: {e}"),
            StartError::Poa(e) => write!(f, "cannot create the POAs: {e}"),
        }
    }
}

impl std::error::Error for StartError {}

impl From<PoaError> for StartError {
    fn from(e: PoaError) -> Self {
        StartError::Poa(e)
    }
}

impl From<SystemException> for StartError {
    fn from(e: SystemException) -> Self {
        StartError::Poa(PoaError::System(e))
    }
}

/// A CosNaming naming service served by an ORB: the root context and the
/// contexts created from it, with their bindings in a [`Store`].
///
/// The root context answers to the object key `NameService`, so clients
/// find it with `corbaloc::host:port/NameService` as with omniNames. The
/// other contexts live in a PERSISTENT POA: when the ORB listens on the
/// same fixed port after a restart, every reference handed out stays
/// valid. Contexts bound from another naming service are followed by
/// calling them (federation).
///
/// ```no_run
/// use rustorb_core::Orb;
/// use rustorb_naming::{LogStore, NamingService};
///
/// let mut args = vec!["-ORBendPoint".to_owned(), "giop:tcp::2809".to_owned()];
/// let orb = Orb::init(&mut args)?;
/// let service = NamingService::start(&orb, LogStore::new("/var/lib/rustorb-names"))?;
/// println!("{}", orb.object_to_string(Some(service.root_context().object()))?);
/// orb.run();
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone)]
pub struct NamingService {
    service: Arc<Service>,
}

impl NamingService {
    /// Starts the naming service on `orb`, with the state loaded from
    /// `store`. Creates the POAs under the RootPOA (starting the server
    /// side of the ORB) and activates their manager; the ORB serves the
    /// requests from then on, whether or not a thread runs
    /// [`Orb::run`]. The root context is created if the store has none.
    pub fn start(orb: &Orb, mut store: impl Store + 'static) -> Result<NamingService, StartError> {
        let mut snapshot = store.load().map_err(StartError::Store)?;
        let root = orb.root_poa()?;
        let manager = root.the_poa_manager();
        let contexts = root.create_poa(
            CONTEXT_POA,
            Some(&manager),
            &[
                Policy::Lifespan(Lifespan::Persistent),
                Policy::IdAssignment(IdAssignment::User),
                Policy::ServantRetention(ServantRetention::NonRetain),
                Policy::RequestProcessing(RequestProcessing::UseServantManager),
            ],
        )?;
        let iterators = root.create_poa(ITERATOR_POA, Some(&manager), &[])?;
        let root_ref =
            contexts.create_reference_with_id(ROOT_KEY.as_bytes(), NAMING_CONTEXT_EXT)?;
        let key_prefix = profile_key(&root_ref)
            .and_then(|key| key.strip_suffix(ROOT_KEY.as_bytes()).map(<[u8]>::to_vec))
            .expect("a POA reference has an IIOP profile with the ObjectId at the end");
        let root_ref = orb.register_object_key(ROOT_KEY, &root_ref)?;
        if !snapshot.contexts.contains_key(ROOT_KEY) {
            let change = Change::CreateContext(ROOT_KEY.to_owned());
            store.record(&change).map_err(StartError::Store)?;
            snapshot.apply(&change);
        }
        let service = Arc::new(Service {
            orb: orb.clone(),
            contexts: contexts.clone(),
            iterators,
            key_prefix,
            root: root_ref,
            state: Mutex::new(State {
                snapshot,
                store: Box::new(store),
                iterators: VecDeque::new(),
            }),
            next_id: AtomicU64::new(0),
        });
        contexts.set_servant_manager(ServantManager::Locator(Arc::new(Locator {
            service: Arc::downgrade(&service),
        })))?;
        manager.activate()?;
        Ok(NamingService { service })
    }

    /// The root context, with the object key `NameService`.
    pub fn root_context(&self) -> crate::NamingContextExt {
        crate::NamingContextExt::unchecked_narrow(self.service.root.clone())
    }

    /// A copy of the current state.
    pub fn snapshot(&self) -> Snapshot {
        self.service.lock().snapshot.clone()
    }
}

/// The IIOP object key of `obj`.
fn profile_key(obj: &ObjectRef) -> Option<Vec<u8>> {
    obj.ior()
        .iiop_profiles()
        .find_map(Result::ok)
        .map(|p| p.object_key)
}

struct Service {
    orb: Orb,
    contexts: Poa,
    iterators: Poa,
    /// The object key of a context without its id.
    key_prefix: Vec<u8>,
    /// The root context, with the key `NameService`.
    root: ObjectRef,
    state: Mutex<State>,
    next_id: AtomicU64,
}

impl fmt::Debug for Service {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NamingService").finish_non_exhaustive()
    }
}

struct State {
    snapshot: Snapshot,
    store: Box<dyn Store>,
    /// The ObjectIds of the live binding iterators, oldest first.
    iterators: VecDeque<Vec<u8>>,
}

impl State {
    /// Records `changes` in the store and applies them.
    fn update(&mut self, changes: &[Change]) -> Result<(), SystemException> {
        for change in changes {
            self.store.record(change).map_err(|e| {
                eprintln!("rustorb-naming: cannot record an update: {e}");
                SystemException::new(SystemExceptionKind::PERSIST_STORE, 0, CompletionStatus::No)
            })?;
            self.snapshot.apply(change);
        }
        if let Err(e) = self.store.checkpoint(&self.snapshot) {
            // The update itself is in the log; the next checkpoint may work.
            eprintln!("rustorb-naming: cannot write a checkpoint: {e}");
        }
        Ok(())
    }
}

/// `OBJECT_NOT_EXIST` for a context or iterator that does not exist (minor
/// 1, as omniNames answers).
fn no_such_object() -> SystemException {
    SystemException::new(
        SystemExceptionKind::OBJECT_NOT_EXIST,
        minor::NO_SUCH_OBJECT,
        CompletionStatus::No,
    )
}

fn invalid_ior() -> SystemException {
    SystemException::new(SystemExceptionKind::INTERNAL, 0, CompletionStatus::No)
}

/// Where the last component of a name is to be bound or looked up.
enum Target {
    /// In the local context with this id.
    Local(String),
    /// In a context of another naming service, to be called with the rest
    /// of the name.
    Remote(NamingContext, Name),
}

impl Service {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The reference of the local context `id`.
    fn context_ref(&self, id: &str) -> Result<ObjectRef, SystemException> {
        if id == ROOT_KEY {
            return Ok(self.root.clone());
        }
        self.contexts
            .create_reference_with_id(id.as_bytes(), NAMING_CONTEXT_EXT)
            .map_err(|e| match e {
                PoaError::System(e) => e,
                _ => invalid_ior(),
            })
    }

    fn to_ior(&self, obj: Option<&ObjectRef>) -> Result<String, SystemException> {
        self.orb.object_to_string(obj)
    }

    fn object_of(&self, ior: &str) -> Result<Option<ObjectRef>, SystemException> {
        self.orb.string_to_object(ior)
    }

    /// The id of the context `ior` refers to, if it is one of this
    /// service's: an IIOP profile with one of the ORB's published
    /// addresses and the key of a context.
    fn local_id(&self, obj: &ObjectRef) -> Option<String> {
        let endpoints = self.orb.endpoints();
        obj.ior()
            .iiop_profiles()
            .filter_map(Result::ok)
            .filter(|p| {
                endpoints
                    .iter()
                    .any(|(host, port)| *port == p.port && host.eq_ignore_ascii_case(&p.host))
            })
            .find_map(|p| {
                if p.object_key == ROOT_KEY.as_bytes() {
                    return Some(ROOT_KEY.to_owned());
                }
                let id = p.object_key.strip_prefix(&self.key_prefix[..])?;
                String::from_utf8(id.to_vec()).ok()
            })
    }

    /// A new context id: 16 hex digits, from the operating system's random
    /// source and a counter.
    fn new_id(&self, st: &State) -> String {
        loop {
            let mut h = RandomState::new().build_hasher();
            h.write_u64(self.next_id.fetch_add(1, Ordering::Relaxed));
            let id = format!("{:016x}", h.finish());
            if !st.snapshot.contexts.contains_key(&id) {
                return id;
            }
        }
    }

    /// Follows `n` except its last component from the context `id`.
    fn target(&self, st: &State, id: &str, n: &[NameComponent]) -> Result<Target, NamingError> {
        let Some((_, path)) = n.split_last() else {
            return Err(NamingError::InvalidName);
        };
        let mut current = id.to_owned();
        for (i, component) in path.iter().enumerate() {
            let bindings = st
                .snapshot
                .contexts
                .get(&current)
                .ok_or_else(no_such_object)?;
            let binding = bindings
                .get(component)
                .ok_or_else(|| NamingError::NotFound {
                    why: NotFoundReason::MissingNode,
                    rest_of_name: n[i..].to_vec(),
                })?;
            if binding.binding_type == BindingType::Object {
                return Err(NamingError::NotFound {
                    why: NotFoundReason::NotContext,
                    rest_of_name: n[i..].to_vec(),
                });
            }
            let next = self.object_of(&binding.ior)?.ok_or_else(no_such_object)?;
            match self.local_id(&next) {
                Some(next_id) => current = next_id,
                None => {
                    return Ok(Target::Remote(
                        NamingContext::unchecked_narrow(next),
                        n[i + 1..].to_vec(),
                    ));
                }
            }
        }
        if !st.snapshot.contexts.contains_key(&current) {
            return Err(no_such_object().into());
        }
        Ok(Target::Local(current))
    }

    fn bind(
        &self,
        id: &str,
        n: &[NameComponent],
        obj: Option<&ObjectRef>,
        binding_type: BindingType,
        rebind: bool,
    ) -> Result<(), NamingError> {
        let ior = self.to_ior(obj)?;
        let mut st = self.lock();
        let context = match self.target(&st, id, n)? {
            Target::Local(context) => context,
            Target::Remote(ctx, rest) => {
                drop(st);
                let (stub, rest) = (ctx.stub(), to_idl_name(&rest));
                let nc = || obj.map(|o| idl::NamingContext::unchecked_narrow(o.clone()));
                match (binding_type, rebind) {
                    (BindingType::Object, false) => stub.bind(&rest, obj)?,
                    (BindingType::Object, true) => stub.rebind(&rest, obj)?,
                    (BindingType::Context, false) => stub.bind_context(&rest, nc().as_ref())?,
                    (BindingType::Context, true) => stub.rebind_context(&rest, nc().as_ref())?,
                }
                return Ok(());
            }
        };
        let component = n.last().expect("checked by target").clone();
        // omniNames lets rebind and rebind_context replace a binding of
        // the other type too.
        if !rebind && st.snapshot.contexts[&context].contains_key(&component) {
            return Err(NamingError::AlreadyBound);
        }
        st.update(&[Change::Bind {
            context,
            component,
            binding: StoredBinding { binding_type, ior },
        }])?;
        Ok(())
    }

    fn resolve(&self, id: &str, n: &[NameComponent]) -> Result<Option<ObjectRef>, NamingError> {
        let st = self.lock();
        let context = match self.target(&st, id, n)? {
            Target::Local(context) => context,
            Target::Remote(ctx, rest) => {
                drop(st);
                return ctx.resolve(&rest);
            }
        };
        let component = n.last().expect("checked by target");
        let binding = st.snapshot.contexts[&context]
            .get(component)
            .ok_or_else(|| NamingError::NotFound {
                why: NotFoundReason::MissingNode,
                rest_of_name: vec![component.clone()],
            })?;
        Ok(self.object_of(&binding.ior)?)
    }

    fn unbind(&self, id: &str, n: &[NameComponent]) -> Result<(), NamingError> {
        let mut st = self.lock();
        let context = match self.target(&st, id, n)? {
            Target::Local(context) => context,
            Target::Remote(ctx, rest) => {
                drop(st);
                return ctx.unbind(&rest);
            }
        };
        let component = n.last().expect("checked by target").clone();
        if !st.snapshot.contexts[&context].contains_key(&component) {
            return Err(NamingError::NotFound {
                why: NotFoundReason::MissingNode,
                rest_of_name: vec![component],
            });
        }
        st.update(&[Change::Unbind { context, component }])?;
        Ok(())
    }

    fn new_context(&self) -> Result<ObjectRef, NamingError> {
        let mut st = self.lock();
        let id = self.new_id(&st);
        let reference = self.context_ref(&id)?;
        st.update(&[Change::CreateContext(id)])?;
        Ok(reference)
    }

    fn bind_new_context(&self, id: &str, n: &[NameComponent]) -> Result<ObjectRef, NamingError> {
        let mut st = self.lock();
        let context = match self.target(&st, id, n)? {
            Target::Local(context) => context,
            Target::Remote(ctx, rest) => {
                drop(st);
                return Ok(ctx.bind_new_context(&rest)?.into_object());
            }
        };
        let component = n.last().expect("checked by target").clone();
        if st.snapshot.contexts[&context].contains_key(&component) {
            return Err(NamingError::AlreadyBound);
        }
        let new_id = self.new_id(&st);
        let reference = self.context_ref(&new_id)?;
        let ior = self.to_ior(Some(&reference))?;
        st.update(&[
            Change::CreateContext(new_id),
            Change::Bind {
                context,
                component,
                binding: StoredBinding {
                    binding_type: BindingType::Context,
                    ior,
                },
            },
        ])?;
        Ok(reference)
    }

    fn destroy(&self, id: &str) -> Result<(), NamingError> {
        let mut st = self.lock();
        let bindings = st.snapshot.contexts.get(id).ok_or_else(no_such_object)?;
        if !bindings.is_empty() {
            return Err(NamingError::NotEmpty);
        }
        st.update(&[Change::DestroyContext(id.to_owned())])?;
        Ok(())
    }

    /// The bindings of the context `id`, sorted by name.
    fn bindings(&self, id: &str) -> Result<Vec<Binding>, SystemException> {
        let st = self.lock();
        let bindings = st.snapshot.contexts.get(id).ok_or_else(no_such_object)?;
        Ok(bindings
            .iter()
            .map(|(component, binding)| Binding {
                binding_name: vec![component.clone()],
                binding_type: binding.binding_type,
            })
            .collect())
    }

    /// An iterator over `rest`, destroying the oldest one if there are too
    /// many.
    fn new_iterator(self: &Arc<Self>, rest: Vec<Binding>) -> Result<ObjectRef, SystemException> {
        let servant: Arc<dyn Servant> = Arc::new(idl::BindingIteratorPoa(IteratorServant {
            rest: Mutex::new(rest.into()),
            service: Arc::downgrade(self),
        }));
        let poa_error = |e: PoaError| match e {
            PoaError::System(e) => e,
            _ => invalid_ior(),
        };
        let oid = self
            .iterators
            .activate_object(Arc::clone(&servant))
            .map_err(poa_error)?;
        let reference = self.iterators.id_to_reference(&oid).map_err(poa_error)?;
        let evicted = {
            let mut st = self.lock();
            st.iterators.push_back(oid);
            (st.iterators.len() > MAX_ITERATORS)
                .then(|| st.iterators.pop_front())
                .flatten()
        };
        if let Some(old) = evicted {
            let _ = self.iterators.deactivate_object(&old);
        }
        Ok(reference)
    }
}

/// Gives each request to a context the servant for its id, or
/// `OBJECT_NOT_EXIST` if there is no such context.
struct Locator {
    service: std::sync::Weak<Service>,
}

impl ServantLocator for Locator {
    fn preinvoke(
        &self,
        id: &[u8],
        _adapter: &Poa,
        _operation: &str,
    ) -> Result<(Arc<dyn Servant>, Cookie), ServantManagerError> {
        let service = self.service.upgrade().ok_or_else(no_such_object)?;
        let id = std::str::from_utf8(id).map_err(|_| no_such_object())?;
        if !service.lock().snapshot.contexts.contains_key(id) {
            return Err(no_such_object().into());
        }
        let servant = idl::NamingContextExtPoa(ContextServant {
            service,
            id: id.to_owned(),
        });
        Ok((Arc::new(servant), Box::new(())))
    }
}

/// The servant of one context.
struct ContextServant {
    service: Arc<Service>,
    id: String,
}

/// A context reference of the generated code.
fn idl_context(obj: ObjectRef) -> Option<idl::NamingContext> {
    Some(idl::NamingContext::unchecked_narrow(obj))
}

impl idl::NamingContextServant for ContextServant {
    fn bind(&self, n: &[idl::NameComponent], obj: Option<&ObjectRef>) -> Result<(), nc::BindError> {
        let n = from_idl_name(n.to_vec());
        Ok(self
            .service
            .bind(&self.id, &n, obj, BindingType::Object, false)?)
    }

    fn rebind(
        &self,
        n: &[idl::NameComponent],
        obj: Option<&ObjectRef>,
    ) -> Result<(), nc::RebindError> {
        let n = from_idl_name(n.to_vec());
        Ok(self
            .service
            .bind(&self.id, &n, obj, BindingType::Object, true)?)
    }

    fn bind_context(
        &self,
        n: &[idl::NameComponent],
        ctx: Option<&idl::NamingContext>,
    ) -> Result<(), nc::BindContextError> {
        let n = from_idl_name(n.to_vec());
        let obj = ctx.map(idl::NamingContext::object);
        Ok(self
            .service
            .bind(&self.id, &n, obj, BindingType::Context, false)?)
    }

    fn rebind_context(
        &self,
        n: &[idl::NameComponent],
        ctx: Option<&idl::NamingContext>,
    ) -> Result<(), nc::RebindContextError> {
        let n = from_idl_name(n.to_vec());
        let obj = ctx.map(idl::NamingContext::object);
        Ok(self
            .service
            .bind(&self.id, &n, obj, BindingType::Context, true)?)
    }

    fn resolve(&self, n: &[idl::NameComponent]) -> Result<Option<ObjectRef>, nc::ResolveError> {
        Ok(self.service.resolve(&self.id, &from_idl_name(n.to_vec()))?)
    }

    fn unbind(&self, n: &[idl::NameComponent]) -> Result<(), nc::UnbindError> {
        Ok(self.service.unbind(&self.id, &from_idl_name(n.to_vec()))?)
    }

    fn new_context(&self) -> Result<Option<idl::NamingContext>, SystemException> {
        match self.service.new_context() {
            Ok(obj) => Ok(idl_context(obj)),
            Err(NamingError::System(e)) => Err(e),
            Err(_) => Err(unlisted()),
        }
    }

    fn bind_new_context(
        &self,
        n: &[idl::NameComponent],
    ) -> Result<Option<idl::NamingContext>, nc::BindNewContextError> {
        let n = from_idl_name(n.to_vec());
        Ok(idl_context(self.service.bind_new_context(&self.id, &n)?))
    }

    fn destroy(&self) -> Result<(), nc::DestroyError> {
        Ok(self.service.destroy(&self.id)?)
    }

    fn list(
        &self,
        how_many: u32,
    ) -> Result<(idl::BindingList, Option<idl::BindingIterator>), SystemException> {
        let s = &self.service;
        let mut bindings = s.bindings(&self.id)?;
        let rest = bindings.split_off(bindings.len().min(how_many as usize));
        let iterator = if rest.is_empty() {
            None
        } else {
            Some(idl::BindingIterator::unchecked_narrow(
                s.new_iterator(rest)?,
            ))
        };
        Ok((bindings.iter().map(idl::Binding::from).collect(), iterator))
    }
}

impl idl::NamingContextExtServant for ContextServant {
    fn to_string(&self, n: &[idl::NameComponent]) -> Result<String, nce::ToStringError> {
        Ok(name::to_string(&from_idl_name(n.to_vec())).map_err(NamingError::from)?)
    }

    fn to_name(&self, sn: &str) -> Result<idl::Name, nce::ToNameError> {
        let n = name::to_name(sn).map_err(NamingError::from)?;
        Ok(to_idl_name(&n))
    }

    fn to_url(&self, addr: &str, sn: &str) -> Result<String, nce::ToUrlError> {
        Ok(crate::to_url(addr, sn)?)
    }

    fn resolve_str(&self, sn: &str) -> Result<Option<ObjectRef>, nce::ResolveStrError> {
        let n = name::to_name(sn).map_err(NamingError::from)?;
        Ok(self.service.resolve(&self.id, &n)?)
    }
}

/// `UNKNOWN`, for a user exception the operation does not declare.
fn unlisted() -> SystemException {
    SystemException::new(
        SystemExceptionKind::UNKNOWN,
        minor::UNLISTED_USER_EXCEPTION,
        CompletionStatus::Maybe,
    )
}

/// The servant of one binding iterator: the bindings not returned yet.
struct IteratorServant {
    rest: Mutex<VecDeque<Binding>>,
    service: std::sync::Weak<Service>,
}

impl idl::BindingIteratorServant for IteratorServant {
    fn next_one(&self) -> Result<(bool, idl::Binding), SystemException> {
        let next = self.rest.lock().unwrap().pop_front();
        // At the end, an empty name bound to an object, as omniNames
        // returns.
        Ok(match next {
            Some(b) => (true, idl::Binding::from(&b)),
            None => (false, idl::Binding::default()),
        })
    }

    fn next_n(&self, how_many: u32) -> Result<(bool, idl::BindingList), SystemException> {
        let chunk: Vec<idl::Binding> = {
            let mut rest = self.rest.lock().unwrap();
            let n = (how_many as usize).min(rest.len());
            rest.drain(..n).map(|b| idl::Binding::from(&b)).collect()
        };
        Ok((!chunk.is_empty(), chunk))
    }

    fn destroy(&self) -> Result<(), SystemException> {
        let oid = PoaCurrent::get_object_id().map_err(|_| no_such_object())?;
        if let Some(service) = self.service.upgrade() {
            service.lock().iterators.retain(|o| *o != oid);
            service
                .iterators
                .deactivate_object(&oid)
                .map_err(|_| no_such_object())?;
        }
        Ok(())
    }
}

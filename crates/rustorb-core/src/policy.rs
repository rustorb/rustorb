//! `CORBA::ORB::create_policy` and the client side policy overrides of
//! the Messaging specification: `Object::set_policy_overrides`, the ORB's
//! `PolicyManager` and the thread's `PolicyCurrent`.
//!
//! The client side policies rustorb has are
//! `Messaging::RelativeRoundtripTimeoutPolicy`, the standard form of the
//! call timeouts: overriding it on an object reference, on the calling
//! thread or on the ORB sets the same timeout as
//! [`ObjectRef::set_call_timeout`], [`set_thread_call_timeout`] and
//! [`Orb::set_call_timeout`] (see `docs/orb-operations.md`); and
//! `Messaging::SyncScopePolicy`, how far a oneway request goes before the
//! call returns (`docs/sync-scope.md`).

use std::cell::Cell;
use std::fmt;
use std::sync::atomic::{AtomicI32, Ordering};
use std::time::Duration;

use crate::pi::sync_scope::{SYNC_WITH_TARGET, SYNC_WITH_TRANSPORT};
use crate::poa::{
    Bidirectional, IdAssignment, IdUniqueness, ImplicitActivation, Lifespan, Policy,
    RequestProcessing, ServantRetention, ThreadPolicy,
};
use crate::timeout::{duration_to_time_t, time_t_to_duration};
use crate::typecode::TCKind;
use crate::{
    Any, CompletionStatus, ObjectRef, Orb, SystemException, SystemExceptionKind, Value, minor,
    set_thread_call_timeout, thread_call_timeout,
};

/// `CORBA::PolicyType`.
pub type PolicyType = u32;

/// `PortableServer::THREAD_POLICY_ID`; the other POA policies follow it,
/// up to `REQUEST_PROCESSING_POLICY_ID` (22).
pub const THREAD_POLICY_ID: PolicyType = 16;
/// `PortableServer::LIFESPAN_POLICY_ID`.
pub const LIFESPAN_POLICY_ID: PolicyType = 17;
/// `PortableServer::ID_UNIQUENESS_POLICY_ID`.
pub const ID_UNIQUENESS_POLICY_ID: PolicyType = 18;
/// `PortableServer::ID_ASSIGNMENT_POLICY_ID`.
pub const ID_ASSIGNMENT_POLICY_ID: PolicyType = 19;
/// `PortableServer::IMPLICIT_ACTIVATION_POLICY_ID`.
pub const IMPLICIT_ACTIVATION_POLICY_ID: PolicyType = 20;
/// `PortableServer::SERVANT_RETENTION_POLICY_ID`.
pub const SERVANT_RETENTION_POLICY_ID: PolicyType = 21;
/// `PortableServer::REQUEST_PROCESSING_POLICY_ID`.
pub const REQUEST_PROCESSING_POLICY_ID: PolicyType = 22;
/// `Messaging::SYNC_SCOPE_POLICY_TYPE`.
pub const SYNC_SCOPE_POLICY_TYPE: PolicyType = 24;
/// `Messaging::RELATIVE_RT_TIMEOUT_POLICY_TYPE`.
pub const RELATIVE_RT_TIMEOUT_POLICY_TYPE: PolicyType = 32;
/// `BiDirPolicy::BIDIRECTIONAL_POLICY_TYPE`.
pub const BIDIRECTIONAL_POLICY_TYPE: PolicyType = 37;

/// A policy that [`Orb::create_policy`] makes: a POA policy, or a client
/// side policy that can override the defaults of a reference, a thread or
/// the ORB.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PolicyValue {
    /// One of the seven POA policies, for `create_POA`.
    Poa(Policy),
    /// `Messaging::RelativeRoundtripTimeoutPolicy`: how long a call may
    /// take, as a `TimeBase::TimeT` (units of 100 nanoseconds).
    RelativeRoundtripTimeout(u64),
    /// `Messaging::SyncScopePolicy`: how far a oneway request goes before
    /// the call returns, one of the `Messaging::SyncScope` values of
    /// [`pi::sync_scope`](crate::pi::sync_scope) (`SYNC_NONE` to
    /// `SYNC_WITH_TARGET`).
    SyncScope(i16),
}

impl PolicyValue {
    /// The policy type (`policy_type`).
    pub fn policy_type(&self) -> PolicyType {
        match self {
            PolicyValue::Poa(p) => match p {
                Policy::Thread(_) => THREAD_POLICY_ID,
                Policy::Lifespan(_) => LIFESPAN_POLICY_ID,
                Policy::IdUniqueness(_) => ID_UNIQUENESS_POLICY_ID,
                Policy::IdAssignment(_) => ID_ASSIGNMENT_POLICY_ID,
                Policy::ImplicitActivation(_) => IMPLICIT_ACTIVATION_POLICY_ID,
                Policy::ServantRetention(_) => SERVANT_RETENTION_POLICY_ID,
                Policy::RequestProcessing(_) => REQUEST_PROCESSING_POLICY_ID,
                Policy::Bidirectional(_) => BIDIRECTIONAL_POLICY_TYPE,
            },
            PolicyValue::RelativeRoundtripTimeout(_) => RELATIVE_RT_TIMEOUT_POLICY_TYPE,
            PolicyValue::SyncScope(_) => SYNC_SCOPE_POLICY_TYPE,
        }
    }

    /// The value as a number: the enumerator of a POA policy, the
    /// `relative_expiry` of a timeout, the `synchronization` of a
    /// `SyncScopePolicy`. The bindings pass policies as this
    /// pair; [`from_raw`](PolicyValue::from_raw) is the inverse.
    pub fn raw_value(&self) -> u64 {
        match *self {
            PolicyValue::Poa(p) => u64::from(match p {
                Policy::Thread(v) => v as u32,
                Policy::Lifespan(v) => v as u32,
                Policy::IdUniqueness(v) => v as u32,
                Policy::IdAssignment(v) => v as u32,
                Policy::ImplicitActivation(v) => v as u32,
                Policy::ServantRetention(v) => v as u32,
                Policy::RequestProcessing(v) => v as u32,
                Policy::Bidirectional(v) => v as u32,
            }),
            PolicyValue::RelativeRoundtripTimeout(t) => t,
            PolicyValue::SyncScope(v) => v as u64,
        }
    }

    /// The policy of type `policy_type` with the value `value` (see
    /// [`raw_value`](PolicyValue::raw_value)), or `None` if rustorb does
    /// not know the type or the value.
    pub fn from_raw(policy_type: PolicyType, value: u64) -> Option<PolicyValue> {
        if policy_type == RELATIVE_RT_TIMEOUT_POLICY_TYPE {
            return Some(PolicyValue::RelativeRoundtripTimeout(value));
        }
        if policy_type == SYNC_SCOPE_POLICY_TYPE {
            return (value <= SYNC_WITH_TARGET as u64)
                .then_some(PolicyValue::SyncScope(value as i16));
        }
        let v = u32::try_from(value).ok()?;
        let p = match policy_type {
            THREAD_POLICY_ID => Policy::Thread(match v {
                0 => ThreadPolicy::OrbCtrl,
                1 => ThreadPolicy::SingleThread,
                2 => ThreadPolicy::MainThread,
                _ => return None,
            }),
            LIFESPAN_POLICY_ID => Policy::Lifespan(match v {
                0 => Lifespan::Transient,
                1 => Lifespan::Persistent,
                _ => return None,
            }),
            ID_UNIQUENESS_POLICY_ID => Policy::IdUniqueness(match v {
                0 => IdUniqueness::Unique,
                1 => IdUniqueness::Multiple,
                _ => return None,
            }),
            ID_ASSIGNMENT_POLICY_ID => Policy::IdAssignment(match v {
                0 => IdAssignment::User,
                1 => IdAssignment::System,
                _ => return None,
            }),
            IMPLICIT_ACTIVATION_POLICY_ID => Policy::ImplicitActivation(match v {
                0 => ImplicitActivation::Implicit,
                1 => ImplicitActivation::NoImplicit,
                _ => return None,
            }),
            SERVANT_RETENTION_POLICY_ID => Policy::ServantRetention(match v {
                0 => ServantRetention::Retain,
                1 => ServantRetention::NonRetain,
                _ => return None,
            }),
            REQUEST_PROCESSING_POLICY_ID => Policy::RequestProcessing(match v {
                0 => RequestProcessing::ActiveObjectMapOnly,
                1 => RequestProcessing::UseDefaultServant,
                2 => RequestProcessing::UseServantManager,
                _ => return None,
            }),
            BIDIRECTIONAL_POLICY_TYPE => Policy::Bidirectional(match v {
                0 => Bidirectional::Normal,
                1 => Bidirectional::Both,
                _ => return None,
            }),
            _ => return None,
        };
        Some(PolicyValue::Poa(p))
    }

    /// The timeout of a `RelativeRoundtripTimeoutPolicy`.
    pub fn relative_expiry(&self) -> Option<Duration> {
        match *self {
            PolicyValue::RelativeRoundtripTimeout(t) => Some(time_t_to_duration(t)),
            _ => None,
        }
    }

    /// The `synchronization` of a `SyncScopePolicy`.
    pub fn synchronization(&self) -> Option<i16> {
        match *self {
            PolicyValue::SyncScope(v) => Some(v),
            _ => None,
        }
    }

    /// The repository id of the policy's interface.
    pub fn repository_id(&self) -> &'static str {
        match self.policy_type() {
            THREAD_POLICY_ID => "IDL:omg.org/PortableServer/ThreadPolicy:2.3",
            LIFESPAN_POLICY_ID => "IDL:omg.org/PortableServer/LifespanPolicy:2.3",
            ID_UNIQUENESS_POLICY_ID => "IDL:omg.org/PortableServer/IdUniquenessPolicy:2.3",
            ID_ASSIGNMENT_POLICY_ID => "IDL:omg.org/PortableServer/IdAssignmentPolicy:2.3",
            IMPLICIT_ACTIVATION_POLICY_ID => {
                "IDL:omg.org/PortableServer/ImplicitActivationPolicy:2.3"
            }
            SERVANT_RETENTION_POLICY_ID => "IDL:omg.org/PortableServer/ServantRetentionPolicy:2.3",
            REQUEST_PROCESSING_POLICY_ID => {
                "IDL:omg.org/PortableServer/RequestProcessingPolicy:2.3"
            }
            BIDIRECTIONAL_POLICY_TYPE => "IDL:omg.org/BiDirPolicy/BidirectionalPolicy:1.0",
            SYNC_SCOPE_POLICY_TYPE => "IDL:omg.org/Messaging/SyncScopePolicy:1.0",
            _ => "IDL:omg.org/Messaging/RelativeRoundtripTimeoutPolicy:1.0",
        }
    }
}

/// `CORBA::PolicyError`, raised by [`Orb::create_policy`]: the
/// `PolicyErrorCode` in `reason`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PolicyError {
    /// One of the `*_POLICY*` constants of this type.
    pub reason: i16,
}

impl PolicyError {
    /// `BAD_POLICY`: the ORB does not know the policy type.
    pub const BAD_POLICY: i16 = 0;
    /// `UNSUPPORTED_POLICY`: the ORB knows the type but does not support
    /// it.
    pub const UNSUPPORTED_POLICY: i16 = 1;
    /// `BAD_POLICY_TYPE`: the any has the wrong type for the policy.
    pub const BAD_POLICY_TYPE: i16 = 2;
    /// `BAD_POLICY_VALUE`: the value is not one of the policy's values.
    pub const BAD_POLICY_VALUE: i16 = 3;
    /// `UNSUPPORTED_POLICY_VALUE`: the ORB does not support the value.
    pub const UNSUPPORTED_POLICY_VALUE: i16 = 4;
}

impl fmt::Display for PolicyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CORBA::PolicyError (reason {})", self.reason)
    }
}

impl std::error::Error for PolicyError {}

/// `CORBA::SetOverrideType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SetOverrideType {
    /// `SET_OVERRIDE`: the policies replace every override.
    Set,
    /// `ADD_OVERRIDE`: the policies replace the overrides of their types
    /// and keep the others.
    Add,
}

/// The repository id of the enum a POA policy's value has.
fn poa_value_id(policy_type: PolicyType) -> Option<&'static str> {
    Some(match policy_type {
        THREAD_POLICY_ID => "IDL:omg.org/PortableServer/ThreadPolicyValue:2.3",
        LIFESPAN_POLICY_ID => "IDL:omg.org/PortableServer/LifespanPolicyValue:2.3",
        ID_UNIQUENESS_POLICY_ID => "IDL:omg.org/PortableServer/IdUniquenessPolicyValue:2.3",
        ID_ASSIGNMENT_POLICY_ID => "IDL:omg.org/PortableServer/IdAssignmentPolicyValue:2.3",
        IMPLICIT_ACTIVATION_POLICY_ID => {
            "IDL:omg.org/PortableServer/ImplicitActivationPolicyValue:2.3"
        }
        SERVANT_RETENTION_POLICY_ID => "IDL:omg.org/PortableServer/ServantRetentionPolicyValue:2.3",
        REQUEST_PROCESSING_POLICY_ID => {
            "IDL:omg.org/PortableServer/RequestProcessingPolicyValue:2.3"
        }
        _ => return None,
    })
}

/// `create_policy` without the ORB: see [`Orb::create_policy`].
pub fn create_policy(policy_type: PolicyType, value: &Any) -> Result<PolicyValue, PolicyError> {
    let tc = value.typecode().unaliased();
    let raw = if policy_type == RELATIVE_RT_TIMEOUT_POLICY_TYPE {
        // TimeBase::TimeT, an alias of unsigned long long.
        match (tc.kind(), value.value()) {
            (TCKind::ULongLong, Value::ULongLong(t)) => *t,
            _ => {
                return Err(PolicyError {
                    reason: PolicyError::BAD_POLICY_TYPE,
                });
            }
        }
    } else if policy_type == SYNC_SCOPE_POLICY_TYPE {
        // Messaging::SyncScope, an alias of short.
        match (tc.kind(), value.value()) {
            (TCKind::Short, Value::Short(v)) => match u64::try_from(*v) {
                Ok(v) => v,
                Err(_) => {
                    return Err(PolicyError {
                        reason: PolicyError::BAD_POLICY_VALUE,
                    });
                }
            },
            _ => {
                return Err(PolicyError {
                    reason: PolicyError::BAD_POLICY_TYPE,
                });
            }
        }
    } else if policy_type == BIDIRECTIONAL_POLICY_TYPE {
        // BiDirPolicy::BidirectionalPolicyValue, an alias of unsigned
        // short.
        match (tc.kind(), value.value()) {
            (TCKind::UShort, Value::UShort(v)) => u64::from(*v),
            _ => {
                return Err(PolicyError {
                    reason: PolicyError::BAD_POLICY_TYPE,
                });
            }
        }
    } else if let Some(id) = poa_value_id(policy_type) {
        // The enum of the policy; its version differs between the IDL
        // of CORBA 2.3 and later (and between ORBs), so it is compared
        // without the version.
        let base = |s: &str| {
            s.rsplit_once(':')
                .map_or(s.to_owned(), |(b, _)| b.to_owned())
        };
        match (tc.kind(), value.value()) {
            (TCKind::Enum, Value::Enum(v)) if tc.id().is_ok_and(|i| base(i) == base(id)) => {
                u64::from(*v)
            }
            _ => {
                return Err(PolicyError {
                    reason: PolicyError::BAD_POLICY_TYPE,
                });
            }
        }
    } else {
        return Err(PolicyError {
            reason: PolicyError::BAD_POLICY,
        });
    };
    PolicyValue::from_raw(policy_type, raw).ok_or(PolicyError {
        reason: PolicyError::BAD_POLICY_VALUE,
    })
}

/// `NO_PERMISSION`, for a policy that cannot override at the scope (CORBA
/// 3.0, 4.9.3.1).
fn no_permission() -> SystemException {
    SystemException::new(SystemExceptionKind::NO_PERMISSION, 0, CompletionStatus::No)
}

/// The client policies one scope (a reference, a thread or the ORB)
/// overrides: the timeout as a `TimeBase::TimeT`, and the
/// `Messaging::SyncScope`.
#[derive(Debug, Clone, Copy, Default)]
struct Scope {
    timeout: Option<u64>,
    sync_scope: Option<i16>,
}

/// The policies the overrides `policies` set on top of `current` (those of
/// the scope).
fn apply_overrides(
    current: Scope,
    policies: &[PolicyValue],
    set_add: SetOverrideType,
) -> Result<Scope, SystemException> {
    check_duplicates(policies)?;
    let mut scope = match set_add {
        SetOverrideType::Set => Scope::default(),
        SetOverrideType::Add => current,
    };
    for p in policies {
        match p {
            PolicyValue::RelativeRoundtripTimeout(t) => scope.timeout = Some(*t),
            PolicyValue::SyncScope(v) => scope.sync_scope = Some(*v),
            PolicyValue::Poa(_) => return Err(no_permission()),
        }
    }
    Ok(scope)
}

/// `BAD_PARAM` when two policies of `policies` have the same type.
fn check_duplicates(policies: &[PolicyValue]) -> Result<(), SystemException> {
    for (i, p) in policies.iter().enumerate() {
        if policies[..i]
            .iter()
            .any(|q| q.policy_type() == p.policy_type())
        {
            return Err(SystemException::new(
                SystemExceptionKind::BAD_PARAM,
                minor::DUPLICATE_POLICY,
                CompletionStatus::No,
            ));
        }
    }
    Ok(())
}

/// The value of a `BidirectionalPolicy`.
fn bidirectional(p: &PolicyValue) -> Option<Bidirectional> {
    match p {
        PolicyValue::Poa(Policy::Bidirectional(v)) => Some(*v),
        _ => None,
    }
}

/// The overrides of the types `types` (every type when empty) of a scope.
fn overrides(scope: Scope, types: &[PolicyType]) -> Vec<PolicyValue> {
    let wanted = |t: PolicyType| types.is_empty() || types.contains(&t);
    let mut out = Vec::new();
    if let Some(t) = scope
        .timeout
        .filter(|_| wanted(RELATIVE_RT_TIMEOUT_POLICY_TYPE))
    {
        out.push(PolicyValue::RelativeRoundtripTimeout(t));
    }
    if let Some(v) = scope.sync_scope.filter(|_| wanted(SYNC_SCOPE_POLICY_TYPE)) {
        out.push(PolicyValue::SyncScope(v));
    }
    out
}

/// A stored timeout (`Some(None)` is "no limit") as a `relative_expiry`,
/// if it is one a policy can say.
fn as_time_t(timeout: Option<Option<Duration>>) -> Option<u64> {
    timeout.flatten().map(duration_to_time_t)
}

/// A `Messaging::SyncScope` that is not set, in an atomic.
const SYNC_SCOPE_UNSET: i32 = -1;

/// A sync scope override stored in an atomic.
pub(crate) fn new_sync_scope_cell(v: Option<i16>) -> AtomicI32 {
    AtomicI32::new(v.map_or(SYNC_SCOPE_UNSET, i32::from))
}

/// The sync scope override of `cell`.
pub(crate) fn load_sync_scope(cell: &AtomicI32) -> Option<i16> {
    i16::try_from(cell.load(Ordering::Relaxed))
        .ok()
        .filter(|v| *v >= 0)
}

/// Sets the sync scope override of `cell`.
pub(crate) fn store_sync_scope(cell: &AtomicI32, v: Option<i16>) {
    cell.store(v.map_or(SYNC_SCOPE_UNSET, i32::from), Ordering::Relaxed);
}

thread_local! {
    static THREAD_SYNC_SCOPE: Cell<Option<i16>> = const { Cell::new(None) };
}

/// The `SyncScopePolicy` override of the calling thread.
pub(crate) fn thread_sync_scope() -> Option<i16> {
    THREAD_SYNC_SCOPE.with(Cell::get)
}

/// The scope of the calling thread.
fn thread_scope() -> Scope {
    Scope {
        timeout: as_time_t(thread_call_timeout()),
        sync_scope: thread_sync_scope(),
    }
}

/// `PolicyCurrent::set_policy_overrides`: the overrides of the calling
/// thread, for every ORB. A `RelativeRoundtripTimeoutPolicy` sets the
/// thread's call timeout ([`set_thread_call_timeout`]); without one (after
/// `SET_OVERRIDE`) the thread has none. A `SyncScopePolicy` sets how far
/// the thread's oneway requests go. `NO_PERMISSION` for a POA policy.
pub fn set_thread_policy_overrides(
    policies: &[PolicyValue],
    set_add: SetOverrideType,
) -> Result<(), SystemException> {
    let scope = apply_overrides(thread_scope(), policies, set_add)?;
    set_thread_call_timeout(scope.timeout.map(|t| Some(time_t_to_duration(t))));
    THREAD_SYNC_SCOPE.with(|c| c.set(scope.sync_scope));
    Ok(())
}

/// `PolicyCurrent::get_policy_overrides`: the overrides of the calling
/// thread of the types `types`, or all of them when `types` is empty.
pub fn thread_policy_overrides(types: &[PolicyType]) -> Vec<PolicyValue> {
    overrides(thread_scope(), types)
}

impl Orb {
    /// `CORBA::ORB::create_policy`: a POA policy from its enumerator, a
    /// `RelativeRoundtripTimeoutPolicy` from a `TimeBase::TimeT` (an
    /// `unsigned long long`, in units of 100 nanoseconds), or a
    /// `SyncScopePolicy` from a `Messaging::SyncScope` (a `short`).
    ///
    /// `PolicyError` with `BAD_POLICY` for a type rustorb does not know,
    /// `BAD_POLICY_TYPE` when the any does not hold the policy's type, and
    /// `BAD_POLICY_VALUE` for an enumerator or a sync scope out of range.
    pub fn create_policy(
        &self,
        policy_type: PolicyType,
        value: &Any,
    ) -> Result<PolicyValue, PolicyError> {
        create_policy(policy_type, value)
    }

    /// The client policies the ORB overrides.
    fn scope(&self) -> Scope {
        Scope {
            timeout: self.call_timeout().map(duration_to_time_t),
            sync_scope: load_sync_scope(&self.inner.sync_scope),
        }
    }

    /// `PolicyManager::set_policy_overrides` of the ORB (the initial
    /// reference `ORBPolicyManager`): a `RelativeRoundtripTimeoutPolicy`
    /// sets [`call_timeout`](Orb::call_timeout); without one (after
    /// `SET_OVERRIDE`) calls have no limit. A `SyncScopePolicy` sets how
    /// far oneway requests go; without one, they return once written
    /// (`SYNC_WITH_TRANSPORT`). A `BidirectionalPolicy` of
    /// `BOTH` makes the ORB offer and accept bidirectional GIOP, like
    /// `-ORBofferBiDirectionalGIOP 1 -ORBacceptBiDirectionalGIOP 1`, for
    /// the connections opened or accepted from then on
    /// (docs/bidir-giop.md). `NO_PERMISSION` for the other POA policies.
    pub fn set_policy_overrides(
        &self,
        policies: &[PolicyValue],
        set_add: SetOverrideType,
    ) -> Result<(), SystemException> {
        check_duplicates(policies)?;
        let others: Vec<PolicyValue> = policies
            .iter()
            .filter(|p| bidirectional(p).is_none())
            .copied()
            .collect();
        let scope = apply_overrides(self.scope(), &others, set_add)?;
        self.set_call_timeout(scope.timeout.map(time_t_to_duration));
        store_sync_scope(&self.inner.sync_scope, scope.sync_scope);
        let bidir = policies.iter().find_map(bidirectional);
        let mut stored = self.inner.bidir_override.lock().unwrap();
        match (bidir, set_add) {
            (Some(v), _) => *stored = Some(v),
            (None, SetOverrideType::Set) => *stored = None,
            (None, SetOverrideType::Add) => {}
        }
        Ok(())
    }

    /// `PolicyManager::get_policy_overrides` of the ORB.
    pub fn policy_overrides(&self, types: &[PolicyType]) -> Vec<PolicyValue> {
        let mut out = overrides(self.scope(), types);
        if types.is_empty() || types.contains(&BIDIRECTIONAL_POLICY_TYPE) {
            if let Some(v) = *self.inner.bidir_override.lock().unwrap() {
                out.push(PolicyValue::Poa(Policy::Bidirectional(v)));
            }
        }
        out
    }
}

impl ObjectRef {
    /// The client policies the reference overrides.
    fn scope(&self) -> Scope {
        Scope {
            timeout: as_time_t(self.call_timeout()),
            sync_scope: self.sync_scope_override(),
        }
    }

    /// `CORBA::Object::set_policy_overrides`: a new reference to the same
    /// object with these overrides, whose call timeout and sync scope are
    /// its own (this reference is unchanged). A
    /// `RelativeRoundtripTimeoutPolicy` sets the new reference's
    /// [`call_timeout`](ObjectRef::set_call_timeout); a `SyncScopePolicy`
    /// how far its oneway requests go. `NO_PERMISSION` for a POA policy.
    pub fn set_policy_overrides(
        &self,
        policies: &[PolicyValue],
        set_add: SetOverrideType,
    ) -> Result<ObjectRef, SystemException> {
        let scope = apply_overrides(self.scope(), policies, set_add)?;
        let obj = self.with_own_settings();
        obj.set_call_timeout(scope.timeout.map(|t| Some(time_t_to_duration(t))));
        obj.set_sync_scope_override(scope.sync_scope);
        Ok(obj)
    }

    /// `CORBA::Object::get_policy_overrides`: the overrides of this
    /// reference of the types `types`, or all of them when `types` is
    /// empty.
    pub fn policy_overrides(&self, types: &[PolicyType]) -> Vec<PolicyValue> {
        overrides(self.scope(), types)
    }

    /// `CORBA::Object::get_client_policy`, and `get_policy` (rustorb reads
    /// no policies from IORs): the policy of type `policy_type` that
    /// applies to calls from this thread, from the reference's overrides,
    /// else the thread's, else the ORB's. `None` when no scope has one,
    /// which for the timeout means calls have no limit, and for the sync
    /// scope that oneway requests return once written
    /// (`SYNC_WITH_TRANSPORT`).
    pub fn client_policy(&self, policy_type: PolicyType) -> Option<PolicyValue> {
        match policy_type {
            RELATIVE_RT_TIMEOUT_POLICY_TYPE => self
                .call_timeout()
                .or_else(thread_call_timeout)
                .unwrap_or_else(|| self.orb().call_timeout())
                .map(|t| PolicyValue::RelativeRoundtripTimeout(duration_to_time_t(t))),
            SYNC_SCOPE_POLICY_TYPE => self
                .sync_scope_override()
                .or_else(thread_sync_scope)
                .or_else(|| load_sync_scope(&self.orb().inner.sync_scope))
                .map(PolicyValue::SyncScope),
            _ => None,
        }
    }

    /// The `Messaging::SyncScope` of a oneway request on this reference
    /// from the calling thread: [`client_policy`](ObjectRef::client_policy),
    /// else `SYNC_WITH_TRANSPORT`.
    pub(crate) fn effective_sync_scope(&self) -> i16 {
        self.client_policy(SYNC_SCOPE_POLICY_TYPE)
            .and_then(|p| p.synchronization())
            .unwrap_or(SYNC_WITH_TRANSPORT)
    }
}

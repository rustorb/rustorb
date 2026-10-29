//! `CORBA::ORB::create_policy` and the client side policy overrides of
//! the Messaging specification: `Object::set_policy_overrides`, the ORB's
//! `PolicyManager` and the thread's `PolicyCurrent`.
//!
//! The one client side policy rustorb has is
//! `Messaging::RelativeRoundtripTimeoutPolicy`, the standard form of the
//! call timeouts: overriding it on an object reference, on the calling
//! thread or on the ORB sets the same timeout as
//! [`ObjectRef::set_call_timeout`], [`set_thread_call_timeout`] and
//! [`Orb::set_call_timeout`] (see `docs/orb-operations.md`).

use std::fmt;
use std::time::Duration;

use crate::poa::{
    IdAssignment, IdUniqueness, ImplicitActivation, Lifespan, Policy, RequestProcessing,
    ServantRetention, ThreadPolicy,
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
/// `Messaging::RELATIVE_RT_TIMEOUT_POLICY_TYPE`.
pub const RELATIVE_RT_TIMEOUT_POLICY_TYPE: PolicyType = 32;

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
            },
            PolicyValue::RelativeRoundtripTimeout(_) => RELATIVE_RT_TIMEOUT_POLICY_TYPE,
        }
    }

    /// The value as a number: the enumerator of a POA policy, the
    /// `relative_expiry` of a timeout. The bindings pass policies as this
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
            }),
            PolicyValue::RelativeRoundtripTimeout(t) => t,
        }
    }

    /// The policy of type `policy_type` with the value `value` (see
    /// [`raw_value`](PolicyValue::raw_value)), or `None` if rustorb does
    /// not know the type or the value.
    pub fn from_raw(policy_type: PolicyType, value: u64) -> Option<PolicyValue> {
        if policy_type == RELATIVE_RT_TIMEOUT_POLICY_TYPE {
            return Some(PolicyValue::RelativeRoundtripTimeout(value));
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
            _ => return None,
        };
        Some(PolicyValue::Poa(p))
    }

    /// The timeout of a `RelativeRoundtripTimeoutPolicy`.
    pub fn relative_expiry(&self) -> Option<Duration> {
        match *self {
            PolicyValue::RelativeRoundtripTimeout(t) => Some(time_t_to_duration(t)),
            PolicyValue::Poa(_) => None,
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

/// The timeout the overrides `policies` set on top of `current` (the
/// timeout of the scope; `None` when it has none).
fn apply_overrides(
    current: Option<u64>,
    policies: &[PolicyValue],
    set_add: SetOverrideType,
) -> Result<Option<u64>, SystemException> {
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
    let mut timeout = match set_add {
        SetOverrideType::Set => None,
        SetOverrideType::Add => current,
    };
    for p in policies {
        match p {
            PolicyValue::RelativeRoundtripTimeout(t) => timeout = Some(*t),
            PolicyValue::Poa(_) => return Err(no_permission()),
        }
    }
    Ok(timeout)
}

/// The overrides of the types `types` (every type when empty) at a scope
/// whose timeout is `timeout`.
fn overrides(timeout: Option<u64>, types: &[PolicyType]) -> Vec<PolicyValue> {
    let wanted = types.is_empty() || types.contains(&RELATIVE_RT_TIMEOUT_POLICY_TYPE);
    timeout
        .filter(|_| wanted)
        .map(PolicyValue::RelativeRoundtripTimeout)
        .into_iter()
        .collect()
}

/// A stored timeout (`Some(None)` is "no limit") as a `relative_expiry`,
/// if it is one a policy can say.
fn as_time_t(timeout: Option<Option<Duration>>) -> Option<u64> {
    timeout.flatten().map(duration_to_time_t)
}

/// `PolicyCurrent::set_policy_overrides`: the overrides of the calling
/// thread, for every ORB. A `RelativeRoundtripTimeoutPolicy` sets the
/// thread's call timeout ([`set_thread_call_timeout`]); without one (after
/// `SET_OVERRIDE`) the thread has none. `NO_PERMISSION` for a POA policy.
pub fn set_thread_policy_overrides(
    policies: &[PolicyValue],
    set_add: SetOverrideType,
) -> Result<(), SystemException> {
    let t = apply_overrides(as_time_t(thread_call_timeout()), policies, set_add)?;
    set_thread_call_timeout(t.map(|t| Some(time_t_to_duration(t))));
    Ok(())
}

/// `PolicyCurrent::get_policy_overrides`: the overrides of the calling
/// thread of the types `types`, or all of them when `types` is empty.
pub fn thread_policy_overrides(types: &[PolicyType]) -> Vec<PolicyValue> {
    overrides(as_time_t(thread_call_timeout()), types)
}

impl Orb {
    /// `CORBA::ORB::create_policy`: a POA policy from its enumerator, or a
    /// `RelativeRoundtripTimeoutPolicy` from a `TimeBase::TimeT` (an
    /// `unsigned long long`, in units of 100 nanoseconds).
    ///
    /// `PolicyError` with `BAD_POLICY` for a type rustorb does not know,
    /// `BAD_POLICY_TYPE` when the any does not hold the policy's type, and
    /// `BAD_POLICY_VALUE` for an enumerator out of range.
    pub fn create_policy(
        &self,
        policy_type: PolicyType,
        value: &Any,
    ) -> Result<PolicyValue, PolicyError> {
        create_policy(policy_type, value)
    }

    /// `PolicyManager::set_policy_overrides` of the ORB (the initial
    /// reference `ORBPolicyManager`): a `RelativeRoundtripTimeoutPolicy`
    /// sets [`call_timeout`](Orb::call_timeout); without one (after
    /// `SET_OVERRIDE`) calls have no limit. `NO_PERMISSION` for a POA
    /// policy.
    pub fn set_policy_overrides(
        &self,
        policies: &[PolicyValue],
        set_add: SetOverrideType,
    ) -> Result<(), SystemException> {
        let current = self.call_timeout().map(duration_to_time_t);
        let t = apply_overrides(current, policies, set_add)?;
        self.set_call_timeout(t.map(time_t_to_duration));
        Ok(())
    }

    /// `PolicyManager::get_policy_overrides` of the ORB.
    pub fn policy_overrides(&self, types: &[PolicyType]) -> Vec<PolicyValue> {
        overrides(self.call_timeout().map(duration_to_time_t), types)
    }
}

impl ObjectRef {
    /// `CORBA::Object::set_policy_overrides`: a new reference to the same
    /// object with these overrides, whose call timeout is its own (this
    /// reference is unchanged). A `RelativeRoundtripTimeoutPolicy` sets
    /// the new reference's [`call_timeout`](ObjectRef::set_call_timeout).
    /// `NO_PERMISSION` for a POA policy.
    pub fn set_policy_overrides(
        &self,
        policies: &[PolicyValue],
        set_add: SetOverrideType,
    ) -> Result<ObjectRef, SystemException> {
        let t = apply_overrides(as_time_t(self.call_timeout()), policies, set_add)?;
        let obj = self.with_own_settings();
        obj.set_call_timeout(t.map(|t| Some(time_t_to_duration(t))));
        Ok(obj)
    }

    /// `CORBA::Object::get_policy_overrides`: the overrides of this
    /// reference of the types `types`, or all of them when `types` is
    /// empty.
    pub fn policy_overrides(&self, types: &[PolicyType]) -> Vec<PolicyValue> {
        overrides(as_time_t(self.call_timeout()), types)
    }

    /// `CORBA::Object::get_client_policy`, and `get_policy` (rustorb reads
    /// no policies from IORs): the policy of type `policy_type` that
    /// applies to calls from this thread, from the reference's overrides,
    /// else the thread's, else the ORB's. `None` when no scope has one,
    /// which for the timeout means calls have no limit.
    pub fn client_policy(&self, policy_type: PolicyType) -> Option<PolicyValue> {
        if policy_type != RELATIVE_RT_TIMEOUT_POLICY_TYPE {
            return None;
        }
        self.call_timeout()
            .or_else(thread_call_timeout)
            .unwrap_or_else(|| self.orb().call_timeout())
            .map(|t| PolicyValue::RelativeRoundtripTimeout(duration_to_time_t(t)))
    }
}

//! Valuetypes and value boxes of generated code.
//!
//! A valuetype instance is shared: a [`ValueHandle`] is a reference to it,
//! and the handles the generator makes for each valuetype wrap one. The
//! state is a generated struct ([`ValueState`]) behind a mutex. An instance
//! written twice in a message is sent once and then as an indirection, and
//! is read back as one instance; instances may refer to each other in
//! cycles, which keep them alive until the cycle is broken.

use std::any::{Any, TypeId};
use std::borrow::Borrow;
use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard, RwLock};

use rustorb_cdr::ValueHeader;

use super::{IdlType, Sys};
use crate::{InStream, OutStream, TypeCode};

/// What the generated code tells the runtime about a valuetype.
#[derive(Debug)]
pub struct ValueInfo {
    /// The repository id.
    pub id: &'static str,
    /// The repository ids a value is sent with: its own, then those of
    /// the base types it is truncatable to. A value with more than one is
    /// sent chunked.
    pub sent_ids: &'static [&'static str],
    /// The repository ids of the type and of all its concrete base types:
    /// where a value of this type may stand.
    pub ancestors: &'static [&'static str],
    /// Makes an instance with the default state, which is then read.
    pub make: fn() -> ValueHandle,
    /// The TypeCode.
    pub type_code: fn() -> TypeCode,
}

/// The state of a valuetype instance: the struct the generator makes for
/// each valuetype, with the state of its base type as a member.
pub trait ValueState: Send + 'static {
    /// The type of the instance.
    fn info(&self) -> &'static ValueInfo;
    /// Writes the state members, those of the base types first.
    fn write_state(&self, out: &mut OutStream<'_>) -> Sys<()>;
    /// Reads the state members, those of the base types first.
    fn read_state(&mut self, input: &mut InStream<'_>) -> Sys<()>;
    /// The state of the type `ty` (this type or a base type) in this one.
    fn view(&self, ty: TypeId) -> Option<&dyn Any>;
    /// The same, to change.
    fn view_mut(&mut self, ty: TypeId) -> Option<&mut dyn Any>;
}

struct ValueObj<S: ?Sized> {
    info: &'static ValueInfo,
    state: Mutex<S>,
}

/// A shared reference to a valuetype instance.
#[derive(Clone)]
pub struct ValueHandle(Arc<ValueObj<dyn ValueState>>);

impl fmt::Debug for ValueHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Not the state, which may refer back to this instance.
        write!(f, "ValueHandle({} @ {:#x})", self.0.info.id, self.key())
    }
}

impl ValueHandle {
    /// A new instance with the state `state`.
    pub fn new<S: ValueState>(state: S) -> ValueHandle {
        let info = state.info();
        let obj: Arc<ValueObj<dyn ValueState>> = Arc::new(ValueObj {
            info,
            state: Mutex::new(state),
        });
        ValueHandle(obj)
    }

    /// The type of the instance.
    pub fn info(&self) -> &'static ValueInfo {
        self.0.info
    }

    /// Whether the instance is of the valuetype `id` or of a type derived
    /// from it.
    pub fn is_a(&self, id: &str) -> bool {
        self.0.info.ancestors.contains(&id)
    }

    /// Whether both are the same instance.
    pub fn ptr_eq(&self, other: &ValueHandle) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    /// A number that identifies the instance while it is alive.
    pub fn key(&self) -> u64 {
        Arc::as_ptr(&self.0) as *const () as usize as u64
    }

    fn lock(&self) -> MutexGuard<'_, dyn ValueState> {
        self.0.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Runs `f` on the state `S` of the instance (its own, or that of a
    /// base type). Panics if the instance has no state `S`, which the
    /// generated handles rule out. Touching the same instance in `f`
    /// deadlocks.
    pub fn with<S: 'static, R>(&self, f: impl FnOnce(&S) -> R) -> R {
        let guard = self.lock();
        let state = guard
            .view(TypeId::of::<S>())
            .and_then(|s| s.downcast_ref::<S>())
            .expect("the state of a base type");
        f(state)
    }

    /// Runs `f` on the state `S` of the instance, to change it.
    pub fn with_mut<S: 'static, R>(&self, f: impl FnOnce(&mut S) -> R) -> R {
        let mut guard = self.lock();
        let state = guard
            .view_mut(TypeId::of::<S>())
            .and_then(|s| s.downcast_mut::<S>())
            .expect("the state of a base type");
        f(state)
    }

    /// Writes `v`, or `null`.
    pub fn write(v: Option<&ValueHandle>, out: &mut OutStream<'_>) -> Sys<()> {
        let Some(v) = v else {
            out.write_null_value();
            return Ok(());
        };
        let ids = v.0.info.sent_ids;
        // The key is checked before the state is locked, so that an
        // instance that refers to itself is written as an indirection.
        if !out.begin_value(v.key(), ids, ids.len() > 1)? {
            return Ok(());
        }
        v.lock().write_state(out)?;
        out.end_value()?;
        Ok(())
    }

    /// Reads a value where one of the valuetype `expected` (or `null`) is.
    ///
    /// A value is read as the first of the types it was sent with that is
    /// `expected` or a type registered with [`register_value`] derived
    /// from it, and is `MARSHAL` if there is none.
    pub fn read(
        input: &mut InStream<'_>,
        expected: &'static ValueInfo,
    ) -> Sys<Option<ValueHandle>> {
        let e = input.marshal_error();
        let header = input.read_value_header().map_err(|_| e.clone())?;
        let (slot, ids) = match header {
            ValueHeader::Null => return Ok(None),
            ValueHeader::Indirection(slot) => {
                let v = input
                    .typed_values
                    .get(slot)
                    .and_then(|v| v.as_ref())
                    .and_then(|v| v.downcast_ref::<ValueHandle>())
                    .filter(|v| v.is_a(expected.id))
                    .ok_or(e)?;
                return Ok(Some(v.clone()));
            }
            ValueHeader::Value { slot, ids, .. } => (slot, ids),
        };
        let info = if ids.is_empty() {
            expected
        } else {
            ids.iter()
                .find_map(|id| {
                    if id == expected.id {
                        Some(expected)
                    } else {
                        lookup(id).filter(|i| i.ancestors.contains(&expected.id))
                    }
                })
                .ok_or_else(|| e.clone())?
        };
        let v = (info.make)();
        set_slot(input, slot, Box::new(v.clone()));
        v.lock().read_state(input)?;
        input.end_value().map_err(|_| e)?;
        Ok(Some(v))
    }
}

fn set_slot(input: &mut InStream<'_>, slot: usize, v: Box<dyn Any>) {
    if input.typed_values.len() <= slot {
        input.typed_values.resize_with(slot + 1, || None);
    }
    input.typed_values[slot] = Some(v);
}

/// The valuetypes known to the readers of generated code, by repository
/// id.
static VALUES: RwLock<BTreeMap<&'static str, &'static ValueInfo>> = RwLock::new(BTreeMap::new());

/// Makes the valuetype `info` known to every ORB of the process, so that
/// a value of it is read as itself where a base type of it is expected
/// (rather than truncated). Its TypeCode is registered for anys as well
/// ([`crate::register_value_type`]). Generated files have a
/// `register_value_types` function that registers all of theirs.
pub fn register_value(info: &'static ValueInfo) {
    VALUES
        .write()
        .unwrap_or_else(|e| e.into_inner())
        .insert(info.id, info);
    crate::register_value_type(&(info.type_code)());
}

fn lookup(id: &str) -> Option<&'static ValueInfo> {
    VALUES
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .get(id)
        .copied()
}

/// Writes the value box `v` of the type `id`, holding a `T`, or `null`.
pub fn write_box<T: IdlType>(
    v: Option<&Arc<T::Rust>>,
    id: &str,
    out: &mut OutStream<'_>,
) -> Sys<()> {
    let Some(v) = v else {
        out.write_null_value();
        return Ok(());
    };
    let key = Arc::as_ptr(v) as *const () as usize as u64;
    if !out.begin_value(key, &[id], false)? {
        return Ok(());
    }
    T::write(v.as_ref().borrow(), out)?;
    out.end_value()?;
    Ok(())
}

/// Reads a value box of the type `id`, holding a `T`, or `null`.
pub fn read_box<T: IdlType>(input: &mut InStream<'_>, id: &str) -> Sys<Option<Arc<T::Rust>>> {
    let e = input.marshal_error();
    let header = input.read_value_header().map_err(|_| e.clone())?;
    match header {
        ValueHeader::Null => Ok(None),
        ValueHeader::Indirection(slot) => input
            .typed_values
            .get(slot)
            .and_then(|v| v.as_ref())
            .and_then(|v| v.downcast_ref::<Arc<T::Rust>>())
            .cloned()
            .map(Some)
            .ok_or(e),
        ValueHeader::Value { slot, ids, .. } => {
            if ids.first().is_some_and(|i| i != id) {
                return Err(e);
            }
            let v = Arc::new(T::read(input)?);
            set_slot(input, slot, Box::new(v.clone()));
            input.end_value().map_err(|_| e)?;
            Ok(Some(v))
        }
    }
}

//! Abstract interfaces of generated code.
//!
//! An abstract interface `A` is passed either as an object reference or as
//! a value of a valuetype that supports it (CORBA 3.4 part 2, 9.3.7). The
//! generator makes an enum `A` with a variant for each: `Object` holds the
//! reference, and `Value` an [`AbstractValue`]: the instance and the
//! operations of `A` on it (the servant trait `AServant`, which the
//! generator implements for the handle of every valuetype that supports
//! `A`).
//!
//! A value that arrives where an abstract interface is expected is read as
//! a `ValueBase` ([`ValueHandle::read_base`]), and then turned into the
//! enum by the function the valuetype's file registers for it with
//! [`register_supported`] (its `register_value_types` does).

use std::any::Any;
use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Arc, RwLock};

use super::{Sys, ValueHandle};
use crate::{InStream, ObjectRef, OutStream};

/// A value passed as an abstract interface: the instance, and the
/// operations of the interface on it.
pub struct AbstractValue<T: ?Sized> {
    handle: ValueHandle,
    ops: Arc<T>,
}

impl<T: ?Sized> AbstractValue<T> {
    /// The instance `handle`, whose operations are `ops` (usually the
    /// typed handle of the same instance).
    pub fn new(handle: ValueHandle, ops: Arc<T>) -> AbstractValue<T> {
        AbstractValue { handle, ops }
    }

    /// The untyped handle of the instance.
    pub fn handle(&self) -> &ValueHandle {
        &self.handle
    }

    /// The operations of the interface on the instance.
    pub fn ops(&self) -> &Arc<T> {
        &self.ops
    }

    /// Whether both are the same instance.
    pub fn ptr_eq(&self, other: &AbstractValue<T>) -> bool {
        self.handle.ptr_eq(&other.handle)
    }
}

impl<T: ?Sized> Clone for AbstractValue<T> {
    fn clone(&self) -> Self {
        AbstractValue {
            handle: self.handle.clone(),
            ops: self.ops.clone(),
        }
    }
}

impl<T: ?Sized> fmt::Debug for AbstractValue<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.handle.fmt(f)
    }
}

/// The operations of a derived abstract interface used as those of a base
/// one: the generator implements the servant trait of each abstract
/// interface for `Upcast<T>` of every `T` that implements it, so that an
/// `Arc<dyn DerivedServant>` becomes an `Arc<dyn BaseServant>` without
/// trait upcasting.
pub struct Upcast<T: ?Sized>(pub Arc<T>);

impl<T: ?Sized> fmt::Debug for Upcast<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Upcast")
    }
}

/// Turns an instance into the enum of an abstract interface it supports
/// (as a `Box<dyn Any>`), or `None` if it is not of the valuetype.
pub type SupportedFn = fn(&ValueHandle) -> Option<Box<dyn Any + Send>>;

type Supported = BTreeMap<(&'static str, &'static str), SupportedFn>;

static SUPPORTED: RwLock<Supported> = RwLock::new(BTreeMap::new());

/// Records that the valuetype `value_id` supports the abstract interface
/// `abstract_id`, and how its instances become the generated enum of the
/// interface. Instances of types derived from `value_id` are turned the
/// same way.
pub fn register_supported(abstract_id: &'static str, value_id: &'static str, f: SupportedFn) {
    SUPPORTED
        .write()
        .unwrap_or_else(|e| e.into_inner())
        .insert((abstract_id, value_id), f);
}

/// The instance `v` as the abstract interface `abstract_id` (its enum
/// `A`), if its type or a base type of it was registered as supporting it
/// with [`register_supported`].
pub fn supported<A: 'static>(abstract_id: &str, v: &ValueHandle) -> Option<A> {
    let table = SUPPORTED.read().unwrap_or_else(|e| e.into_inner());
    v.info().ancestors.iter().find_map(|id| {
        let f = table.get(&(abstract_id, *id))?;
        f(v)?.downcast::<A>().ok().map(|a| *a)
    })
}

/// Writes an abstract interface as the object reference `obj` (TRUE and
/// the IOR); `None` is the nil reference.
pub fn write_abstract_object(obj: Option<&ObjectRef>, out: &mut OutStream<'_>) -> Sys<()> {
    out.write_bool(true);
    out.write_object(obj)
}

/// Writes an abstract interface as the value `v` (FALSE and the value).
pub fn write_abstract_value(v: &ValueHandle, out: &mut OutStream<'_>) -> Sys<()> {
    out.write_bool(false);
    ValueHandle::write(Some(v), out)
}

/// Reads the abstract interface `abstract_id`: an object reference made
/// into an `A` by `object`, a value made into one by [`supported`], or
/// `None` for a nil reference or a null value. A value of a type that is
/// not known to support the interface is `MARSHAL`.
pub fn read_abstract<A: 'static>(
    input: &mut InStream<'_>,
    abstract_id: &str,
    object: fn(ObjectRef) -> A,
) -> Sys<Option<A>> {
    if input.read_bool().map_err(|_| input.marshal_error())? {
        return Ok(input.read_object()?.map(object));
    }
    match ValueHandle::read_base(input)? {
        None => Ok(None),
        Some(v) => supported(abstract_id, &v)
            .map(Some)
            .ok_or_else(|| input.marshal_error()),
    }
}

use std::fmt::Write as _;
use std::ops::Deref;

use rustorb_core::giop::{Corbaloc, ObjAddr};
use rustorb_core::name::{self, Name, NameComponent};
use rustorb_core::{
    CompletionStatus, Exception, InStream, ObjectRef, OutStream, Reply, SystemException,
    SystemExceptionKind,
};

use crate::NamingError;
use crate::repository_id::{
    ALREADY_BOUND, BINDING_ITERATOR, CANNOT_PROCEED, INVALID_ADDRESS, INVALID_NAME, NAMING_CONTEXT,
    NAMING_CONTEXT_EXT, NOT_EMPTY, NOT_FOUND,
};

/// `CosNaming::BindingType`: what a name is bound to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BindingType {
    /// `nobject`: an object, bound with `bind` or `rebind`.
    Object,
    /// `ncontext`: a naming context, bound with `bind_context`,
    /// `rebind_context` or `bind_new_context`.
    Context,
}

impl BindingType {
    /// The IDL name, `nobject` or `ncontext`.
    pub fn name(self) -> &'static str {
        match self {
            BindingType::Object => "nobject",
            BindingType::Context => "ncontext",
        }
    }
}

/// `CosNaming::Binding`: one entry of a context, from `list` or a
/// [`BindingIterator`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Binding {
    /// The name of the entry, one component long.
    pub binding_name: Name,
    /// What the name is bound to.
    pub binding_type: BindingType,
}

impl Binding {
    fn read(input: &mut InStream<'_>) -> Result<Binding, SystemException> {
        let binding_name = name::read_name(input)?;
        let binding_type = match input.read::<u32>()? {
            0 => BindingType::Object,
            1 => BindingType::Context,
            _ => return Err(marshal()),
        };
        Ok(Binding {
            binding_name,
            binding_type,
        })
    }
}

fn read_bindings(input: &mut InStream<'_>) -> Result<Vec<Binding>, SystemException> {
    // A binding is at least an empty name and the type (8 bytes).
    let len = input.read_len(8).map_err(|_| marshal())?;
    (0..len).map(|_| Binding::read(input)).collect()
}

fn marshal() -> SystemException {
    SystemException::new(SystemExceptionKind::MARSHAL, 0, CompletionStatus::Yes)
}

/// Whether `obj` supports `repository_id`: from the IOR's type id when it
/// says so, otherwise by asking the object (`_is_a`).
fn supports(obj: &ObjectRef, repository_id: &str, also: &[&str]) -> Result<bool, SystemException> {
    if obj.type_id() == repository_id || also.contains(&obj.type_id()) {
        return Ok(true);
    }
    obj.is_a(repository_id).map_err(|e| match e {
        Exception::System(e) => e,
        Exception::User(_) => SystemException::new(
            SystemExceptionKind::UNKNOWN,
            rustorb_core::minor::UNLISTED_USER_EXCEPTION,
            CompletionStatus::Yes,
        ),
    })
}

/// Sends `operation` to `obj` and maps its exceptions, `declared` being the
/// user exceptions it may raise.
fn call(
    obj: &ObjectRef,
    operation: &str,
    declared: &[&str],
    args: impl Fn(&mut OutStream<'_>) -> Result<(), SystemException>,
) -> Result<Reply, NamingError> {
    obj.request(operation)
        .args(args)
        .invoke()
        .map_err(|e| NamingError::from_exception(e, declared))
}

/// A stub for `CosNaming::NamingContext`.
///
/// Names are given as slices of [`NameComponent`]; [`to_name`](crate::to_name)
/// makes one from a string such as `"app/Echo"`.
#[derive(Debug, Clone)]
pub struct NamingContext {
    obj: ObjectRef,
}

impl NamingContext {
    /// Narrows `obj` to a naming context: `None` if it is not one. Asks
    /// the object (`_is_a`) unless the IOR's type id is the repository id
    /// of `NamingContext` or `NamingContextExt`.
    pub fn narrow(obj: ObjectRef) -> Result<Option<NamingContext>, SystemException> {
        Ok(supports(&obj, NAMING_CONTEXT, &[NAMING_CONTEXT_EXT])?
            .then(|| NamingContext::unchecked_narrow(obj)))
    }

    /// Uses `obj` as a naming context without checking its type.
    pub fn unchecked_narrow(obj: ObjectRef) -> NamingContext {
        NamingContext { obj }
    }

    /// The object reference.
    pub fn object(&self) -> &ObjectRef {
        &self.obj
    }

    /// The object reference.
    pub fn into_object(self) -> ObjectRef {
        self.obj
    }

    /// `bind`: binds `n` to `obj`. Raises `NotFound`, `CannotProceed`,
    /// `InvalidName` or `AlreadyBound`.
    pub fn bind(&self, n: &[NameComponent], obj: &ObjectRef) -> Result<(), NamingError> {
        self.bind_op("bind", n, Some(obj), true)
    }

    /// `rebind`: binds `n` to `obj`, replacing an object binding. Raises
    /// `NotFound`, `CannotProceed` or `InvalidName`.
    pub fn rebind(&self, n: &[NameComponent], obj: &ObjectRef) -> Result<(), NamingError> {
        self.bind_op("rebind", n, Some(obj), false)
    }

    /// `bind_context`: binds `n` to the context `nc`, so that names are
    /// resolved through it. Raises `NotFound`, `CannotProceed`,
    /// `InvalidName` or `AlreadyBound`.
    pub fn bind_context(&self, n: &[NameComponent], nc: &NamingContext) -> Result<(), NamingError> {
        self.bind_op("bind_context", n, Some(&nc.obj), true)
    }

    /// `rebind_context`: binds `n` to the context `nc`, replacing a context
    /// binding. Raises `NotFound`, `CannotProceed` or `InvalidName`.
    pub fn rebind_context(
        &self,
        n: &[NameComponent],
        nc: &NamingContext,
    ) -> Result<(), NamingError> {
        self.bind_op("rebind_context", n, Some(&nc.obj), false)
    }

    pub(crate) fn bind_op(
        &self,
        operation: &str,
        n: &[NameComponent],
        obj: Option<&ObjectRef>,
        already_bound: bool,
    ) -> Result<(), NamingError> {
        let declared: &[&str] = if already_bound {
            &[NOT_FOUND, CANNOT_PROCEED, INVALID_NAME, ALREADY_BOUND]
        } else {
            &[NOT_FOUND, CANNOT_PROCEED, INVALID_NAME]
        };
        call(&self.obj, operation, declared, |out| {
            name::write_name(out, n)?;
            out.write_object(obj)
        })?;
        Ok(())
    }

    /// `resolve`: the object `n` is bound to (`None` if it is bound to a
    /// nil reference). Raises `NotFound`, `CannotProceed` or
    /// `InvalidName`.
    pub fn resolve(&self, n: &[NameComponent]) -> Result<Option<ObjectRef>, NamingError> {
        let reply = call(
            &self.obj,
            "resolve",
            &[NOT_FOUND, CANNOT_PROCEED, INVALID_NAME],
            |out| name::write_name(out, n),
        )?;
        Ok(reply.input().read_object()?)
    }

    /// `unbind`: removes the binding of `n`. Raises `NotFound`,
    /// `CannotProceed` or `InvalidName`.
    pub fn unbind(&self, n: &[NameComponent]) -> Result<(), NamingError> {
        call(
            &self.obj,
            "unbind",
            &[NOT_FOUND, CANNOT_PROCEED, INVALID_NAME],
            |out| name::write_name(out, n),
        )?;
        Ok(())
    }

    /// `new_context`: a new context, not bound to any name, in the same
    /// naming service.
    pub fn new_context(&self) -> Result<NamingContext, NamingError> {
        let reply = call(&self.obj, "new_context", &[], |_| Ok(()))?;
        read_context(&reply)
    }

    /// `bind_new_context`: creates a context and binds it to `n`. Raises
    /// `NotFound`, `CannotProceed`, `InvalidName` or `AlreadyBound`.
    pub fn bind_new_context(&self, n: &[NameComponent]) -> Result<NamingContext, NamingError> {
        let reply = call(
            &self.obj,
            "bind_new_context",
            &[NOT_FOUND, CANNOT_PROCEED, INVALID_NAME, ALREADY_BOUND],
            |out| name::write_name(out, n),
        )?;
        read_context(&reply)
    }

    /// `destroy`: deletes this context, which must have no bindings (or
    /// `NotEmpty` is raised). Names bound to it elsewhere are not unbound.
    pub fn destroy(&self) -> Result<(), NamingError> {
        call(&self.obj, "destroy", &[NOT_EMPTY], |_| Ok(()))?;
        Ok(())
    }

    /// `list`: up to `how_many` bindings of this context, and an iterator
    /// over the rest if there are more.
    pub fn list(
        &self,
        how_many: u32,
    ) -> Result<(Vec<Binding>, Option<BindingIterator>), NamingError> {
        let reply = call(&self.obj, "list", &[], |out| {
            out.write_u32(how_many);
            Ok(())
        })?;
        let mut input = reply.input();
        let bindings = read_bindings(&mut input)?;
        let iterator = input.read_object()?.map(|obj| BindingIterator { obj });
        Ok((bindings, iterator))
    }
}

fn read_context(reply: &Reply) -> Result<NamingContext, NamingError> {
    match reply.input().read_object()? {
        Some(obj) => Ok(NamingContext::unchecked_narrow(obj)),
        // The operation returns a context; nil is a broken reply.
        None => Err(marshal().into()),
    }
}

/// A stub for `CosNaming::NamingContextExt`: a [`NamingContext`] (which it
/// dereferences to) that also works with stringified names and
/// `corbaname:` URLs.
#[derive(Debug, Clone)]
pub struct NamingContextExt {
    ctx: NamingContext,
}

impl NamingContextExt {
    /// Narrows `obj` to a `NamingContextExt`: `None` if it is not one. Asks
    /// the object (`_is_a`) unless the IOR's type id is the repository id
    /// of `NamingContextExt`.
    pub fn narrow(obj: ObjectRef) -> Result<Option<NamingContextExt>, SystemException> {
        Ok(supports(&obj, NAMING_CONTEXT_EXT, &[])?
            .then(|| NamingContextExt::unchecked_narrow(obj)))
    }

    /// Uses `obj` as a `NamingContextExt` without checking its type.
    pub fn unchecked_narrow(obj: ObjectRef) -> NamingContextExt {
        NamingContextExt {
            ctx: NamingContext::unchecked_narrow(obj),
        }
    }

    /// The context as a plain [`NamingContext`].
    pub fn context(&self) -> &NamingContext {
        &self.ctx
    }

    /// `to_string`: the naming service writes `n` as a stringified name.
    /// Raises `InvalidName`. [`to_string`](crate::to_string) does the same
    /// locally.
    pub fn to_string(&self, n: &[NameComponent]) -> Result<String, NamingError> {
        let reply = call(&self.obj, "to_string", &[INVALID_NAME], |out| {
            name::write_name(out, n)
        })?;
        Ok(reply.input().read_string()?)
    }

    /// `to_name`: the naming service parses a stringified name. Raises
    /// `InvalidName`. [`to_name`](crate::to_name) does the same locally.
    pub fn to_name(&self, sn: &str) -> Result<Name, NamingError> {
        let reply = call(&self.obj, "to_name", &[INVALID_NAME], |out| {
            out.write_string(sn)
        })?;
        Ok(name::read_name(&mut reply.input())?)
    }

    /// `to_url`: the naming service makes a `corbaname:` URL from a
    /// `corbaloc` address list and a stringified name. Raises
    /// `InvalidAddress` or `InvalidName`. [`to_url`](crate::to_url) does
    /// the same locally.
    pub fn to_url(&self, addr: &str, sn: &str) -> Result<String, NamingError> {
        let reply = call(
            &self.obj,
            "to_url",
            &[INVALID_ADDRESS, INVALID_NAME],
            |out| {
                out.write_string(addr)?;
                out.write_string(sn)
            },
        )?;
        Ok(reply.input().read_string()?)
    }

    /// `resolve_str`: resolves a stringified name. Raises `NotFound`,
    /// `CannotProceed` or `InvalidName`.
    pub fn resolve_str(&self, sn: &str) -> Result<Option<ObjectRef>, NamingError> {
        let reply = call(
            &self.obj,
            "resolve_str",
            &[NOT_FOUND, CANNOT_PROCEED, INVALID_NAME],
            |out| out.write_string(sn),
        )?;
        Ok(reply.input().read_object()?)
    }
}

impl Deref for NamingContextExt {
    type Target = NamingContext;

    fn deref(&self) -> &NamingContext {
        &self.ctx
    }
}

impl From<NamingContextExt> for NamingContext {
    fn from(ext: NamingContextExt) -> NamingContext {
        ext.ctx
    }
}

/// A stub for `CosNaming::BindingIterator`, from [`NamingContext::list`].
///
/// Call [`destroy`](BindingIterator::destroy) when done; dropping the stub
/// leaves the iterator in the naming service.
#[derive(Debug, Clone)]
pub struct BindingIterator {
    obj: ObjectRef,
}

impl BindingIterator {
    /// Narrows `obj` to a binding iterator: `None` if it is not one.
    pub fn narrow(obj: ObjectRef) -> Result<Option<BindingIterator>, SystemException> {
        Ok(supports(&obj, BINDING_ITERATOR, &[])?.then(|| BindingIterator { obj }))
    }

    /// The object reference.
    pub fn object(&self) -> &ObjectRef {
        &self.obj
    }

    /// `next_one`: the next binding, or `None` at the end.
    pub fn next_one(&self) -> Result<Option<Binding>, NamingError> {
        let reply = call(&self.obj, "next_one", &[], |_| Ok(()))?;
        let mut input = reply.input();
        let more = input.read::<bool>()?;
        let binding = Binding::read(&mut input)?;
        Ok(more.then_some(binding))
    }

    /// `next_n`: up to `how_many` more bindings, and whether any were
    /// returned (`false` at the end).
    pub fn next_n(&self, how_many: u32) -> Result<(bool, Vec<Binding>), NamingError> {
        let reply = call(&self.obj, "next_n", &[], |out| {
            out.write_u32(how_many);
            Ok(())
        })?;
        let mut input = reply.input();
        let more = input.read::<bool>()?;
        let bindings = read_bindings(&mut input)?;
        Ok((more, bindings))
    }

    /// `destroy`: releases the iterator in the naming service.
    pub fn destroy(&self) -> Result<(), NamingError> {
        call(&self.obj, "destroy", &[], |_| Ok(()))?;
        Ok(())
    }
}

/// Makes a `corbaname:` URL from a `corbaloc` address list (such as
/// `:host:2809` or `iiop:1.2@host`) and a stringified name, escaping the
/// name as a URL, like `NamingContextExt::to_url`: `to_url(":host:2809",
/// "a/b c")` is `corbaname::host:2809#a/b%20c`.
///
/// An empty name gives the URL of the context itself, `corbaname:{addr}`.
/// An address list that `corbaloc:` does not accept, or with a protocol
/// other than `iiop` and `rir`, is `InvalidAddress`; a malformed name is
/// `InvalidName`.
pub fn to_url(addr: &str, sn: &str) -> Result<String, NamingError> {
    let valid = !addr.contains(['/', '#'])
        && Corbaloc::parse(&format!("corbaloc:{addr}/x")).is_ok_and(|loc| {
            loc.addresses
                .iter()
                .all(|a| !matches!(a, ObjAddr::Other { .. }))
        });
    if !valid {
        return Err(NamingError::InvalidAddress);
    }
    // An empty name is the context itself, as `corbaname:` without `#`
    // (omniNames answers the same).
    if sn.is_empty() {
        return Ok(format!("corbaname:{addr}"));
    }
    name::to_name(sn)?;
    let mut url = format!("corbaname:{addr}#");
    for b in sn.bytes() {
        if b.is_ascii_alphanumeric() || b";/:?@&=+$,-_.!~*'()".contains(&b) {
            url.push(b as char);
        } else {
            let _ = write!(url, "%{b:02X}");
        }
    }
    Ok(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_url_escapes_the_name() {
        assert_eq!(
            to_url(":host:2809", "a/b c").unwrap(),
            "corbaname::host:2809#a/b%20c"
        );
        assert_eq!(
            to_url("iiop:1.2@host,:other", "x.y/z\\/w").unwrap(),
            "corbaname:iiop:1.2@host,:other#x.y/z%5C/w"
        );
        assert_eq!(to_url("rir:", "a").unwrap(), "corbaname:rir:#a");
        assert_eq!(to_url(":host", "").unwrap(), "corbaname::host");
    }

    #[test]
    fn to_url_checks_address_and_name() {
        for addr in ["host:2809", "", "iiop:host/x", ":host#a"] {
            assert!(
                matches!(to_url(addr, "a"), Err(NamingError::InvalidAddress)),
                "{addr:?}"
            );
        }
        assert!(matches!(
            to_url(":host", "a//b"),
            Err(NamingError::InvalidName)
        ));
    }
}

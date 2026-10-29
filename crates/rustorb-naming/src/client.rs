use std::fmt::Write as _;
use std::ops::Deref;

use rustorb_core::giop::{Corbaloc, ObjAddr};
use rustorb_core::name::{self, Name, NameComponent};
use rustorb_core::{ObjectRef, SystemException};

use crate::NamingError;
use crate::idl::cos_naming as idl;
use crate::repository_id::{NAMING_CONTEXT, NAMING_CONTEXT_EXT};

/// A name of the generated code.
pub(crate) fn to_idl_name(n: &[NameComponent]) -> idl::Name {
    n.iter()
        .map(|c| idl::NameComponent {
            id: c.id.clone(),
            kind: c.kind.clone(),
        })
        .collect()
}

/// A name from the generated code.
pub(crate) fn from_idl_name(n: idl::Name) -> Name {
    n.into_iter()
        .map(|c| NameComponent {
            id: c.id,
            kind: c.kind,
        })
        .collect()
}

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

impl From<idl::Binding> for Binding {
    fn from(b: idl::Binding) -> Binding {
        Binding {
            binding_name: from_idl_name(b.binding_name),
            binding_type: match b.binding_type {
                idl::BindingType::Nobject => BindingType::Object,
                idl::BindingType::Ncontext => BindingType::Context,
            },
        }
    }
}

impl From<&Binding> for idl::Binding {
    fn from(b: &Binding) -> idl::Binding {
        idl::Binding {
            binding_name: to_idl_name(&b.binding_name),
            binding_type: match b.binding_type {
                BindingType::Object => idl::BindingType::Nobject,
                BindingType::Context => idl::BindingType::Ncontext,
            },
        }
    }
}

fn bindings(list: idl::BindingList) -> Vec<Binding> {
    list.into_iter().map(Binding::from).collect()
}

/// Whether `obj` supports `repository_id`: from the IOR's type id when it
/// says so, otherwise by asking the object (`_is_a`).
fn supports(obj: &ObjectRef, repository_id: &str, also: &[&str]) -> Result<bool, SystemException> {
    if also.contains(&obj.type_id()) {
        return Ok(true);
    }
    rustorb_core::idl::is_a(obj, repository_id)
}

/// A context the operation returns; nil is a broken reply.
fn context(nc: Option<idl::NamingContext>) -> Result<NamingContext, NamingError> {
    let nc = nc.ok_or_else(|| {
        SystemException::new(
            rustorb_core::SystemExceptionKind::MARSHAL,
            0,
            rustorb_core::CompletionStatus::Yes,
        )
    })?;
    Ok(NamingContext::unchecked_narrow(nc.into_object()))
}

/// A stub for `CosNaming::NamingContext`, over the one `rustorb-idl -brust`
/// generates ([`crate::idl::cos_naming::NamingContext`]) with the names of
/// [`rustorb_core::name`] and the errors of [`NamingError`].
///
/// Names are given as slices of [`NameComponent`]; [`to_name`](crate::to_name)
/// makes one from a string such as `"app/Echo"`.
#[derive(Debug, Clone)]
pub struct NamingContext {
    stub: idl::NamingContext,
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
        NamingContext {
            stub: idl::NamingContext::unchecked_narrow(obj),
        }
    }

    /// The object reference.
    pub fn object(&self) -> &ObjectRef {
        self.stub.object()
    }

    /// The object reference.
    pub fn into_object(self) -> ObjectRef {
        self.stub.into_object()
    }

    /// The generated stub.
    pub fn stub(&self) -> &idl::NamingContext {
        &self.stub
    }

    /// `bind`: binds `n` to `obj`. Raises `NotFound`, `CannotProceed`,
    /// `InvalidName` or `AlreadyBound`.
    pub fn bind(&self, n: &[NameComponent], obj: &ObjectRef) -> Result<(), NamingError> {
        Ok(self.stub.bind(&to_idl_name(n), Some(obj))?)
    }

    /// `rebind`: binds `n` to `obj`, replacing an object binding. Raises
    /// `NotFound`, `CannotProceed` or `InvalidName`.
    pub fn rebind(&self, n: &[NameComponent], obj: &ObjectRef) -> Result<(), NamingError> {
        Ok(self.stub.rebind(&to_idl_name(n), Some(obj))?)
    }

    /// `bind_context`: binds `n` to the context `nc`, so that names are
    /// resolved through it. Raises `NotFound`, `CannotProceed`,
    /// `InvalidName` or `AlreadyBound`.
    pub fn bind_context(&self, n: &[NameComponent], nc: &NamingContext) -> Result<(), NamingError> {
        Ok(self.stub.bind_context(&to_idl_name(n), Some(&nc.stub))?)
    }

    /// `rebind_context`: binds `n` to the context `nc`, replacing a context
    /// binding. Raises `NotFound`, `CannotProceed` or `InvalidName`.
    pub fn rebind_context(
        &self,
        n: &[NameComponent],
        nc: &NamingContext,
    ) -> Result<(), NamingError> {
        Ok(self.stub.rebind_context(&to_idl_name(n), Some(&nc.stub))?)
    }

    /// `resolve`: the object `n` is bound to (`None` if it is bound to a
    /// nil reference). Raises `NotFound`, `CannotProceed` or
    /// `InvalidName`.
    pub fn resolve(&self, n: &[NameComponent]) -> Result<Option<ObjectRef>, NamingError> {
        Ok(self.stub.resolve(&to_idl_name(n))?)
    }

    /// `unbind`: removes the binding of `n`. Raises `NotFound`,
    /// `CannotProceed` or `InvalidName`.
    pub fn unbind(&self, n: &[NameComponent]) -> Result<(), NamingError> {
        Ok(self.stub.unbind(&to_idl_name(n))?)
    }

    /// `new_context`: a new context, not bound to any name, in the same
    /// naming service.
    pub fn new_context(&self) -> Result<NamingContext, NamingError> {
        context(self.stub.new_context()?)
    }

    /// `bind_new_context`: creates a context and binds it to `n`. Raises
    /// `NotFound`, `CannotProceed`, `InvalidName` or `AlreadyBound`.
    pub fn bind_new_context(&self, n: &[NameComponent]) -> Result<NamingContext, NamingError> {
        context(self.stub.bind_new_context(&to_idl_name(n))?)
    }

    /// `destroy`: deletes this context, which must have no bindings (or
    /// `NotEmpty` is raised). Names bound to it elsewhere are not unbound.
    pub fn destroy(&self) -> Result<(), NamingError> {
        Ok(self.stub.destroy()?)
    }

    /// `list`: up to `how_many` bindings of this context, and an iterator
    /// over the rest if there are more.
    pub fn list(
        &self,
        how_many: u32,
    ) -> Result<(Vec<Binding>, Option<BindingIterator>), NamingError> {
        let (list, iterator) = self.stub.list(how_many)?;
        Ok((
            bindings(list),
            iterator.map(|stub| BindingIterator { stub }),
        ))
    }
}

/// A stub for `CosNaming::NamingContextExt`: a [`NamingContext`] (which it
/// dereferences to) that also works with stringified names and
/// `corbaname:` URLs.
#[derive(Debug, Clone)]
pub struct NamingContextExt {
    ctx: NamingContext,
    stub: idl::NamingContextExt,
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
            ctx: NamingContext::unchecked_narrow(obj.clone()),
            stub: idl::NamingContextExt::unchecked_narrow(obj),
        }
    }

    /// The context as a plain [`NamingContext`].
    pub fn context(&self) -> &NamingContext {
        &self.ctx
    }

    /// The generated stub.
    pub fn stub(&self) -> &idl::NamingContextExt {
        &self.stub
    }

    /// `to_string`: the naming service writes `n` as a stringified name.
    /// Raises `InvalidName`. [`to_string`](crate::to_string) does the same
    /// locally.
    pub fn to_string(&self, n: &[NameComponent]) -> Result<String, NamingError> {
        Ok(self.stub.to_string(&to_idl_name(n))?)
    }

    /// `to_name`: the naming service parses a stringified name. Raises
    /// `InvalidName`. [`to_name`](crate::to_name) does the same locally.
    pub fn to_name(&self, sn: &str) -> Result<Name, NamingError> {
        Ok(from_idl_name(self.stub.to_name(sn)?))
    }

    /// `to_url`: the naming service makes a `corbaname:` URL from a
    /// `corbaloc` address list and a stringified name. Raises
    /// `InvalidAddress` or `InvalidName`. [`to_url`](crate::to_url) does
    /// the same locally.
    pub fn to_url(&self, addr: &str, sn: &str) -> Result<String, NamingError> {
        Ok(self.stub.to_url(addr, sn)?)
    }

    /// `resolve_str`: resolves a stringified name. Raises `NotFound`,
    /// `CannotProceed` or `InvalidName`.
    pub fn resolve_str(&self, sn: &str) -> Result<Option<ObjectRef>, NamingError> {
        Ok(self.stub.resolve_str(sn)?)
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
    stub: idl::BindingIterator,
}

impl BindingIterator {
    /// Narrows `obj` to a binding iterator: `None` if it is not one.
    pub fn narrow(obj: ObjectRef) -> Result<Option<BindingIterator>, SystemException> {
        Ok(idl::BindingIterator::narrow(&obj)?.map(|stub| BindingIterator { stub }))
    }

    /// The object reference.
    pub fn object(&self) -> &ObjectRef {
        self.stub.object()
    }

    /// `next_one`: the next binding, or `None` at the end.
    pub fn next_one(&self) -> Result<Option<Binding>, NamingError> {
        let (more, binding) = self.stub.next_one()?;
        Ok(more.then(|| binding.into()))
    }

    /// `next_n`: up to `how_many` more bindings, and whether any were
    /// returned (`false` at the end).
    pub fn next_n(&self, how_many: u32) -> Result<(bool, Vec<Binding>), NamingError> {
        let (more, list) = self.stub.next_n(how_many)?;
        Ok((more, bindings(list)))
    }

    /// `destroy`: releases the iterator in the naming service.
    pub fn destroy(&self) -> Result<(), NamingError> {
        Ok(self.stub.destroy()?)
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

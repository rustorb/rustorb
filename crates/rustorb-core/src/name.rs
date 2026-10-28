//! CosNaming names and their string form, which `corbaname:` URLs use.
//!
//! The naming service client in `rustorb-naming` re-exports these; they
//! live here because [`Orb::string_to_object`](crate::Orb::string_to_object)
//! resolves `corbaname:` URLs itself.

use std::fmt;

use crate::{InStream, OutStream, SystemException};

/// `CosNaming::NameComponent`: one step of a [`Name`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NameComponent {
    /// The identifier.
    pub id: String,
    /// The kind, often empty.
    pub kind: String,
}

impl NameComponent {
    /// A component with `id` and `kind`.
    pub fn new(id: impl Into<String>, kind: impl Into<String>) -> NameComponent {
        NameComponent {
            id: id.into(),
            kind: kind.into(),
        }
    }

    /// Writes the component as the `NameComponent` struct.
    pub fn write(&self, out: &mut OutStream<'_>) -> Result<(), SystemException> {
        out.write_string(&self.id)?;
        out.write_string(&self.kind)
    }

    /// Reads a `NameComponent` struct.
    pub fn read(input: &mut InStream<'_>) -> Result<NameComponent, SystemException> {
        Ok(NameComponent {
            id: input.read_string()?,
            kind: input.read_string()?,
        })
    }
}

/// `CosNaming::Name`: a sequence of components, from the outermost context.
pub type Name = Vec<NameComponent>;

/// Writes a `CosNaming::Name`.
pub fn write_name(out: &mut OutStream<'_>, name: &[NameComponent]) -> Result<(), SystemException> {
    out.write_len(name.len())?;
    for component in name {
        component.write(out)?;
    }
    Ok(())
}

/// Reads a `CosNaming::Name`.
pub fn read_name(input: &mut InStream<'_>) -> Result<Name, SystemException> {
    // Each component takes at least two empty strings (8 bytes).
    let len = input.read_len(8).map_err(crate::stream::marshal)?;
    (0..len).map(|_| NameComponent::read(input)).collect()
}

/// A stringified name is malformed, or a name has no components
/// (`CosNaming::NamingContext::InvalidName` when the conversion is local).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidName;

impl fmt::Display for InvalidName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("invalid name")
    }
}

impl std::error::Error for InvalidName {}

/// Parses a stringified name, as `NamingContextExt::to_name` does:
/// components are separated by `/`, the id and kind of a component by
/// `.`, and `\` escapes the next character. `a.b/c\/d/.e` is the name
/// `[("a", "b"), ("c/d", ""), ("", "e")]`; `.` alone is a component with
/// an empty id and kind.
///
/// An empty string, an empty component (`a//b`), a component ending in
/// `.` (`a.`), a second unescaped `.` in a component, or a trailing `\`
/// are invalid.
pub fn to_name(s: &str) -> Result<Name, InvalidName> {
    if s.is_empty() {
        return Err(InvalidName);
    }
    let mut name = Vec::new();
    let mut id = String::new();
    let mut kind = String::new();
    // Whether the current component has seen its `.`, and any character.
    let mut in_kind = false;
    let mut seen = false;
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                let escaped = chars.next().ok_or(InvalidName)?;
                if in_kind { &mut kind } else { &mut id }.push(escaped);
                seen = true;
            }
            '/' => {
                if !seen || trailing_dot(in_kind, &id, &kind) {
                    return Err(InvalidName);
                }
                name.push(NameComponent::new(
                    std::mem::take(&mut id),
                    std::mem::take(&mut kind),
                ));
                in_kind = false;
                seen = false;
            }
            '.' => {
                if in_kind {
                    return Err(InvalidName);
                }
                in_kind = true;
                seen = true;
            }
            c => {
                if in_kind { &mut kind } else { &mut id }.push(c);
                seen = true;
            }
        }
    }
    if !seen || trailing_dot(in_kind, &id, &kind) {
        return Err(InvalidName);
    }
    name.push(NameComponent::new(id, kind));
    Ok(name)
}

/// Whether a component ends with the `.` before an empty kind, as in `a.`,
/// which is not allowed (only `.` alone has an empty kind after its `.`).
fn trailing_dot(in_kind: bool, id: &str, kind: &str) -> bool {
    in_kind && kind.is_empty() && !id.is_empty()
}

/// Writes a name in the string form [`to_name`] reads, as
/// `NamingContextExt::to_string` does. An empty name is invalid.
pub fn to_string(name: &[NameComponent]) -> Result<String, InvalidName> {
    if name.is_empty() {
        return Err(InvalidName);
    }
    let mut s = String::new();
    for (i, component) in name.iter().enumerate() {
        if i > 0 {
            s.push('/');
        }
        if component.id.is_empty() && component.kind.is_empty() {
            s.push('.');
            continue;
        }
        escape_into(&mut s, &component.id);
        if !component.kind.is_empty() {
            s.push('.');
            escape_into(&mut s, &component.kind);
        }
    }
    Ok(s)
}

fn escape_into(s: &mut String, part: &str) {
    for c in part.chars() {
        if matches!(c, '/' | '.' | '\\') {
            s.push('\\');
        }
        s.push(c);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nc(id: &str, kind: &str) -> NameComponent {
        NameComponent::new(id, kind)
    }

    #[test]
    fn parses_components_and_escapes() {
        assert_eq!(
            to_name("a.b/c\\/d/.e").unwrap(),
            vec![nc("a", "b"), nc("c/d", ""), nc("", "e")]
        );
        assert_eq!(to_name("x").unwrap(), vec![nc("x", "")]);
        assert_eq!(to_name(".").unwrap(), vec![nc("", "")]);
        assert_eq!(to_name("a\\.b.c").unwrap(), vec![nc("a.b", "c")]);
        assert_eq!(to_name("a\\\\b").unwrap(), vec![nc("a\\b", "")]);
    }

    #[test]
    fn rejects_malformed_names() {
        for s in ["", "a//b", "/a", "a/", "a.b.c", "a.", "a./b", "a\\"] {
            assert_eq!(to_name(s), Err(InvalidName), "{s:?}");
        }
    }

    #[test]
    fn writes_and_reads_back() {
        let name = vec![
            nc("a", "b"),
            nc("c/d", ""),
            nc("", "e"),
            nc("", ""),
            nc("x.y", "z\\"),
        ];
        let s = to_string(&name).unwrap();
        assert_eq!(s, "a.b/c\\/d/.e/./x\\.y.z\\\\");
        assert_eq!(to_name(&s).unwrap(), name);
        assert_eq!(to_string(&[]), Err(InvalidName));
    }
}

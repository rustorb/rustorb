//! TypeCodes (CORBA 3.0, 4.11).
//!
//! A [`TypeCode`] is an immutable description of an IDL type. The nodes of
//! one TypeCode and of all the types it contains live in one shared arena,
//! and a TypeCode is a handle to a node of it, so that a recursive type
//! (`struct Node { sequence<Node> children; }`) is a node whose descendant
//! refers back to it by index, with no reference cycle. Taking a member
//! type or the content type returns a handle into the same arena.
//!
//! TypeCodes are made with the constructors ([`TypeCode::struct_tc`],
//! [`TypeCode::sequence`]...), which copy the member types into the new
//! arena, or read from CDR. A recursive type is made with
//! [`TypeCode::recursive`], a placeholder that the constructor of the
//! enclosing struct, union or valuetype with the same repository id
//! replaces with a reference to itself, as `ORB::create_recursive_tc`.

mod cdr;

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::sync::{Arc, LazyLock};

use crate::{Any, Value};

/// The kinds of TypeCodes (`CORBA::TCKind`), with their values on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u32)]
#[allow(missing_docs)]
pub enum TCKind {
    Null = 0,
    Void = 1,
    Short = 2,
    Long = 3,
    UShort = 4,
    ULong = 5,
    Float = 6,
    Double = 7,
    Boolean = 8,
    Char = 9,
    Octet = 10,
    Any = 11,
    TypeCode = 12,
    Principal = 13,
    Objref = 14,
    Struct = 15,
    Union = 16,
    Enum = 17,
    String = 18,
    Sequence = 19,
    Array = 20,
    Alias = 21,
    Except = 22,
    LongLong = 23,
    ULongLong = 24,
    LongDouble = 25,
    WChar = 26,
    WString = 27,
    Fixed = 28,
    Value = 29,
    ValueBox = 30,
    Native = 31,
    AbstractInterface = 32,
    LocalInterface = 33,
    Component = 34,
    Home = 35,
    Event = 36,
}

impl TCKind {
    const ALL: [TCKind; 37] = [
        TCKind::Null,
        TCKind::Void,
        TCKind::Short,
        TCKind::Long,
        TCKind::UShort,
        TCKind::ULong,
        TCKind::Float,
        TCKind::Double,
        TCKind::Boolean,
        TCKind::Char,
        TCKind::Octet,
        TCKind::Any,
        TCKind::TypeCode,
        TCKind::Principal,
        TCKind::Objref,
        TCKind::Struct,
        TCKind::Union,
        TCKind::Enum,
        TCKind::String,
        TCKind::Sequence,
        TCKind::Array,
        TCKind::Alias,
        TCKind::Except,
        TCKind::LongLong,
        TCKind::ULongLong,
        TCKind::LongDouble,
        TCKind::WChar,
        TCKind::WString,
        TCKind::Fixed,
        TCKind::Value,
        TCKind::ValueBox,
        TCKind::Native,
        TCKind::AbstractInterface,
        TCKind::LocalInterface,
        TCKind::Component,
        TCKind::Home,
        TCKind::Event,
    ];

    /// The kind with the value `v`, if there is one.
    pub fn from_u32(v: u32) -> Option<TCKind> {
        TCKind::ALL.get(v as usize).copied()
    }

    /// The value of the kind on the wire.
    pub fn as_u32(self) -> u32 {
        self as u32
    }

    /// Whether TypeCodes of this kind have no parameters.
    pub fn is_simple(self) -> bool {
        matches!(
            self,
            TCKind::Null
                | TCKind::Void
                | TCKind::Short
                | TCKind::Long
                | TCKind::UShort
                | TCKind::ULong
                | TCKind::Float
                | TCKind::Double
                | TCKind::Boolean
                | TCKind::Char
                | TCKind::Octet
                | TCKind::Any
                | TCKind::TypeCode
                | TCKind::Principal
                | TCKind::LongLong
                | TCKind::ULongLong
                | TCKind::LongDouble
                | TCKind::WChar
        )
    }

    /// Whether this kind can be the discriminator of a union.
    pub fn is_discriminator(self) -> bool {
        matches!(
            self,
            TCKind::Short
                | TCKind::Long
                | TCKind::UShort
                | TCKind::ULong
                | TCKind::LongLong
                | TCKind::ULongLong
                | TCKind::Boolean
                | TCKind::Char
                | TCKind::WChar
                | TCKind::Enum
        )
    }

    /// The IDL name of the kind without the `tk_` prefix (`"struct"`).
    pub fn name(self) -> &'static str {
        match self {
            TCKind::Null => "null",
            TCKind::Void => "void",
            TCKind::Short => "short",
            TCKind::Long => "long",
            TCKind::UShort => "ushort",
            TCKind::ULong => "ulong",
            TCKind::Float => "float",
            TCKind::Double => "double",
            TCKind::Boolean => "boolean",
            TCKind::Char => "char",
            TCKind::Octet => "octet",
            TCKind::Any => "any",
            TCKind::TypeCode => "TypeCode",
            TCKind::Principal => "Principal",
            TCKind::Objref => "objref",
            TCKind::Struct => "struct",
            TCKind::Union => "union",
            TCKind::Enum => "enum",
            TCKind::String => "string",
            TCKind::Sequence => "sequence",
            TCKind::Array => "array",
            TCKind::Alias => "alias",
            TCKind::Except => "except",
            TCKind::LongLong => "longlong",
            TCKind::ULongLong => "ulonglong",
            TCKind::LongDouble => "longdouble",
            TCKind::WChar => "wchar",
            TCKind::WString => "wstring",
            TCKind::Fixed => "fixed",
            TCKind::Value => "value",
            TCKind::ValueBox => "value_box",
            TCKind::Native => "native",
            TCKind::AbstractInterface => "abstract_interface",
            TCKind::LocalInterface => "local_interface",
            TCKind::Component => "component",
            TCKind::Home => "home",
            TCKind::Event => "event",
        }
    }
}

/// The errors of the TypeCode operations (`CORBA::TypeCode::BadKind` and
/// `Bounds`) and of the constructors (`BAD_PARAM` and `BAD_TYPECODE` in
/// the standard mapping of `ORB::create_*_tc`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeCodeError {
    /// The operation does not apply to this kind of TypeCode.
    BadKind,
    /// The member index is out of range.
    Bounds,
    /// A constructor was given parameters that make no valid type: a
    /// member type that cannot be a member (`void`, an exception...), a
    /// discriminator type that cannot be one, duplicate labels or names,
    /// an invalid name...
    BadParam,
}

impl fmt::Display for TypeCodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            TypeCodeError::BadKind => "the operation does not apply to this kind of TypeCode",
            TypeCodeError::Bounds => "member index out of range",
            TypeCodeError::BadParam => "invalid TypeCode parameters",
        })
    }
}

impl std::error::Error for TypeCodeError {}

/// The member of a valuetype is private (`CORBA::PRIVATE_MEMBER`).
pub const PRIVATE_MEMBER: i16 = 0;
/// The member of a valuetype is public (`CORBA::PUBLIC_MEMBER`).
pub const PUBLIC_MEMBER: i16 = 1;

/// A node of an arena. Children are indexes into the same arena.
#[derive(Debug, Clone)]
enum Node {
    /// A kind without parameters.
    Simple(TCKind),
    /// `string` or `wstring`, with its bound (0 for none).
    String {
        kind: TCKind,
        bound: u32,
    },
    Fixed {
        digits: u16,
        scale: i16,
    },
    /// Interfaces and `native`: objref, abstract_interface,
    /// local_interface, native, component and home.
    Named {
        kind: TCKind,
        id: String,
        name: String,
    },
    /// struct and except.
    Struct {
        kind: TCKind,
        id: String,
        name: String,
        members: Vec<(String, u32)>,
    },
    Union {
        id: String,
        name: String,
        disc: u32,
        /// The index of the default member, or -1.
        default: i32,
        /// Label (None for the default member), name and type.
        members: Vec<(Option<i128>, String, u32)>,
    },
    Enum {
        id: String,
        name: String,
        members: Vec<String>,
    },
    Sequence {
        content: u32,
        bound: u32,
    },
    Array {
        content: u32,
        length: u32,
    },
    /// alias and value_box.
    Alias {
        kind: TCKind,
        id: String,
        name: String,
        content: u32,
    },
    /// value and event.
    Value {
        kind: TCKind,
        id: String,
        name: String,
        modifier: i16,
        base: Option<u32>,
        /// Name, type and visibility.
        members: Vec<(String, u32, i16)>,
    },
    /// A recursive reference to the enclosing type with this id, not yet
    /// resolved ([`TypeCode::recursive`]).
    Recursive {
        id: String,
    },
}

impl Node {
    fn kind(&self) -> TCKind {
        match self {
            Node::Simple(k) => *k,
            Node::String { kind, .. }
            | Node::Named { kind, .. }
            | Node::Struct { kind, .. }
            | Node::Alias { kind, .. }
            | Node::Value { kind, .. } => *kind,
            Node::Fixed { .. } => TCKind::Fixed,
            Node::Union { .. } => TCKind::Union,
            Node::Enum { .. } => TCKind::Enum,
            Node::Sequence { .. } => TCKind::Sequence,
            Node::Array { .. } => TCKind::Array,
            // A placeholder has no kind of its own; it stands for a
            // struct, union or valuetype.
            Node::Recursive { .. } => TCKind::Null,
        }
    }

    fn id_name(&self) -> Option<(&str, &str)> {
        match self {
            Node::Named { id, name, .. }
            | Node::Struct { id, name, .. }
            | Node::Union { id, name, .. }
            | Node::Enum { id, name, .. }
            | Node::Alias { id, name, .. }
            | Node::Value { id, name, .. } => Some((id, name)),
            _ => None,
        }
    }

    /// Calls `f` on every child index.
    fn children_mut(&mut self, mut f: impl FnMut(&mut u32)) {
        match self {
            Node::Struct { members, .. } => members.iter_mut().for_each(|(_, t)| f(t)),
            Node::Union { disc, members, .. } => {
                f(disc);
                members.iter_mut().for_each(|(_, _, t)| f(t));
            }
            Node::Sequence { content, .. }
            | Node::Array { content, .. }
            | Node::Alias { content, .. } => f(content),
            Node::Value { base, members, .. } => {
                if let Some(b) = base {
                    f(b);
                }
                members.iter_mut().for_each(|(_, t, _)| f(t));
            }
            _ => {}
        }
    }
}

#[derive(Debug)]
struct Arena {
    nodes: Vec<Node>,
}

/// An IDL type (`CORBA::TypeCode`).
///
/// Cloning is cheap: the clone shares the arena. `==` is
/// [`TypeCode::equal`].
#[derive(Clone)]
pub struct TypeCode {
    arena: Arc<Arena>,
    index: u32,
}

static SIMPLE: LazyLock<Vec<TypeCode>> = LazyLock::new(|| {
    TCKind::ALL
        .iter()
        .map(|&k| {
            let node = if k.is_simple() {
                Node::Simple(k)
            } else {
                // Not used: only simple kinds are looked up.
                Node::Simple(TCKind::Null)
            };
            TypeCode::single(node)
        })
        .collect()
});

static UNBOUNDED_STRING: LazyLock<TypeCode> = LazyLock::new(|| {
    TypeCode::single(Node::String {
        kind: TCKind::String,
        bound: 0,
    })
});
static UNBOUNDED_WSTRING: LazyLock<TypeCode> = LazyLock::new(|| {
    TypeCode::single(Node::String {
        kind: TCKind::WString,
        bound: 0,
    })
});
static OBJECT: LazyLock<TypeCode> = LazyLock::new(|| {
    TypeCode::single(Node::Named {
        kind: TCKind::Objref,
        id: "IDL:omg.org/CORBA/Object:1.0".into(),
        name: "Object".into(),
    })
});

impl TypeCode {
    fn single(node: Node) -> TypeCode {
        TypeCode {
            arena: Arc::new(Arena { nodes: vec![node] }),
            index: 0,
        }
    }

    fn node(&self) -> &Node {
        &self.arena.nodes[self.index as usize]
    }

    fn at(&self, index: u32) -> TypeCode {
        TypeCode {
            arena: Arc::clone(&self.arena),
            index,
        }
    }

    // --- constructors -------------------------------------------------------

    /// The TypeCode of a kind without parameters (`TC_long`, `TC_any`...);
    /// `BadKind` for other kinds.
    pub fn simple(kind: TCKind) -> Result<TypeCode, TypeCodeError> {
        if !kind.is_simple() {
            return Err(TypeCodeError::BadKind);
        }
        Ok(SIMPLE[kind as usize].clone())
    }

    /// `TC_null`.
    pub fn null() -> TypeCode {
        SIMPLE[TCKind::Null as usize].clone()
    }

    /// `TC_void`.
    pub fn void() -> TypeCode {
        SIMPLE[TCKind::Void as usize].clone()
    }

    /// `TC_Object`, the TypeCode of `Object`.
    pub fn object() -> TypeCode {
        OBJECT.clone()
    }

    /// `string`, or `string<bound>` when `bound` is not 0.
    pub fn string(bound: u32) -> TypeCode {
        if bound == 0 {
            return UNBOUNDED_STRING.clone();
        }
        TypeCode::single(Node::String {
            kind: TCKind::String,
            bound,
        })
    }

    /// `wstring`, or `wstring<bound>` when `bound` is not 0.
    pub fn wstring(bound: u32) -> TypeCode {
        if bound == 0 {
            return UNBOUNDED_WSTRING.clone();
        }
        TypeCode::single(Node::String {
            kind: TCKind::WString,
            bound,
        })
    }

    /// `fixed<digits, scale>`: 1 to 31 digits, a scale from 0 to `digits`.
    pub fn fixed(digits: u16, scale: i16) -> Result<TypeCode, TypeCodeError> {
        if !(1..=31).contains(&digits) || scale < 0 || scale as u16 > digits {
            return Err(TypeCodeError::BadParam);
        }
        Ok(TypeCode::single(Node::Fixed { digits, scale }))
    }

    /// `sequence<content>`, or `sequence<content, bound>` when `bound` is
    /// not 0. `content` may be a [`TypeCode::recursive`] placeholder.
    pub fn sequence(content: &TypeCode, bound: u32) -> Result<TypeCode, TypeCodeError> {
        check_member(content)?;
        Ok(Builder::build(None, |b| {
            let content = b.import(content);
            Node::Sequence { content, bound }
        }))
    }

    /// An array of `length` (at least 1) elements of `content`.
    pub fn array(content: &TypeCode, length: u32) -> Result<TypeCode, TypeCodeError> {
        check_member(content)?;
        if length == 0 {
            return Err(TypeCodeError::BadParam);
        }
        Ok(Builder::build(None, |b| {
            let content = b.import(content);
            Node::Array { content, length }
        }))
    }

    /// A typedef: `typedef content name`, with repository id `id`.
    pub fn alias(id: &str, name: &str, content: &TypeCode) -> Result<TypeCode, TypeCodeError> {
        check_name(name)?;
        check_member(content)?;
        Ok(Builder::build(None, |b| {
            let content = b.import(content);
            Node::Alias {
                kind: TCKind::Alias,
                id: id.into(),
                name: name.into(),
                content,
            }
        }))
    }

    /// A value box: `valuetype name content`.
    pub fn value_box(id: &str, name: &str, content: &TypeCode) -> Result<TypeCode, TypeCodeError> {
        check_name(name)?;
        check_member(content)?;
        if matches!(content.kind(), TCKind::Value | TCKind::ValueBox) {
            return Err(TypeCodeError::BadParam);
        }
        Ok(Builder::build(None, |b| {
            let content = b.import(content);
            Node::Alias {
                kind: TCKind::ValueBox,
                id: id.into(),
                name: name.into(),
                content,
            }
        }))
    }

    /// The TypeCode of an interface (`Objref`, `AbstractInterface` or
    /// `LocalInterface`), a `Component`, a `Home` or a `Native` type.
    pub fn interface(kind: TCKind, id: &str, name: &str) -> Result<TypeCode, TypeCodeError> {
        if !matches!(
            kind,
            TCKind::Objref
                | TCKind::AbstractInterface
                | TCKind::LocalInterface
                | TCKind::Component
                | TCKind::Home
                | TCKind::Native
        ) {
            return Err(TypeCodeError::BadKind);
        }
        check_name(name)?;
        Ok(TypeCode::single(Node::Named {
            kind,
            id: id.into(),
            name: name.into(),
        }))
    }

    /// The TypeCode of an object reference to the interface `id`.
    pub fn objref(id: &str, name: &str) -> Result<TypeCode, TypeCodeError> {
        TypeCode::interface(TCKind::Objref, id, name)
    }

    /// A struct. Recursive placeholders with the id `id` in the member
    /// types become references to the struct.
    pub fn struct_tc<S: AsRef<str>>(
        id: &str,
        name: &str,
        members: &[(S, TypeCode)],
    ) -> Result<TypeCode, TypeCodeError> {
        TypeCode::struct_like(TCKind::Struct, id, name, members)
    }

    /// An exception, made like a struct.
    pub fn exception<S: AsRef<str>>(
        id: &str,
        name: &str,
        members: &[(S, TypeCode)],
    ) -> Result<TypeCode, TypeCodeError> {
        TypeCode::struct_like(TCKind::Except, id, name, members)
    }

    fn struct_like<S: AsRef<str>>(
        kind: TCKind,
        id: &str,
        name: &str,
        members: &[(S, TypeCode)],
    ) -> Result<TypeCode, TypeCodeError> {
        check_name(name)?;
        check_names(members.iter().map(|(n, _)| n.as_ref()))?;
        for (_, tc) in members {
            check_member(tc)?;
        }
        Ok(Builder::build(Some(id), |b| Node::Struct {
            kind,
            id: id.into(),
            name: name.into(),
            members: members
                .iter()
                .map(|(n, tc)| (n.as_ref().to_owned(), b.import(tc)))
                .collect(),
        }))
    }

    /// A union with the discriminator type `discriminator`. Each member has
    /// a label, `None` for the `default` member, which there can be at most
    /// one of. A label is the value of the discriminator as a number:
    /// `boolean` is 0 or 1, `char` and `wchar` their code, an enumerator
    /// its position. A member with several labels is given once for each.
    pub fn union_tc<S: AsRef<str>>(
        id: &str,
        name: &str,
        discriminator: &TypeCode,
        members: &[(Option<i128>, S, TypeCode)],
    ) -> Result<TypeCode, TypeCodeError> {
        check_name(name)?;
        let disc = discriminator.unaliased();
        if !disc.kind().is_discriminator() {
            return Err(TypeCodeError::BadParam);
        }
        let mut default = -1;
        let mut labels = HashSet::new();
        for (i, (label, _, tc)) in members.iter().enumerate() {
            check_member(tc)?;
            match label {
                None if default >= 0 => return Err(TypeCodeError::BadParam),
                None => default = i as i32,
                Some(l) => {
                    if !disc.label_in_range(*l) || !labels.insert(*l) {
                        return Err(TypeCodeError::BadParam);
                    }
                }
            }
        }
        // The same name is allowed only on consecutive entries (one member
        // with several labels).
        let mut seen = HashSet::new();
        let mut prev: Option<&str> = None;
        for (_, n, _) in members {
            let n = n.as_ref();
            if prev != Some(n) && !n.is_empty() && !seen.insert(n.to_ascii_lowercase()) {
                return Err(TypeCodeError::BadParam);
            }
            prev = Some(n);
        }
        Ok(Builder::build(Some(id), |b| {
            let disc = b.import(discriminator);
            Node::Union {
                id: id.into(),
                name: name.into(),
                disc,
                default,
                members: members
                    .iter()
                    .map(|(l, n, tc)| (*l, n.as_ref().to_owned(), b.import(tc)))
                    .collect(),
            }
        }))
    }

    /// An enum with the given enumerators.
    pub fn enum_tc<S: AsRef<str>>(
        id: &str,
        name: &str,
        members: &[S],
    ) -> Result<TypeCode, TypeCodeError> {
        check_name(name)?;
        check_names(members.iter().map(|n| n.as_ref()))?;
        Ok(TypeCode::single(Node::Enum {
            id: id.into(),
            name: name.into(),
            members: members.iter().map(|n| n.as_ref().to_owned()).collect(),
        }))
    }

    /// A valuetype (`kind` `Value` or `Event`), with its type modifier
    /// (`VM_NONE` 0, `VM_CUSTOM` 1, `VM_ABSTRACT` 2, `VM_TRUNCATABLE` 3),
    /// concrete base type and members with their visibility.
    pub fn value_tc<S: AsRef<str>>(
        kind: TCKind,
        id: &str,
        name: &str,
        modifier: i16,
        base: Option<&TypeCode>,
        members: &[(S, TypeCode, i16)],
    ) -> Result<TypeCode, TypeCodeError> {
        if !matches!(kind, TCKind::Value | TCKind::Event) {
            return Err(TypeCodeError::BadKind);
        }
        check_name(name)?;
        check_names(members.iter().map(|(n, _, _)| n.as_ref()))?;
        for (_, tc, _) in members {
            check_member(tc)?;
        }
        if let Some(base) = base {
            if !matches!(base.kind(), TCKind::Value | TCKind::Event | TCKind::Null) {
                return Err(TypeCodeError::BadParam);
            }
        }
        Ok(Builder::build(Some(id), |b| {
            let base = base
                .filter(|t| t.kind() != TCKind::Null)
                .map(|t| b.import(t));
            Node::Value {
                kind,
                id: id.into(),
                name: name.into(),
                modifier,
                base,
                members: members
                    .iter()
                    .map(|(n, tc, v)| (n.as_ref().to_owned(), b.import(tc), *v))
                    .collect(),
            }
        }))
    }

    /// A placeholder for the struct, union or valuetype with the
    /// repository id `id`, to use in its own member types
    /// (`ORB::create_recursive_tc`). Most operations on it fail with
    /// `BadKind` until it is embedded in the type it stands for.
    pub fn recursive(id: &str) -> TypeCode {
        TypeCode::single(Node::Recursive { id: id.into() })
    }

    // --- operations -------------------------------------------------------

    /// The kind of the type. An unresolved recursive placeholder is `Null`.
    pub fn kind(&self) -> TCKind {
        self.node().kind()
    }

    /// Whether this is an unresolved [`TypeCode::recursive`] placeholder.
    pub fn is_recursive_placeholder(&self) -> bool {
        matches!(self.node(), Node::Recursive { .. })
    }

    /// The repository id of a named type (objref, struct, union, enum,
    /// alias, except, valuetypes, native, interfaces).
    pub fn id(&self) -> Result<&str, TypeCodeError> {
        match self.node() {
            Node::Recursive { id } => Ok(id),
            n => n.id_name().map(|(id, _)| id).ok_or(TypeCodeError::BadKind),
        }
    }

    /// The simple name of a named type.
    pub fn name(&self) -> Result<&str, TypeCodeError> {
        self.node()
            .id_name()
            .map(|(_, name)| name)
            .ok_or(TypeCodeError::BadKind)
    }

    /// The number of members of a struct, union, enum, exception or
    /// valuetype.
    pub fn member_count(&self) -> Result<u32, TypeCodeError> {
        Ok(match self.node() {
            Node::Struct { members, .. } => members.len(),
            Node::Union { members, .. } => members.len(),
            Node::Enum { members, .. } => members.len(),
            Node::Value { members, .. } => members.len(),
            _ => return Err(TypeCodeError::BadKind),
        } as u32)
    }

    /// The name of a member (or of an enumerator).
    pub fn member_name(&self, index: u32) -> Result<&str, TypeCodeError> {
        let i = index as usize;
        let name = match self.node() {
            Node::Struct { members, .. } => members.get(i).map(|m| &m.0),
            Node::Union { members, .. } => members.get(i).map(|m| &m.1),
            Node::Enum { members, .. } => members.get(i),
            Node::Value { members, .. } => members.get(i).map(|m| &m.0),
            _ => return Err(TypeCodeError::BadKind),
        };
        name.map(String::as_str).ok_or(TypeCodeError::Bounds)
    }

    /// The type of a member of a struct, union, exception or valuetype.
    pub fn member_type(&self, index: u32) -> Result<TypeCode, TypeCodeError> {
        let i = index as usize;
        let tc = match self.node() {
            Node::Struct { members, .. } => members.get(i).map(|m| m.1),
            Node::Union { members, .. } => members.get(i).map(|m| m.2),
            Node::Value { members, .. } => members.get(i).map(|m| m.1),
            _ => return Err(TypeCodeError::BadKind),
        };
        tc.map(|t| self.at(t)).ok_or(TypeCodeError::Bounds)
    }

    /// The label of a union member as a number (see
    /// [`TypeCode::union_tc`]), `None` for the default member.
    pub fn member_label_value(&self, index: u32) -> Result<Option<i128>, TypeCodeError> {
        match self.node() {
            Node::Union { members, .. } => members
                .get(index as usize)
                .map(|m| m.0)
                .ok_or(TypeCodeError::Bounds),
            _ => Err(TypeCodeError::BadKind),
        }
    }

    /// The label of a union member as an Any of the discriminator type,
    /// or the octet 0 for the default member, as `TypeCode::member_label`.
    pub fn member_label(&self, index: u32) -> Result<Any, TypeCodeError> {
        let label = self.member_label_value(index)?;
        Ok(match label {
            None => Any::new(
                TypeCode::simple(TCKind::Octet).expect("simple kind"),
                Value::Octet(0),
            ),
            Some(l) => {
                let disc = self.discriminator_type()?;
                let value = disc.label_to_value(l).ok_or(TypeCodeError::BadParam)?;
                Any::new(disc, value)
            }
        })
    }

    /// The discriminator type of a union.
    pub fn discriminator_type(&self) -> Result<TypeCode, TypeCodeError> {
        match self.node() {
            Node::Union { disc, .. } => Ok(self.at(*disc)),
            _ => Err(TypeCodeError::BadKind),
        }
    }

    /// The index of the default member of a union, or -1.
    pub fn default_index(&self) -> Result<i32, TypeCodeError> {
        match self.node() {
            Node::Union { default, .. } => Ok(*default),
            _ => Err(TypeCodeError::BadKind),
        }
    }

    /// The bound of a string or sequence (0 for none), or the length of an
    /// array.
    pub fn length(&self) -> Result<u32, TypeCodeError> {
        match self.node() {
            Node::String { bound, .. } | Node::Sequence { bound, .. } => Ok(*bound),
            Node::Array { length, .. } => Ok(*length),
            _ => Err(TypeCodeError::BadKind),
        }
    }

    /// The element type of a sequence or array, or the type an alias or
    /// value box stands for.
    pub fn content_type(&self) -> Result<TypeCode, TypeCodeError> {
        match self.node() {
            Node::Sequence { content, .. }
            | Node::Array { content, .. }
            | Node::Alias { content, .. } => Ok(self.at(*content)),
            _ => Err(TypeCodeError::BadKind),
        }
    }

    /// The number of digits of a `fixed`.
    pub fn fixed_digits(&self) -> Result<u16, TypeCodeError> {
        match self.node() {
            Node::Fixed { digits, .. } => Ok(*digits),
            _ => Err(TypeCodeError::BadKind),
        }
    }

    /// The scale of a `fixed`.
    pub fn fixed_scale(&self) -> Result<i16, TypeCodeError> {
        match self.node() {
            Node::Fixed { scale, .. } => Ok(*scale),
            _ => Err(TypeCodeError::BadKind),
        }
    }

    /// The visibility of a valuetype member.
    pub fn member_visibility(&self, index: u32) -> Result<i16, TypeCodeError> {
        match self.node() {
            Node::Value { members, .. } => members
                .get(index as usize)
                .map(|m| m.2)
                .ok_or(TypeCodeError::Bounds),
            _ => Err(TypeCodeError::BadKind),
        }
    }

    /// The type modifier of a valuetype.
    pub fn type_modifier(&self) -> Result<i16, TypeCodeError> {
        match self.node() {
            Node::Value { modifier, .. } => Ok(*modifier),
            _ => Err(TypeCodeError::BadKind),
        }
    }

    /// The concrete base type of a valuetype, `None` if it has none.
    pub fn concrete_base_type(&self) -> Result<Option<TypeCode>, TypeCodeError> {
        match self.node() {
            Node::Value { base, .. } => Ok(base.map(|b| self.at(b))),
            _ => Err(TypeCodeError::BadKind),
        }
    }

    /// The type with the aliases (typedefs) removed.
    pub fn unaliased(&self) -> TypeCode {
        let mut index = self.index;
        while let Node::Alias {
            kind: TCKind::Alias,
            content,
            ..
        } = &self.arena.nodes[index as usize]
        {
            index = *content;
        }
        self.at(index)
    }

    /// Whether the two TypeCodes are the same type with the same names and
    /// aliases (`TypeCode::equal`).
    pub fn equal(&self, other: &TypeCode) -> bool {
        Compare {
            equivalent: false,
            seen: HashSet::new(),
        }
        .eq(self, other)
    }

    /// Whether the two TypeCodes describe types with the same CDR, ignoring
    /// aliases and names; types with repository ids are compared by id
    /// (`TypeCode::equivalent`).
    pub fn equivalent(&self, other: &TypeCode) -> bool {
        Compare {
            equivalent: true,
            seen: HashSet::new(),
        }
        .eq(self, other)
    }

    /// The TypeCode without names and member names; repository ids and
    /// aliases stay (`TypeCode::get_compact_typecode`).
    pub fn compact(&self) -> TypeCode {
        let mut nodes = self.arena.nodes.clone();
        for node in &mut nodes {
            match node {
                Node::Named { name, .. } | Node::Alias { name, .. } => name.clear(),
                Node::Enum { name, members, .. } => {
                    name.clear();
                    members.iter_mut().for_each(String::clear);
                }
                Node::Struct { name, members, .. } => {
                    name.clear();
                    members.iter_mut().for_each(|m| m.0.clear());
                }
                Node::Union { name, members, .. } => {
                    name.clear();
                    members.iter_mut().for_each(|m| m.1.clear());
                }
                Node::Value { name, members, .. } => {
                    name.clear();
                    members.iter_mut().for_each(|m| m.0.clear());
                }
                _ => {}
            }
        }
        TypeCode {
            arena: Arc::new(Arena { nodes }),
            index: self.index,
        }
    }

    /// Whether a union label fits this discriminator type.
    fn label_in_range(&self, l: i128) -> bool {
        let (lo, hi): (i128, i128) = match self.kind() {
            TCKind::Short => (i16::MIN.into(), i16::MAX.into()),
            TCKind::UShort => (0, u16::MAX.into()),
            TCKind::Long => (i32::MIN.into(), i32::MAX.into()),
            TCKind::ULong => (0, u32::MAX.into()),
            TCKind::LongLong => (i64::MIN.into(), i64::MAX.into()),
            TCKind::ULongLong => (0, u64::MAX.into()),
            TCKind::Boolean => (0, 1),
            TCKind::Char => (0, 255),
            TCKind::WChar => (0, u16::MAX.into()),
            TCKind::Enum => (0, i128::from(self.member_count().unwrap_or(0)) - 1),
            _ => return false,
        };
        (lo..=hi).contains(&l)
    }

    /// The discriminator value of a label of this (discriminator) type.
    pub(crate) fn label_to_value(&self, l: i128) -> Option<Value> {
        let t = self.unaliased();
        if !t.label_in_range(l) {
            return None;
        }
        Some(match t.kind() {
            TCKind::Short => Value::Short(l as i16),
            TCKind::UShort => Value::UShort(l as u16),
            TCKind::Long => Value::Long(l as i32),
            TCKind::ULong => Value::ULong(l as u32),
            TCKind::LongLong => Value::LongLong(l as i64),
            TCKind::ULongLong => Value::ULongLong(l as u64),
            TCKind::Boolean => Value::Bool(l != 0),
            TCKind::Char => Value::Char(char::from(l as u8)),
            TCKind::WChar => Value::WChar(l as u16),
            TCKind::Enum => Value::Enum(l as u32),
            _ => return None,
        })
    }

    /// The index of the union member the discriminator value `disc`
    /// selects, or `None` when no label matches and there is no default.
    pub fn union_member_for(&self, disc: &Value) -> Result<Option<u32>, TypeCodeError> {
        let Node::Union {
            members, default, ..
        } = self.node()
        else {
            return Err(TypeCodeError::BadKind);
        };
        let key = disc.label_key();
        Ok(members
            .iter()
            .position(|m| m.0.is_some() && m.0 == key)
            .map(|i| i as u32)
            .or(u32::try_from(*default).ok()))
    }

    /// A key that identifies the node, for caches of converted TypeCodes:
    /// two TypeCodes with the same key are the same node of the same
    /// arena.
    pub fn node_key(&self) -> (usize, u32) {
        (Arc::as_ptr(&self.arena) as usize, self.index)
    }
}

impl PartialEq for TypeCode {
    fn eq(&self, other: &TypeCode) -> bool {
        self.equal(other)
    }
}

impl fmt::Debug for TypeCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.node() {
            Node::Recursive { id } => write!(f, "TypeCode(recursive {id})"),
            n => match n.id_name() {
                Some((id, _)) => write!(f, "TypeCode({} {id})", n.kind().name()),
                None => match n {
                    Node::String { bound, .. } if *bound != 0 => {
                        write!(f, "TypeCode({}<{bound}>)", n.kind().name())
                    }
                    Node::Sequence { content, bound } => {
                        write!(f, "TypeCode(sequence<{:?}", self.at(*content))?;
                        if *bound != 0 {
                            write!(f, ", {bound}")?;
                        }
                        f.write_str(">)")
                    }
                    Node::Array { content, length } => {
                        write!(f, "TypeCode({:?}[{length}])", self.at(*content))
                    }
                    Node::Fixed { digits, scale } => {
                        write!(f, "TypeCode(fixed<{digits}, {scale}>)")
                    }
                    _ => write!(f, "TypeCode({})", n.kind().name()),
                },
            },
        }
    }
}

fn check_name(name: &str) -> Result<(), TypeCodeError> {
    // An empty name is allowed (compact TypeCodes); otherwise an IDL
    // identifier.
    let mut chars = name.chars();
    match chars.next() {
        None => Ok(()),
        Some(c) if c.is_alphabetic() || c == '_' => {
            if chars.all(|c| c.is_alphanumeric() || c == '_') {
                Ok(())
            } else {
                Err(TypeCodeError::BadParam)
            }
        }
        Some(_) => Err(TypeCodeError::BadParam),
    }
}

/// Member names must be identifiers and differ ignoring case (empty
/// names, as in compact TypeCodes, are not compared).
fn check_names<'a>(names: impl Iterator<Item = &'a str>) -> Result<(), TypeCodeError> {
    let mut seen = HashSet::new();
    for n in names {
        check_name(n)?;
        if !n.is_empty() && !seen.insert(n.to_ascii_lowercase()) {
            return Err(TypeCodeError::BadParam);
        }
    }
    Ok(())
}

/// Types that cannot be members, elements or alias contents.
fn check_member(tc: &TypeCode) -> Result<(), TypeCodeError> {
    match tc.kind() {
        TCKind::Void | TCKind::Except => Err(TypeCodeError::BadParam),
        TCKind::Null if !tc.is_recursive_placeholder() => Err(TypeCodeError::BadParam),
        _ => Ok(()),
    }
}

/// Copies TypeCodes into a new arena whose first node is the one being
/// made.
struct Builder<'a> {
    nodes: Vec<Node>,
    /// Imported nodes: (arena, index) to the new index.
    map: HashMap<(usize, u32), u32>,
    /// The repository id of the node being made, which recursive
    /// placeholders with that id refer to.
    id: Option<&'a str>,
}

impl<'a> Builder<'a> {
    fn build(id: Option<&'a str>, make: impl FnOnce(&mut Builder<'a>) -> Node) -> TypeCode {
        let mut b = Builder {
            nodes: vec![Node::Simple(TCKind::Null)],
            map: HashMap::new(),
            id,
        };
        let node = make(&mut b);
        b.nodes[0] = node;
        TypeCode {
            arena: Arc::new(Arena { nodes: b.nodes }),
            index: 0,
        }
    }

    fn import(&mut self, tc: &TypeCode) -> u32 {
        let key = tc.node_key();
        if let Some(&i) = self.map.get(&key) {
            return i;
        }
        let node = tc.node();
        if let (Node::Recursive { id }, Some(own)) = (node, self.id) {
            if id == own {
                return 0;
            }
        }
        let i = self.nodes.len() as u32;
        self.map.insert(key, i);
        self.nodes.push(Node::Simple(TCKind::Null));
        let mut copy = node.clone();
        copy.children_mut(|child| *child = self.import(&tc.at(*child)));
        self.nodes[i as usize] = copy;
        i
    }
}

/// `equal` and `equivalent`.
struct Compare {
    equivalent: bool,
    /// Pairs of nodes being compared, assumed equal while they are (for
    /// recursive types).
    seen: HashSet<((usize, u32), (usize, u32))>,
}

impl Compare {
    fn eq(&mut self, a: &TypeCode, b: &TypeCode) -> bool {
        let (a, b) = if self.equivalent {
            (a.unaliased(), b.unaliased())
        } else {
            (a.clone(), b.clone())
        };
        let key = (a.node_key(), b.node_key());
        if key.0 == key.1 || !self.seen.insert(key) {
            return true;
        }
        let (na, nb) = (a.node(), b.node());
        if let (Node::Recursive { id: x }, Node::Recursive { id: y }) = (na, nb) {
            return x == y;
        }
        if na.kind() != nb.kind() {
            return false;
        }
        if let (Some((ia, name_a)), Some((ib, name_b))) = (na.id_name(), nb.id_name()) {
            if self.equivalent {
                if !ia.is_empty() && !ib.is_empty() {
                    return ia == ib;
                }
            } else if ia != ib || name_a != name_b {
                return false;
            }
        }
        let equivalent = self.equivalent;
        let names = move |x: &str, y: &str| equivalent || x == y;
        match (na, nb) {
            (Node::String { bound: x, .. }, Node::String { bound: y, .. }) => x == y,
            (
                Node::Fixed {
                    digits: d1,
                    scale: s1,
                },
                Node::Fixed {
                    digits: d2,
                    scale: s2,
                },
            ) => d1 == d2 && s1 == s2,
            (Node::Struct { members: m1, .. }, Node::Struct { members: m2, .. }) => {
                m1.len() == m2.len()
                    && m1.iter().zip(m2).all(|((n1, _), (n2, _))| names(n1, n2))
                    && m1
                        .iter()
                        .zip(m2)
                        .all(|((_, t1), (_, t2))| self.eq(&a.at(*t1), &b.at(*t2)))
            }
            (
                Node::Union {
                    disc: d1,
                    default: f1,
                    members: m1,
                    ..
                },
                Node::Union {
                    disc: d2,
                    default: f2,
                    members: m2,
                    ..
                },
            ) => {
                f1 == f2
                    && m1.len() == m2.len()
                    && m1
                        .iter()
                        .zip(m2)
                        .all(|((l1, n1, _), (l2, n2, _))| l1 == l2 && names(n1, n2))
                    && self.eq(&a.at(*d1), &b.at(*d2))
                    && m1
                        .iter()
                        .zip(m2)
                        .all(|((_, _, t1), (_, _, t2))| self.eq(&a.at(*t1), &b.at(*t2)))
            }
            (Node::Enum { members: m1, .. }, Node::Enum { members: m2, .. }) => {
                m1.len() == m2.len() && m1.iter().zip(m2).all(|(x, y)| names(x, y))
            }
            (
                Node::Sequence {
                    content: c1,
                    bound: b1,
                },
                Node::Sequence {
                    content: c2,
                    bound: b2,
                },
            ) => b1 == b2 && self.eq(&a.at(*c1), &b.at(*c2)),
            (
                Node::Array {
                    content: c1,
                    length: l1,
                },
                Node::Array {
                    content: c2,
                    length: l2,
                },
            ) => l1 == l2 && self.eq(&a.at(*c1), &b.at(*c2)),
            (Node::Alias { content: c1, .. }, Node::Alias { content: c2, .. }) => {
                self.eq(&a.at(*c1), &b.at(*c2))
            }
            (
                Node::Value {
                    modifier: v1,
                    base: b1,
                    members: m1,
                    ..
                },
                Node::Value {
                    modifier: v2,
                    base: b2,
                    members: m2,
                    ..
                },
            ) => {
                v1 == v2
                    && m1.len() == m2.len()
                    && m1
                        .iter()
                        .zip(m2)
                        .all(|((n1, _, x), (n2, _, y))| x == y && names(n1, n2))
                    && match (b1, b2) {
                        (None, None) => true,
                        (Some(x), Some(y)) => self.eq(&a.at(*x), &b.at(*y)),
                        _ => false,
                    }
                    && m1
                        .iter()
                        .zip(m2)
                        .all(|((_, t1, _), (_, t2, _))| self.eq(&a.at(*t1), &b.at(*t2)))
            }
            // Simple kinds and the named ones without other parameters.
            _ => true,
        }
    }
}

#[cfg(test)]
mod tests;

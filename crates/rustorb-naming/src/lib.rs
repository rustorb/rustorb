//! CosNaming (Naming Service): NamingContext, NamingContextExt and
//! BindingIterator, with persistence behind a storage trait.
//!
//! The client side is implemented so far: [`NamingContext`],
//! [`NamingContextExt`] and [`BindingIterator`] are stubs for the
//! interfaces of the CosNaming IDL (Naming Service 1.3), written by hand
//! until the IDL compiler exists. The user exceptions of the operations
//! are the variants of [`NamingError`].
//!
//! ```no_run
//! use rustorb_core::Orb;
//! use rustorb_naming::{NamingContextExt, to_name};
//!
//! let mut args = vec![
//!     "-ORBInitRef".to_owned(),
//!     "NameService=corbaloc::localhost:2809/NameService".to_owned(),
//! ];
//! let orb = Orb::init(&mut args)?;
//! let obj = orb.resolve_initial_references("NameService")?;
//! let root = NamingContextExt::narrow(obj)?.ok_or("not a NamingContextExt")?;
//! root.bind_new_context(&to_name("app")?)?;
//! let echo = root.resolve_str("app/Echo")?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! Names and their string form ([`Name`], [`to_name`], [`to_string`]) come
//! from [`rustorb_core::name`], which `corbaname:` URLs use too:
//! `Orb::string_to_object("corbaname::host:2809#app/Echo")` resolves the
//! name without this crate.

mod client;
mod error;

pub use client::{Binding, BindingIterator, BindingType, NamingContext, NamingContextExt, to_url};
pub use error::{NamingError, NotFoundReason};
pub use rustorb_core::name::{InvalidName, Name, NameComponent, to_name, to_string};

/// Repository ids of the CosNaming interfaces and exceptions.
pub mod repository_id {
    /// `CosNaming::NamingContext`.
    pub const NAMING_CONTEXT: &str = "IDL:omg.org/CosNaming/NamingContext:1.0";
    /// `CosNaming::NamingContextExt`.
    pub const NAMING_CONTEXT_EXT: &str = "IDL:omg.org/CosNaming/NamingContextExt:1.0";
    /// `CosNaming::BindingIterator`.
    pub const BINDING_ITERATOR: &str = "IDL:omg.org/CosNaming/BindingIterator:1.0";
    /// `CosNaming::NamingContext::NotFound`.
    pub const NOT_FOUND: &str = "IDL:omg.org/CosNaming/NamingContext/NotFound:1.0";
    /// `CosNaming::NamingContext::CannotProceed`.
    pub const CANNOT_PROCEED: &str = "IDL:omg.org/CosNaming/NamingContext/CannotProceed:1.0";
    /// `CosNaming::NamingContext::InvalidName`.
    pub const INVALID_NAME: &str = "IDL:omg.org/CosNaming/NamingContext/InvalidName:1.0";
    /// `CosNaming::NamingContext::AlreadyBound`.
    pub const ALREADY_BOUND: &str = "IDL:omg.org/CosNaming/NamingContext/AlreadyBound:1.0";
    /// `CosNaming::NamingContext::NotEmpty`.
    pub const NOT_EMPTY: &str = "IDL:omg.org/CosNaming/NamingContext/NotEmpty:1.0";
    /// `CosNaming::NamingContextExt::InvalidAddress`.
    pub const INVALID_ADDRESS: &str = "IDL:omg.org/CosNaming/NamingContextExt/InvalidAddress:1.0";
}

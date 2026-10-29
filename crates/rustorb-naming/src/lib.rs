//! CosNaming (Naming Service): NamingContext, NamingContextExt and
//! BindingIterator, with persistence behind a storage trait.
//!
//! The client side: [`NamingContext`], [`NamingContextExt`] and
//! [`BindingIterator`] are stubs for the interfaces of the CosNaming IDL
//! (Naming Service 1.3), over the ones `rustorb-idl -brust` generates in
//! [`idl::cos_naming`], with the names of [`rustorb_core::name`]. The user
//! exceptions of the operations are the variants of [`NamingError`].
//! Code generated from IDL that uses CosNaming refers to [`idl::cos_naming`]
//! (`-Wbextern=CosNaming=rustorb_naming::idl::cos_naming`).
//!
//! The server side: [`NamingService`] serves the naming contexts on an
//! ORB, with the root context at `corbaloc::host:port/NameService` like
//! omniNames, and keeps the bindings in a [`Store`]: [`LogStore`] (an
//! append-only log and checkpoints) by default, or [`MemoryStore`]. The
//! `rustorb-names` binary is the daemon.
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
mod server;
pub mod store;

/// The code `rustorb-idl -brust` generates from the CosNaming IDL
/// (`crates/rustorb-codegen/generate-rust.sh` writes `src/idl/`).
pub mod idl {
    include!("idl/cos_naming.rs");
}

pub use client::{Binding, BindingIterator, BindingType, NamingContext, NamingContextExt, to_url};
pub use error::{NamingError, NotFoundReason};
pub use rustorb_core::name::{InvalidName, Name, NameComponent, to_name, to_string};
pub use server::{NamingService, ROOT_KEY, StartError};
pub use store::{LogStore, MemoryStore, Store};

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

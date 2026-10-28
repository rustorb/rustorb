//! ORB core: connection management, request dispatch and the POA.
//!
//! Network I/O runs on an internal multi-threaded tokio runtime, and
//! servants are invoked on a separate pool of long-lived ORB threads.
//! The public API is synchronous.
//!
//! The client side is implemented so far:
//!
//! - [`Orb`] owns the runtime and a pool of GIOP connections, one per
//!   server endpoint. [`Orb::string_to_object`] accepts stringified IORs
//!   and `corbaloc:` URLs, and [`Orb::resolve_initial_references`] the
//!   `-ORBInitRef` / `-ORBDefaultInitRef` options.
//! - [`ObjectRef::request`] builds a request whose arguments are written to
//!   an [`OutStream`]; [`Request::invoke`] returns a [`Reply`] to read with
//!   an [`InStream`], or an [`Exception`]. Requests use GIOP 1.0 to 1.2 as
//!   the target's profile allows, follow `LOCATION_FORWARD`, answer
//!   `NEEDS_ADDRESSING_MODE`, try alternate addresses, and are sent again
//!   after a `CloseConnection` from the server.
//! - [`CodeSets`] negotiates the transmission code sets from the IOR and
//!   converts `string` data; rustorb's native code sets are UTF-8 and
//!   UTF-16.

mod codeset;
mod connection;
mod exception;
mod object;
mod orb;
mod stream;

pub use codeset::CodeSets;
pub use exception::{
    Exception, OMG_VMCID, SystemException, SystemExceptionKind, UserException, minor,
};
pub use object::{ObjectRef, Reply, Request};
pub use orb::{Orb, OrbOptions, ResolveError};
pub use rustorb_cdr as cdr;
pub use rustorb_giop as giop;
pub use rustorb_giop::CompletionStatus;
pub use stream::{InStream, OutStream};

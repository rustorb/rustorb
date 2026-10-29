//! ORB core: connection management, request dispatch and the POA.
//!
//! Network I/O runs on an internal multi-threaded tokio runtime, and
//! servants are invoked on a separate pool of long-lived ORB threads.
//! The public API is synchronous.
//!
//! The client side:
//!
//! - [`Orb`] owns the runtime and a pool of GIOP connections, one per
//!   server endpoint. [`Orb::string_to_object`] accepts stringified IORs,
//!   `corbaloc:` and `corbaname:` URLs, and
//!   [`Orb::resolve_initial_references`] the
//!   `-ORBInitRef` / `-ORBDefaultInitRef` options.
//! - [`ObjectRef::request`] builds a request whose arguments are written to
//!   an [`OutStream`]; [`Request::invoke`] returns a [`Reply`] to read with
//!   an [`InStream`], or an [`Exception`]. Requests use GIOP 1.0 to 1.2 as
//!   the target's profile allows, follow `LOCATION_FORWARD`, answer
//!   `NEEDS_ADDRESSING_MODE`, try alternate addresses, and are sent again
//!   after a `CloseConnection` from the server.
//! - [`name`] has the CosNaming name type and its string form, used for
//!   `corbaname:` and by the naming service client in `rustorb-naming`.
//! - [`TypeCode`] describes IDL types, and [`Any`] holds a [`Value`] of
//!   any type; [`OutStream::write_any`] and [`InStream::read_any`] marshal
//!   them, and [`OutStream::write_value`] / [`InStream::read_value`]
//!   marshal any value as its TypeCode says.
//! - [`CodeSets`] negotiates the transmission code sets from the IOR and
//!   converts `string` data; rustorb's native code sets are UTF-8 and
//!   UTF-16.
//!
//! The server side, started by [`Orb::root_poa`]:
//!
//! - The ORB listens on [`OrbOptions::endpoints`] and answers GIOP 1.0 to
//!   1.2 requests in the version they came in.
//! - [`Poa`] is the RootPOA and the POAs created under it, with their
//!   [`Policy`] values, active object maps and [`PoaManager`]s. Object keys
//!   of PERSISTENT POAs have omniORB's format, so references survive a
//!   switch between the two.
//! - A [`Servant`] reads its arguments from an [`InStream`] and writes the
//!   result or a user exception to a [`ServerReply`]. Servants run on a
//!   pool of long-lived upcall threads, never on the I/O threads, and
//!   requests of one connection run in parallel. SINGLE_THREAD_MODEL POAs
//!   run their requests one at a time, and MAIN_THREAD_MODEL ones on the
//!   thread in [`Orb::run`] or [`Orb::perform_work`].
//! - Objects not in the active object map can be served by a default
//!   servant, a [`ServantActivator`] or a [`ServantLocator`], and child
//!   POAs created on demand by an [`AdapterActivator`]. [`PoaCurrent`]
//!   tells a servant which object a request is for.
//! - Calls to objects of the same ORB do not use the network: the request
//!   is marshalled and dispatched on the calling thread.
//! - [`Orb::run`] waits for [`Orb::shutdown`], which finishes the running
//!   requests and closes every connection with `CloseConnection`.

mod codeset;
mod connection;
mod exception;
mod fixed;
pub mod idl;
pub mod name;
mod object;
mod orb;
mod poa;
mod pool;
mod servant;
mod server;
mod stream;
pub mod typecode;
mod value;

pub use codeset::CodeSets;
pub use exception::{
    Exception, OMG_VMCID, SystemException, SystemExceptionKind, UserException, minor,
};
pub use fixed::{Fixed, ParseFixedError};
pub use object::{ObjectRef, Reply, Request};
pub use orb::{EndpointAddr, Orb, OrbOptions, ResolveError};
pub use poa::{
    IdAssignment, IdUniqueness, ImplicitActivation, Lifespan, ObjectId, Poa, PoaCurrent, PoaError,
    PoaManager, PoaManagerState, Policies, Policy, RequestProcessing, ServantRetention,
    ThreadPolicy,
};
pub use rustorb_cdr as cdr;
pub use rustorb_giop as giop;
pub use rustorb_giop::CompletionStatus;
pub use servant::{
    AdapterActivator, Cookie, Servant, ServantActivator, ServantLocator, ServantManager,
    ServantManagerError, ServerReply,
};
pub use stream::{InStream, OutStream};
pub use typecode::{TCKind, TypeCode, TypeCodeError};
pub use value::{
    Any, Value, ValueRef, ValueTypeResolver, lookup_value_type, register_value_type,
    set_value_type_resolver, unregister_value_type,
};

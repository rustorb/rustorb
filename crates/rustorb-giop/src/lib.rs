//! GIOP/IIOP message types, IOR and corbaloc handling.
//!
//! Accepts GIOP 1.0 to 1.2 and sends GIOP 1.2.
//!
//! - [`MessageHeader`] parses and writes the 12-byte GIOP header.
//! - [`decode_message`] turns one complete message into a [`Message`] whose
//!   body can be read with a CDR decoder that keeps the message alignment.
//! - [`MessageEncoder`] writes a message header and lets the caller append
//!   the body; it patches the message size when finished. Replies use the
//!   version of the request, so every version can be written.
//! - [`FragmentAssembler`] joins GIOP 1.1/1.2 fragments into one message,
//!   keeping the alignment of each fragment.
//! - [`Ior`] reads and writes stringified IORs; [`IiopProfile`] interprets
//!   IIOP 1.0 to 1.2 profile bodies and their [`TaggedComponent`]s, with
//!   typed access to the ORB type, code sets and alternate addresses.
//! - [`Corbaloc`], [`CorbaName`] and [`ObjectUrl`] parse and write
//!   `corbaloc:` and `corbaname:` URLs.

mod component;
mod corbaloc;
mod error;
mod fragment;
mod header;
mod iiop;
mod ior;
mod message;
mod service_context;
mod writer;

pub use component::{
    AlternateIiopAddress, CodeSetComponent, CodeSetComponentInfo, SslSecTrans, TaggedComponent,
    association_options, component_tag, orb_type, orb_type_component,
};
pub use corbaloc::{
    CorbaName, Corbaloc, DEFAULT_CORBALOC_PORT, DEFAULT_CORBANAME_KEY, IiopAddr, ObjAddr, ObjectUrl,
};
pub use error::{Error, Result};
pub use fragment::{AssembledMessage, FragmentAssembler};
pub use header::{MessageHeader, MsgType, Version};
pub use iiop::IiopProfile;
pub use ior::{Ior, TAG_INTERNET_IOP, TAG_MULTIPLE_COMPONENTS, TaggedProfile};
pub use message::{
    AddressingDisposition, Body, CancelRequestHeader, CompletionStatus, FragmentHeader,
    LocateReplyHeader, LocateRequestHeader, LocateStatus, Message, ReplyHeader, ReplyStatus,
    RequestHeader, SystemExceptionBody, TargetAddress, decode_message, response_flags,
};
pub use service_context::{CodeSetContext, ServiceContext, code_set, service_id};
pub use writer::{MessageEncoder, OutMessage};

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
//! - [`FragmentAssembler`] joins GIOP 1.1/1.2 fragments into one message.
//!
//! IOR parsing beyond the raw [`Ior`] structure and corbaloc come later.

mod error;
mod fragment;
mod header;
mod ior;
mod message;
mod service_context;
mod writer;

pub use error::{Error, Result};
pub use fragment::FragmentAssembler;
pub use header::{MessageHeader, MsgType, Version};
pub use ior::{Ior, TAG_INTERNET_IOP, TAG_MULTIPLE_COMPONENTS, TaggedProfile};
pub use message::{
    AddressingDisposition, Body, CancelRequestHeader, CompletionStatus, FragmentHeader,
    LocateReplyHeader, LocateRequestHeader, LocateStatus, Message, ReplyHeader, ReplyStatus,
    RequestHeader, SystemExceptionBody, TargetAddress, decode_message, response_flags,
};
pub use service_context::{CodeSetContext, ServiceContext, code_set, service_id};
pub use writer::MessageEncoder;

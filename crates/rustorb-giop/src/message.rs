use std::sync::Arc;

use rustorb_cdr::{ByteOrder, CdrDecode, CdrEncode, Decoder, Encoder, Encoding, Segment};

use crate::{Error, Ior, MessageHeader, MsgType, Result, ServiceContext, TaggedProfile, Version};

/// Values of the GIOP 1.2 `response_flags` octet.
pub mod response_flags {
    /// Oneway with no synchronisation (`SYNC_NONE`, `SYNC_WITH_TRANSPORT`).
    pub const NONE: u8 = 0x00;
    /// Oneway answered once the server has the request (`SYNC_WITH_SERVER`).
    pub const SYNC_WITH_SERVER: u8 = 0x01;
    /// Normal two-way call (`SYNC_WITH_TARGET`).
    pub const SYNC_WITH_TARGET: u8 = 0x03;
}

/// `GIOP::AddressingDisposition`, the kind of [`TargetAddress`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AddressingDisposition {
    /// `KeyAddr` (0).
    Key,
    /// `ProfileAddr` (1).
    Profile,
    /// `ReferenceAddr` (2).
    Reference,
}

impl AddressingDisposition {
    /// The discriminator value.
    pub fn to_i16(self) -> i16 {
        self as i16
    }

    /// Converts a discriminator value.
    pub fn from_i16(value: i16) -> Result<AddressingDisposition> {
        match value {
            0 => Ok(AddressingDisposition::Key),
            1 => Ok(AddressingDisposition::Profile),
            2 => Ok(AddressingDisposition::Reference),
            other => Err(Error::InvalidAddressingDisposition(other)),
        }
    }
}

/// `GIOP::TargetAddress`, the object a request is sent to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetAddress {
    /// The object key from the profile.
    Key(Vec<u8>),
    /// A whole profile.
    Profile(TaggedProfile),
    /// An IOR and the index of the profile that was used.
    Reference {
        /// Index into `ior.profiles`.
        selected_profile_index: u32,
        /// The full reference.
        ior: Ior,
    },
}

impl TargetAddress {
    /// The addressing disposition of this target.
    pub fn disposition(&self) -> AddressingDisposition {
        match self {
            TargetAddress::Key(_) => AddressingDisposition::Key,
            TargetAddress::Profile(_) => AddressingDisposition::Profile,
            TargetAddress::Reference { .. } => AddressingDisposition::Reference,
        }
    }

    /// The object key when addressed by key.
    pub fn object_key(&self) -> Option<&[u8]> {
        match self {
            TargetAddress::Key(key) => Some(key),
            _ => None,
        }
    }

    pub(crate) fn encode(&self, enc: &mut Encoder) -> Result<()> {
        enc.write_i16(self.disposition().to_i16());
        match self {
            TargetAddress::Key(key) => enc.write_octet_seq(key)?,
            TargetAddress::Profile(profile) => profile.encode(enc)?,
            TargetAddress::Reference {
                selected_profile_index,
                ior,
            } => {
                enc.write_u32(*selected_profile_index);
                ior.encode(enc)?;
            }
        }
        Ok(())
    }

    pub(crate) fn decode(dec: &mut Decoder<'_>) -> Result<TargetAddress> {
        Ok(match AddressingDisposition::from_i16(dec.read_i16()?)? {
            AddressingDisposition::Key => TargetAddress::Key(dec.read_octet_seq()?.to_vec()),
            AddressingDisposition::Profile => TargetAddress::Profile(TaggedProfile::decode(dec)?),
            AddressingDisposition::Reference => TargetAddress::Reference {
                selected_profile_index: dec.read_u32()?,
                ior: Ior::decode(dec)?,
            },
        })
    }
}

/// The header of a `Request` message, in its GIOP 1.2 shape.
///
/// GIOP 1.0/1.1 requests are mapped onto it: `response_expected` becomes
/// [`response_flags::SYNC_WITH_TARGET`] or [`response_flags::NONE`], and the
/// object key becomes [`TargetAddress::Key`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestHeader {
    /// Request id, unique per connection and direction.
    pub request_id: u32,
    /// See [`response_flags`].
    pub response_flags: u8,
    /// The target object.
    pub target: TargetAddress,
    /// Operation name, or `_get_x`/`_set_x` for attributes.
    pub operation: String,
    /// Service contexts.
    pub service_context: Vec<ServiceContext>,
    /// `requesting_principal`, only sent by GIOP 1.0 and 1.1 and usually
    /// empty.
    pub requesting_principal: Vec<u8>,
}

impl RequestHeader {
    /// Whether the client waits for a reply.
    pub fn response_expected(&self) -> bool {
        self.response_flags & 0x01 != 0
    }
}

/// `GIOP::ReplyStatusType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReplyStatus {
    /// `NO_EXCEPTION` (0).
    NoException,
    /// `USER_EXCEPTION` (1).
    UserException,
    /// `SYSTEM_EXCEPTION` (2); the body is a [`SystemExceptionBody`].
    SystemException,
    /// `LOCATION_FORWARD` (3); the body is an [`Ior`].
    LocationForward,
    /// `LOCATION_FORWARD_PERM` (4), GIOP 1.2.
    LocationForwardPerm,
    /// `NEEDS_ADDRESSING_MODE` (5), GIOP 1.2; the body is an addressing
    /// disposition.
    NeedsAddressingMode,
}

impl ReplyStatus {
    /// The wire value.
    pub fn to_u32(self) -> u32 {
        self as u32
    }

    /// Converts a wire value.
    pub fn from_u32(value: u32) -> Result<ReplyStatus> {
        Ok(match value {
            0 => ReplyStatus::NoException,
            1 => ReplyStatus::UserException,
            2 => ReplyStatus::SystemException,
            3 => ReplyStatus::LocationForward,
            4 => ReplyStatus::LocationForwardPerm,
            5 => ReplyStatus::NeedsAddressingMode,
            other => return Err(Error::InvalidReplyStatus(other)),
        })
    }
}

/// The header of a `Reply` message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplyHeader {
    /// Id of the request this answers.
    pub request_id: u32,
    /// Outcome of the request.
    pub reply_status: ReplyStatus,
    /// Service contexts.
    pub service_context: Vec<ServiceContext>,
}

/// The header of a `CancelRequest` message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CancelRequestHeader {
    /// Id of the request to cancel.
    pub request_id: u32,
}

/// The header of a `LocateRequest` message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocateRequestHeader {
    /// Request id.
    pub request_id: u32,
    /// The object to locate; always [`TargetAddress::Key`] for GIOP 1.0/1.1.
    pub target: TargetAddress,
}

/// `GIOP::LocateStatusType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LocateStatus {
    /// `UNKNOWN_OBJECT` (0).
    UnknownObject,
    /// `OBJECT_HERE` (1).
    ObjectHere,
    /// `OBJECT_FORWARD` (2); the body is an [`Ior`].
    ObjectForward,
    /// `OBJECT_FORWARD_PERM` (3), GIOP 1.2.
    ObjectForwardPerm,
    /// `LOC_SYSTEM_EXCEPTION` (4), GIOP 1.2.
    LocSystemException,
    /// `LOC_NEEDS_ADDRESSING_MODE` (5), GIOP 1.2.
    LocNeedsAddressingMode,
}

impl LocateStatus {
    /// The wire value.
    pub fn to_u32(self) -> u32 {
        self as u32
    }

    /// Converts a wire value.
    pub fn from_u32(value: u32) -> Result<LocateStatus> {
        Ok(match value {
            0 => LocateStatus::UnknownObject,
            1 => LocateStatus::ObjectHere,
            2 => LocateStatus::ObjectForward,
            3 => LocateStatus::ObjectForwardPerm,
            4 => LocateStatus::LocSystemException,
            5 => LocateStatus::LocNeedsAddressingMode,
            other => return Err(Error::InvalidLocateStatus(other)),
        })
    }
}

/// The header of a `LocateReply` message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocateReplyHeader {
    /// Id of the locate request this answers.
    pub request_id: u32,
    /// Outcome.
    pub locate_status: LocateStatus,
}

/// The header of a `Fragment` message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FragmentHeader {
    /// Id of the fragmented message; GIOP 1.2 only.
    pub request_id: Option<u32>,
}

/// The body of a message: the bytes after the message header, kept with
/// their offset in the message so that CDR alignment is preserved.
///
/// The bytes are shared with the buffer the message was read into, not
/// copied: cloning a body is cheap.
#[derive(Debug, Clone)]
pub struct Body {
    buf: Arc<Vec<u8>>,
    start: usize,
    offset: usize,
    segments: Vec<Segment>,
    byte_order: ByteOrder,
    version: Encoding,
}

impl PartialEq for Body {
    fn eq(&self, other: &Body) -> bool {
        self.as_bytes() == other.as_bytes()
            && self.offset == other.offset
            && self.segments == other.segments
            && self.byte_order == other.byte_order
            && self.version == other.version
    }
}

impl Eq for Body {}

impl Body {
    fn new(dec: &Decoder<'_>, src: &Source<'_>) -> Body {
        let start = src.buf.len() - dec.remaining();
        Body {
            buf: Arc::clone(src.buf),
            start,
            offset: dec.position(),
            segments: src
                .segments
                .iter()
                .filter(|s| s.start > start)
                .map(|s| Segment {
                    start: s.start - start,
                    offset: s.offset,
                })
                .collect(),
            byte_order: dec.byte_order(),
            version: dec.encoding(),
        }
    }

    /// The body bytes.
    pub fn as_bytes(&self) -> &[u8] {
        &self.buf[self.start..]
    }

    /// Offset of the first body byte from the start of the message.
    pub fn offset(&self) -> usize {
        self.offset
    }

    /// Byte order of the body.
    pub fn byte_order(&self) -> ByteOrder {
        self.byte_order
    }

    /// Whether the body is empty.
    pub fn is_empty(&self) -> bool {
        self.as_bytes().is_empty()
    }

    /// Where the alignment restarts inside the body: one segment for each
    /// fragment after the first in a message joined by a
    /// [`FragmentAssembler`](crate::FragmentAssembler), empty otherwise.
    pub fn segments(&self) -> &[Segment] {
        &self.segments
    }

    /// A CDR decoder over the body with the message's alignment.
    pub fn decoder(&self) -> Decoder<'_> {
        let mut dec =
            Decoder::with_offset(self.as_bytes(), self.byte_order, self.version, self.offset);
        dec.set_segments(&self.segments);
        dec.set_shared(&self.buf);
        dec
    }
}

/// The message a body is decoded from: its bytes and alignment segments.
struct Source<'a> {
    buf: &'a Arc<Vec<u8>>,
    segments: &'a [Segment],
}

/// A decoded GIOP message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    /// `Request`; the body holds the in and inout arguments.
    Request {
        /// Request header.
        header: RequestHeader,
        /// Arguments.
        body: Body,
    },
    /// `Reply`; the body depends on the reply status.
    Reply {
        /// Reply header.
        header: ReplyHeader,
        /// Result, exception or forward reference.
        body: Body,
    },
    /// `CancelRequest`.
    CancelRequest(CancelRequestHeader),
    /// `LocateRequest`.
    LocateRequest(LocateRequestHeader),
    /// `LocateReply`; the body depends on the locate status.
    LocateReply {
        /// Locate reply header.
        header: LocateReplyHeader,
        /// Forward reference or exception, if any.
        body: Body,
    },
    /// `CloseConnection`.
    CloseConnection,
    /// `MessageError`.
    MessageError,
    /// `Fragment`; the body continues an earlier message.
    Fragment {
        /// Fragment header.
        header: FragmentHeader,
        /// Continuation bytes.
        body: Body,
    },
}

impl Message {
    /// The message type.
    pub fn msg_type(&self) -> MsgType {
        match self {
            Message::Request { .. } => MsgType::Request,
            Message::Reply { .. } => MsgType::Reply,
            Message::CancelRequest(_) => MsgType::CancelRequest,
            Message::LocateRequest(_) => MsgType::LocateRequest,
            Message::LocateReply { .. } => MsgType::LocateReply,
            Message::CloseConnection => MsgType::CloseConnection,
            Message::MessageError => MsgType::MessageError,
            Message::Fragment { .. } => MsgType::Fragment,
        }
    }
}

/// Decodes one complete GIOP message: the header followed by exactly
/// `message_size` bytes.
///
/// A message with the more-fragments flag set is decoded only as far as
/// its own bytes allow; pass fragments through a
/// [`FragmentAssembler`](crate::FragmentAssembler) first to get the whole
/// message.
pub fn decode_message(buf: &[u8]) -> Result<(MessageHeader, Message)> {
    decode_segmented(&Arc::new(buf.to_vec()), &[])
}

/// Decodes a message whose alignment restarts at `segments`, as one joined
/// from fragments. The bodies share `buf`.
pub(crate) fn decode_segmented(
    buf: &Arc<Vec<u8>>,
    segments: &[Segment],
) -> Result<(MessageHeader, Message)> {
    let header = MessageHeader::parse(buf)?;
    let expected = header.total_len();
    if buf.len() < expected {
        return Err(Error::Truncated {
            expected,
            actual: buf.len(),
        });
    }
    if buf.len() > expected {
        return Err(Error::TrailingBytes(buf.len() - expected));
    }
    let mut dec = Decoder::new(buf, header.byte_order, header.version.cdr_version());
    dec.set_segments(segments);
    dec.skip(MessageHeader::SIZE)?;
    let src = Source { buf, segments };
    let message = decode_body(&header, &mut dec, &src)?;
    Ok((header, message))
}

fn decode_body(header: &MessageHeader, dec: &mut Decoder<'_>, src: &Source<'_>) -> Result<Message> {
    let v12 = header.version >= Version::V1_2;
    Ok(match header.msg_type {
        MsgType::Request => {
            let header = if v12 {
                let request_id = dec.read_u32()?;
                let response_flags = dec.read_octet()?;
                dec.skip(3)?;
                let target = TargetAddress::decode(dec)?;
                let operation = dec.read_string()?;
                let service_context = Vec::decode(dec)?;
                RequestHeader {
                    request_id,
                    response_flags,
                    target,
                    operation,
                    service_context,
                    requesting_principal: Vec::new(),
                }
            } else {
                let service_context = Vec::decode(dec)?;
                let request_id = dec.read_u32()?;
                let response_expected = dec.read_bool()?;
                if header.version.minor == 1 {
                    dec.skip(3)?;
                }
                let object_key = dec.read_octet_seq()?.to_vec();
                let operation = dec.read_string()?;
                let requesting_principal = dec.read_octet_seq()?.to_vec();
                RequestHeader {
                    request_id,
                    response_flags: if response_expected {
                        response_flags::SYNC_WITH_TARGET
                    } else {
                        response_flags::NONE
                    },
                    target: TargetAddress::Key(object_key),
                    operation,
                    service_context,
                    requesting_principal,
                }
            };
            Message::Request {
                header,
                body: body_after_header(dec, src, v12),
            }
        }
        MsgType::Reply => {
            let header = if v12 {
                let request_id = dec.read_u32()?;
                let reply_status = ReplyStatus::from_u32(dec.read_u32()?)?;
                let service_context = Vec::decode(dec)?;
                ReplyHeader {
                    request_id,
                    reply_status,
                    service_context,
                }
            } else {
                let service_context = Vec::decode(dec)?;
                let request_id = dec.read_u32()?;
                let reply_status = ReplyStatus::from_u32(dec.read_u32()?)?;
                ReplyHeader {
                    request_id,
                    reply_status,
                    service_context,
                }
            };
            Message::Reply {
                header,
                body: body_after_header(dec, src, v12),
            }
        }
        MsgType::CancelRequest => Message::CancelRequest(CancelRequestHeader {
            request_id: dec.read_u32()?,
        }),
        MsgType::LocateRequest => {
            let request_id = dec.read_u32()?;
            let target = if v12 {
                TargetAddress::decode(dec)?
            } else {
                TargetAddress::Key(dec.read_octet_seq()?.to_vec())
            };
            Message::LocateRequest(LocateRequestHeader { request_id, target })
        }
        MsgType::LocateReply => {
            let header = LocateReplyHeader {
                request_id: dec.read_u32()?,
                locate_status: LocateStatus::from_u32(dec.read_u32()?)?,
            };
            Message::LocateReply {
                header,
                body: body_after_header(dec, src, v12),
            }
        }
        MsgType::CloseConnection => Message::CloseConnection,
        MsgType::MessageError => Message::MessageError,
        MsgType::Fragment => {
            let request_id = if v12 { Some(dec.read_u32()?) } else { None };
            Message::Fragment {
                header: FragmentHeader { request_id },
                body: Body::new(dec, src),
            }
        }
    })
}

/// In GIOP 1.2 a body is aligned to 8 bytes. Senders may leave out the
/// padding when there is no body, so padding is only skipped when there
/// are bytes past it.
fn body_after_header(dec: &mut Decoder<'_>, src: &Source<'_>, v12: bool) -> Body {
    if v12 {
        let pad = (8 - dec.position() % 8) % 8;
        if dec.remaining() > pad {
            dec.skip(pad).expect("checked remaining");
        } else {
            dec.skip(dec.remaining()).expect("checked remaining");
        }
    }
    Body::new(dec, src)
}

/// `CORBA::CompletionStatus`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CompletionStatus {
    /// `COMPLETED_YES` (0).
    Yes,
    /// `COMPLETED_NO` (1).
    No,
    /// `COMPLETED_MAYBE` (2).
    Maybe,
}

/// The body of a `SYSTEM_EXCEPTION` reply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemExceptionBody {
    /// Repository id such as `IDL:omg.org/CORBA/OBJECT_NOT_EXIST:1.0`.
    pub exception_id: String,
    /// Minor code.
    pub minor_code_value: u32,
    /// Whether the operation completed.
    pub completion_status: CompletionStatus,
}

impl CdrEncode for SystemExceptionBody {
    fn encode(&self, enc: &mut Encoder) -> rustorb_cdr::Result<()> {
        enc.write_string(&self.exception_id)?;
        enc.write_u32(self.minor_code_value);
        enc.write_u32(self.completion_status as u32);
        Ok(())
    }
}

impl CdrDecode for SystemExceptionBody {
    fn decode(dec: &mut Decoder<'_>) -> rustorb_cdr::Result<Self> {
        let exception_id = dec.read_string()?;
        let minor_code_value = dec.read_u32()?;
        let completion_status = match dec.read_u32()? {
            0 => CompletionStatus::Yes,
            1 => CompletionStatus::No,
            2 => CompletionStatus::Maybe,
            other => return Err(rustorb_cdr::Error::InvalidEnumValue(other)),
        };
        Ok(SystemExceptionBody {
            exception_id,
            minor_code_value,
            completion_status,
        })
    }
}

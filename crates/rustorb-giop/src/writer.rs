use std::io::IoSlice;

use rustorb_cdr::{ByteOrder, CdrEncode, Encoder, Octets};

use crate::{
    CancelRequestHeader, Error, LocateReplyHeader, LocateRequestHeader, MessageHeader, MsgType,
    ReplyHeader, RequestHeader, Result, Version,
};

/// Writes one GIOP message.
///
/// A constructor writes the message header and the message-specific
/// header; [`body`](MessageEncoder::body) gives an encoder for the body,
/// aligned as the version requires; [`finish`](MessageEncoder::finish)
/// fills in the message size.
#[derive(Debug)]
pub struct MessageEncoder {
    enc: Encoder,
    version: Version,
    align_body: bool,
    /// Length before body padding, and length after it, once `body` was
    /// called.
    body_start: Option<(usize, usize)>,
}

impl MessageEncoder {
    /// Starts a message of any type with an empty body.
    pub fn new(
        version: Version,
        order: ByteOrder,
        msg_type: MsgType,
        more_fragments: bool,
    ) -> Result<MessageEncoder> {
        if !version.is_supported() {
            return Err(Error::UnsupportedVersion(version));
        }
        if msg_type == MsgType::Fragment && version == Version::V1_0
            || more_fragments && version == Version::V1_0
        {
            return Err(Error::FragmentNotSupported);
        }
        let header = MessageHeader {
            version,
            byte_order: order,
            more_fragments,
            msg_type,
            message_size: 0,
        };
        let mut enc = Encoder::new(order, version.cdr_version());
        // Room for the headers and a small body without growing.
        enc.reserve(256);
        enc.write_octets(&header.to_bytes());
        Ok(MessageEncoder {
            enc,
            version,
            align_body: false,
            body_start: None,
        })
    }

    /// Starts a `Request`.
    pub fn request(
        version: Version,
        order: ByteOrder,
        header: &RequestHeader,
    ) -> Result<MessageEncoder> {
        let mut m = MessageEncoder::new(version, order, MsgType::Request, false)?;
        let enc = &mut m.enc;
        if version >= Version::V1_2 {
            enc.write_u32(header.request_id);
            enc.write_octet(header.response_flags);
            enc.write_octets(&[0; 3]);
            header.target.encode(enc)?;
            enc.write_string(&header.operation)?;
            header.service_context.encode(enc)?;
            m.align_body = true;
        } else {
            let key = header.target.object_key().ok_or(Error::TargetNotKey)?;
            header.service_context.encode(enc)?;
            enc.write_u32(header.request_id);
            enc.write_bool(header.response_expected());
            if version.minor == 1 {
                enc.write_octets(&[0; 3]);
            }
            enc.write_octet_seq(key)?;
            enc.write_string(&header.operation)?;
            enc.write_octet_seq(&header.requesting_principal)?;
        }
        Ok(m)
    }

    /// Starts a `Reply`.
    pub fn reply(
        version: Version,
        order: ByteOrder,
        header: &ReplyHeader,
    ) -> Result<MessageEncoder> {
        let mut m = MessageEncoder::new(version, order, MsgType::Reply, false)?;
        let enc = &mut m.enc;
        if version >= Version::V1_2 {
            enc.write_u32(header.request_id);
            enc.write_u32(header.reply_status.to_u32());
            header.service_context.encode(enc)?;
            m.align_body = true;
        } else {
            header.service_context.encode(enc)?;
            enc.write_u32(header.request_id);
            enc.write_u32(header.reply_status.to_u32());
        }
        Ok(m)
    }

    /// Writes a `CancelRequest`.
    pub fn cancel_request(
        version: Version,
        order: ByteOrder,
        header: &CancelRequestHeader,
    ) -> Result<Vec<u8>> {
        let mut m = MessageEncoder::new(version, order, MsgType::CancelRequest, false)?;
        m.enc.write_u32(header.request_id);
        m.finish()
    }

    /// Writes a `LocateRequest`.
    pub fn locate_request(
        version: Version,
        order: ByteOrder,
        header: &LocateRequestHeader,
    ) -> Result<Vec<u8>> {
        let mut m = MessageEncoder::new(version, order, MsgType::LocateRequest, false)?;
        m.enc.write_u32(header.request_id);
        if version >= Version::V1_2 {
            header.target.encode(&mut m.enc)?;
        } else {
            let key = header.target.object_key().ok_or(Error::TargetNotKey)?;
            m.enc.write_octet_seq(key)?;
        }
        m.finish()
    }

    /// Starts a `LocateReply`.
    pub fn locate_reply(
        version: Version,
        order: ByteOrder,
        header: &LocateReplyHeader,
    ) -> Result<MessageEncoder> {
        let mut m = MessageEncoder::new(version, order, MsgType::LocateReply, false)?;
        m.enc.write_u32(header.request_id);
        m.enc.write_u32(header.locate_status.to_u32());
        m.align_body = version >= Version::V1_2;
        Ok(m)
    }

    /// Writes a `CloseConnection`.
    pub fn close_connection(version: Version, order: ByteOrder) -> Result<Vec<u8>> {
        MessageEncoder::new(version, order, MsgType::CloseConnection, false)?.finish()
    }

    /// Writes a `MessageError`.
    pub fn message_error(version: Version, order: ByteOrder) -> Result<Vec<u8>> {
        MessageEncoder::new(version, order, MsgType::MessageError, false)?.finish()
    }

    /// Starts a `Fragment`. `request_id` is required for GIOP 1.2 and
    /// ignored for GIOP 1.1. The caller writes the continuation bytes with
    /// [`body`](MessageEncoder::body).
    pub fn fragment(
        version: Version,
        order: ByteOrder,
        request_id: u32,
        more_fragments: bool,
    ) -> Result<MessageEncoder> {
        let mut m = MessageEncoder::new(version, order, MsgType::Fragment, more_fragments)?;
        if version >= Version::V1_2 {
            m.enc.write_u32(request_id);
        }
        Ok(m)
    }

    /// The encoder for the message body. In GIOP 1.2 the first call pads
    /// to an 8-byte boundary for Request, Reply and LocateReply bodies.
    pub fn body(&mut self) -> &mut Encoder {
        if self.body_start.is_none() {
            let before = self.enc.len();
            if self.align_body {
                self.enc.align(8);
            }
            self.body_start = Some((before, self.enc.len()));
            self.enc.allow_shared(true);
        }
        &mut self.enc
    }

    /// The version of the message.
    pub fn version(&self) -> Version {
        self.version
    }

    /// Fills in the message size and returns the message bytes. Body
    /// padding is dropped again if no body was written.
    pub fn finish(self) -> Result<Vec<u8>> {
        Ok(self.finish_shared()?.into_vec())
    }

    /// Like [`finish`](MessageEncoder::finish), but large octet sequences
    /// written with
    /// [`write_octet_seq_shared`](Encoder::write_octet_seq_shared) stay
    /// where they are, for a vectored write.
    pub fn finish_shared(mut self) -> Result<OutMessage> {
        if let Some((before, after)) = self.body_start {
            if self.enc.len() == after {
                self.enc.truncate(before);
            }
        }
        let len = self.enc.len();
        let header_len = match self.body_start {
            Some((before, _)) => before,
            None => len,
        };
        let size =
            u32::try_from(len - MessageHeader::SIZE).map_err(|_| Error::MessageTooLarge(len))?;
        let size = match self.enc.byte_order() {
            ByteOrder::Big => size.to_be_bytes(),
            ByteOrder::Little => size.to_le_bytes(),
        };
        self.enc.as_bytes_mut()[8..12].copy_from_slice(&size);
        let (bytes, shared) = self.enc.into_parts();
        Ok(OutMessage {
            bytes,
            shared,
            len,
            header_len,
        })
    }
}

/// A whole message to send: its bytes, with large octet sequences that
/// were not copied in kept apart.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OutMessage {
    bytes: Vec<u8>,
    /// Each with the length of `bytes` at which it goes.
    shared: Vec<(usize, Octets)>,
    len: usize,
    /// Where the body starts (before its padding): the message header and
    /// the message-specific header come before it.
    header_len: usize,
}

impl OutMessage {
    /// The length of the message.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether the message has no bytes.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The whole message, if it is one piece.
    pub fn as_single(&self) -> Option<&[u8]> {
        self.shared.is_empty().then_some(&self.bytes[..])
    }

    /// The pieces of the message from offset `from` on, in order, for a
    /// vectored write; at most `max` of them.
    pub fn slices_from(&self, from: usize, max: usize) -> Vec<IoSlice<'_>> {
        let mut out = Vec::new();
        let mut offset = 0;
        for piece in self.pieces() {
            if out.len() == max {
                break;
            }
            let end = offset + piece.len();
            if end > from {
                out.push(IoSlice::new(&piece[from.saturating_sub(offset)..]));
            }
            offset = end;
        }
        out
    }

    /// The pieces of the message in order.
    pub fn pieces(&self) -> impl Iterator<Item = &[u8]> {
        let mut from = 0;
        let mut shared = self.shared.iter();
        let mut pending: Option<&[u8]> = None;
        std::iter::from_fn(move || {
            if let Some(octets) = pending.take() {
                return Some(octets);
            }
            match shared.next() {
                Some((at, octets)) => {
                    let piece = &self.bytes[from..*at];
                    from = *at;
                    pending = Some(octets.as_slice());
                    Some(piece)
                }
                None if from != usize::MAX => {
                    let piece = &self.bytes[from..];
                    from = usize::MAX;
                    Some(piece)
                }
                None => None,
            }
        })
    }

    /// Splits a GIOP 1.2 `Request`, `Reply` or `LocateReply` longer than
    /// `size` bytes into a first message and `Fragment` messages of at
    /// most `size` bytes each (`size` rounded down to a multiple of 8, at
    /// least 64), returned one after the other in one `OutMessage`. Other
    /// messages, and messages that fit, are returned as they are.
    ///
    /// The first message keeps the whole message-specific header. Every
    /// message but the last is a multiple of 8 bytes long, so the body
    /// keeps its alignment whether it is read relative to the start of
    /// the whole message or of each `Fragment` (16 bytes of headers).
    pub fn fragment(self, size: usize) -> OutMessage {
        if self.len <= size || self.bytes.len() < MessageHeader::SIZE + 4 {
            return self;
        }
        let size = (size & !7).max(64);
        if self.len <= size {
            return self;
        }
        let head = &self.bytes[..MessageHeader::SIZE];
        let version = Version {
            major: head[4],
            minor: head[5],
        };
        let flags = head[6];
        let fragmentable = matches!(head[7], 0 | 1 | 4); // Request, Reply, LocateReply
        if version != Version::V1_2 || !fragmentable || flags & 2 != 0 {
            return self;
        }
        let first = size.max((self.header_len + 7) & !7);
        if first >= self.len {
            return self;
        }
        let order = if flags & 1 != 0 {
            ByteOrder::Little
        } else {
            ByteOrder::Big
        };
        let u32_bytes = |v: u32| match order {
            ByteOrder::Big => v.to_be_bytes(),
            ByteOrder::Little => v.to_le_bytes(),
        };
        let request_id = [
            self.bytes[12],
            self.bytes[13],
            self.bytes[14],
            self.bytes[15],
        ];
        let len = self.len;
        let whole = self.into_vec();
        let per = size - MessageHeader::SIZE - 4;
        let count = 1 + (len - first).div_ceil(per);
        let mut out = Vec::with_capacity(len + count * (MessageHeader::SIZE + 4));
        out.extend_from_slice(&whole[..first]);
        out[6] |= 2;
        out[8..12].copy_from_slice(&u32_bytes((first - MessageHeader::SIZE) as u32));
        let mut at = first;
        while at < len {
            let end = (at + per).min(len);
            let more = end < len;
            out.extend_from_slice(&whole[..6]);
            out.push(flags & 1 | if more { 2 } else { 0 });
            out.push(7); // Fragment
            out.extend_from_slice(&u32_bytes((end - at + 4) as u32));
            out.extend_from_slice(&request_id);
            out.extend_from_slice(&whole[at..end]);
            at = end;
        }
        OutMessage::from(out)
    }

    /// The message as one buffer; copies only if octets were kept apart.
    pub fn into_vec(self) -> Vec<u8> {
        if self.shared.is_empty() {
            return self.bytes;
        }
        let mut out = Vec::with_capacity(self.len);
        for piece in self.pieces() {
            out.extend_from_slice(piece);
        }
        out
    }
}

impl From<Vec<u8>> for OutMessage {
    fn from(bytes: Vec<u8>) -> OutMessage {
        let len = bytes.len();
        OutMessage {
            bytes,
            shared: Vec::new(),
            len,
            header_len: len,
        }
    }
}

use rustorb_cdr::{ByteOrder, CdrEncode, Encoder};

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
        }
        &mut self.enc
    }

    /// The version of the message.
    pub fn version(&self) -> Version {
        self.version
    }

    /// Fills in the message size and returns the message bytes. Body
    /// padding is dropped again if no body was written.
    pub fn finish(mut self) -> Result<Vec<u8>> {
        if let Some((before, after)) = self.body_start {
            if self.enc.len() == after {
                self.enc.truncate(before);
            }
        }
        let len = self.enc.len();
        let size =
            u32::try_from(len - MessageHeader::SIZE).map_err(|_| Error::MessageTooLarge(len))?;
        let size = match self.enc.byte_order() {
            ByteOrder::Big => size.to_be_bytes(),
            ByteOrder::Little => size.to_le_bytes(),
        };
        self.enc.as_bytes_mut()[8..12].copy_from_slice(&size);
        Ok(self.enc.into_bytes())
    }
}

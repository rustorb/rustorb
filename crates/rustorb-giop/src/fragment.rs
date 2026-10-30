use std::collections::HashMap;
use std::sync::Arc;

use rustorb_cdr::{ByteOrder, Segment};

use crate::message::decode_segmented;
use crate::{Error, Message, MessageHeader, MsgType, Result, Version};

/// Joins fragmented GIOP 1.1 and 1.2 messages.
///
/// Feed every complete message received on one connection to
/// [`push`](FragmentAssembler::push). Unfragmented messages are returned
/// as they are; a fragmented one is returned as a single message, with the
/// more-fragments flag cleared and the size updated, once its last
/// fragment arrives.
///
/// GIOP 1.1 allows one fragmented message at a time per connection. GIOP
/// 1.2 interleaves them by request id, which is the first field of every
/// message that can be fragmented.
///
/// The data in each `Fragment` is aligned relative to the start of that
/// `Fragment` message, as omniORB writes and reads it. In GIOP 1.2 the
/// data of a fragment starts at offset 16 and every fragment but the last
/// ends on an 8-byte boundary, so this is the same as aligning the joined
/// message. In GIOP 1.1 it is not:
/// omniORB ends each fragment on an 8-byte boundary and continues at
/// offset 12 of the next, so an 8-byte value after a fragment boundary
/// would be read 4 bytes off. The joined message therefore carries a
/// [`Segment`] for each fragment, which [`AssembledMessage::decode`] passes
/// on to the body decoder.
#[derive(Debug)]
pub struct FragmentAssembler {
    max_message_size: usize,
    pending_1_1: Option<Pending>,
    pending_1_2: HashMap<u32, Pending>,
}

#[derive(Debug)]
struct Pending {
    version: Version,
    byte_order: ByteOrder,
    data: Vec<u8>,
    segments: Vec<Segment>,
}

/// A whole message returned by a [`FragmentAssembler`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssembledMessage {
    data: Vec<u8>,
    segments: Vec<Segment>,
}

impl AssembledMessage {
    /// The message bytes, header included. For a joined message the
    /// header has the more-fragments flag cleared and the total size.
    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }

    /// Consumes the message and returns its bytes.
    pub fn into_bytes(self) -> Vec<u8> {
        self.data
    }

    /// Where each fragment after the first starts in the message, with the
    /// offset its alignment restarts at. Empty for an unfragmented message.
    pub fn segments(&self) -> &[Segment] {
        &self.segments
    }

    /// Whether the message was joined from fragments.
    pub fn was_fragmented(&self) -> bool {
        !self.segments.is_empty()
    }

    /// Decodes the message, keeping the alignment of each fragment. The
    /// bodies get a copy of the bytes; [`into_decoded`](Self::into_decoded)
    /// shares them instead.
    pub fn decode(&self) -> Result<(MessageHeader, Message)> {
        decode_segmented(&Arc::new(self.data.clone()), &self.segments)
    }

    /// Decodes the message like [`decode`](Self::decode), handing its
    /// bytes to the bodies without copying them.
    pub fn into_decoded(self) -> Result<(MessageHeader, Message)> {
        decode_segmented(&Arc::new(self.data), &self.segments)
    }
}

impl Default for FragmentAssembler {
    fn default() -> FragmentAssembler {
        FragmentAssembler::new(FragmentAssembler::DEFAULT_MAX_MESSAGE_SIZE)
    }
}

impl FragmentAssembler {
    /// Default limit on the size of a reassembled message: 64 MiB.
    pub const DEFAULT_MAX_MESSAGE_SIZE: usize = 64 * 1024 * 1024;

    /// Creates an assembler that rejects reassembled messages longer than
    /// `max_message_size` bytes, header included.
    pub fn new(max_message_size: usize) -> FragmentAssembler {
        FragmentAssembler {
            max_message_size,
            pending_1_1: None,
            pending_1_2: HashMap::new(),
        }
    }

    /// Whether any fragmented message is incomplete.
    pub fn has_pending(&self) -> bool {
        self.pending_1_1.is_some() || !self.pending_1_2.is_empty()
    }

    /// Drops an incomplete GIOP 1.2 message, as on a `CancelRequest`.
    pub fn cancel(&mut self, request_id: u32) -> bool {
        self.pending_1_2.remove(&request_id).is_some()
    }

    /// Takes one complete message and returns a whole message when one is
    /// ready.
    pub fn push(&mut self, msg: &[u8]) -> Result<Option<AssembledMessage>> {
        self.push_message(msg)
    }

    /// Like [`push`](Self::push), but takes the buffer the message was read
    /// into: an unfragmented message, and the first fragment of a
    /// fragmented one, keep it instead of copying it.
    pub fn push_owned(&mut self, msg: Vec<u8>) -> Result<Option<AssembledMessage>> {
        self.push_message(msg)
    }

    fn push_message<M>(&mut self, owned: M) -> Result<Option<AssembledMessage>>
    where
        M: AsRef<[u8]> + Into<Vec<u8>>,
    {
        let msg = owned.as_ref();
        let header = MessageHeader::parse(msg)?;
        let expected = header.total_len();
        if msg.len() != expected {
            return Err(if msg.len() < expected {
                Error::Truncated {
                    expected,
                    actual: msg.len(),
                }
            } else {
                Error::TrailingBytes(msg.len() - expected)
            });
        }
        let v12 = header.version >= Version::V1_2;

        if header.msg_type != MsgType::Fragment {
            if !header.more_fragments {
                return Ok(Some(AssembledMessage {
                    data: owned.into(),
                    segments: Vec::new(),
                }));
            }
            self.check_size(msg.len())?;
            let id = if v12 {
                Some(request_id(&header, msg)?)
            } else {
                None
            };
            let pending = Pending {
                version: header.version,
                byte_order: header.byte_order,
                data: owned.into(),
                segments: Vec::new(),
            };
            if let Some(id) = id {
                if self.pending_1_2.contains_key(&id) {
                    return Err(Error::FragmentInProgress);
                }
                self.pending_1_2.insert(id, pending);
            } else {
                if self.pending_1_1.is_some() {
                    return Err(Error::FragmentInProgress);
                }
                self.pending_1_1 = Some(pending);
            }
            return Ok(None);
        }

        let (mut pending, data_start, id) = if v12 {
            let id = request_id(&header, msg)?;
            let pending = self
                .pending_1_2
                .remove(&id)
                .ok_or(Error::UnexpectedFragment)?;
            (pending, MessageHeader::SIZE + 4, Some(id))
        } else {
            let pending = self.pending_1_1.take().ok_or(Error::UnexpectedFragment)?;
            (pending, MessageHeader::SIZE, None)
        };
        if pending.version != header.version || pending.byte_order != header.byte_order {
            return Err(Error::FragmentMismatch);
        }
        let data = &msg[data_start..];
        self.check_size(pending.data.len() + data.len())?;
        if !data.is_empty() {
            pending.segments.push(Segment {
                start: pending.data.len(),
                offset: data_start,
            });
        }
        pending.data.extend_from_slice(data);

        if header.more_fragments {
            match id {
                Some(id) => {
                    self.pending_1_2.insert(id, pending);
                }
                None => self.pending_1_1 = Some(pending),
            }
            return Ok(None);
        }

        let mut whole = MessageHeader::parse(&pending.data)?;
        whole.more_fragments = false;
        whole.message_size = u32::try_from(pending.data.len() - MessageHeader::SIZE)
            .map_err(|_| Error::MessageTooLarge(pending.data.len()))?;
        pending.data[..MessageHeader::SIZE].copy_from_slice(&whole.to_bytes());
        Ok(Some(AssembledMessage {
            data: pending.data,
            segments: pending.segments,
        }))
    }

    fn check_size(&self, len: usize) -> Result<()> {
        if len > self.max_message_size {
            Err(Error::FragmentTooLarge(len))
        } else {
            Ok(())
        }
    }
}

/// Reads the request id that starts every fragmentable GIOP 1.2 message.
fn request_id(header: &MessageHeader, msg: &[u8]) -> Result<u32> {
    let bytes = msg
        .get(MessageHeader::SIZE..MessageHeader::SIZE + 4)
        .ok_or(Error::Truncated {
            expected: MessageHeader::SIZE + 4,
            actual: msg.len(),
        })?;
    let bytes = [bytes[0], bytes[1], bytes[2], bytes[3]];
    Ok(match header.byte_order {
        ByteOrder::Big => u32::from_be_bytes(bytes),
        ByteOrder::Little => u32::from_le_bytes(bytes),
    })
}

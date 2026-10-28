use std::collections::HashMap;

use rustorb_cdr::ByteOrder;

use crate::{Error, MessageHeader, MsgType, Result, Version};

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
    pub fn push(&mut self, msg: &[u8]) -> Result<Option<Vec<u8>>> {
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
                return Ok(Some(msg.to_vec()));
            }
            self.check_size(msg.len())?;
            let pending = Pending {
                version: header.version,
                byte_order: header.byte_order,
                data: msg.to_vec(),
            };
            if v12 {
                let id = request_id(&header, msg)?;
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

        let (mut pending, data, id) = if v12 {
            let id = request_id(&header, msg)?;
            let pending = self
                .pending_1_2
                .remove(&id)
                .ok_or(Error::UnexpectedFragment)?;
            (pending, &msg[MessageHeader::SIZE + 4..], Some(id))
        } else {
            let pending = self.pending_1_1.take().ok_or(Error::UnexpectedFragment)?;
            (pending, &msg[MessageHeader::SIZE..], None)
        };
        if pending.version != header.version || pending.byte_order != header.byte_order {
            return Err(Error::FragmentMismatch);
        }
        self.check_size(pending.data.len() + data.len())?;
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
        Ok(Some(pending.data))
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

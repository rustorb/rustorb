use std::fmt;

use rustorb_cdr::{ByteOrder, Encoding};

use crate::{Error, Result};

/// A GIOP protocol version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Version {
    /// Major version, always 1 for supported messages.
    pub major: u8,
    /// Minor version.
    pub minor: u8,
}

impl Version {
    /// GIOP 1.0.
    pub const V1_0: Version = Version { major: 1, minor: 0 };
    /// GIOP 1.1.
    pub const V1_1: Version = Version { major: 1, minor: 1 };
    /// GIOP 1.2.
    pub const V1_2: Version = Version { major: 1, minor: 2 };

    /// Whether this version can be read and written.
    pub fn is_supported(self) -> bool {
        self.major == 1 && self.minor <= 2
    }

    /// The CDR rules for this version.
    pub fn cdr_version(self) -> Encoding {
        match self.minor {
            0 => Encoding::Giop1_0,
            1 => Encoding::Giop1_1,
            _ => Encoding::Giop1_2,
        }
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

/// A GIOP message type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MsgType {
    /// `Request` (0).
    Request,
    /// `Reply` (1).
    Reply,
    /// `CancelRequest` (2).
    CancelRequest,
    /// `LocateRequest` (3).
    LocateRequest,
    /// `LocateReply` (4).
    LocateReply,
    /// `CloseConnection` (5).
    CloseConnection,
    /// `MessageError` (6).
    MessageError,
    /// `Fragment` (7), GIOP 1.1 and later.
    Fragment,
}

impl MsgType {
    /// The value in the header.
    pub fn to_u8(self) -> u8 {
        self as u8
    }

    /// Converts a header value, rejecting `Fragment` for GIOP 1.0.
    pub fn from_u8(value: u8, version: Version) -> Result<MsgType> {
        Ok(match value {
            0 => MsgType::Request,
            1 => MsgType::Reply,
            2 => MsgType::CancelRequest,
            3 => MsgType::LocateRequest,
            4 => MsgType::LocateReply,
            5 => MsgType::CloseConnection,
            6 => MsgType::MessageError,
            7 if version.minor >= 1 => MsgType::Fragment,
            other => return Err(Error::UnknownMessageType(other)),
        })
    }
}

/// The 12-byte header that starts every GIOP message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MessageHeader {
    /// Protocol version.
    pub version: Version,
    /// Byte order of the header size and the whole message.
    pub byte_order: ByteOrder,
    /// More fragments follow (GIOP 1.1 and later).
    pub more_fragments: bool,
    /// Message type.
    pub msg_type: MsgType,
    /// Bytes after the header.
    pub message_size: u32,
}

impl MessageHeader {
    /// Size of the header in bytes.
    pub const SIZE: usize = 12;

    /// The magic bytes.
    pub const MAGIC: [u8; 4] = *b"GIOP";

    /// Flag bit for little-endian (GIOP 1.1 and later).
    const FLAG_LITTLE_ENDIAN: u8 = 0x01;
    /// Flag bit for "more fragments follow" (GIOP 1.1 and later).
    const FLAG_FRAGMENT: u8 = 0x02;

    /// Parses a header from the first 12 bytes of `buf`.
    ///
    /// The message size is not checked against any limit; the caller
    /// decides how large a message it is willing to buffer.
    pub fn parse(buf: &[u8]) -> Result<MessageHeader> {
        let Some(bytes) = buf.first_chunk::<{ Self::SIZE }>() else {
            return Err(Error::Truncated {
                expected: Self::SIZE,
                actual: buf.len(),
            });
        };
        let magic = [bytes[0], bytes[1], bytes[2], bytes[3]];
        if magic != Self::MAGIC {
            return Err(Error::BadMagic(magic));
        }
        let version = Version {
            major: bytes[4],
            minor: bytes[5],
        };
        if !version.is_supported() {
            return Err(Error::UnsupportedVersion(version));
        }
        let flags = bytes[6];
        let (byte_order, more_fragments) = if version.minor == 0 {
            let order = ByteOrder::from_flag(flags).map_err(|_| Error::InvalidFlags(flags))?;
            (order, false)
        } else {
            let order = if flags & Self::FLAG_LITTLE_ENDIAN != 0 {
                ByteOrder::Little
            } else {
                ByteOrder::Big
            };
            (order, flags & Self::FLAG_FRAGMENT != 0)
        };
        let msg_type = MsgType::from_u8(bytes[7], version)?;
        let size = [bytes[8], bytes[9], bytes[10], bytes[11]];
        let message_size = match byte_order {
            ByteOrder::Big => u32::from_be_bytes(size),
            ByteOrder::Little => u32::from_le_bytes(size),
        };
        Ok(MessageHeader {
            version,
            byte_order,
            more_fragments,
            msg_type,
            message_size,
        })
    }

    /// The flags octet as written in the header.
    pub fn flags(&self) -> u8 {
        if self.version.minor == 0 {
            self.byte_order.flag()
        } else {
            let mut flags = self.byte_order.flag();
            if self.more_fragments {
                flags |= Self::FLAG_FRAGMENT;
            }
            flags
        }
    }

    /// Writes the header.
    pub fn to_bytes(&self) -> [u8; Self::SIZE] {
        let size = match self.byte_order {
            ByteOrder::Big => self.message_size.to_be_bytes(),
            ByteOrder::Little => self.message_size.to_le_bytes(),
        };
        let m = Self::MAGIC;
        [
            m[0],
            m[1],
            m[2],
            m[3],
            self.version.major,
            self.version.minor,
            self.flags(),
            self.msg_type.to_u8(),
            size[0],
            size[1],
            size[2],
            size[3],
        ]
    }

    /// Total length of the message including the header.
    pub fn total_len(&self) -> usize {
        Self::SIZE + self.message_size as usize
    }
}

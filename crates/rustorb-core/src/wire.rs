//! Reading GIOP messages off a stream, shared by client and server
//! connections.

use std::io;

use rustorb_giop::{MessageHeader, Version};
use tokio::io::{AsyncRead, AsyncReadExt, BufReader};

/// How much a connection reads ahead: a small message and the header of
/// the next one usually come in with one read. Bodies larger than this
/// are read straight into their own buffer.
const READ_AHEAD: usize = 16 * 1024;

/// The read half of a connection, with read-ahead.
pub(crate) fn buffered<R: AsyncRead>(reader: R) -> BufReader<R> {
    BufReader::with_capacity(READ_AHEAD, reader)
}

/// Why [`read_message`] returned no message.
#[derive(Debug)]
pub(crate) enum ReadError {
    /// The stream ended, or failed, before a message header.
    Closed(io::Error),
    /// The header is not a GIOP header.
    BadHeader(String),
    /// The message is longer than the limit; carries its version.
    TooLarge(usize, Version),
    /// The stream ended, or failed, inside a message.
    Truncated(io::Error),
}

/// Reads one complete message, header included, into a buffer of its own
/// (not zeroed first, and not copied again by the caller). Returns its
/// header too.
pub(crate) async fn read_message<R: AsyncRead + Unpin>(
    reader: &mut R,
    max_message_size: usize,
) -> Result<(MessageHeader, Vec<u8>), ReadError> {
    let mut header = [0u8; MessageHeader::SIZE];
    reader
        .read_exact(&mut header)
        .await
        .map_err(ReadError::Closed)?;
    let parsed = MessageHeader::parse(&header).map_err(|e| ReadError::BadHeader(e.to_string()))?;
    let total = parsed.total_len();
    if total > max_message_size {
        return Err(ReadError::TooLarge(total, parsed.version));
    }
    let mut msg = Vec::with_capacity(total);
    msg.extend_from_slice(&header);
    while msg.len() < total {
        // `read_buf` fills spare capacity without zeroing it; `take` keeps
        // it from reading past the message.
        let left = (total - msg.len()) as u64;
        match (&mut *reader).take(left).read_buf(&mut msg).await {
            Ok(0) => {
                return Err(ReadError::Truncated(io::ErrorKind::UnexpectedEof.into()));
            }
            Ok(_) => {}
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(ReadError::Truncated(e)),
        }
    }
    Ok((parsed, msg))
}

use std::fmt;
use std::ops::{Deref, Range};
use std::sync::Arc;

/// A `sequence<octet>` that shares its bytes instead of copying them.
///
/// Read from a received message with
/// [`Decoder::read_octet_seq_shared`](crate::Decoder::read_octet_seq_shared),
/// it keeps the message buffer and points into it. Written with
/// [`Encoder::write_octet_seq_shared`](crate::Encoder::write_octet_seq_shared),
/// a large one is sent from where it is, without being copied into the
/// message. Cloning is cheap. Made from a `Vec<u8>` without copying.
#[derive(Clone)]
pub struct Octets {
    buf: Arc<Vec<u8>>,
    range: Range<usize>,
}

impl Octets {
    /// No octets.
    pub fn new() -> Octets {
        Octets::from(Vec::new())
    }

    /// The bytes `range` of `buf`.
    ///
    /// # Panics
    ///
    /// If `range` is not within `buf`.
    pub fn slice_of(buf: Arc<Vec<u8>>, range: Range<usize>) -> Octets {
        assert!(range.start <= range.end && range.end <= buf.len());
        Octets { buf, range }
    }

    /// The bytes.
    pub fn as_slice(&self) -> &[u8] {
        &self.buf[self.range.clone()]
    }

    /// The bytes as a vector of their own: no copy when nothing else
    /// shares them and they are the whole buffer.
    pub fn into_vec(self) -> Vec<u8> {
        if self.range.start == 0 && self.range.end == self.buf.len() {
            match Arc::try_unwrap(self.buf) {
                Ok(v) => v,
                Err(buf) => buf.to_vec(),
            }
        } else {
            self.as_slice().to_vec()
        }
    }
}

impl Default for Octets {
    fn default() -> Octets {
        Octets::new()
    }
}

impl Deref for Octets {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        self.as_slice()
    }
}

impl AsRef<[u8]> for Octets {
    fn as_ref(&self) -> &[u8] {
        self.as_slice()
    }
}

impl From<Vec<u8>> for Octets {
    fn from(v: Vec<u8>) -> Octets {
        let range = 0..v.len();
        Octets {
            buf: Arc::new(v),
            range,
        }
    }
}

impl FromIterator<u8> for Octets {
    fn from_iter<I: IntoIterator<Item = u8>>(iter: I) -> Octets {
        Octets::from(iter.into_iter().collect::<Vec<u8>>())
    }
}

impl From<&[u8]> for Octets {
    fn from(v: &[u8]) -> Octets {
        Octets::from(v.to_vec())
    }
}

impl From<Octets> for Vec<u8> {
    fn from(o: Octets) -> Vec<u8> {
        o.into_vec()
    }
}

impl PartialEq for Octets {
    fn eq(&self, other: &Octets) -> bool {
        self.as_slice() == other.as_slice()
    }
}

impl Eq for Octets {}

impl PartialEq<[u8]> for Octets {
    fn eq(&self, other: &[u8]) -> bool {
        self.as_slice() == other
    }
}

impl PartialEq<Vec<u8>> for Octets {
    fn eq(&self, other: &Vec<u8>) -> bool {
        self.as_slice() == other.as_slice()
    }
}

impl std::hash::Hash for Octets {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.as_slice().hash(state);
    }
}

impl fmt::Debug for Octets {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.as_slice(), f)
    }
}

//! The two kinds of stream a GIOP connection runs over: TCP (`giop:tcp`,
//! `giop:ssl`) and, on Unix, a Unix domain socket (omniORB's `giop:unix`,
//! docs/unix-transport.md). The connection code reads and writes through
//! these and does not care which one it has.

use std::io::{self, IoSlice};
use std::mem::MaybeUninit;
use std::net::SocketAddr;
use std::pin::Pin;
use std::task::{Context, Poll};

use socket2::SockRef;
use tokio::io::{AsyncWrite, Interest};
use tokio::net::{TcpStream, tcp};

#[cfg(unix)]
use tokio::net::{UnixStream, unix};

/// A connected stream.
#[derive(Debug)]
pub(crate) enum Stream {
    Tcp(TcpStream),
    #[cfg(unix)]
    Unix(UnixStream),
}

impl Stream {
    /// The TCP stream, if it is one (TLS runs on TCP only).
    pub(crate) fn tcp(&self) -> Option<&TcpStream> {
        match self {
            Stream::Tcp(s) => Some(s),
            #[cfg(unix)]
            Stream::Unix(_) => None,
        }
    }

    /// The address of the peer: `None` for a Unix socket.
    pub(crate) fn peer_addr(&self) -> Option<SocketAddr> {
        match self {
            Stream::Tcp(s) => s.peer_addr().ok(),
            #[cfg(unix)]
            Stream::Unix(_) => None,
        }
    }

    pub(crate) fn into_split(self) -> (ReadHalf, WriteHalf) {
        match self {
            Stream::Tcp(s) => {
                let (r, w) = s.into_split();
                (ReadHalf::Tcp(r), WriteHalf::Tcp(w))
            }
            #[cfg(unix)]
            Stream::Unix(s) => {
                let (r, w) = s.into_split();
                (ReadHalf::Unix(r), WriteHalf::Unix(w))
            }
        }
    }
}

/// The read half of a [`Stream`].
#[derive(Debug)]
pub(crate) enum ReadHalf {
    Tcp(tcp::OwnedReadHalf),
    #[cfg(unix)]
    Unix(unix::OwnedReadHalf),
}

impl ReadHalf {
    /// The address of the peer: `None` for a Unix socket.
    pub(crate) fn peer_addr(&self) -> Option<SocketAddr> {
        match self {
            ReadHalf::Tcp(s) => s.peer_addr().ok(),
            #[cfg(unix)]
            ReadHalf::Unix(_) => None,
        }
    }

    /// Reads straight from the socket, bypassing the runtime's readiness.
    pub(crate) fn recv(&self, buf: &mut [MaybeUninit<u8>]) -> io::Result<usize> {
        match self {
            ReadHalf::Tcp(s) => SockRef::from(s.as_ref()).recv(buf),
            #[cfg(unix)]
            ReadHalf::Unix(s) => SockRef::from(s.as_ref()).recv(buf),
        }
    }

    /// Reads through the runtime's readiness, so that a `WouldBlock`
    /// clears it.
    pub(crate) fn try_recv(&self, buf: &mut [MaybeUninit<u8>]) -> io::Result<usize> {
        match self {
            ReadHalf::Tcp(s) => {
                let s = s.as_ref();
                s.try_io(Interest::READABLE, || SockRef::from(s).recv(buf))
            }
            #[cfg(unix)]
            ReadHalf::Unix(s) => {
                let s = s.as_ref();
                s.try_io(Interest::READABLE, || SockRef::from(s).recv(buf))
            }
        }
    }

    /// Forgets the readiness the runtime has seen.
    pub(crate) fn clear_readable(&self) {
        let would_block = || Err::<(), _>(io::ErrorKind::WouldBlock.into());
        let _ = match self {
            ReadHalf::Tcp(s) => s.as_ref().try_io(Interest::READABLE, would_block),
            #[cfg(unix)]
            ReadHalf::Unix(s) => s.as_ref().try_io(Interest::READABLE, would_block),
        };
    }

    /// Waits until the socket may be read.
    pub(crate) async fn readable(&self) -> io::Result<()> {
        match self {
            ReadHalf::Tcp(s) => s.readable().await,
            #[cfg(unix)]
            ReadHalf::Unix(s) => s.readable().await,
        }
    }

    /// Shuts the read direction down.
    pub(crate) fn shutdown_read(&self) {
        let _ = match self {
            ReadHalf::Tcp(s) => SockRef::from(s.as_ref()).shutdown(std::net::Shutdown::Read),
            #[cfg(unix)]
            ReadHalf::Unix(s) => SockRef::from(s.as_ref()).shutdown(std::net::Shutdown::Read),
        };
    }

    #[cfg(unix)]
    pub(crate) fn raw_fd(&self) -> std::os::fd::RawFd {
        use std::os::fd::AsRawFd;
        match self {
            ReadHalf::Tcp(s) => s.as_ref().as_raw_fd(),
            ReadHalf::Unix(s) => s.as_ref().as_raw_fd(),
        }
    }
}

/// The write half of a [`Stream`].
#[derive(Debug)]
pub(crate) enum WriteHalf {
    Tcp(tcp::OwnedWriteHalf),
    #[cfg(unix)]
    Unix(unix::OwnedWriteHalf),
}

impl WriteHalf {
    pub(crate) fn try_write(&self, buf: &[u8]) -> io::Result<usize> {
        match self {
            WriteHalf::Tcp(s) => s.try_write(buf),
            #[cfg(unix)]
            WriteHalf::Unix(s) => s.try_write(buf),
        }
    }

    pub(crate) fn try_write_vectored(&self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        match self {
            WriteHalf::Tcp(s) => s.try_write_vectored(bufs),
            #[cfg(unix)]
            WriteHalf::Unix(s) => s.try_write_vectored(bufs),
        }
    }

    pub(crate) async fn writable(&self) -> io::Result<()> {
        match self {
            WriteHalf::Tcp(s) => s.writable().await,
            #[cfg(unix)]
            WriteHalf::Unix(s) => s.writable().await,
        }
    }
}

impl AsyncWrite for WriteHalf {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        match self.get_mut() {
            WriteHalf::Tcp(s) => Pin::new(s).poll_write(cx, buf),
            #[cfg(unix)]
            WriteHalf::Unix(s) => Pin::new(s).poll_write(cx, buf),
        }
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        match self.get_mut() {
            WriteHalf::Tcp(s) => Pin::new(s).poll_write_vectored(cx, bufs),
            #[cfg(unix)]
            WriteHalf::Unix(s) => Pin::new(s).poll_write_vectored(cx, bufs),
        }
    }

    fn is_write_vectored(&self) -> bool {
        true
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.get_mut() {
            WriteHalf::Tcp(s) => Pin::new(s).poll_flush(cx),
            #[cfg(unix)]
            WriteHalf::Unix(s) => Pin::new(s).poll_flush(cx),
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.get_mut() {
            WriteHalf::Tcp(s) => Pin::new(s).poll_shutdown(cx),
            #[cfg(unix)]
            WriteHalf::Unix(s) => Pin::new(s).poll_shutdown(cx),
        }
    }
}

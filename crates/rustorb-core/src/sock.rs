//! The two kinds of stream a GIOP connection runs over: TCP (`giop:tcp`,
//! `giop:ssl`) and, on Unix, a Unix domain socket (omniORB's `giop:unix`,
//! docs/unix-transport.md). The connection code reads and writes through
//! these and does not care which one it has.
//!
//! On Unix the halves of a split stream register with the runtime's poller
//! separately, so that the reading can be taken off the poller while a
//! thread reads the socket itself (docs/read-ownership.md, "epoll から外す").

use std::io::{self, IoSlice};
use std::net::SocketAddr;
use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::io::AsyncWrite;
use tokio::net::TcpStream;

#[cfg(unix)]
use tokio::net::UnixStream;

#[cfg(not(unix))]
pub(crate) use self::tokio_halves::{ReadHalf, WriteHalf};
#[cfg(unix)]
pub(crate) use self::unix_halves::{ReadHalf, WriteHalf};

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

    /// Splits the stream into its read and write halves. Runs on the
    /// runtime (the halves register with its poller).
    pub(crate) fn into_split(self) -> io::Result<(ReadHalf, WriteHalf)> {
        #[cfg(unix)]
        return unix_halves::split(self);
        #[cfg(not(unix))]
        return tokio_halves::split(self);
    }
}

impl AsyncWrite for WriteHalf {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.get_mut().poll_write_with(cx, |s| s.try_write(buf))
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        self.get_mut()
            .poll_write_with(cx, |s| s.try_write_vectored(bufs))
    }

    fn is_write_vectored(&self) -> bool {
        true
    }

    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(self.get_mut().shutdown_write())
    }
}

/// The halves on Unix: our own registrations of the socket.
#[cfg(unix)]
mod unix_halves {
    use std::io::{self, IoSlice, Write};
    use std::mem::MaybeUninit;
    use std::net::SocketAddr;
    use std::os::fd::{AsFd, AsRawFd, BorrowedFd, OwnedFd, RawFd};
    use std::sync::{Arc, Mutex};
    use std::task::{Context, Poll, Waker};

    use socket2::SockRef;
    use tokio::io::Interest;
    use tokio::io::unix::AsyncFd;

    use super::Stream;
    use crate::stats::{self, Counter};

    /// The socket of a split stream, shared by its halves.
    #[derive(Debug)]
    enum Sock {
        Tcp(std::net::TcpStream),
        Unix(std::os::unix::net::UnixStream),
    }

    impl AsFd for Sock {
        fn as_fd(&self) -> BorrowedFd<'_> {
            match self {
                Sock::Tcp(s) => s.as_fd(),
                Sock::Unix(s) => s.as_fd(),
            }
        }
    }

    /// The socket as the write half registers it: holds it open.
    #[derive(Debug)]
    struct Shared(Arc<Sock>);

    impl AsRawFd for Shared {
        fn as_raw_fd(&self) -> RawFd {
            self.0.as_fd().as_raw_fd()
        }
    }

    pub(super) fn split(stream: Stream) -> io::Result<(ReadHalf, WriteHalf)> {
        let peer = stream.peer_addr();
        // `into_std` takes the stream off the poller; it stays
        // non-blocking.
        let sock = Arc::new(match stream {
            Stream::Tcp(s) => Sock::Tcp(s.into_std()?),
            Stream::Unix(s) => Sock::Unix(s.into_std()?),
        });
        // Writability only: data arriving does not wake the poller for
        // this registration.
        // SAFETY: `Shared` holds the socket open while it is registered.
        let write = unsafe {
            AsyncFd::register_with_interest(Shared(Arc::clone(&sock)), Interest::WRITABLE)?
        };
        // The reading registers a descriptor of its own: the poller keys
        // registrations by descriptor, and taking the reading off must not
        // take the writing off.
        let dup = sock.as_fd().try_clone_to_owned()?;
        let read = ReadHalf {
            watch: Mutex::new(Watch {
                reg: None,
                waker: None,
                failed: None,
            }),
            dup,
            sock,
            handle: tokio::runtime::Handle::current(),
            peer,
        };
        read.register()?;
        Ok((read, WriteHalf { io: write }))
    }

    /// The reading's registration with the poller.
    #[derive(Debug)]
    struct Watch {
        /// Registered for readability; `None` while a thread reads.
        reg: Option<AsyncFd<RawFd>>,
        /// The reader task's waker, from its last wait: a registration
        /// made by a thread wakes it.
        waker: Option<Waker>,
        /// Registering again failed: the task's wait ends with this.
        failed: Option<io::Error>,
    }

    /// The read half of a [`Stream`].
    ///
    /// The reader task waits for the socket through the runtime's poller.
    /// While a thread reads the socket itself (and waits with `poll`), the
    /// reading is off the poller ([`unwatch`](ReadHalf::unwatch)), so data
    /// arriving wakes only that thread; [`watch`](ReadHalf::watch) puts it
    /// back when the task may read again.
    #[derive(Debug)]
    pub(crate) struct ReadHalf {
        // Dropped before `dup`: deregistered, then closed.
        watch: Mutex<Watch>,
        dup: OwnedFd,
        sock: Arc<Sock>,
        handle: tokio::runtime::Handle,
        peer: Option<SocketAddr>,
    }

    impl ReadHalf {
        /// The address of the peer: `None` for a Unix socket.
        pub(crate) fn peer_addr(&self) -> Option<SocketAddr> {
            self.peer
        }

        /// Reads straight from the socket, bypassing the runtime's
        /// readiness.
        pub(crate) fn recv(&self, buf: &mut [MaybeUninit<u8>]) -> io::Result<usize> {
            SockRef::from(&*self.sock).recv(buf)
        }

        /// Has the socket count as readable (to `poll` and the runtime's
        /// poller) only once `n` bytes are there, or the stream ended or
        /// failed: `SO_RCVLOWAT`, on Linux TCP only (elsewhere it does
        /// nothing). The kernel lowers a value larger than half the receive
        /// buffer to that. Reads that find fewer bytes still return them.
        pub(crate) fn set_recv_low_water(&self, n: usize) {
            #[cfg(target_os = "linux")]
            if let Sock::Tcp(_) = &*self.sock {
                let v = libc::c_int::try_from(n).unwrap_or(libc::c_int::MAX);
                // SAFETY: a valid socket and an int option of its size.
                // A failure leaves the value as it was, which only costs
                // wake-ups.
                unsafe {
                    libc::setsockopt(
                        self.raw_fd(),
                        libc::SOL_SOCKET,
                        libc::SO_RCVLOWAT,
                        (&v as *const libc::c_int).cast(),
                        std::mem::size_of::<libc::c_int>() as libc::socklen_t,
                    );
                }
            }
            #[cfg(not(target_os = "linux"))]
            let _ = n;
        }

        /// Reads through the runtime's readiness, so that a `WouldBlock`
        /// clears it.
        pub(crate) fn try_recv(&self, buf: &mut [MaybeUninit<u8>]) -> io::Result<usize> {
            let watch = self.watch.lock().unwrap();
            match &watch.reg {
                Some(reg) => reg.try_io(Interest::READABLE, |_| self.recv(buf)),
                None => self.recv(buf),
            }
        }

        /// Forgets the readiness the runtime has seen.
        pub(crate) fn clear_readable(&self) {
            let watch = self.watch.lock().unwrap();
            if let Some(reg) = &watch.reg {
                let _ = reg.try_io(Interest::READABLE, |_| {
                    Err::<(), _>(io::ErrorKind::WouldBlock.into())
                });
            }
        }

        /// Whether the socket may be read, for the reader task. Off the
        /// poller, it stays pending until [`watch`](Self::watch).
        pub(crate) fn poll_readable(&self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            let mut watch = self.watch.lock().unwrap();
            match &mut watch.waker {
                Some(w) => w.clone_from(cx.waker()),
                None => watch.waker = Some(cx.waker().clone()),
            }
            if let Some(e) = watch.failed.take() {
                return Poll::Ready(Err(e));
            }
            match &watch.reg {
                Some(reg) => reg.poll_read_ready(cx).map_ok(|_| ()),
                None => Poll::Pending,
            }
        }

        /// Takes the reading off the poller: a thread reads the socket.
        pub(crate) fn unwatch(&self) {
            let reg = self.watch.lock().unwrap().reg.take();
            if reg.is_some() {
                stats::count(Counter::unwatches);
            }
            // Deregistered here, outside the lock.
            drop(reg);
        }

        /// Puts the reading back on the poller, for the reader task, and
        /// makes the task's wait see it. What is on the socket already
        /// shows as readiness at once.
        pub(crate) fn watch(&self) {
            if let Err(e) = self.register() {
                // The task's wait ends with the error, which closes the
                // connection.
                let mut watch = self.watch.lock().unwrap();
                watch.failed = Some(e);
                if let Some(waker) = &watch.waker {
                    waker.wake_by_ref();
                }
            }
        }

        fn register(&self) -> io::Result<()> {
            let mut watch = self.watch.lock().unwrap();
            if watch.reg.is_some() {
                return Ok(());
            }
            let reg = {
                let _runtime = self.handle.enter();
                // SAFETY: `dup` stays open while registered: `watch` is
                // dropped first.
                unsafe {
                    AsyncFd::register_with_interest(self.dup.as_raw_fd(), Interest::READABLE)?
                }
            };
            stats::count(Counter::watches);
            if let Some(waker) = &watch.waker {
                // Leaves the task's waker with the new registration,
                // without waking the task unless the socket is ready.
                let mut cx = Context::from_waker(waker);
                if reg.poll_read_ready(&mut cx).is_ready() {
                    waker.wake_by_ref();
                }
            }
            watch.reg = Some(reg);
            Ok(())
        }

        /// Shuts the read direction down.
        pub(crate) fn shutdown_read(&self) {
            let _ = SockRef::from(&*self.sock).shutdown(std::net::Shutdown::Read);
        }

        pub(crate) fn raw_fd(&self) -> RawFd {
            self.sock.as_fd().as_raw_fd()
        }
    }

    /// The write half of a [`Stream`]. Dropping it shuts the write
    /// direction down.
    #[derive(Debug)]
    pub(crate) struct WriteHalf {
        io: AsyncFd<Shared>,
    }

    impl WriteHalf {
        fn write(&self, f: impl FnOnce(&Sock) -> io::Result<usize>) -> io::Result<usize> {
            self.io.try_io(Interest::WRITABLE, |s| f(&s.0))
        }

        pub(crate) fn try_write(&self, buf: &[u8]) -> io::Result<usize> {
            self.write(|s| match s {
                Sock::Tcp(s) => (&*s).write(buf),
                Sock::Unix(s) => (&*s).write(buf),
            })
        }

        pub(crate) fn try_write_vectored(&self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
            self.write(|s| match s {
                Sock::Tcp(s) => (&*s).write_vectored(bufs),
                Sock::Unix(s) => (&*s).write_vectored(bufs),
            })
        }

        pub(crate) async fn writable(&self) -> io::Result<()> {
            // The guard keeps the readiness; a write that blocks clears it.
            self.io.writable().await.map(|_| ())
        }

        pub(super) fn poll_write_with(
            &mut self,
            cx: &mut Context<'_>,
            mut f: impl FnMut(&Self) -> io::Result<usize>,
        ) -> Poll<io::Result<usize>> {
            loop {
                if let Err(e) = std::task::ready!(self.io.poll_write_ready(cx)) {
                    return Poll::Ready(Err(e));
                }
                match f(self) {
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
                    result => return Poll::Ready(result),
                }
            }
        }

        pub(super) fn shutdown_write(&self) -> io::Result<()> {
            SockRef::from(&*self.io.get_ref().0).shutdown(std::net::Shutdown::Write)
        }
    }

    impl Drop for WriteHalf {
        fn drop(&mut self) {
            let _ = self.shutdown_write();
        }
    }
}

/// The halves elsewhere: tokio's own. A thread never reads the socket
/// there, so the reading never leaves the poller.
#[cfg(not(unix))]
mod tokio_halves {
    use std::io::{self, IoSlice};
    use std::mem::MaybeUninit;
    use std::net::SocketAddr;
    use std::task::{Context, Poll};

    use socket2::SockRef;
    use tokio::io::Interest;
    use tokio::net::tcp;

    use super::Stream;

    pub(super) fn split(stream: Stream) -> io::Result<(ReadHalf, WriteHalf)> {
        match stream {
            Stream::Tcp(s) => {
                let (r, w) = s.into_split();
                Ok((ReadHalf(r), WriteHalf(w)))
            }
        }
    }

    /// The read half of a [`Stream`].
    #[derive(Debug)]
    pub(crate) struct ReadHalf(tcp::OwnedReadHalf);

    impl ReadHalf {
        /// The address of the peer.
        pub(crate) fn peer_addr(&self) -> Option<SocketAddr> {
            self.0.peer_addr().ok()
        }

        /// Does nothing here (see the Unix one).
        pub(crate) fn set_recv_low_water(&self, _: usize) {}

        /// Reads straight from the socket, bypassing the runtime's
        /// readiness.
        pub(crate) fn recv(&self, buf: &mut [MaybeUninit<u8>]) -> io::Result<usize> {
            SockRef::from(self.0.as_ref()).recv(buf)
        }

        /// Reads through the runtime's readiness, so that a `WouldBlock`
        /// clears it.
        pub(crate) fn try_recv(&self, buf: &mut [MaybeUninit<u8>]) -> io::Result<usize> {
            let s = self.0.as_ref();
            s.try_io(Interest::READABLE, || SockRef::from(s).recv(buf))
        }

        /// Forgets the readiness the runtime has seen.
        pub(crate) fn clear_readable(&self) {
            let _ = self.0.as_ref().try_io(Interest::READABLE, || {
                Err::<(), _>(io::ErrorKind::WouldBlock.into())
            });
        }

        /// Whether the socket may be read, for the reader task.
        pub(crate) fn poll_readable(&self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            self.0.as_ref().poll_read_ready(cx)
        }

        /// Nothing to do: a thread never reads here.
        pub(crate) fn unwatch(&self) {}

        /// Nothing to do: the reading never leaves the poller here.
        pub(crate) fn watch(&self) {}

        /// Shuts the read direction down.
        pub(crate) fn shutdown_read(&self) {
            let _ = SockRef::from(self.0.as_ref()).shutdown(std::net::Shutdown::Read);
        }
    }

    /// The write half of a [`Stream`]. Dropping it shuts the write
    /// direction down.
    #[derive(Debug)]
    pub(crate) struct WriteHalf(tcp::OwnedWriteHalf);

    impl WriteHalf {
        pub(crate) fn try_write(&self, buf: &[u8]) -> io::Result<usize> {
            self.0.try_write(buf)
        }

        pub(crate) fn try_write_vectored(&self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
            self.0.try_write_vectored(bufs)
        }

        pub(crate) async fn writable(&self) -> io::Result<()> {
            self.0.writable().await
        }

        pub(super) fn poll_write_with(
            &mut self,
            cx: &mut Context<'_>,
            mut f: impl FnMut(&Self) -> io::Result<usize>,
        ) -> Poll<io::Result<usize>> {
            loop {
                if let Err(e) = std::task::ready!(self.0.as_ref().poll_write_ready(cx)) {
                    return Poll::Ready(Err(e));
                }
                match f(self) {
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
                    result => return Poll::Ready(result),
                }
            }
        }

        pub(super) fn shutdown_write(&self) -> io::Result<()> {
            SockRef::from(self.0.as_ref()).shutdown(std::net::Shutdown::Write)
        }
    }
}

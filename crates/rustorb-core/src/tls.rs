//! TLS for IIOP (SSLIOP, docs/ssliop.md): the rustls configurations made
//! from the ORB options, the handshake, and the record layer between a
//! connection's socket and its GIOP messages.
//!
//! A connection keeps its own way of reading and writing the socket
//! (docs/read-ownership.md); with TLS, what it reads goes through
//! [`Tls::read`] and what it writes through [`Tls::encrypt`] and
//! [`Tls::flush`]. rustls keeps the records not written yet in order,
//! so whichever side flushes them, they reach the socket in order.

use std::io::{self, Read, Write};
use std::mem::MaybeUninit;
use std::sync::{Arc, Mutex};

use rustls::client::WebPkiServerVerifier;
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::CryptoProvider;
use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName, UnixTime};
use rustls::server::WebPkiClientVerifier;
use rustls::{
    CertificateError, ClientConfig, ClientConnection, DigitallySignedStruct, RootCertStore,
    ServerConfig, ServerConnection, SignatureScheme,
};
use rustorb_giop::OutMessage;
use tokio::net::TcpStream;

use crate::{ClientAuth, TlsOptions};

/// At most this much plaintext is taken from rustls with one read.
const PLAIN_CHUNK: usize = 64 * 1024;

fn provider() -> Arc<CryptoProvider> {
    Arc::new(rustls::crypto::ring::default_provider())
}

fn certs(path: &std::path::Path) -> Result<Vec<CertificateDer<'static>>, String> {
    let certs = CertificateDer::pem_file_iter(path)
        .and_then(|it| it.collect::<Result<Vec<_>, _>>())
        .map_err(|e| format!("{}: {e}", path.display()))?;
    if certs.is_empty() {
        return Err(format!("{}: no certificate", path.display()));
    }
    Ok(certs)
}

fn roots(options: &TlsOptions) -> Result<RootCertStore, String> {
    let path = options
        .ca_file
        .as_deref()
        .ok_or("no CA file (-ORBsslCAFile)")?;
    let mut roots = RootCertStore::empty();
    for cert in certs(path)? {
        roots
            .add(cert)
            .map_err(|e| format!("{}: {e}", path.display()))?;
    }
    Ok(roots)
}

/// This ORB's certificate chain and key: the chain from `cert_file`, or
/// else from `key_file` (one file holding both, as omniORB's key file
/// does). `None` when neither is given.
#[allow(clippy::type_complexity)]
fn identity(
    options: &TlsOptions,
) -> Result<Option<(Vec<CertificateDer<'static>>, PrivateKeyDer<'static>)>, String> {
    let Some(key_file) = options.key_file.as_deref() else {
        return match options.cert_file {
            Some(_) => Err("a certificate file without a key file (-ORBsslKeyFile)".into()),
            None => Ok(None),
        };
    };
    let chain = certs(options.cert_file.as_deref().unwrap_or(key_file))?;
    let key = PrivateKeyDer::from_pem_file(key_file)
        .map_err(|e| format!("{}: {e}", key_file.display()))?;
    Ok(Some((chain, key)))
}

/// The configuration of client connections.
pub(crate) fn client_config(options: &TlsOptions) -> Result<Arc<ClientConfig>, String> {
    let roots = Arc::new(roots(options)?);
    let builder = ClientConfig::builder_with_provider(provider())
        .with_safe_default_protocol_versions()
        .map_err(|e| e.to_string())?;
    let builder = if options.verify_host {
        builder.with_root_certificates(roots)
    } else {
        let inner = WebPkiServerVerifier::builder_with_provider(roots, provider())
            .build()
            .map_err(|e| e.to_string())?;
        builder
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(AnyName(inner)))
    };
    let mut config = match identity(options)? {
        Some((chain, key)) => builder
            .with_client_auth_cert(chain, key)
            .map_err(|e| e.to_string())?,
        None => builder.with_no_client_auth(),
    };
    // GIOP has nothing to resume, and an endpoint is not a web site.
    config.enable_sni = false;
    Ok(Arc::new(config))
}

/// The configuration of server connections.
pub(crate) fn server_config(options: &TlsOptions) -> Result<Arc<ServerConfig>, String> {
    let (chain, key) =
        identity(options)?.ok_or("a TLS endpoint without a key file (-ORBsslKeyFile)")?;
    let builder = ServerConfig::builder_with_provider(provider())
        .with_safe_default_protocol_versions()
        .map_err(|e| e.to_string())?;
    let builder = match options.client_auth {
        ClientAuth::None => builder.with_no_client_auth(),
        auth => {
            let roots = Arc::new(roots(options)?);
            let verifier = WebPkiClientVerifier::builder_with_provider(roots, provider());
            let verifier = if auth == ClientAuth::Optional {
                verifier.allow_unauthenticated()
            } else {
                verifier
            };
            builder.with_client_cert_verifier(verifier.build().map_err(|e| e.to_string())?)
        }
    };
    let config = builder
        .with_single_cert(chain, key)
        .map_err(|e| e.to_string())?;
    Ok(Arc::new(config))
}

/// Checks the server's chain like rustls does, but not whether the
/// certificate names the host connected to (`-ORBsslVerifyHost 0`).
#[derive(Debug)]
struct AnyName(Arc<WebPkiServerVerifier>);

impl ServerCertVerifier for AnyName {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        server_name: &ServerName<'_>,
        ocsp_response: &[u8],
        now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        match self
            .0
            .verify_server_cert(end_entity, intermediates, server_name, ocsp_response, now)
        {
            Err(rustls::Error::InvalidCertificate(
                CertificateError::NotValidForName | CertificateError::NotValidForNameContext { .. },
            )) => Ok(ServerCertVerified::assertion()),
            other => other,
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        self.0.verify_tls12_signature(message, cert, dss)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        self.0.verify_tls13_signature(message, cert, dss)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.0.supported_verify_schemes()
    }
}

/// The socket as rustls reads and writes it, without waiting.
struct Sock<'a>(&'a TcpStream);

impl Read for Sock<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.try_read(buf)
    }
}

impl Write for Sock<'_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.try_write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn invalid(e: rustls::Error) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, e)
}

/// Runs the handshake of `conn` on `stream`.
async fn handshake(stream: &TcpStream, mut conn: rustls::Connection) -> io::Result<Arc<Tls>> {
    conn.set_buffer_limit(None);
    loop {
        while conn.wants_write() {
            match conn.write_tls(&mut Sock(stream)) {
                Ok(_) => {}
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => stream.writable().await?,
                Err(e) => return Err(e),
            }
        }
        if !conn.is_handshaking() {
            return Ok(Arc::new(Tls {
                conn: Mutex::new(conn),
            }));
        }
        match conn.read_tls(&mut Sock(stream)) {
            Ok(0) => return Err(io::ErrorKind::UnexpectedEof.into()),
            Ok(_) => {
                if let Err(e) = conn.process_new_packets() {
                    // Tell the peer why (the alert is queued), if it reads.
                    let _ = conn.write_tls(&mut Sock(stream));
                    return Err(invalid(e));
                }
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => stream.readable().await?,
            Err(e) => return Err(e),
        }
    }
}

/// The client handshake with the server at `host`.
pub(crate) async fn connect(
    stream: &TcpStream,
    config: Arc<ClientConfig>,
    host: &str,
) -> io::Result<Arc<Tls>> {
    let host = host.trim_start_matches('[').trim_end_matches(']');
    let name = ServerName::try_from(host.to_owned())
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
    let conn = ClientConnection::new(config, name).map_err(invalid)?;
    handshake(stream, conn.into()).await
}

/// The server handshake with a client that connected.
pub(crate) async fn accept(stream: &TcpStream, config: Arc<ServerConfig>) -> io::Result<Arc<Tls>> {
    let conn = ServerConnection::new(config).map_err(invalid)?;
    handshake(stream, conn.into()).await
}

/// The TLS state of one connection after the handshake.
pub(crate) struct Tls {
    conn: Mutex<rustls::Connection>,
}

impl std::fmt::Debug for Tls {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Tls").finish_non_exhaustive()
    }
}

/// Reads one byte source into spare capacity; `WouldBlock` when nothing is
/// there now.
pub(crate) type RawRead<'a> = dyn FnMut(&mut [MaybeUninit<u8>]) -> io::Result<usize> + 'a;

struct Recv<'a, 'b>(&'a mut RawRead<'b>);

impl Read for Recv<'_, '_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        // SAFETY: the source only writes initialised bytes into the slice.
        let buf = unsafe { &mut *(buf as *mut [u8] as *mut [MaybeUninit<u8>]) };
        (self.0)(buf)
    }
}

struct Send<'a, F>(&'a mut F);

impl<F: FnMut(&[u8]) -> io::Result<usize>> Write for Send<'_, F> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        (self.0)(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Tls {
    /// Reads plaintext into `buf`, taking records from `recv` as needed:
    /// `WouldBlock` only when no plaintext is left and `recv` has nothing
    /// more now, `Ok(0)` when the peer closed with `close_notify`.
    pub(crate) fn read(
        &self,
        buf: &mut [MaybeUninit<u8>],
        recv: &mut RawRead<'_>,
    ) -> io::Result<usize> {
        let n = buf.len().min(PLAIN_CHUNK);
        let buf = &mut buf[..n];
        buf.fill(MaybeUninit::new(0));
        // SAFETY: every byte was just initialised.
        let buf = unsafe { &mut *(buf as *mut [MaybeUninit<u8>] as *mut [u8]) };
        let mut conn = self.conn.lock().unwrap();
        loop {
            match conn.reader().read(buf) {
                Ok(n) => return Ok(n),
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
                Err(e) => return Err(e),
            }
            if conn.read_tls(&mut Recv(recv))? > 0 {
                // Records to answer (a key update) are queued, and go out
                // with the next message written.
                conn.process_new_packets().map_err(invalid)?;
            }
        }
    }

    /// Encrypts `msg` from byte `from` on. Every byte is taken (the
    /// buffers have no limit); the records wait for [`flush`](Self::flush).
    pub(crate) fn encrypt(&self, msg: &OutMessage, from: usize) {
        let mut conn = self.conn.lock().unwrap();
        let mut skip = from;
        for piece in msg.pieces() {
            if skip >= piece.len() {
                skip -= piece.len();
                continue;
            }
            let _ = conn.writer().write_all(&piece[skip..]);
            skip = 0;
        }
    }

    /// Queues `close_notify`.
    pub(crate) fn close_notify(&self) {
        self.conn.lock().unwrap().send_close_notify();
    }

    /// Writes the records waiting with `write`, which does not wait.
    /// Returns whether every one was written; `false` when `write` would
    /// block.
    pub(crate) fn flush(
        &self,
        mut write: impl FnMut(&[u8]) -> io::Result<usize>,
    ) -> io::Result<bool> {
        let mut conn = self.conn.lock().unwrap();
        while conn.wants_write() {
            match conn.write_tls(&mut Send(&mut write)) {
                Ok(0) => return Err(io::ErrorKind::WriteZero.into()),
                Ok(_) => {}
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => return Ok(false),
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => return Err(e),
            }
        }
        Ok(true)
    }
}

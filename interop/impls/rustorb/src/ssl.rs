//! `INTEROP_SSL=DIR` (run-matrix.sh --ssl): the endpoint is TLS only, with
//! the test CA and certificate in DIR, and certificates are required both
//! ways, as omniORB requires by default.

/// The ORB arguments for the endpoint at `host:port`. With
/// `INTEROP_UNIX=DIR` (run-matrix.sh --unix) it is a Unix socket in DIR
/// only (omniORB's `giop:unix`).
pub fn endpoint_args(host: &str, port: u16) -> Vec<String> {
    if let Ok(dir) = std::env::var("INTEROP_UNIX") {
        if !dir.is_empty() {
            return vec![
                "-ORBendPoint".into(),
                format!("giop:unix:{dir}/{host}-{port}.sock"),
            ];
        }
    }
    match std::env::var("INTEROP_SSL") {
        Ok(dir) if !dir.is_empty() => vec![
            "-ORBendPoint".into(),
            format!("giop:ssl:{host}:{port}"),
            "-ORBsslCAFile".into(),
            format!("{dir}/ca.pem"),
            "-ORBsslKeyFile".into(),
            format!("{dir}/server-bundle.pem"),
            "-ORBsslVerifyClient".into(),
            "required".into(),
        ],
        _ => vec!["-ORBendPoint".into(), format!("giop:tcp:{host}:{port}")],
    }
}

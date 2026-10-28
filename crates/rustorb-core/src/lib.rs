//! ORB core: connection management, request dispatch and the POA.
//!
//! Network I/O runs on an internal multi-threaded tokio runtime, and
//! servants are invoked on a separate pool of long-lived ORB threads.
//! The public API is synchronous.

//! Stack for the caller's callbacks on Windows.
//!
//! A thread's stack on Windows is committed a page at a time, as code
//! touches the guard page below what is committed; a touch further down
//! faults. The code the JVM compiles touches the stack once, far below
//! where it runs, at its start (its stack bang), and so expects the pages
//! it skips to be committed already. Inside a call into rustorb they may
//! not be: rustorb's own frames (large in a debug build) can take the
//! stack below what the JVM committed for the thread, and a callback into
//! the JVM then faulted past the guard page and crashed it.
//!
//! So before a callback, [`commit`] touches the stack page by page down to
//! a margin below where it is; the pages stay committed for the thread.

/// How far below the current frame the stack is committed.
#[cfg(windows)]
const MARGIN: usize = 128 * 1024;

/// Commits the stack down to [`MARGIN`] below here (Windows only).
#[cfg(windows)]
#[inline(never)]
pub(crate) fn commit() {
    // A frame larger than a page is probed page by page from the top
    // (__chkstk), which commits every page on the way.
    let mut frame = std::mem::MaybeUninit::<[u8; MARGIN]>::uninit();
    std::hint::black_box(frame.as_mut_ptr());
}

#[cfg(not(windows))]
#[inline(always)]
pub(crate) fn commit() {}

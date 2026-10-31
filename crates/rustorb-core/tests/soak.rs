//! Long run: client ORBs, connections, objects and POAs made and dropped
//! over and over, while the memory, file descriptors and threads of the
//! process are sampled. None of them may keep growing (issue #49).
//!
//! `RUSTORB_SOAK_SECS` sets the length (5 seconds by default; the
//! scheduled workflow, .github/workflows/soak.yml, runs it for longer). The
//! samples come from /proc on Linux, from `proc_pidinfo` on macOS and from
//! the process and Tool Help functions on Windows (where the descriptors
//! are the process's handles); elsewhere the workload runs without the
//! checks. This is the only test of its file, so nothing else runs in the
//! process meanwhile.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use rustorb_core::{
    CompletionStatus, EndpointAddr, Exception, InStream, ObjectRef, Orb, OrbOptions, Servant,
    ServerReply, SystemException, SystemExceptionKind, minor,
};

const SOAK_ID: &str = "IDL:test/Soak:1.0";

#[derive(Default)]
struct SoakServant {
    oneways: AtomicUsize,
}

impl Servant for SoakServant {
    fn repository_id(&self) -> &str {
        SOAK_ID
    }

    fn dispatch(
        &self,
        operation: &str,
        input: &mut InStream<'_>,
        reply: &mut ServerReply,
    ) -> Result<(), SystemException> {
        match operation {
            "echo_string" => {
                let s = input.read_string()?;
                reply.output().write_string(&s)
            }
            "post" => {
                self.oneways.fetch_add(1, Ordering::Relaxed);
                Ok(())
            }
            _ => Err(SystemException::new(
                SystemExceptionKind::BAD_OPERATION,
                minor::UNKNOWN_OPERATION,
                CompletionStatus::No,
            )),
        }
    }
}

fn echo_string(obj: &ObjectRef, s: &str) -> Result<String, Exception> {
    let reply = obj
        .request("echo_string")
        .args(|out| out.write_string(s))
        .invoke()?;
    Ok(reply.input().read_string()?)
}

#[derive(Debug, Clone, Copy)]
struct Sample {
    rss_kib: u64,
    /// File descriptors; handles on Windows.
    fds: usize,
    threads: usize,
}

/// How far the descriptors and threads may be above the warm-up sample at
/// the end. Windows handles also count events, keys and the like that the
/// system keeps for a while, so they get more room.
#[cfg(not(windows))]
const SLACK: usize = 2;
#[cfg(windows)]
const SLACK: usize = 16;

#[cfg(target_os = "linux")]
fn sample() -> Option<Sample> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let field = |name: &str| -> Option<u64> {
        status
            .lines()
            .find_map(|l| l.strip_prefix(name))?
            .split_whitespace()
            .next()?
            .parse()
            .ok()
    };
    Some(Sample {
        rss_kib: field("VmRSS:")?,
        threads: field("Threads:")? as usize,
        fds: std::fs::read_dir("/proc/self/fd").ok()?.count(),
    })
}

#[cfg(target_os = "macos")]
fn sample() -> Option<Sample> {
    use std::mem::{size_of, zeroed};
    // SAFETY: proc_pidinfo writes at most the size given into the buffers,
    // which are plain C structs.
    unsafe {
        let pid = libc::getpid();
        let mut task: libc::proc_taskinfo = zeroed();
        let size = size_of::<libc::proc_taskinfo>() as libc::c_int;
        if libc::proc_pidinfo(pid, libc::PROC_PIDTASKINFO, 0, (&raw mut task).cast(), size) != size
        {
            return None;
        }
        // With no buffer, PROC_PIDLISTFDS gives the size the list needs;
        // ask with room for more in case descriptors open meanwhile.
        let needed = libc::proc_pidinfo(pid, libc::PROC_PIDLISTFDS, 0, std::ptr::null_mut(), 0);
        if needed <= 0 {
            return None;
        }
        let mut fds: Vec<libc::proc_fdinfo> =
            vec![zeroed(); needed as usize / size_of::<libc::proc_fdinfo>() + 16];
        let got = libc::proc_pidinfo(
            pid,
            libc::PROC_PIDLISTFDS,
            0,
            fds.as_mut_ptr().cast(),
            (fds.len() * size_of::<libc::proc_fdinfo>()) as libc::c_int,
        );
        if got <= 0 {
            return None;
        }
        Some(Sample {
            rss_kib: task.pti_resident_size / 1024,
            fds: got as usize / size_of::<libc::proc_fdinfo>(),
            threads: usize::try_from(task.pti_threadnum).ok()?,
        })
    }
}

#[cfg(windows)]
fn sample() -> Option<Sample> {
    use std::mem::{size_of, zeroed};
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First, Thread32Next,
    };
    use windows_sys::Win32::System::ProcessStatus::{
        K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS,
    };
    use windows_sys::Win32::System::Threading::{
        GetCurrentProcess, GetCurrentProcessId, GetProcessHandleCount,
    };
    // SAFETY: the functions write only into the structs passed with their
    // sizes; the snapshot handle is closed before returning.
    unsafe {
        let process = GetCurrentProcess();
        let mut memory: PROCESS_MEMORY_COUNTERS = zeroed();
        memory.cb = size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
        if K32GetProcessMemoryInfo(process, &mut memory, memory.cb) == 0 {
            return None;
        }
        let mut handles = 0;
        if GetProcessHandleCount(process, &mut handles) == 0 {
            return None;
        }
        // The snapshot lists the threads of every process.
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            return None;
        }
        let me = GetCurrentProcessId();
        let mut entry: THREADENTRY32 = zeroed();
        entry.dwSize = size_of::<THREADENTRY32>() as u32;
        let mut threads = 0;
        let mut more = Thread32First(snapshot, &mut entry) != 0;
        while more {
            if entry.th32OwnerProcessID == me {
                threads += 1;
            }
            more = Thread32Next(snapshot, &mut entry) != 0;
        }
        CloseHandle(snapshot);
        Some(Sample {
            rss_kib: (memory.WorkingSetSize / 1024) as u64,
            // The snapshot's own handle is closed already.
            fds: handles as usize,
            threads,
        })
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
fn sample() -> Option<Sample> {
    None
}

/// One round: a client ORB with its own connection and runtime, calls
/// from several threads, an object activated and deactivated, a child
/// POA created and destroyed, and the client ORB destroyed.
fn round(server: &Orb, ior: &str, servant: &Arc<SoakServant>, n: usize) {
    let client = Orb::with_options(OrbOptions {
        io_threads: 1,
        ..OrbOptions::default()
    })
    .unwrap();
    let obj = client.string_to_object(ior).unwrap().unwrap();
    let callers: Vec<_> = (0..4)
        .map(|t| {
            let obj = obj.clone();
            thread::spawn(move || {
                for i in 0..25 {
                    let s = format!("round {n} thread {t} call {i}");
                    assert_eq!(echo_string(&obj, &s).unwrap(), s);
                }
                obj.request("post").send_oneway().unwrap();
            })
        })
        .collect();

    // Objects and POAs come and go on the server meanwhile.
    let root = server.root_poa().unwrap();
    let child = root
        .create_poa(&format!("round{n}"), Some(&root.the_poa_manager()), &[])
        .unwrap();
    let id = child.activate_object(servant.clone()).unwrap();
    let temp = child.id_to_reference(&id).unwrap();
    let temp = client
        .string_to_object(&server.object_to_string(Some(&temp)).unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(echo_string(&temp, "temp").unwrap(), "temp");
    child.deactivate_object(&id).unwrap();
    child.destroy(true, true).unwrap();

    for c in callers {
        c.join().unwrap();
    }
    client.destroy().unwrap();
}

#[test]
fn resources_do_not_grow_over_a_long_run() {
    let secs: u64 = std::env::var("RUSTORB_SOAK_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(5);
    let length = Duration::from_secs(secs);
    let server = Orb::with_options(OrbOptions {
        endpoints: vec![EndpointAddr {
            host: "127.0.0.1".into(),
            port: 0,
        }],
        min_server_threads: 2,
        server_thread_idle_timeout: Duration::from_millis(100),
        ..OrbOptions::default()
    })
    .unwrap();
    let servant = Arc::new(SoakServant::default());
    let root = server.root_poa().unwrap();
    let id = root.activate_object(servant.clone()).unwrap();
    let obj = root.id_to_reference(&id).unwrap();
    root.the_poa_manager().activate().unwrap();
    let ior = server.object_to_string(Some(&obj)).unwrap();

    // Warm up: the allocator, the server's runtime and its pool settle.
    let start = Instant::now();
    let mut n = 0;
    while start.elapsed() < length / 5 || n < 10 {
        round(&server, &ior, &servant, n);
        n += 1;
    }
    let settle = || {
        // Upcall threads beyond the minimum end when idle.
        thread::sleep(Duration::from_millis(500));
        sample()
    };
    let base = settle();
    let mut peak = base;
    let mut half = None;
    while start.elapsed() < length {
        round(&server, &ior, &servant, n);
        n += 1;
        if let (Some(p), Some(s)) = (peak.as_mut(), sample()) {
            p.rss_kib = p.rss_kib.max(s.rss_kib);
            p.fds = p.fds.max(s.fds);
            p.threads = p.threads.max(s.threads);
        }
        if half.is_none() && start.elapsed() >= length * 3 / 5 {
            half = settle();
        }
    }
    let end = settle();
    eprintln!("{n} rounds in {:.1?}", start.elapsed());
    eprintln!("after warm-up: {base:?}");
    eprintln!("halfway:       {half:?}");
    eprintln!("at the end:    {end:?}");
    eprintln!("peak:          {peak:?}");

    if let (Some(base), Some(half), Some(end)) = (base, half, end) {
        // Every client connection and runtime is gone after its round.
        assert!(
            end.fds <= base.fds + SLACK,
            "file descriptors grew from {} to {}",
            base.fds,
            end.fds
        );
        assert!(
            end.threads <= base.threads + SLACK,
            "threads grew from {} to {}",
            base.threads,
            end.threads
        );
        // Memory: the second half of the run may not add more than a
        // little over what the first half settled at.
        let allowed = (half.rss_kib / 10).max(8 * 1024);
        assert!(
            end.rss_kib <= half.rss_kib + allowed,
            "resident memory grew from {} KiB halfway to {} KiB",
            half.rss_kib,
            end.rss_kib
        );
    }
    assert!(servant.oneways.load(Ordering::Relaxed) > 0);
    server.destroy().unwrap();
}

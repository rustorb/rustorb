//! rustorb load client for `interop/run-load.sh`: calls an Echo server
//! (resolved from the naming service) as fast as it can for a while and
//! prints the rate of each phase.
//!
//! Options: `--naming CORBALOC --name PATH [--seconds S] [--threads N]
//! [--connections N]`. The phases, each `--seconds` long:
//!
//! - `small`: `echo_long` from N threads, all on one connection
//! - `string`: `echo_string` of 100 characters, the same way
//! - `connections`: `echo_long` from N client ORBs, a connection each
//! - `large`: `echo_octets` of 1 MiB from 4 threads
//!
//! Each line reads `PHASE calls=N errors=N rate=N/s`; a failed call
//! makes the exit status 1.

mod idl;

use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use idl::interop_test::Echo;
use rustorb_core::{Orb, OrbOptions};
use rustorb_naming::{NamingContext, to_name};

struct Args {
    naming: String,
    name: String,
    seconds: u64,
    threads: usize,
    connections: usize,
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args {
        naming: String::new(),
        name: String::new(),
        seconds: 5,
        threads: 8,
        connections: 16,
    };
    let mut iter = std::env::args().skip(1);
    while let Some(a) = iter.next() {
        let mut value = || iter.next().ok_or_else(|| format!("{a} needs a value"));
        match a.as_str() {
            "--naming" => args.naming = value()?,
            "--name" => args.name = value()?,
            "--seconds" => args.seconds = value()?.parse().map_err(|e| format!("{a}: {e}"))?,
            "--threads" => args.threads = value()?.parse().map_err(|e| format!("{a}: {e}"))?,
            "--connections" => {
                args.connections = value()?.parse().map_err(|e| format!("{a}: {e}"))?
            }
            _ => return Err(format!("unknown argument: {a}")),
        }
    }
    if args.naming.is_empty() || args.name.is_empty() {
        return Err("--naming and --name are needed".into());
    }
    Ok(args)
}

/// The Echo object, waiting while the server starts.
fn resolve(orb: &Orb, args: &Args) -> Result<Echo, String> {
    let root = orb
        .string_to_object(&args.naming)
        .map_err(|e| format!("{}: {e}", args.naming))?
        .ok_or("nil naming service")?;
    let root = NamingContext::unchecked_narrow(root);
    let name = to_name(&args.name).map_err(|e| format!("--name {}: {e}", args.name))?;
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let found = root
            .resolve(&name)
            .map_err(|e| e.to_string())
            .and_then(|obj| obj.ok_or_else(|| "nil".to_owned()))
            .and_then(|obj| {
                Echo::narrow(&obj)
                    .map_err(|e| e.to_string())?
                    .ok_or_else(|| "not an Echo".to_owned())
            });
        match found {
            Ok(echo) if echo.echo_long(1) == Ok(1) => return Ok(echo),
            Ok(_) | Err(_) if Instant::now() < deadline => {
                thread::sleep(Duration::from_millis(200))
            }
            Ok(echo) => return Err(format!("echo_long: {:?}", echo.echo_long(1))),
            Err(e) => return Err(format!("{}: {e}", args.name)),
        }
    }
}

/// Runs `call` from each of `workers` until `seconds` pass and prints the
/// totals. Returns whether every call succeeded.
fn phase<W: Send + 'static>(
    label: &str,
    seconds: u64,
    workers: Vec<W>,
    call: impl Fn(&W, usize) -> bool + Send + Sync + 'static,
) -> bool {
    let stop = Arc::new(AtomicBool::new(false));
    let calls = Arc::new(AtomicUsize::new(0));
    let errors = Arc::new(AtomicUsize::new(0));
    let call = Arc::new(call);
    let start = Instant::now();
    let handles: Vec<_> = workers
        .into_iter()
        .map(|w| {
            let (stop, calls, errors, call) =
                (stop.clone(), calls.clone(), errors.clone(), call.clone());
            thread::spawn(move || {
                let mut i = 0;
                while !stop.load(Ordering::Relaxed) {
                    if call(&w, i) {
                        calls.fetch_add(1, Ordering::Relaxed);
                    } else {
                        errors.fetch_add(1, Ordering::Relaxed);
                    }
                    i += 1;
                }
            })
        })
        .collect();
    thread::sleep(Duration::from_secs(seconds));
    stop.store(true, Ordering::Relaxed);
    for h in handles {
        let _ = h.join();
    }
    let took = start.elapsed().as_secs_f64();
    let (calls, errors) = (
        calls.load(Ordering::Relaxed),
        errors.load(Ordering::Relaxed),
    );
    println!(
        "{label} calls={calls} errors={errors} rate={:.0}/s",
        calls as f64 / took
    );
    errors == 0
}

fn run() -> Result<bool, String> {
    let args = parse_args()?;
    let orb = Orb::with_options(OrbOptions::default()).map_err(|e| format!("ORB_init: {e}"))?;
    let echo = resolve(&orb, &args)?;
    let mut ok = true;

    let echoes = vec![echo.clone(); args.threads];
    ok &= phase("small", args.seconds, echoes.clone(), |e: &Echo, i| {
        e.echo_long(i as i32) == Ok(i as i32)
    });
    let text = "x".repeat(100);
    ok &= phase("string", args.seconds, echoes, move |e: &Echo, _| {
        e.echo_string(&text).as_deref() == Ok(text.as_str())
    });

    // Each worker has its own ORB, and so its own connection.
    let ior = orb
        .object_to_string(Some(echo.as_ref()))
        .map_err(|e| e.to_string())?;
    let mut clients = Vec::new();
    for _ in 0..args.connections {
        let o = Orb::with_options(OrbOptions {
            io_threads: 1,
            ..OrbOptions::default()
        })
        .map_err(|e| format!("ORB_init: {e}"))?;
        let obj = o
            .string_to_object(&ior)
            .map_err(|e| e.to_string())?
            .ok_or("nil")?;
        clients.push((Echo::unchecked_narrow(obj), o));
    }
    let orbs: Vec<Orb> = clients.iter().map(|(_, o)| o.clone()).collect();
    ok &= phase(
        "connections",
        args.seconds,
        clients,
        |(e, _): &(Echo, Orb), i| e.echo_long(i as i32) == Ok(i as i32),
    );
    for o in orbs {
        let _ = o.destroy();
    }

    let data: Arc<Vec<u8>> = Arc::new((0..1 << 20).map(|i| i as u8).collect());
    ok &= phase(
        "large",
        args.seconds,
        vec![echo.clone(); 4],
        move |e: &Echo, _| match e.echo_octets(&data) {
            Ok(back) => back == *data,
            Err(err) => {
                eprintln!("echo_octets: {err}");
                false
            }
        },
    );
    let _ = orb.destroy();
    Ok(ok)
}

fn main() -> ExitCode {
    match run() {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("rustorb-load-client: {e}");
            ExitCode::FAILURE
        }
    }
}

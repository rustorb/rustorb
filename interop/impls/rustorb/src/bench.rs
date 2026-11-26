//! rustorb (Rust) program of the rustorb / omniORB comparison benchmark
//! (`interop/bench`, `docs/omniorb-comparison.md`).
//!
//! ```text
//! rustorb-bench serve  [ORB options]                 serves Bench::Target, prints its IOR
//! rustorb-bench client --ior IOR [options] [ORB options]
//! rustorb-bench inproc [options] [ORB options]       servant and caller in one ORB
//! ```
//!
//! Options: `--cases LIST` (comma-separated, see `interop/bench/README.md`),
//! `--seconds S` per case (default 2), `--warmup S` (default 0.3). Every
//! case prints one JSON line on stdout; a failed call makes the exit status
//! 1.

mod idl;

use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use idl::bench::{Sample, Target, TargetPoa, TargetServant};
use rustorb_core::{Any, Orb, SystemException};

struct Impl {
    orb: Orb,
    posted: AtomicU32,
}

impl TargetServant for Impl {
    fn ping(&self) -> Result<(), SystemException> {
        Ok(())
    }
    fn echo_long(&self, v: i32) -> Result<i32, SystemException> {
        Ok(v)
    }
    fn echo_string(&self, v: &str) -> Result<String, SystemException> {
        Ok(v.to_owned())
    }
    fn echo_octets(&self, v: &[u8]) -> Result<Vec<u8>, SystemException> {
        Ok(v.to_vec())
    }
    fn put_octets(&self, v: &[u8]) -> Result<u32, SystemException> {
        Ok(v.len() as u32)
    }
    fn get_octets(&self, n: u32) -> Result<Vec<u8>, SystemException> {
        Ok(vec![7u8; n as usize])
    }
    fn echo_samples(&self, v: &[Sample]) -> Result<Vec<Sample>, SystemException> {
        Ok(v.to_vec())
    }
    fn echo_any(&self, v: &Any) -> Result<Any, SystemException> {
        Ok(v.clone())
    }
    fn post(&self, _v: i32) -> Result<(), SystemException> {
        self.posted.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
    fn take_posted(&self) -> Result<u32, SystemException> {
        Ok(self.posted.swap(0, Ordering::Relaxed))
    }
    fn shutdown(&self) -> Result<(), SystemException> {
        let orb = self.orb.clone();
        thread::spawn(move || {
            let _ = orb.shutdown(false);
        });
        Ok(())
    }
}

struct Opts {
    mode: String,
    ior: String,
    cases: Vec<String>,
    seconds: f64,
    warmup: f64,
}

const DEFAULT_CASES: &str = "ping,long,string,octets:64,octets:1024,octets:16384,octets:262144,\
octets:1048576,octets:8388608,put:1048576,get:1048576,samples:100,any,oneway,threads:4";

fn parse(mut args: Vec<String>) -> Result<(Opts, Vec<String>), String> {
    if args.is_empty() {
        return Err("usage: rustorb-bench serve|client|inproc [options]".into());
    }
    let mode = args.remove(0);
    let mut o = Opts {
        mode,
        ior: String::new(),
        cases: Vec::new(),
        seconds: 2.0,
        warmup: 0.3,
    };
    let mut cases = DEFAULT_CASES.to_owned();
    let mut rest = vec!["rustorb-bench".to_owned()];
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        let mut value = || it.next().ok_or_else(|| format!("{a} needs a value"));
        match a.as_str() {
            "--ior" => o.ior = value()?,
            "--cases" => cases = value()?,
            "--seconds" => o.seconds = value()?.parse().map_err(|e| format!("{e}"))?,
            "--warmup" => o.warmup = value()?.parse().map_err(|e| format!("{e}"))?,
            _ => rest.push(a),
        }
    }
    o.cases = cases
        .split(',')
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect();
    Ok((o, rest))
}

/// Runs `call` until `seconds` pass (after `warmup`), timing each call,
/// and prints the JSON line of the case.
fn timed(
    case: &str,
    o: &Opts,
    bytes: usize,
    mut call: impl FnMut() -> Result<(), String>,
) -> Result<(), String> {
    let warm_end = Instant::now() + Duration::from_secs_f64(o.warmup);
    let mut warm = 0;
    while Instant::now() < warm_end {
        call()?;
        warm += 1;
    }
    let mut lat: Vec<u64> = Vec::with_capacity(1 << 16);
    let start = Instant::now();
    let end = start + Duration::from_secs_f64(o.seconds);
    loop {
        let t = Instant::now();
        call()?;
        let now = Instant::now();
        lat.push((now - t).as_nanos() as u64);
        if now >= end {
            break;
        }
    }
    let took = start.elapsed().as_secs_f64();
    report(case, lat, took, bytes, warm);
    Ok(())
}

fn report(case: &str, mut lat: Vec<u64>, took: f64, bytes: usize, warm: usize) {
    lat.sort_unstable();
    let n = lat.len();
    let pct = |p: f64| lat[((n - 1) as f64 * p) as usize] as f64 / 1000.0;
    let mean = lat.iter().sum::<u64>() as f64 / n as f64 / 1000.0;
    let rate = n as f64 / took;
    println!(
        "{{\"case\":\"{case}\",\"calls\":{n},\"seconds\":{took:.3},\"rate\":{rate:.1},\
\"mean_us\":{mean:.2},\"p50_us\":{:.2},\"p90_us\":{:.2},\"p99_us\":{:.2},\"max_us\":{:.2},\
\"mb_per_s\":{:.2},\"total_calls\":{}}}",
        pct(0.5),
        pct(0.9),
        pct(0.99),
        pct(1.0),
        rate * bytes as f64 / 1e6,
        n + warm
    );
}

fn err(e: SystemException) -> String {
    e.to_string()
}

fn run_case(t: &Target, case: &str, o: &Opts) -> Result<(), String> {
    let (name, arg) = case.split_once(':').unwrap_or((case, ""));
    let n: usize = if arg.is_empty() {
        0
    } else {
        arg.parse().map_err(|_| format!("bad case {case}"))?
    };
    match name {
        "first" => {
            // The first call on a fresh reference: the connection is made.
            let t0 = Instant::now();
            t.ping().map_err(err)?;
            let us = t0.elapsed().as_secs_f64() * 1e6;
            println!("{{\"case\":\"first\",\"calls\":1,\"p50_us\":{us:.2},\"mean_us\":{us:.2}}}");
            Ok(())
        }
        "ping" => timed(case, o, 0, || t.ping().map_err(err)),
        "long" => {
            let mut i = 0;
            timed(case, o, 0, || {
                i += 1;
                let r = t.echo_long(i).map_err(err)?;
                if r != i {
                    Err("echo_long mismatch".into())
                } else {
                    Ok(())
                }
            })
        }
        "string" => {
            let s = "x".repeat(if n == 0 { 100 } else { n });
            timed(case, o, 0, || {
                let r = t.echo_string(&s).map_err(err)?;
                if r.len() != s.len() {
                    Err("echo_string mismatch".into())
                } else {
                    Ok(())
                }
            })
        }
        "octets" => {
            let v = vec![1u8; n];
            timed(case, o, 2 * n, || {
                let r = t.echo_octets(&v).map_err(err)?;
                if r.len() != n {
                    Err("echo_octets mismatch".into())
                } else {
                    Ok(())
                }
            })
        }
        "put" => {
            let v = vec![1u8; n];
            timed(case, o, n, || {
                let r = t.put_octets(&v).map_err(err)?;
                if r as usize != n {
                    Err("put_octets mismatch".into())
                } else {
                    Ok(())
                }
            })
        }
        "get" => timed(case, o, n, || {
            let r = t.get_octets(n as u32).map_err(err)?;
            if r.len() != n {
                Err("get_octets mismatch".into())
            } else {
                Ok(())
            }
        }),
        "samples" => {
            let v: Vec<Sample> = (0..n as i32)
                .map(|i| Sample {
                    id: i,
                    x: 1.5,
                    y: 2.5,
                    z: 3.5,
                    name: format!("sample-{i}"),
                })
                .collect();
            timed(case, o, 0, || {
                let r = t.echo_samples(&v).map_err(err)?;
                if r.len() != n {
                    Err("echo_samples mismatch".into())
                } else {
                    Ok(())
                }
            })
        }
        "any" => {
            let s = Sample {
                id: 1,
                x: 1.5,
                y: 2.5,
                z: 3.5,
                name: "sample".into(),
            };
            let a = Any::insert::<Sample>(&s).map_err(err)?;
            timed(case, o, 0, || {
                let r = t.echo_any(&a).map_err(err)?;
                match r.extract::<Sample>() {
                    Some(v) if v.id == 1 => Ok(()),
                    _ => Err("echo_any mismatch".into()),
                }
            })
        }
        "oneway" => {
            t.take_posted().map_err(err)?;
            let start = Instant::now();
            let end = start + Duration::from_secs_f64(o.seconds);
            let mut sent: u64 = 0;
            while Instant::now() < end {
                for _ in 0..64 {
                    t.post(sent as i32).map_err(err)?;
                    sent += 1;
                }
            }
            let send_took = start.elapsed().as_secs_f64();
            // The server has them all once take_posted (on the same
            // connection, after them) has its reply; wait for stragglers
            // a server may still be dispatching.
            let mut got = t.take_posted().map_err(err)? as u64;
            let deadline = Instant::now() + Duration::from_secs(10);
            while got < sent && Instant::now() < deadline {
                thread::sleep(Duration::from_millis(1));
                got += t.take_posted().map_err(err)? as u64;
            }
            let took = start.elapsed().as_secs_f64();
            println!(
                "{{\"case\":\"{case}\",\"calls\":{sent},\"seconds\":{took:.3},\"rate\":{:.1},\
\"send_rate\":{:.1},\"received\":{got},\"total_calls\":{sent}}}",
                sent as f64 / took,
                sent as f64 / send_took
            );
            if got != sent {
                return Err(format!("oneway: sent {sent}, received {got}"));
            }
            Ok(())
        }
        "threads" => {
            let stop = Arc::new(AtomicBool::new(false));
            let calls = Arc::new(AtomicU64::new(0));
            let failed = Arc::new(Mutex::new(None::<String>));
            let warm = Duration::from_secs_f64(o.warmup);
            let mut hs = Vec::new();
            for w in 0..n.max(1) {
                let (t, stop, calls, failed) =
                    (t.clone(), stop.clone(), calls.clone(), failed.clone());
                hs.push(thread::spawn(move || {
                    let mut i = w as i32 * 1_000_000;
                    while !stop.load(Ordering::Relaxed) {
                        i += 1;
                        match t.echo_long(i) {
                            Ok(r) if r == i => {
                                calls.fetch_add(1, Ordering::Relaxed);
                            }
                            Ok(_) => {
                                *failed.lock().unwrap() = Some("echo_long mismatch".into());
                                break;
                            }
                            Err(e) => {
                                *failed.lock().unwrap() = Some(e.to_string());
                                break;
                            }
                        }
                    }
                }));
            }
            thread::sleep(warm);
            let c0 = calls.load(Ordering::Relaxed);
            let start = Instant::now();
            thread::sleep(Duration::from_secs_f64(o.seconds));
            let c1 = calls.load(Ordering::Relaxed);
            let took = start.elapsed().as_secs_f64();
            stop.store(true, Ordering::Relaxed);
            for h in hs {
                let _ = h.join();
            }
            if let Some(e) = failed.lock().unwrap().take() {
                return Err(e);
            }
            println!(
                "{{\"case\":\"{case}\",\"calls\":{},\"seconds\":{took:.3},\"rate\":{:.1},\
\"total_calls\":{}}}",
                c1 - c0,
                (c1 - c0) as f64 / took,
                calls.load(Ordering::Relaxed)
            );
            Ok(())
        }
        _ => Err(format!("unknown case {case}")),
    }
}

fn run() -> Result<(), String> {
    let (o, mut orb_args) = parse(std::env::args().skip(1).collect())?;
    let orb = Orb::init(&mut orb_args).map_err(|e| format!("ORB_init: {e}"))?;
    let target = match o.mode.as_str() {
        "serve" | "inproc" => {
            let poa = orb.root_poa().map_err(|e| format!("RootPOA: {e}"))?;
            let (_, target) = TargetPoa(Impl {
                orb: orb.clone(),
                posted: AtomicU32::new(0),
            })
            .activate(&poa)
            .map_err(|e| format!("activate: {e}"))?;
            poa.the_poa_manager()
                .activate()
                .map_err(|e| format!("activate: {e}"))?;
            if o.mode == "serve" {
                let ior = orb.object_to_string(Some(target.object())).map_err(err)?;
                println!("{ior}");
                use std::io::Write;
                let _ = std::io::stdout().flush();
                orb.run();
                return Ok(());
            }
            target
        }
        "client" => {
            let obj = orb
                .string_to_object(&o.ior)
                .map_err(err)?
                .ok_or("nil reference")?;
            Target::unchecked_narrow(obj)
        }
        m => return Err(format!("unknown mode {m}")),
    };
    for case in &o.cases {
        run_case(&target, case, &o).map_err(|e| format!("{case}: {e}"))?;
    }
    let _ = orb.shutdown(true);
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("rustorb-bench: {e}");
            ExitCode::FAILURE
        }
    }
}

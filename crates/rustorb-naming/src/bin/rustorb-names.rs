//! `rustorb-names`: the rustorb naming service daemon, a replacement for
//! omniNames.
//!
//! ```text
//! rustorb-names -start [PORT] [-logdir DIR]   first run (PORT defaults to 2809)
//! rustorb-names [-logdir DIR]                 later runs, on the same port
//! rustorb-names -memory [-start PORT]         no persistence
//! ```
//!
//! The root context answers to `corbaloc::HOST:PORT/NameService`. The
//! bindings are kept in DIR (`-logdir`, else `$RUSTORB_NAMES_LOGDIR`, else
//! `/var/lib/rustorb-names`) as an append-only log and checkpoints, and the
//! port in `naming.port`, so that later runs listen on it again and every
//! reference handed out stays valid. ORB options (`-ORBendPoint`,
//! `-ORBendPointPublish`, ...) are accepted; with `-ignoreport` the
//! endpoints come from them only.

use std::path::PathBuf;
use std::process::ExitCode;
use std::{env, fs};

use rustorb_core::Orb;
use rustorb_naming::{LogStore, MemoryStore, NamingService};

const DEFAULT_PORT: u16 = 2809;
const DEFAULT_LOGDIR: &str = "/var/lib/rustorb-names";
const PORT_FILE: &str = "naming.port";

const USAGE: &str = "\
usage: rustorb-names [-start [PORT]] [-logdir DIR] [-memory] [-ignoreport]
                     [-checkpoint N] [ORB options]

  -start [PORT]   first run: listen on PORT (default 2809) and remember it
  -logdir DIR     where the naming data is kept
                  (default $RUSTORB_NAMES_LOGDIR, else /var/lib/rustorb-names)
  -memory         keep nothing: start empty and forget everything at exit
  -ignoreport     listen only on the -ORBendPoint addresses
  -checkpoint N   write a checkpoint every N updates (default 1000)";

struct Args {
    start: Option<u16>,
    logdir: Option<PathBuf>,
    memory: bool,
    ignore_port: bool,
    checkpoint: usize,
    orb_args: Vec<String>,
}

fn parse_args(args: Vec<String>) -> Result<Args, String> {
    let mut parsed = Args {
        start: None,
        logdir: None,
        memory: false,
        ignore_port: false,
        checkpoint: 1000,
        orb_args: Vec::new(),
    };
    let mut iter = args.into_iter().peekable();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "-start" => {
                let port = match iter.peek() {
                    Some(p) if !p.starts_with('-') => {
                        let p = iter.next().unwrap_or_default();
                        p.parse().map_err(|_| format!("bad port: {p}"))?
                    }
                    _ => DEFAULT_PORT,
                };
                parsed.start = Some(port);
            }
            "-logdir" => {
                parsed.logdir = Some(iter.next().ok_or("-logdir needs a directory")?.into());
            }
            "-memory" => parsed.memory = true,
            "-ignoreport" => parsed.ignore_port = true,
            "-checkpoint" => {
                let n = iter.next().ok_or("-checkpoint needs a number")?;
                parsed.checkpoint = n.parse().map_err(|_| format!("bad number: {n}"))?;
            }
            "-h" | "-help" | "--help" => return Err(String::new()),
            _ if arg.starts_with("-ORB") => {
                parsed.orb_args.push(arg);
                if let Some(value) = iter.next_if(|v| !v.starts_with('-')) {
                    parsed.orb_args.push(value);
                }
            }
            _ => return Err(format!("unknown option: {arg}")),
        }
    }
    Ok(parsed)
}

fn run(args: Args) -> Result<(), String> {
    let logdir = args
        .logdir
        .or_else(|| env::var_os("RUSTORB_NAMES_LOGDIR").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from(DEFAULT_LOGDIR));
    let port_file = logdir.join(PORT_FILE);
    let port = if args.memory {
        args.start.unwrap_or(DEFAULT_PORT)
    } else {
        let saved = fs::read_to_string(&port_file)
            .ok()
            .and_then(|s| s.trim().parse::<u16>().ok());
        match (args.start, saved) {
            (Some(port), _) => {
                fs::create_dir_all(&logdir)
                    .and_then(|()| fs::write(&port_file, format!("{port}\n")))
                    .map_err(|e| format!("cannot write {}: {e}", port_file.display()))?;
                port
            }
            (None, Some(port)) => port,
            (None, None) => {
                return Err(format!(
                    "no naming data in {}: start the first time with -start [PORT]",
                    logdir.display()
                ));
            }
        }
    };

    let mut orb_args = args.orb_args;
    if !args.ignore_port
        && !orb_args
            .iter()
            .any(|a| a.starts_with("-ORBendPoint") && !a.starts_with("-ORBendPointPublish"))
    {
        orb_args.push("-ORBendPoint".into());
        orb_args.push(format!("giop:tcp::{port}"));
    }
    let orb = Orb::init(&mut orb_args).map_err(|e| format!("bad ORB options: {e}"))?;
    let service = if args.memory {
        NamingService::start(&orb, MemoryStore)
    } else {
        NamingService::start(
            &orb,
            LogStore::new(&logdir).checkpoint_every(args.checkpoint),
        )
    }
    .map_err(|e| e.to_string())?;
    let root = service.root_context();
    let ior = orb
        .object_to_string(Some(root.object()))
        .map_err(|e| e.to_string())?;
    if !args.memory {
        eprintln!("rustorb-names: naming data in {}", logdir.display());
    }
    for (host, port) in orb.endpoints() {
        eprintln!("rustorb-names: listening as corbaloc::{host}:{port}/NameService");
    }
    eprintln!("rustorb-names: root context is {ior}");
    orb.run();
    Ok(())
}

fn main() -> ExitCode {
    let args = match parse_args(env::args().skip(1).collect()) {
        Ok(args) => args,
        Err(e) => {
            if !e.is_empty() {
                eprintln!("rustorb-names: {e}");
            }
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    match run(args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("rustorb-names: {e}");
            ExitCode::FAILURE
        }
    }
}

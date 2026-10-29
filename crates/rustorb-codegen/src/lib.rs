//! IDL code generator for Rust, Python, C++11 and Java, driven by the
//! AST of [idl_parser](https://github.com/sugarsweetrobotics/idl_parser).
//!
//! The generator itself is written in Python, next to idl_parser: see the
//! `rustorb-idl` package in `python/` and `docs/python-codegen.md`. This
//! crate holds [`Config`], which runs `rustorb-idl -brust` from a build
//! script (`docs/rust-mapping.md`):
//!
//! ```no_run
//! // in main() of build.rs
//! rustorb_codegen::Config::new()
//!     .include("idl")
//!     .compile(&["idl/Example.idl"]) // $OUT_DIR/example.rs
//!     .unwrap();
//! ```
//!
//! and in the crate, `include!(concat!(env!("OUT_DIR"), "/example.rs"));`.
//!
//! Licensed under GPL-3.0-or-later. Code produced by the generator is not
//! covered by this license, and a build dependency on this crate does not
//! make the crate that uses it GPL.

use std::env;
use std::ffi::OsString;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Runs `rustorb-idl -brust` on IDL files, from a build script.
///
/// The generator is the `rustorb-idl` command, or the one the environment
/// variable `RUSTORB_IDL` names (a path, or a command and its first
/// arguments separated by spaces, such as `python3 -m rustorb_idl`).
#[derive(Debug, Clone, Default)]
pub struct Config {
    includes: Vec<PathBuf>,
    defines: Vec<String>,
    externs: Vec<(String, String)>,
    out_dir: Option<PathBuf>,
    program: Option<Vec<OsString>>,
}

impl Config {
    /// A configuration with no options.
    pub fn new() -> Config {
        Config::default()
    }

    /// Looks for `#include` files in `dir` (`-I`).
    pub fn include(&mut self, dir: impl AsRef<Path>) -> &mut Config {
        self.includes.push(dir.as_ref().to_owned());
        self
    }

    /// Defines a preprocessor macro (`-D NAME` or `-D NAME=VALUE`).
    pub fn define(&mut self, name: &str, value: Option<&str>) -> &mut Config {
        self.defines.push(match value {
            Some(v) => format!("{name}={v}"),
            None => name.to_owned(),
        });
        self
    }

    /// Refers to the definitions of the IDL module `module` under the Rust
    /// path `path` instead of writing them (`-Wbextern`), for code
    /// generated in another crate: `extern_module("CosNaming",
    /// "rustorb_naming::idl::cos_naming")`.
    pub fn extern_module(&mut self, module: &str, path: &str) -> &mut Config {
        self.externs.push((module.to_owned(), path.to_owned()));
        self
    }

    /// Writes the output to `dir` instead of `$OUT_DIR`.
    pub fn out_dir(&mut self, dir: impl AsRef<Path>) -> &mut Config {
        self.out_dir = Some(dir.as_ref().to_owned());
        self
    }

    /// Runs `program` (and its first arguments) as the generator.
    pub fn program<I, S>(&mut self, program: I) -> &mut Config
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
    {
        self.program = Some(program.into_iter().map(Into::into).collect());
        self
    }

    /// Generates the code of `files`, one `.rs` file each in the output
    /// directory, and tells Cargo to run the build script again when one
    /// of them changes.
    pub fn compile<P: AsRef<Path>>(&self, files: &[P]) -> Result<(), Error> {
        let out_dir = match &self.out_dir {
            Some(d) => d.clone(),
            None => PathBuf::from(env::var_os("OUT_DIR").ok_or(Error::NoOutDir)?),
        };
        let program = match &self.program {
            Some(p) => p.clone(),
            None => match env::var("RUSTORB_IDL") {
                Ok(p) => p.split_whitespace().map(OsString::from).collect(),
                Err(_) => vec![OsString::from("rustorb-idl")],
            },
        };
        let (exe, first) = program.split_first().ok_or(Error::NoProgram)?;
        let mut cmd = Command::new(exe);
        cmd.args(first).arg("-brust").arg("-C").arg(&out_dir);
        for dir in &self.includes {
            cmd.arg("-I").arg(dir);
        }
        for d in &self.defines {
            cmd.arg("-D").arg(d);
        }
        for (module, path) in &self.externs {
            cmd.arg(format!("-Wbextern={module}={path}"));
        }
        for f in files {
            cmd.arg(f.as_ref());
            println!("cargo:rerun-if-changed={}", f.as_ref().display());
        }
        println!("cargo:rerun-if-env-changed=RUSTORB_IDL");
        let status = cmd.status().map_err(Error::Run)?;
        if !status.success() {
            return Err(Error::Failed(status.code()));
        }
        Ok(())
    }
}

/// Why [`Config::compile`] failed.
#[derive(Debug)]
pub enum Error {
    /// `OUT_DIR` is not set (not in a build script) and no output
    /// directory was given.
    NoOutDir,
    /// The program to run is empty.
    NoProgram,
    /// The generator could not be started.
    Run(io::Error),
    /// The generator failed, with this exit code; it wrote why to stderr.
    Failed(Option<i32>),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NoOutDir => f.write_str("no output directory: OUT_DIR is not set"),
            Error::NoProgram => f.write_str("no rustorb-idl program given"),
            Error::Run(e) => write!(f, "cannot run rustorb-idl: {e}"),
            Error::Failed(Some(code)) => write!(f, "rustorb-idl failed with exit code {code}"),
            Error::Failed(None) => f.write_str("rustorb-idl was killed"),
        }
    }
}

impl std::error::Error for Error {}

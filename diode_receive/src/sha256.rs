//! Tiny standalone sha256sum for the release workflow.
//!
//! Usage: `diode_sha256 FILE [FILE ...]`
//!
//! Prints `<hex>  <name>` per argument, matching the `sha256sum`
//! output format, so the workflow can `tee` the result straight into a
//! `.sha256` or `SHA256SUMS` manifest without relying on any host
//! tool that the OpenBSD build image may not ship.

use std::env;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use sha2::Sha256;
use sha2::Digest;

fn hash_file(path: &Path) -> io::Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: diode_sha256 FILE [FILE ...]");
        return ExitCode::FAILURE;
    }
    let mut any_failed = false;
    for arg in &args {
        let path = PathBuf::from(arg);
        match hash_file(&path) {
            Ok(hex) => {
                // Two-space separator, like POSIX `sha256sum`.
                if let Err(e) = writeln!(io::stdout(), "{hex}  {}", path.display()) {
                    eprintln!("error: {e}");
                    any_failed = true;
                }
            }
            Err(e) => {
                eprintln!("error: {path:?}: {e}");
                any_failed = true;
            }
        }
    }
    if any_failed { ExitCode::FAILURE } else { ExitCode::SUCCESS }
}

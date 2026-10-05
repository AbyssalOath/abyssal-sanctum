//! Running system commands, and the checks and confirmations that guard
//! anything that changes a disk (ADR-0006).

use std::fs;
use std::io::{BufRead, Write};
use std::os::unix::fs::MetadataExt as _;
use std::process::{Command, Stdio};

/// The effective user is root. /proc/self belongs to the process's
/// effective user id.
pub(crate) fn require_root() -> Result<(), String> {
    match fs::metadata("/proc/self") {
        Ok(m) if m.uid() == 0 => Ok(()),
        Ok(_) => Err("this needs root (the live system logs in as root)".to_owned()),
        Err(e) => Err(format!("cannot determine the current user: {e}")),
    }
}

/// Run a command with inherited input and output; fail on a non-zero exit.
pub(crate) fn run(program: &str, args: &[&str]) -> Result<(), String> {
    let status = Command::new(program)
        .args(args)
        .status()
        .map_err(|e| format!("cannot run {program}: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{program} {} failed ({status})", args.join(" ")))
    }
}

/// Run a command and return its standard output; fail on a non-zero exit,
/// with its standard error in the message.
pub(crate) fn output(program: &str, args: &[&str]) -> Result<String, String> {
    let out = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("cannot run {program}: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(format!(
            "{program} {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

/// Standard output of a command, or an empty string if it failed.
pub(crate) fn output_or_empty(program: &str, args: &[&str]) -> String {
    output(program, args).unwrap_or_default()
}

/// Ask the user to type `expected` exactly. Used before anything that writes
/// to a disk the user has not already chosen to write to.
pub(crate) fn confirm_typed(question: &str, expected: &str) -> Result<(), String> {
    crate::term::out!("{question}\nType {expected} to continue: ");
    let _ = std::io::stdout().flush();
    let mut line = String::new();
    std::io::stdin()
        .lock()
        .read_line(&mut line)
        .map_err(|e| format!("cannot read the answer: {e}"))?;
    if line.trim() == expected {
        Ok(())
    } else {
        Err("not confirmed; nothing was changed".to_owned())
    }
}

/// SHA-256 of a file, read in chunks.
pub(crate) fn sha256_file(path: &std::path::Path) -> Result<String, String> {
    use sha2::{Digest, Sha256};
    use std::fmt::Write as _;
    use std::io::Read as _;
    let mut file =
        fs::File::open(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 1 << 20];
    loop {
        let n = file
            .read(&mut buffer)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }
    let mut hex = String::with_capacity(64);
    for b in hasher.finalize() {
        let _ = write!(hex, "{b:02x}");
    }
    Ok(hex)
}

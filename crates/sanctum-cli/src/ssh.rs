//! `sanctum ssh enable|disable|status`: remote access to the live system.
//!
//! SSH is off and firewalled by default (ADR-0006). Enabling it is an
//! explicit act with three parts: a way to log in (a root password or an
//! authorized key), a running sshd, and port 22 open in the firewall.

use crate::term::{out, outln};
use std::fs;
use std::io::Write as _;
use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
use std::path::Path;
use std::process::Command;

use crate::term;

const AUTHORIZED_KEYS: &str = "/root/.ssh/authorized_keys";
const NFT_SET: [&str; 3] = ["inet", "sanctum", "tcp_open"];

/// The effective user is root. /proc/self belongs to the process's
/// effective user id.
fn require_root() -> Result<(), String> {
    match fs::metadata("/proc/self") {
        Ok(m) if m.uid() == 0 => Ok(()),
        Ok(_) => Err("this needs root (the live system logs in as root)".to_owned()),
        Err(e) => Err(format!("cannot determine the current user: {e}")),
    }
}

fn run(program: &str, args: &[&str]) -> Result<(), String> {
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

fn output(program: &str, args: &[&str]) -> String {
    Command::new(program)
        .args(args)
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default()
}

fn sshd_active() -> bool {
    Command::new("systemctl")
        .args(["-q", "is-active", "sshd.service"])
        .status()
        .is_ok_and(|s| s.success())
}

fn port_open() -> bool {
    let set = output("nft", &["list", "set", NFT_SET[0], NFT_SET[1], NFT_SET[2]]);
    set.lines()
        .filter(|l| l.contains("elements"))
        .any(|l| l.split(|c: char| !c.is_ascii_digit()).any(|n| n == "22"))
}

/// `passwd -S root` reports P when root has a usable password.
fn root_has_password() -> bool {
    output("passwd", &["-S", "root"])
        .split_whitespace()
        .nth(1)
        .is_some_and(|s| s == "P")
}

fn valid_public_key(line: &str) -> bool {
    let mut parts = line.split_whitespace();
    let kind = parts.next().unwrap_or("");
    let body = parts.next().unwrap_or("");
    (kind.starts_with("ssh-") || kind.starts_with("ecdsa-") || kind.starts_with("sk-"))
        && body.len() > 40
}

fn add_authorized_key(key_file: &Path) -> Result<(), String> {
    let text = fs::read_to_string(key_file)
        .map_err(|e| format!("cannot read {}: {e}", key_file.display()))?;
    let keys: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect();
    if keys.is_empty() || !keys.iter().all(|k| valid_public_key(k)) {
        return Err(format!(
            "{} does not look like an OpenSSH public key file (*.pub)",
            key_file.display()
        ));
    }
    let dir = Path::new(AUTHORIZED_KEYS)
        .parent()
        .ok_or("bad authorized_keys path")?;
    fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    fs::set_permissions(dir, fs::Permissions::from_mode(0o700)).map_err(|e| e.to_string())?;
    let existing = fs::read_to_string(AUTHORIZED_KEYS).unwrap_or_default();
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(AUTHORIZED_KEYS)
        .map_err(|e| format!("cannot open {AUTHORIZED_KEYS}: {e}"))?;
    for key in keys {
        if !existing.lines().any(|l| l.trim() == key) {
            writeln!(file, "{key}").map_err(|e| e.to_string())?;
        }
    }
    fs::set_permissions(AUTHORIZED_KEYS, fs::Permissions::from_mode(0o600))
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn print_connection_details() {
    outln!(
        "\n{}",
        term::bold("Host key fingerprints (compare them when you connect):")
    );
    let mut keys: Vec<_> = fs::read_dir("/etc/ssh")
        .map(|d| {
            d.flatten()
                .map(|e| e.path())
                .filter(|p| {
                    p.file_name()
                        .and_then(|n| n.to_str())
                        .is_some_and(|n| n.starts_with("ssh_host_") && n.ends_with(".pub"))
                })
                .collect()
        })
        .unwrap_or_default();
    keys.sort();
    for key in keys {
        out!(
            "  {}",
            output("ssh-keygen", &["-lf", &key.to_string_lossy()])
        );
    }
    outln!("\n{}", term::bold("Addresses:"));
    let addrs = output("ip", &["-o", "addr", "show", "scope", "global"]);
    let mut any = false;
    for line in addrs.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if let (Some(dev), Some(addr)) = (fields.get(1), fields.get(3)) {
            let ip = addr.split('/').next().unwrap_or(addr);
            outln!("  {dev:<10} ssh root@{ip}");
            any = true;
        }
    }
    if !any {
        outln!("  none: connect to a network first (nmtui)");
    }
    outln!("\nTurn remote access off again with: sanctum ssh disable");
}

pub(crate) fn enable(key: Option<&Path>) -> Result<(), String> {
    require_root()?;
    match key {
        Some(path) => {
            add_authorized_key(path)?;
            outln!(
                "Added the key from {} to {AUTHORIZED_KEYS}.",
                path.display()
            );
        }
        None if root_has_password() => {
            outln!("Root already has a password; logins will use it.");
        }
        None => {
            outln!("Set a root password for SSH logins (or use --key FILE instead):");
            run("passwd", &["root"])?;
        }
    }
    run("systemctl", &["start", "sshd.service"])?;
    if !port_open() {
        run(
            "nft",
            &[
                "add", "element", NFT_SET[0], NFT_SET[1], NFT_SET[2], "{ 22 }",
            ],
        )?;
    }
    outln!(
        "{}",
        term::accent("SSH is on: sshd is running and port 22 is open in the firewall.")
    );
    print_connection_details();
    Ok(())
}

pub(crate) fn disable() -> Result<(), String> {
    require_root()?;
    run("systemctl", &["stop", "sshd.service"])?;
    if port_open() {
        run(
            "nft",
            &[
                "delete", "element", NFT_SET[0], NFT_SET[1], NFT_SET[2], "{ 22 }",
            ],
        )?;
    }
    outln!("SSH is off: sshd is stopped and port 22 is closed.");
    outln!(
        "{}",
        term::dim(
            "Existing sessions stay open until they log out. The root password and keys stay until reboot."
        )
    );
    Ok(())
}

pub(crate) fn status() -> Result<(), String> {
    let active = sshd_active();
    let open = port_open();
    outln!("sshd:       {}", if active { "running" } else { "stopped" });
    outln!("port 22:    {}", if open { "open" } else { "closed" });
    outln!(
        "logins:     {}",
        match (root_has_password(), Path::new(AUTHORIZED_KEYS).exists()) {
            (true, true) => "root password and authorized keys",
            (true, false) => "root password",
            (false, true) => "authorized keys",
            (false, false) => "none set (nobody can log in)",
        }
    );
    if active && open {
        print_connection_details();
    } else {
        outln!("\nTurn it on with: sanctum ssh enable");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_public_keys() {
        assert!(valid_public_key(
            "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIK0wmN/Cr3JXqmLW7u+g9pTh+wyqDHpSQEQQOtkadFS7 user@host"
        ));
        assert!(!valid_public_key("-----BEGIN OPENSSH PRIVATE KEY-----"));
        assert!(!valid_public_key("ssh-ed25519"));
    }
}

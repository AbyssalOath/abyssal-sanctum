//! `sanctum secureboot status|forget`: Secure Boot state, and removing
//! Sanctum's machine owner key (MOK) from a machine again (ADR-0011).
//!
//! A technician enrols Sanctum's certificate once per machine in MokManager
//! so that Sanctum boots with Secure Boot on. While it is enrolled, anything
//! signed with Sanctum's key boots on that machine. `forget` asks shim to
//! remove it at the next boot, which the technician confirms in MokManager.

use crate::sys;
use crate::term::{self, outln};
use std::path::{Path, PathBuf};
use std::process::Command;

/// The subject of Sanctum's signing certificate (build/secureboot.conf).
pub(crate) const CERT_CN: &str = "Abyssal Sanctum Secure Boot Signing";

/// UEFI global variables (SecureBoot, SetupMode).
const GLOBAL_GUID: &str = "8be4df61-93ca-11d2-aa0d-00e098032b8c";
/// shim's variables (MokListRT, MokNew, MokDel).
const SHIM_GUID: &str = "605dab50-e046-4300-abb6-3dd810dd8b23";

/// What the firmware and kernel say about Secure Boot.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct State {
    pub(crate) uefi: bool,
    pub(crate) secure_boot: Option<bool>,
    pub(crate) setup_mode: Option<bool>,
    /// shim started this boot (it publishes MokListRT).
    pub(crate) via_shim: bool,
    /// A key enrolment is waiting for the next boot.
    pub(crate) pending_enroll: bool,
    /// A key deletion is waiting for the next boot.
    pub(crate) pending_delete: bool,
    /// The kernel lockdown mode (none, integrity, confidentiality).
    pub(crate) lockdown: Option<String>,
}

fn efivars(root: &Path) -> PathBuf {
    root.join("sys/firmware/efi/efivars")
}

/// A one-byte boolean UEFI variable. efivarfs prefixes the value with four
/// bytes of attributes.
fn efi_flag(root: &Path, name: &str) -> Option<bool> {
    let data = std::fs::read(efivars(root).join(format!("{name}-{GLOBAL_GUID}"))).ok()?;
    data.get(4).map(|b| *b == 1)
}

fn shim_var(root: &Path, name: &str) -> bool {
    efivars(root).join(format!("{name}-{SHIM_GUID}")).exists()
}

/// The active mode in `[none] integrity confidentiality`.
fn parse_lockdown(text: &str) -> Option<String> {
    let start = text.find('[')?;
    let end = text[start..].find(']')?;
    Some(text[start + 1..start + end].to_owned())
}

pub(crate) fn read_state(root: &Path) -> State {
    let uefi = root.join("sys/firmware/efi").is_dir();
    State {
        uefi,
        secure_boot: efi_flag(root, "SecureBoot"),
        setup_mode: efi_flag(root, "SetupMode"),
        via_shim: shim_var(root, "MokListRT")
            || root
                .join("sys/firmware/efi/mok-variables/MokListRT")
                .exists(),
        pending_enroll: shim_var(root, "MokNew"),
        pending_delete: shim_var(root, "MokDel"),
        lockdown: std::fs::read_to_string(root.join("sys/kernel/security/lockdown"))
            .ok()
            .and_then(|t| parse_lockdown(&t)),
    }
}

/// One key from `mokutil --list-enrolled` or `--list-delete`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MokKey {
    /// 1-based position, as `mokutil --export` numbers its files.
    pub(crate) index: usize,
    pub(crate) sha1: String,
    pub(crate) subject: String,
}

impl MokKey {
    pub(crate) fn is_sanctum(&self) -> bool {
        self.subject
            .split(',')
            .any(|part| part.trim() == format!("CN={CERT_CN}"))
    }
}

/// Parse mokutil's key listing: `[key N]` blocks with a `SHA1 Fingerprint:`
/// line and the certificate's `Subject:` line.
pub(crate) fn parse_keys(text: &str) -> Vec<MokKey> {
    let mut keys: Vec<MokKey> = Vec::new();
    for line in text.lines().map(str::trim) {
        if let Some(rest) = line.strip_prefix("[key ") {
            let index = rest.trim_end_matches(']').parse().unwrap_or(keys.len() + 1);
            keys.push(MokKey {
                index,
                sha1: String::new(),
                subject: String::new(),
            });
        } else if let Some(fp) = line.strip_prefix("SHA1 Fingerprint:") {
            // Without [key N] headers (older mokutil), a fingerprint starts a key.
            if keys.last().is_none_or(|k| !k.sha1.is_empty()) {
                keys.push(MokKey {
                    index: keys.len() + 1,
                    sha1: String::new(),
                    subject: String::new(),
                });
            }
            if let Some(key) = keys.last_mut() {
                key.sha1 = fp.trim().to_ascii_lowercase();
            }
        } else if let Some(subject) = line.strip_prefix("Subject: ")
            && let Some(key) = keys.last_mut()
            && key.subject.is_empty()
        {
            key.subject = subject.trim().to_owned();
        }
    }
    keys.retain(|k| !k.sha1.is_empty());
    keys
}

fn mokutil(args: &[&str]) -> String {
    sys::output_or_empty("mokutil", args)
}

fn sanctum_keys(listing: &str) -> Vec<MokKey> {
    parse_keys(listing)
        .into_iter()
        .filter(MokKey::is_sanctum)
        .collect()
}

fn on_off(value: Option<bool>) -> String {
    match value {
        Some(true) => term::good("on"),
        Some(false) => term::caution("off"),
        None => term::dim("unknown"),
    }
}

pub(crate) fn status() -> Result<(), String> {
    let state = read_state(&crate::paths::sys_root());
    if !state.uefi {
        outln!("Firmware:        BIOS (legacy). Secure Boot does not apply.");
        return Ok(());
    }
    outln!("Firmware:        UEFI");
    let mut secure = on_off(state.secure_boot);
    if state.setup_mode == Some(true) {
        secure.push_str(&term::caution(" (setup mode: no platform key)"));
    }
    outln!("Secure Boot:     {secure}");
    outln!(
        "Boot chain:      {}",
        if state.via_shim {
            "shim (Sanctum's signed boot chain)"
        } else {
            "direct (no shim; an unsigned Sanctum ISO, or Secure Boot is off)"
        }
    );
    if let Some(mode) = &state.lockdown {
        outln!("Kernel lockdown: {mode}");
    }

    if !state.via_shim {
        outln!(
            "{}",
            term::dim(
                "Machine owner keys are only visible when Sanctum boots through shim; \
                 boot the signed Sanctum ISO to check or remove the Sanctum key."
            )
        );
        return Ok(());
    }
    let enrolled = sanctum_keys(&mokutil(&["--list-enrolled"]));
    let deleting = sanctum_keys(&mokutil(&["--list-delete"]));
    if enrolled.is_empty() {
        outln!(
            "Sanctum key:     {}",
            term::good("not enrolled on this machine")
        );
    } else {
        for key in &enrolled {
            outln!(
                "Sanctum key:     {} (SHA-1 {})",
                term::caution("enrolled on this machine"),
                key.sha1
            );
        }
        if deleting.is_empty() {
            outln!("                 Remove it when you are done: sanctum secureboot forget");
        }
    }
    if !deleting.is_empty() {
        outln!(
            "Pending:         {}",
            term::caution("removal requested; confirm it in MokManager at the next boot")
        );
    } else if state.pending_delete || state.pending_enroll {
        outln!("Pending:         a MokManager request waits for the next boot");
    }
    Ok(())
}

/// Export the enrolled keys and return the files holding Sanctum's.
fn export_sanctum_keys(keys: &[MokKey], dir: &Path) -> Result<Vec<PathBuf>, String> {
    let _ = std::fs::remove_dir_all(dir);
    std::fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    let status = Command::new("mokutil")
        .arg("--export")
        .current_dir(dir)
        .status()
        .map_err(|e| format!("cannot run mokutil: {e}"))?;
    if !status.success() {
        return Err(format!("mokutil --export failed ({status})"));
    }
    let mut files = Vec::new();
    for key in keys {
        let file = dir.join(format!("MOK-{:04}.der", key.index));
        // Check the exported file really is the key we mean to delete.
        let fp = sys::output(
            "openssl",
            &[
                "x509",
                "-inform",
                "DER",
                "-noout",
                "-fingerprint",
                "-sha1",
                "-in",
                &file.to_string_lossy(),
            ],
        )?;
        let fp = fp
            .rsplit('=')
            .next()
            .unwrap_or("")
            .trim()
            .to_ascii_lowercase();
        if fp != key.sha1 {
            return Err(format!(
                "{} does not match the enrolled key {} (found {fp})",
                file.display(),
                key.sha1
            ));
        }
        files.push(file);
    }
    Ok(files)
}

pub(crate) fn forget() -> Result<(), String> {
    sys::require_root()?;
    let state = read_state(&crate::paths::sys_root());
    if !state.uefi {
        return Err(
            "this machine booted in BIOS mode: there is no Secure Boot key to remove".to_owned(),
        );
    }
    if !state.via_shim {
        return Err(
            "Sanctum did not boot through shim, so the machine owner keys cannot be read. \
             Boot the signed Sanctum ISO (Secure Boot on or off) and run this again."
                .to_owned(),
        );
    }
    let enrolled = sanctum_keys(&mokutil(&["--list-enrolled"]));
    if enrolled.is_empty() {
        outln!("The Sanctum key is not enrolled on this machine. Nothing to do.");
        return Ok(());
    }
    if !sanctum_keys(&mokutil(&["--list-delete"])).is_empty() {
        outln!(
            "{}",
            term::caution(
                "A removal is already requested. Reboot with the Sanctum USB stick still \
                 plugged in and confirm it in MokManager (see below)."
            )
        );
        print_next_steps();
        return Ok(());
    }

    let dir = crate::paths::run_dir().join("mok-export");
    let files = export_sanctum_keys(&enrolled, &dir)?;
    for key in &enrolled {
        outln!("Removing: {} (SHA-1 {})", key.subject, key.sha1);
    }
    outln!(
        "mokutil now asks twice for a one-time password. You type it again in \
         MokManager at the next boot; it is not needed afterwards."
    );
    let mut args = vec!["--delete".to_owned()];
    args.extend(files.iter().map(|f| f.to_string_lossy().into_owned()));
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let result = sys::run("mokutil", &args);
    let _ = std::fs::remove_dir_all(&dir);
    result?;

    if sanctum_keys(&mokutil(&["--list-delete"])).is_empty() {
        return Err("mokutil did not record the removal request; nothing was changed".to_owned());
    }
    outln!("{}", term::good("Removal requested."));
    print_next_steps();
    Ok(())
}

fn print_next_steps() {
    outln!(
        "\nTo finish:\n\
         1. Reboot with the Sanctum USB stick still plugged in (shim on the stick does the removal).\n\
         2. A blue screen says \"Press any key to perform MOK management\". Press a key within\n   \
            10 seconds. If you miss it, the request is dropped: run `sanctum secureboot forget` again.\n\
         3. Choose \"Delete MOK\", then \"Continue\", then \"Yes\", and type the one-time password.\n\
         4. Choose \"Reboot\". The key is gone: Sanctum no longer boots with Secure Boot on\n   \
            on this machine until the key is enrolled again."
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    const LISTING: &str = "\
[key 1]
SHA1 Fingerprint: 2b:b0:10:e2:4d:94:c6:32:24:58:89:ba:aa:9e:d0:f3:d5:ef:1f:68
Certificate:
    Data:
        Issuer: C=US, ST=Massachusetts, L=Cambridge, O=Red Hat, Inc., OU=Fedora Secure Boot CA 20200709, CN=fedoraca
        Subject: C=US, ST=Massachusetts, L=Cambridge, O=Red Hat, Inc., OU=Fedora Secure Boot CA 20200709, CN=fedoraca
        Subject Public Key Info:
[key 2]
SHA1 Fingerprint: 73:58:b0:48:dc:e7:4f:a7:d0:91:c2:6e:c6:2d:61:13:b0:06:7b:d9
Certificate:
    Data:
        Issuer: CN=Abyssal Sanctum Secure Boot Signing
        Subject: CN=Abyssal Sanctum Secure Boot Signing
        Subject Public Key Info:
            X509v3 Subject Key Identifier:
";

    #[test]
    fn parses_mokutil_listing() {
        let keys = parse_keys(LISTING);
        assert_eq!(keys.len(), 2);
        assert_eq!(keys[0].index, 1);
        assert!(keys[0].subject.ends_with("CN=fedoraca"));
        assert!(!keys[0].is_sanctum());
        assert_eq!(keys[1].index, 2);
        assert_eq!(
            keys[1].sha1,
            "73:58:b0:48:dc:e7:4f:a7:d0:91:c2:6e:c6:2d:61:13:b0:06:7b:d9"
        );
        assert!(keys[1].is_sanctum());
        assert_eq!(sanctum_keys(LISTING).len(), 1);
        assert!(parse_keys("MokListRT is empty\n").is_empty());
    }

    #[test]
    fn similar_names_are_not_sanctum() {
        let key = MokKey {
            index: 1,
            sha1: "aa".to_owned(),
            subject: "CN=Abyssal Sanctum Secure Boot Signing Evil".to_owned(),
        };
        assert!(!key.is_sanctum());
    }

    #[test]
    fn cert_name_matches_build_config() {
        let conf = include_str!("../../../build/secureboot.conf");
        assert!(conf.contains(&format!("SIGNING_CERT_CN=\"{CERT_CN}\"")));
    }

    #[test]
    fn lockdown_mode() {
        assert_eq!(
            parse_lockdown("[none] integrity confidentiality\n").as_deref(),
            Some("none")
        );
        assert_eq!(
            parse_lockdown("none [integrity] confidentiality").as_deref(),
            Some("integrity")
        );
        assert_eq!(parse_lockdown(""), None);
    }

    #[test]
    fn reads_firmware_state() {
        let root = std::env::temp_dir().join(format!("sanctum-sb-{}", std::process::id()));
        let vars = efivars(&root);
        std::fs::create_dir_all(&vars).unwrap();
        std::fs::write(
            vars.join(format!("SecureBoot-{GLOBAL_GUID}")),
            [6, 0, 0, 0, 1],
        )
        .unwrap();
        std::fs::write(
            vars.join(format!("SetupMode-{GLOBAL_GUID}")),
            [6, 0, 0, 0, 0],
        )
        .unwrap();
        std::fs::write(vars.join(format!("MokListRT-{SHIM_GUID}")), [6, 0, 0, 0]).unwrap();
        std::fs::write(vars.join(format!("MokDel-{SHIM_GUID}")), [7, 0, 0, 0]).unwrap();
        let lockdown = root.join("sys/kernel/security");
        std::fs::create_dir_all(&lockdown).unwrap();
        std::fs::write(
            lockdown.join("lockdown"),
            "[none] integrity confidentiality\n",
        )
        .unwrap();

        let state = read_state(&root);
        assert_eq!(
            state,
            State {
                uefi: true,
                secure_boot: Some(true),
                setup_mode: Some(false),
                via_shim: true,
                pending_enroll: false,
                pending_delete: true,
                lockdown: Some("none".to_owned()),
            }
        );
        std::fs::remove_dir_all(&root).unwrap();
        assert_eq!(read_state(&root), State::default());
    }
}

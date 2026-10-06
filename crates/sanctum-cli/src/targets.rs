//! `sanctum targets`: find the installed systems on this machine's disks.
//!
//! Every volume is classified from lsblk. Filesystems that may hold an
//! installed system are inspected by probe-fs: through their existing mount
//! if they are mounted, otherwise through a short-lived read-only mount with
//! journal replay disabled, inside a private mount namespace that nothing
//! else can see (ADR-0010). Nothing is written to any disk.

use std::collections::BTreeMap;
use std::process::Command;

use serde::Serialize;

use crate::blockdev::{self, Volume};
use crate::disks::human_size;
use crate::paths;
use crate::sys;
use crate::term::{self, outln};

/// Filesystems probe-fs knows how to inspect read-only.
const PROBED: [&str; 9] = [
    "ntfs", "ext2", "ext3", "ext4", "xfs", "btrfs", "f2fs", "vfat", "exfat",
];

/// The label that marks Sanctum's own data partition (ADR-0010).
pub(crate) const DATA_LABEL: &str = "SANCTUM_DATA";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct Target {
    pub(crate) device: String,
    pub(crate) fstype: String,
    pub(crate) label: String,
    pub(crate) size: u64,
    /// windows, linux, data, bitlocker, luks, lvm, raid, sanctum-data,
    /// swap, unsupported, error
    pub(crate) kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) os: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) hibernated: Option<bool>,
    /// For Linux on btrfs: the subvolume holding the root, such as "/@".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) root: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) mountpoint: Option<String>,
    pub(crate) note: String,
}

/// Turn probe-fs output (key=value lines) into a map.
pub(crate) fn parse_probe(output: &str) -> BTreeMap<String, String> {
    output
        .lines()
        .filter_map(|l| l.split_once('='))
        .map(|(k, v)| (k.trim().to_owned(), v.trim().to_owned()))
        .filter(|(_, v)| !v.is_empty())
        .collect()
}

/// A readable Windows name. Windows 11 still reports "Windows 10" in
/// ProductName; build 22000 and later are Windows 11.
pub(crate) fn windows_name(probe: &BTreeMap<String, String>) -> String {
    let mut name = probe
        .get("os")
        .cloned()
        .unwrap_or_else(|| "Windows (version unknown)".to_owned());
    let build: u32 = probe.get("build").and_then(|b| b.parse().ok()).unwrap_or(0);
    if build >= 22000 {
        name = name.replace("Windows 10", "Windows 11");
    }
    if let Some(version) = probe.get("version") {
        name = format!("{name} {version}");
    }
    if build > 0 {
        name = format!("{name} (build {build})");
    }
    name
}

/// Run probe-fs on a volume: through its mountpoint, or read-only in a
/// private mount namespace.
pub(crate) fn probe(volume: &Volume) -> BTreeMap<String, String> {
    let probe_fs = paths::libexec_dir().join("probe-fs");
    let output = if let Some(mp) = volume.mountpoints.first() {
        Command::new(&probe_fs).args(["--path", mp]).output()
    } else {
        Command::new("unshare")
            .args(["--mount", "--propagation", "private"])
            .arg(&probe_fs)
            .args(["--device", &volume.path, &volume.fstype])
            .output()
    };
    match output {
        Ok(o) => parse_probe(&String::from_utf8_lossy(&o.stdout)),
        Err(e) => BTreeMap::from([("error".to_owned(), format!("cannot run probe-fs: {e}"))]),
    }
}

/// Classify one volume. `probe_fn` inspects a filesystem; tests replace it.
pub(crate) fn classify(
    v: &Volume,
    probe_fn: &dyn Fn(&Volume) -> BTreeMap<String, String>,
) -> Option<Target> {
    if v.fstype.is_empty() || blockdev::is_boot_medium(v) {
        return None;
    }
    let mut t = Target {
        device: v.path.clone(),
        fstype: v.fstype.clone(),
        label: v.label.clone(),
        size: v.size,
        kind: String::new(),
        os: None,
        hibernated: None,
        root: None,
        mountpoint: v.mountpoints.first().cloned(),
        note: String::new(),
    };
    let (kind, note) = match v.fstype.as_str() {
        "BitLocker" => (
            "bitlocker",
            format!(
                "encrypted: sanctum mount {} (asks for the password or recovery key)",
                v.path
            ),
        ),
        "crypto_LUKS" => (
            "luks",
            format!(
                "encrypted: sanctum mount {} (asks for the passphrase)",
                v.path
            ),
        ),
        "LVM2_member" => (
            "lvm",
            "LVM physical volume: activate with vgchange -ay, then run targets again".to_owned(),
        ),
        "linux_raid_member" => (
            "raid",
            "RAID member: assemble with mdadm --assemble --scan --readonly, then run targets again"
                .to_owned(),
        ),
        "swap" => ("swap", "swap (never activated by Sanctum)".to_owned()),
        _ if v.label == DATA_LABEL => ("sanctum-data", "Sanctum data partition".to_owned()),
        fs if PROBED.contains(&fs) => {
            let probe = probe_fn(v);
            if let Some(error) = probe.get("error") {
                ("error", error.clone())
            } else {
                match probe.get("kind").map(String::as_str) {
                    Some("windows") => {
                        t.os = Some(windows_name(&probe));
                        let hibernated = probe.get("hibernated").map(|h| h == "yes");
                        t.hibernated = hibernated;
                        let note = if hibernated == Some(true) {
                            "hibernated or Fast Startup: read-only only (boot Windows and shut down fully to write)"
                        } else {
                            "Windows installation"
                        };
                        ("windows", note.to_owned())
                    }
                    Some("linux") => {
                        t.os = probe.get("os").cloned();
                        t.root = probe.get("root").filter(|r| *r != "/").cloned();
                        ("linux", "Linux installation".to_owned())
                    }
                    Some("data") => ("data", "no operating system found".to_owned()),
                    _ => ("unsupported", "not inspected".to_owned()),
                }
            }
        }
        _ => ("unsupported", format!("{} is not inspected", v.fstype)),
    };
    t.kind = kind.to_owned();
    t.note = note;
    Some(t)
}

pub(crate) fn find_targets() -> Result<Vec<Target>, String> {
    sys::require_root()?;
    Ok(blockdev::volumes()?
        .iter()
        .filter_map(|v| classify(v, &probe))
        .collect())
}

pub(crate) fn run(json: bool) -> Result<(), String> {
    let targets = find_targets()?;
    if json {
        outln!(
            "{}",
            serde_json::to_string_pretty(&targets).map_err(|e| e.to_string())?
        );
        return Ok(());
    }
    if targets.is_empty() {
        outln!("No filesystems found.");
        return Ok(());
    }
    for t in &targets {
        let what = match (t.kind.as_str(), &t.os) {
            ("windows" | "linux", Some(os)) => term::accent(os),
            (kind, _) => kind.to_owned(),
        };
        outln!(
            "{}  {}  {}  {}",
            term::bold(&t.device),
            human_size(t.size),
            if t.fstype.is_empty() { "-" } else { &t.fstype },
            what
        );
        let mut details = vec![t.note.clone()];
        if let Some(root) = &t.root {
            details.push(format!("root in subvolume {root}"));
        }
        if let Some(mp) = &t.mountpoint {
            details.push(format!("mounted at {mp}"));
        }
        let line = details.join("; ");
        if t.hibernated == Some(true) {
            outln!("  {}", term::caution(&line));
        } else {
            outln!("  {}", term::dim(&line));
        }
    }
    outln!(
        "\n{}",
        term::dim("Inspected read-only; nothing was written. Mount one with: sanctum mount DEVICE")
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn volume(path: &str, fstype: &str, label: &str) -> Volume {
        Volume {
            path: path.to_owned(),
            kname: path.trim_start_matches("/dev/").to_owned(),
            kind: "part".to_owned(),
            fstype: fstype.to_owned(),
            label: label.to_owned(),
            size: 1 << 30,
            mountpoints: vec![],
            parent: String::new(),
        }
    }

    fn probe_with(text: &'static str) -> impl Fn(&Volume) -> BTreeMap<String, String> {
        move |_| parse_probe(text)
    }

    #[test]
    fn windows_hibernated_is_flagged() {
        let p = probe_with(
            "kind=windows\nos=Windows 10 Pro\nversion=23H2\nbuild=22631\nhibernated=yes\n",
        );
        let t = classify(&volume("/dev/sda3", "ntfs", "OS"), &p).expect("classified");
        assert_eq!(t.kind, "windows");
        assert_eq!(t.os.as_deref(), Some("Windows 11 Pro 23H2 (build 22631)"));
        assert_eq!(t.hibernated, Some(true));
        assert!(t.note.contains("read-only only"));
    }

    #[test]
    fn linux_on_btrfs_reports_subvolume() {
        let p = probe_with("kind=linux\nos=Fedora Linux 42\nroot=/root\n");
        let t = classify(&volume("/dev/nvme0n1p3", "btrfs", ""), &p).expect("classified");
        assert_eq!(t.kind, "linux");
        assert_eq!(t.root.as_deref(), Some("/root"));
    }

    #[test]
    fn encrypted_and_containers_are_not_probed() {
        let never = |_: &Volume| -> BTreeMap<String, String> { panic!("must not probe") };
        assert_eq!(
            classify(&volume("/dev/sda2", "BitLocker", ""), &never).map(|t| t.kind),
            Some("bitlocker".into())
        );
        assert_eq!(
            classify(&volume("/dev/sda2", "crypto_LUKS", ""), &never).map(|t| t.kind),
            Some("luks".into())
        );
        assert_eq!(
            classify(&volume("/dev/sdb1", "linux_raid_member", ""), &never).map(|t| t.kind),
            Some("raid".into())
        );
        assert_eq!(
            classify(&volume("/dev/sdc1", "ext4", DATA_LABEL), &never).map(|t| t.kind),
            Some("sanctum-data".into())
        );
        let mut boot = volume("/dev/sr0", "iso9660", "SANCTUM_0_1_0");
        boot.mountpoints = vec!["/run/archiso/bootmnt".into()];
        assert_eq!(classify(&boot, &never), None);
    }

    #[test]
    fn probe_errors_are_reported() {
        let p = probe_with("error=read-only mount failed: bad superblock\n");
        let t = classify(&volume("/dev/sdb1", "ext4", ""), &p).expect("classified");
        assert_eq!(t.kind, "error");
        assert!(t.note.contains("bad superblock"));
    }
}

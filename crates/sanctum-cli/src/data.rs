//! The Sanctum data partition (ADR-0010): an ext4 filesystem labelled
//! SANCTUM_DATA, on a second USB stick or any other disk the technician
//! chooses. It is mounted at /sanctum automatically and keeps ClamAV
//! databases, update packs and case reports across reboots. Without it,
//! Sanctum works in RAM and everything is lost at shutdown.

use std::path::{Path, PathBuf};

use crate::blockdev;
use crate::paths;
use crate::sys;
use crate::targets::DATA_LABEL;
use crate::term::{self, outln};

/// Sub-directories of the workspace.
pub(crate) const LAYOUT: [&str; 4] = ["clamav", "cases", "updates", "quarantine"];

#[derive(Debug)]
pub(crate) struct Workspace {
    pub(crate) root: PathBuf,
    /// On the data partition (survives reboot), not in RAM.
    pub(crate) persistent: bool,
}

impl Workspace {
    pub(crate) fn dir(&self, name: &str) -> Result<PathBuf, String> {
        let dir = self.root.join(name);
        std::fs::create_dir_all(&dir)
            .map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
        Ok(dir)
    }
}

fn is_mountpoint(path: &Path) -> bool {
    !sys::output_or_empty(
        "findmnt",
        &[
            "--noheadings",
            "--output",
            "TARGET",
            "--mountpoint",
            &path.to_string_lossy(),
        ],
    )
    .trim()
    .is_empty()
}

/// The data partition if it is mounted, otherwise a workspace in RAM.
pub(crate) fn workspace() -> Workspace {
    let data = paths::data_mount();
    if is_mountpoint(&data) {
        Workspace {
            root: data,
            persistent: true,
        }
    } else {
        Workspace {
            root: paths::ram_workspace(),
            persistent: false,
        }
    }
}

/// Print a warning when results will not survive a reboot.
pub(crate) fn warn_if_volatile(ws: &Workspace) {
    if !ws.persistent {
        outln!(
            "{}",
            term::caution(&format!(
                "No data partition: working in RAM ({}). Everything there is lost at reboot. \
                 See `sanctum docs getting-started/data-partition`.",
                ws.root.display()
            ))
        );
    }
}

pub(crate) fn status() -> Result<(), String> {
    let ws = workspace();
    if ws.persistent {
        let source = sys::output_or_empty(
            "findmnt",
            &[
                "--noheadings",
                "--output",
                "SOURCE",
                &ws.root.to_string_lossy(),
            ],
        );
        let df = sys::output_or_empty(
            "df",
            &[
                "--human-readable",
                "--output=size,avail",
                &ws.root.to_string_lossy(),
            ],
        );
        let space = df
            .lines()
            .nth(1)
            .unwrap_or("")
            .split_whitespace()
            .collect::<Vec<_>>();
        outln!(
            "Data partition: {} at {}",
            term::good(source.trim()),
            ws.root.display()
        );
        if let [size, avail] = space.as_slice() {
            outln!("Space:          {avail} free of {size}");
        }
    } else {
        warn_if_volatile(&ws);
    }
    for name in LAYOUT {
        let dir = ws.root.join(name);
        let entries = std::fs::read_dir(&dir).map(Iterator::count).unwrap_or(0);
        outln!("  {:<12} {} item(s)", format!("{name}/"), entries);
    }
    Ok(())
}

/// Describe the filesystems on a volume and its partitions, e.g. "sda2 ntfs WINDOWS".
fn existing_filesystems(all: &[blockdev::Volume], volume: &blockdev::Volume) -> Vec<String> {
    all.iter()
        .filter(|v| v.path == volume.path || v.parent == volume.kname)
        .filter(|v| !v.fstype.is_empty())
        .map(|v| {
            format!("{} {} {}", v.kname, v.fstype, v.label)
                .trim_end()
                .to_owned()
        })
        .collect()
}

/// Wait for the kernel and udev to show the first partition of a new GPT.
fn wait_for_partition(disk: &str) -> Result<String, String> {
    for _ in 0..20 {
        sys::run("udevadm", &["settle"])?;
        if let Some(part) = blockdev::volumes()?
            .into_iter()
            .find(|v| v.parent == disk && v.kind == "part")
        {
            return Ok(part.path);
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    Err(format!("the new partition on {disk} did not appear"))
}

/// Format a disk or partition as the Sanctum data partition.
pub(crate) fn init(device: &str, yes: bool) -> Result<(), String> {
    sys::require_root()?;
    let volume = blockdev::find(device)?;
    let all = blockdev::volumes()?;
    let in_use = all.iter().any(|v| {
        (v.path == volume.path || v.parent == volume.kname)
            && (!v.mountpoints.is_empty() || blockdev::is_boot_medium(v))
    });
    if in_use {
        return Err(format!(
            "{device} (or a partition on it) is mounted or holds the Sanctum boot medium"
        ));
    }
    if !matches!(volume.kind.as_str(), "disk" | "part") {
        return Err(format!(
            "{device} is a {}; choose a disk or a partition",
            volume.kind
        ));
    }
    let existing = existing_filesystems(&all, &volume);
    if yes && !existing.is_empty() {
        // `--yes` is for scripts on blank media; erasing data needs a human.
        return Err(format!(
            "{device} holds data ({}); run without --yes and type the device name to erase it",
            existing.join(", ")
        ));
    }
    if !yes {
        if !existing.is_empty() {
            outln!("{device} currently holds: {}", existing.join(", "));
        }
        sys::confirm_typed(
            &format!(
                "{device} will be ERASED and formatted as the Sanctum data partition \
                 (ext4, label {DATA_LABEL}). Everything on it is lost."
            ),
            device,
        )?;
    }
    let partition = if volume.kind == "disk" {
        // A whole disk gets a GPT with one Linux partition.
        sys::run("sgdisk", &["--zap-all", device])?;
        sys::run(
            "sgdisk",
            &[
                "--new=1:0:0",
                "--typecode=1:8300",
                &format!("--change-name=1:{DATA_LABEL}"),
                device,
            ],
        )?;
        wait_for_partition(&volume.kname)?
    } else {
        volume.path.clone()
    };
    sys::run("mkfs.ext4", &["-F", "-q", "-L", DATA_LABEL, &partition])?;
    sys::run("udevadm", &["settle"])?;
    outln!("{partition} is now the Sanctum data partition.");
    let ws = workspace();
    if ws.persistent {
        for name in LAYOUT {
            ws.dir(name)?;
        }
        outln!("Mounted at {}.", ws.root.display());
    } else {
        outln!(
            "It is mounted at {} automatically in a moment (if not, re-plug it).",
            paths::data_mount().display()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vol(kname: &str, kind: &str, fstype: &str, label: &str, parent: &str) -> blockdev::Volume {
        blockdev::Volume {
            path: format!("/dev/{kname}"),
            kname: kname.to_owned(),
            kind: kind.to_owned(),
            fstype: fstype.to_owned(),
            label: label.to_owned(),
            parent: parent.to_owned(),
            ..Default::default()
        }
    }

    #[test]
    fn existing_filesystems_lists_disk_and_partitions() {
        let all = vec![
            vol("sda", "disk", "", "", ""),
            vol("sda1", "part", "vfat", "", "sda"),
            vol("sda2", "part", "ntfs", "Windows", "sda"),
            vol("sda3", "part", "", "", "sda"),
            vol("sdb", "disk", "", "", ""),
        ];
        assert_eq!(
            existing_filesystems(&all, &all[0]),
            vec!["sda1 vfat", "sda2 ntfs Windows"]
        );
        assert!(existing_filesystems(&all, &all[4]).is_empty());
        let whole = vec![vol("vda", "disk", "ntfs", "WINTEST", "")];
        assert_eq!(
            existing_filesystems(&whole, &whole[0]),
            vec!["vda ntfs WINTEST"]
        );
    }
}

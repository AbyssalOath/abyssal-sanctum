//! `sanctum mount` and `sanctum umount`: mount a target's filesystem
//! safely (ADR-0006, ADR-0010).
//!
//! - Read-only by default, with journal replay disabled, and the block
//!   device itself set read-only in the kernel.
//! - Read-write only on request, after typing the device name, and never
//!   for a hibernated Windows volume (Fast Startup), which would lose data.
//! - BitLocker and LUKS volumes are unlocked first (cryptsetup asks for the
//!   key), read-only unless read-write was requested.
//! - NTFS is mounted with NTFS-3G, with alternate data streams visible as
//!   extended attributes, so `sanctum scan` can check them.

use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::blockdev::{self, Volume};
use crate::paths;
use crate::sys;
use crate::targets;
use crate::term::{self, outln};

/// Device-mapper names created by Sanctum start with this, so `umount`
/// knows which ones to close.
const MAPPER_PREFIX: &str = "sanctum-";

/// The mount command for a filesystem type.
pub(crate) fn mount_command(fstype: &str, rw: bool) -> Result<(String, String), String> {
    let mode = if rw { "rw" } else { "ro" };
    let (program, options) = match (fstype, rw) {
        ("ntfs", _) => ("ntfs-3g", format!("{mode},streams_interface=xattr")),
        ("ext2" | "ext3" | "ext4", false) => ("mount", "ro,noload".to_owned()),
        ("xfs", false) => ("mount", "ro,norecovery".to_owned()),
        ("btrfs", false) => ("mount", "ro,rescue=nologreplay".to_owned()),
        ("f2fs", false) => ("mount", "ro,norecovery".to_owned()),
        ("ext2" | "ext3" | "ext4" | "xfs" | "btrfs" | "f2fs" | "vfat" | "exfat", _) => {
            ("mount", mode.to_owned())
        }
        (other, _) => return Err(format!("Sanctum does not mount {other} filesystems")),
    };
    Ok((program.to_owned(), options))
}

/// Marker for a device Sanctum set read-only, under the runtime directory.
fn setro_marker(device: &str) -> PathBuf {
    paths::run_dir()
        .join("setro")
        .join(device.trim_start_matches('/').replace('/', "_"))
}

fn is_read_only(device: &str) -> bool {
    sys::output_or_empty("blockdev", &["--getro", device]).trim() == "1"
}

fn mapper_name(volume: &Volume) -> String {
    format!("{MAPPER_PREFIX}{}", volume.kname)
}

/// Unlock an encrypted volume (or reuse an earlier unlock) and return the
/// unlocked device.
fn unlock(volume: &Volume, rw: bool) -> Result<Volume, String> {
    let name = mapper_name(volume);
    let mapped = format!("/dev/mapper/{name}");
    if !Path::new(&mapped).exists() {
        let kind = if volume.fstype == "BitLocker" {
            "bitlk"
        } else {
            "luks"
        };
        outln!(
            "Unlocking {} ({}){}",
            volume.path,
            volume.fstype,
            if rw { "" } else { ", read-only" }
        );
        let mut args = vec!["open", "--type", kind];
        if !rw {
            args.push("--readonly");
        }
        args.extend([volume.path.as_str(), name.as_str()]);
        sys::run("cryptsetup", &args)?;
    }
    blockdev::find(&mapped)
}

/// Is this NTFS volume hibernated (Windows Fast Startup)?
fn ntfs_hibernated(volume: &Volume) -> bool {
    targets::probe(volume)
        .get("hibernated")
        .is_some_and(|h| h == "yes")
}

pub(crate) fn mount(device: &str, rw: bool, at: Option<&Path>, yes: bool) -> Result<(), String> {
    sys::require_root()?;
    let original = blockdev::find(device)?;
    if blockdev::is_boot_medium(&original) {
        return Err(format!("{device} is the Sanctum boot medium"));
    }
    if original.label == targets::DATA_LABEL {
        return Err(format!(
            "{device} is the Sanctum data partition; it is mounted at {} automatically",
            paths::data_mount().display()
        ));
    }
    let volume = match original.fstype.as_str() {
        "BitLocker" | "crypto_LUKS" => unlock(&original, rw)?,
        "LVM2_member" => return Err("an LVM physical volume: activate it with vgchange -ay, then mount the logical volume (sanctum targets lists them)".to_owned()),
        "linux_raid_member" => return Err("a RAID member: assemble it with mdadm --assemble --scan --readonly, then mount the array".to_owned()),
        "swap" => return Err("a swap partition has no files to mount".to_owned()),
        "" => return Err(format!("{device} has no filesystem that lsblk recognises")),
        _ => original.clone(),
    };
    if let Some(mp) = volume.mountpoints.first() {
        return Err(format!("{} is already mounted at {mp}", volume.path));
    }
    let (program, options) = mount_command(&volume.fstype, rw)?;

    if rw {
        if volume.fstype == "ntfs" && ntfs_hibernated(&volume) {
            return Err(format!(
                "{} holds a hibernated Windows (or one shut down with Fast Startup). \
                 Writing to it would lose data. Mount it read-only, or boot Windows and \
                 shut it down fully (Shift + Shut down) first",
                original.path
            ));
        }
        if !yes {
            sys::confirm_typed(
                &format!(
                    "{} will be mounted read-write: anything done there changes the disk.",
                    volume.path
                ),
                &original.path,
            )?;
        }
        sys::run("blockdev", &["--setrw", &volume.path])?;
    } else if !is_read_only(&volume.path) {
        // Belt and braces: the kernel refuses writes to the device too.
        // Remember that Sanctum did this, so umount can undo it.
        sys::run("blockdev", &["--setro", &volume.path])?;
        let marker = setro_marker(&volume.path);
        if let Some(dir) = marker.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(&marker, b"");
    }

    let target = at.map_or_else(
        || paths::mount_base().join(&original.kname),
        Path::to_path_buf,
    );
    std::fs::create_dir_all(&target)
        .map_err(|e| format!("cannot create {}: {e}", target.display()))?;
    let target_str = target.to_string_lossy().into_owned();
    let result = if program == "mount" {
        sys::run(
            "mount",
            &[
                "-t",
                &volume.fstype,
                "-o",
                &options,
                "--",
                &volume.path,
                &target_str,
            ],
        )
    } else {
        // ntfs-3g does not accept "--"; device paths start with /dev/.
        sys::run(&program, &["-o", &options, &volume.path, &target_str])
    };
    if let Err(e) = result {
        let _ = std::fs::remove_dir(&target);
        return Err(e);
    }
    outln!(
        "Mounted {} ({}) at {} {}",
        volume.path,
        volume.fstype,
        target_str,
        if rw {
            term::caution("read-write")
        } else {
            term::good("read-only")
        }
    );
    outln!(
        "{}",
        term::dim("Unmount with: sanctum umount (or sanctum umount --all)")
    );
    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct SanctumMount {
    pub(crate) target: PathBuf,
    pub(crate) source: String,
}

/// Mounts under the Sanctum mount base, deepest first.
pub(crate) fn sanctum_mounts() -> Result<Vec<SanctumMount>, String> {
    let json = sys::output(
        "findmnt",
        &["--json", "--list", "--output", "TARGET,SOURCE"],
    )?;
    let tree: Value = serde_json::from_str(&json).map_err(|e| e.to_string())?;
    let base = paths::mount_base();
    let mut mounts: Vec<SanctumMount> = tree
        .get("filesystems")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|f| {
            let target = PathBuf::from(f.get("target")?.as_str()?);
            let source = f.get("source")?.as_str()?.to_owned();
            target
                .starts_with(&base)
                .then_some(SanctumMount { target, source })
        })
        .collect();
    mounts.sort_by_key(|m| std::cmp::Reverse(m.target.components().count()));
    Ok(mounts)
}

pub(crate) fn umount(what: Option<&str>, all: bool) -> Result<(), String> {
    sys::require_root()?;
    let mounts = sanctum_mounts()?;
    let chosen: Vec<&SanctumMount> = match (what, all) {
        (_, true) => mounts.iter().collect(),
        (Some(w), false) => {
            let real = std::fs::canonicalize(w)
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_else(|_| w.to_owned());
            mounts
                .iter()
                .filter(|m| m.target.to_string_lossy() == real || m.source == real || m.source == w)
                .collect()
        }
        (None, false) => match mounts.as_slice() {
            [only] => vec![only],
            [] => return Err("nothing is mounted by Sanctum".to_owned()),
            _ => return Err("several filesystems are mounted; name one, or use --all".to_owned()),
        },
    };
    if chosen.is_empty() {
        return Err(format!("{} is not mounted by Sanctum", what.unwrap_or("")));
    }
    for m in chosen {
        sys::run("umount", &[&m.target.to_string_lossy()])?;
        let _ = std::fs::remove_dir(&m.target);
        let marker = setro_marker(&m.source);
        if marker.exists() {
            sys::run("blockdev", &["--setrw", &m.source])?;
            let _ = std::fs::remove_file(marker);
        }
        if let Some(name) = m
            .source
            .strip_prefix("/dev/mapper/")
            .filter(|n| n.starts_with(MAPPER_PREFIX))
        {
            sys::run("cryptsetup", &["close", name])?;
        }
        outln!("Unmounted {} ({})", m.target.display(), m.source);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_only_mounts_disable_journal_replay() {
        assert_eq!(mount_command("ext4", false).expect("ok").1, "ro,noload");
        assert_eq!(mount_command("xfs", false).expect("ok").1, "ro,norecovery");
        assert_eq!(
            mount_command("btrfs", false).expect("ok").1,
            "ro,rescue=nologreplay"
        );
        assert_eq!(mount_command("ext4", true).expect("ok").1, "rw");
    }

    #[test]
    fn ntfs_uses_ntfs3g_with_streams_as_xattrs() {
        let (program, options) = mount_command("ntfs", false).expect("ok");
        assert_eq!(program, "ntfs-3g");
        assert_eq!(options, "ro,streams_interface=xattr");
    }

    #[test]
    fn unknown_filesystems_are_refused() {
        assert!(mount_command("zfs_member", false).is_err());
    }
}

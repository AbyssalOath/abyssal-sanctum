# ADR-0010: Inspecting targets, mounting them, and the data partition

- **Status:** Accepted
- **Date:** 2026-10-05

## Context

The antimalware track (AV-1) needs Sanctum to find the installed systems on
a machine, mount them for scanning, and keep databases and reports across
reboots. ADR-0006 says Sanctum never mounts or writes to a disk by itself.
This record sets out how the new commands keep to that, and the one
deliberate exception.

## Decision

### `sanctum targets` inspects through private read-only mounts

Recognising an installation (a Windows folder, an `os-release` file) and
hibernation (the first bytes of `hiberfil.sys`) needs a look inside the
filesystem. `probe-fs` does this:

- in a **private mount namespace** (`unshare --mount --propagation
  private`), so the mount is never visible to the rest of the system and
  disappears when the probe exits;
- **read-only with journal replay disabled**: `ro,noload` (ext2/3/4),
  `ro,norecovery` (XFS, F2FS), `ro,rescue=nologreplay` (Btrfs), NTFS-3G
  read-only;
- treating every file it reads as hostile: `os-release` is parsed as text,
  never sourced.

Encrypted volumes, LVM and RAID members are reported but never opened by
`targets`. The scan boot test checks that the test images are byte-for-byte
unchanged afterwards.

### `sanctum mount` is read-only unless asked, and refuses what loses data

- Read-only by default, with the same no-replay options, and the block
  device set read-only in the kernel (`blockdev --setro`). `sanctum umount`
  removes that flag again if Sanctum set it.
- Read-write needs `--rw` and typing the device name (or `--yes` in
  scripts).
- Read-write is **refused for a hibernated Windows volume** (hibernation or
  Fast Startup): writing to it loses the hibernated session's data.
- BitLocker and LUKS are unlocked with cryptsetup, `--readonly` unless
  read-write was asked for, under device-mapper names starting with
  `sanctum-`, which `umount` closes again.
- NTFS uses NTFS-3G with `streams_interface=xattr`, so alternate data
  streams can be scanned.
- Mounts live under `/mnt/sanctum/<device>`.

### The data partition is the one automatic mount

An **ext4 filesystem labelled `SANCTUM_DATA`** is mounted at `/sanctum` when
it appears, through a udev rule and `sanctum.mount`:

- options `rw,nosuid,nodev,noexec,noatime`: nothing on it can run;
- ext4 only: journaled, so databases and (from AV-2) the quarantine
  survive a pulled stick better than on FAT or exFAT;
- `sanctum.nodata` on the kernel command line turns it off;
- `sanctum data init` creates one, after the device name is typed.

The self-test accepts a mount at `/sanctum` only when its source carries the
label and the mount has `noexec` and `nosuid`.

Without a data partition, Sanctum works in RAM (`/root/sanctum`) and warns
that results are lost at reboot.

## Consequences

- A disk labelled `SANCTUM_DATA` is mounted read-write automatically. A
  machine under investigation could carry such a disk, by accident or on
  purpose. `sanctum.nodata` is the escape hatch, and the forensic boot mode
  (Phase 5) will turn the automatic mount off.
- `targets` reads every filesystem it can. That reading is what gives
  ADR-0006 meaning: it is done in a way that cannot write.
- Writing to a target is always a separate, explicit step.

# Abyssal Sanctum -- Gap Analysis

Snapshot date: 2026-10-02. This document compares the project brief with the
current repository, the build host, the Arch ecosystem and the sibling Abyssal
projects. It lists what is missing and which decisions have to be made before
building. The phased plan built from it is in [ROADMAP.md](../../ROADMAP.md).

## 1. Current state

| Area | State |
| --- | --- |
| Repository | `cargo init` output only: `Cargo.toml` (edition 2024, no deps), `src/main.rs` (hello world), `.gitignore` (`/target`). No commits. |
| Branch | `master`. Every sibling repo uses `main`, and so does the brief's PR target. |
| Build host | Fedora 42. Docker and podman are installed. `pacman`, `mkarchiso`, `xorriso`, `qemu-system-x86_64`, OVMF, `shellcheck` and `shfmt` are not. |
| Rust | rustc 1.97.1 is installed. Warden pins MSRV 1.93. |
| GitHub org | Siblings live under `AbyssalOath` (Warden, Arsenal). |
| License | Warden is AGPL-3.0-only and Arsenal is AGPL-3.0-or-later. Sanctum has no license yet. |

Nothing in the brief has been implemented yet. Every item below is a gap. This
document is about the gaps that are **not obvious from the brief**: technical
problems, hidden conflicts and decisions the brief leaves open.

## 2. Build-system gaps

### 2.1 archiso cannot run natively on the dev host

`mkarchiso` needs an Arch userland (pacman, pacstrap) and root. On Fedora it
has to run inside an `archlinux` container:

- It needs to be **rootful** (`sudo podman` or docker) with `--privileged`.
  Rootless podman fails in pacstrap's mount and mknod steps.
- Fedora's SELinux requires `:Z` or `--security-opt label=disable` on bind
  mounts.
- The same container image works on GitHub's Ubuntu runners
  (`docker run --privileged`). One builder works in both places, which fits
  the brief's rule that the build must not depend on the developer's machine.

**Decision:** `scripts/build/build-iso.sh` is a thin host wrapper. It starts a
pinned container, and the real work runs inside it in
`scripts/build/in-container.sh`. Running the inner script directly on an Arch
host is supported too.

### 2.2 "Reproducible" on a rolling distro

The brief's top priority is reproducible builds, but Arch is rolling. If the
inputs are not pinned, building the same commit two days apart gives different
ISOs. What is needed:

1. **Pin the package set** to an Arch Linux Archive snapshot
   (`https://archive.archlinux.org/repos/YYYY/MM/DD/$repo/os/$arch`) in the
   profile's `pacman.conf`. "Updating the base" then means bumping one date in
   a reviewed commit.
2. **Pin the container image** by digest, not `archlinux:latest`.
3. **Pin archiso itself.** Install the archiso version from the same snapshot,
   not whatever is current. archiso is at v91. Boot-mode names and
   `profiledef.sh` keys have changed between releases, so the profile has to
   match the archiso version it was copied from.
4. Export `SOURCE_DATE_EPOCH` from the commit timestamp. mkarchiso uses it for
   file times, the ISO UUID and xorriso dates.
5. Pin the Rust toolchain with `rust-toolchain.toml`, and use `--locked`
   builds.

Caveats to state honestly in the docs:

- The archive server is rate-limited and not meant for heavy CI use.
  Downloaded packages have to be cached between CI runs (the cache key is the
  snapshot date).
- An old snapshot with a new keyring is fine. A new snapshot with an old
  container keyring fails. The build should update `archlinux-keyring` before
  anything else.
- Bit-for-bit reproducibility is a later, verifiable goal (Phase 8: two
  independent builds plus `diffoscope`). v0.1 promises **pinned and
  repeatable** builds, not bit-identical ones.

### 2.3 Live-system pacman and partial upgrades

The build `pacman.conf` and the one inside the live system are separate files.
If the live system points at current mirrors while its installed packages come
from a snapshot, `pacman -S foo` becomes a partial upgrade and can break
libraries. **Decision:** point the live `/etc/pacman.conf` at the same snapshot
date. Document how to run `pacman -Syu` against current mirrors (it needs a lot
of RAM).

### 2.4 Profile base: copy `releng`, then remove

`baseline` is too bare: it has no networking setup, no accessibility and no
memtest entries. `releng` is the official install ISO profile and is proven on
real hardware. **Decision:** copy `releng` from the pinned archiso version into
the repo and record the upstream version in a header comment, so later archiso
updates can be diffed. Things that have to be **removed or changed** from
releng to meet the brief's security rules:

| releng default | Problem | Action |
| --- | --- | --- |
| `sshd.service` enabled | Brief: SSH off unless explicitly enabled | Do not enable. Provide `sanctum ssh enable`. |
| `cloud-init` and its units | On an incident-response system, an attached datasource (a disk labelled `cidata` or `config-2`) could configure the live system | Remove the package |
| `reflector` / `choose-mirror` | Network traffic on boot. Not needed with a pinned snapshot. | Remove |
| Guest agents (qemu-guest-agent, hyperv, open-vm-tools) | Exposed control channels | Keep only what boot needs. Document. |
| No firewall | Brief: conservative firewall | `nftables` with default-deny inbound, enabled at boot |
| `iso_name`/`iso_label`/`install_dir` = arch | Branding and boot identity | `iso_name=abyssal-sanctum`, `iso_version=v0.1.0`. mkarchiso then produces `abyssal-sanctum-v0.1.0-x86_64.iso`, the exact name the brief asks for. Keep `install_dir` short and lowercase. |

### 2.5 Package list structure and rationale

The brief requires a documented reason for every package. mkarchiso reads one
`packages.x86_64` and ignores `#` comments. **Decision:** keep package lists
per category in `build/packages/<category>.list`, with a `pkg  # reason`
comment on every line. The build combines them into `packages.x86_64`. CI
fails if any of these is true:

- a line has no reason,
- a package is listed twice,
- a package does not exist in the pinned snapshot (`pacman -Sp` in the
  container),
- a package comes from anywhere other than `core` or `extra`.

### 2.6 ISO size

GitHub release assets are limited to 2 GiB each. Rough compressed sizes from
the live repos:

| Component | Size |
| --- | --- |
| `linux` | 161 MB |
| `linux-lts` | 155 MB (second kernel, optional) |
| Firmware (`linux-firmware` is now split; `-intel` alone is 133 MB) | 400+ MB total |
| `firefox` | 85 MB |
| `clamav` binaries | 29 MB |
| ClamAV signature databases | about 250-400 MB uncompressed |
| `wireshark-cli` + `wireshark-qt` | 30 MB |

A GUI ISO with firmware, a browser and ClamAV databases can reach 1.5-2 GB.

**Decided (2026-10-02):** the ISO is hosted on SourceForge or the project
site, and the GitHub Release holds the checksum, attestation and a download
link, so GitHub's limit does not apply
([ADR-0005](../architecture/decisions/0005-iso-hosting-and-size-budget.md)).
The build warns at 2.5 GiB and fails at 3 GiB, and CI shows the size in each
run so growth is visible.

## 3. Boot and platform gaps

### 3.1 Secure Boot (the biggest gap the brief does not mention)

The Arch ISO is not signed for Secure Boot, and `shim-signed` is not in the
official repos. On most current PCs, a technician would have to **turn Secure
Boot off** to boot Sanctum. On Windows machines with BitLocker this has a real
cost: changing Secure Boot state changes PCR7, so Windows asks for the
**BitLocker recovery key** on its next boot. The technician might not have that
key.

Options:

1. **v0.1:** document clearly that Sanctum does not support Secure Boot, and
   put the BitLocker warning in the boot docs and the README.
2. Later: a self-signed chain with `sbctl`/`mokutil` (both official) using a
   Microsoft-signed shim from another distro, with the user enrolling a MOK.
   This works, but it is complex and the user has to take part.
3. Long term: getting Sanctum's own shim through Microsoft's shim review. That
   is heavy and needs a stable organisation and key custody.

Classed as **research (Phase 9)**. The limitation has to be documented from
v0.1.

### 3.2 BIOS, UEFI, USB and Ventoy

releng already produces a hybrid ISO that boots on BIOS (syslinux) and UEFI
(systemd-boot), and that boots from USB after `dd`. Additions:

- memtest86+ and memtest86+-efi boot entries (both official). This covers the
  brief's RAM diagnostics.
- Boot entries: default, `nomodeset` (safe graphics), `copytoram`,
  accessibility (speech), serial console, and later forensic mode (section
  6.2).
- **Ventoy** is part of the test matrix. Many technicians carry it. archiso
  usually works with it, but releases need to confirm that.
- **No Plymouth or quiet boot.** Boot messages help with diagnosis, and hiding
  them works against what the system is for.

### 3.3 Kernel choice

`linux` (7.2) supports the newest hardware. `linux-lts` (6.18) is more stable.
For a recovery system, newer hardware support usually matters more: a new
laptop's NVMe or Wi-Fi might not work on an older kernel.

**Decided (2026-10-02):** `linux-hardened`, which follows mainline stable
(7.2.8.hardened1 in the pinned snapshot) and adds hardening. The trade-offs to
check on real hardware are in
[ADR-0004](../architecture/decisions/0004-linux-hardened-kernel.md).

## 4. Safety gaps: things that write to disks without being asked

The brief says "no automatic disk modification". A standard live system breaks
that in several quiet ways, and the brief does not list them.

| Mechanism | What it writes | Fix |
| --- | --- | --- |
| **mdadm auto-assembly** (udev) | Assembling a degraded or dirty array can **start a resync**, which writes to member disks | Mask the udev assembly rule. Assemble only when the user asks (ADR-0006). |
| **LVM event auto-activation** | Usually no metadata writes, but it exposes LVs that GUI tools might then mount | An empty `auto_activation_volume_list` (ADR-0006) |
| **Journal replay on "read-only" mounts** | ext3/ext4 and XFS **replay the journal** on a plain `ro` mount, which writes to the device | The mount helper uses `ro,noload` (ext4) or `ro,norecovery` (XFS), plus `blockdev --setro` |
| **NTFS with hibernation / Fast Startup** | A read-write mount of a hibernated Windows volume loses data, and it destroys `hiberfil.sys` (memory evidence) | Mount NTFS read-only by default. Detect hibernation and warn. |
| **Desktop automount** (udisks2 + thunar-volman / gvfs) | Mounts read-write as soon as a disk appears | Do not install thunar-volman. Turn off udisks automount. Test it in CI. |
| **Swap auto-activation** | Using a swap partition on the target overwrites evidence | Empty fstab. Add `systemd.gpt_auto=0` to the kernel command line. |
| **`fsck` at mount** | None in a live system, since there is no fstab, but GUI tools can offer to repair | Document. Never run repair from a wrapper without confirmation. |

These are **Phase 1 defaults**, not later work. They decide whether Sanctum
can honestly call itself non-destructive.

There is one limit Sanctum cannot remove: third-party tools such as `gparted`,
`fdisk` and `mkfs` are destructive by design and cannot be wrapped. The policy:

- Sanctum's own commands always ask for confirmation, by typing the target
  device name.
- The tool catalog marks each tool as `read-only`, `modifies` or `destructive`.
- The menus show that marking next to the tool.

## 5. Security toolkit gaps

### 5.1 ClamAV is useless offline without databases

The `clamav` package contains no signatures. Offline scanning needs databases
on the ISO. Problems:

- **Size:** about 250-400 MB (see section 2.6).
- **Freshness:** the databases change every day. A release ISO is out of date
  within a week.
- **CI fetching:** `database.clamav.net` rate-limits heavily and blocks some
  cloud IP ranges. Cisco's guidance for this case is to run a `cvdupdate`
  mirror.
- **Reproducibility:** the databases have to count as a recorded input (with
  their hashes in the build manifest). Otherwise no two builds match.
- **Redistribution terms:** the right to bundle the official CVDs in a
  redistributed ISO **must be confirmed before shipping**. Many rescue
  distributions avoid bundling them.

**Recommendation:**

- v0.1 ships ClamAV with `freshclam` ready to use for online sessions.
- The databases are bundled only if the redistribution terms allow it, and
  they are fetched through a cached `cvdupdate` step at release time.
- Phase 8 adds "load signature databases from a USB data partition" for
  air-gapped use.

### 5.2 YARA with no rules

`yara` 4.5.8 is official, but it has no rules. Public rule sets have different
licenses (for example, signature-base uses DRL 1.1). **Recommendation:**

- v0.1 ships the binary plus a guide on bringing your own rules.
- Curated rules arrive through **Warden's signed content bundles** (Phase 6).
  They are minisign-verified and include rollback and expiry protection, so
  that work does not have to be redone here.

### 5.3 Rootkit detection runs on the wrong system by default

`rkhunter`, `unhide` and `lynis` check the **running** system. In Sanctum the
running system is the clean live environment, so their results there tell you
nothing. Offline rootkit investigation means looking at the **mounted target**.

- `chkrootkit` (which has `-r ROOTDIR`) is **not in the official repos**.
- rkhunter's support for a separate root directory is limited.
- Warden's `system-check --root <mounted image>` is built for exactly this
  case.

**Recommendation:** in v0.1, document a manual offline method using rkhunter's
file-property checks and hash comparison against the target's package
database. Make Warden the offline rootkit tool from Phase 6.

### 5.4 Memory forensics is mostly out of reach

Booting Sanctum on the suspect machine **erases that machine's RAM**. Memory
has to be captured from the running suspect OS (LiME or AVML on Linux,
winpmem on Windows) before rebooting, and that is outside Sanctum. What is
realistic:

- Analyse captured images with `volatility3` (2.28, official).
- Treat `hiberfil.sys` and `pagefile.sys` as memory evidence. This is another
  reason NTFS has to be read-only by default.
- Document the "capture before reboot" rule at the start of the forensic
  workflow guide.

### 5.5 Tools that are not in the official repos

Checked against archlinux.org on 2026-10-02:

| Not official | Use or replacement |
| --- | --- |
| `chkrootkit` | Warden `system-check --root` (Phase 6). rkhunter in the meantime. |
| `dc3dd`, `dcfldd` | `ddrescue` (official) plus `sha256sum` through `tee`. Hashing during acquisition can come from `sanctum image` (Phase 5). |
| `hashdeep` | `sha256sum`/`b3sum` + `ssdeep` (official). Recursive hash manifests from `sanctum hash` (Phase 5, Rust). |
| `nwipe` | Not needed. Wiping disks is out of scope for a recovery tool. |
| `dislocker` | `cryptsetup` 2.8 opens BitLocker (BITLK) natively. |
| `ms-sys` | For Windows MBR repair, document the limit. A UEFI system needs `efibootmgr` plus copying files from `Windows\Boot\EFI`. |
| `plaso`, `bulk_extractor`, `afflib`, `scalpel`, `ext4magic`, `extundelete`, `regripper` | Deferred. Each needs its own written case for an exception (see section 8.2). |

Confirmed **official** and suitable for v0.1: `smartmontools`, `nvme-cli`,
`hdparm`, `sdparm`, `gptfdisk`, `parted`, `gparted`, `testdisk` (includes
photorec), `ddrescue`, `partclone`, `fsarchiver`, `clonezilla`, `rsync`,
`borg`, `restic`, `sleuthkit`, `libewf`, `foremost`, `clamav`, `yara`,
`rkhunter`, `unhide`, `lynis`, `volatility3`, `binwalk`,
`perl-image-exiftool`, `ssdeep`, `tcpdump`, `wireshark-cli`/`-qt`, `nmap`,
`iperf3`, `mtr`, `bind`, `ethtool`, `iw`, `iwd`, `networkmanager`, `openssh`,
`nftables`, `lshw`, `dmidecode`, `hwinfo`, `inxi`, `lm_sensors`, `stress-ng`,
`memtester`, `memtest86+`(`-efi`), `gsmartcontrol`, `fwupd`, `tpm2-tools`,
`efibootmgr`, `grub`, `os-prober`, `arch-install-scripts`, `ntfs-3g`,
`ntfsprogs`, `hivex`, `chntpw`, `wimlib`, `exfatprogs`, `dosfstools`,
`btrfs-progs`, `xfsprogs`, `e2fsprogs`, `f2fs-tools`, `cryptsetup`, `lvm2`,
`mdadm`, `libguestfs`.

Not every official package should be shipped. Section 2.5's rationale rule
applies to each one.

## 6. Recovery-domain gaps

### 6.1 Windows recovery: what Linux cannot do

The brief says not to assume Windows tools run under Linux. The docs need to
say exactly where the limits are:

- **NTFS repair:** Linux cannot repair NTFS properly. `ntfsfix` only clears
  the dirty flag and resets the log, which forces `chkdsk` on the next Windows
  boot. Real repair means running `chkdsk` from Windows RE.
- **Password recovery:** `chntpw` works on **local accounts only**. It cannot
  reset Microsoft accounts. It does nothing if the volume is BitLocker-locked
  and the key is unavailable.
- **Boot repair:** `bcdedit` and `bcdboot` do not exist on Linux. A UEFI fix
  means rebuilding the ESP contents from `Windows\Boot\EFI` and adding an
  entry with `efibootmgr`. A broken BCD store can be edited with `hivex`, but
  that is expert-only and fragile.
- **BitLocker:** `cryptsetup open --type bitlk` with a recovery key or
  password works. TPM-only protectors cannot be unlocked from Sanctum.

### 6.2 Forensic boot mode

Mature forensic live systems offer a boot entry where every block device is
read-only from the start. Proposal:

- A `sanctum.forensic=1` boot entry.
- It adds a udev rule that sets every non-boot block device read-only
  (`blockdev --setro`) when it appears.
- A visible banner states that the mode is active.
- `sanctum disk rw <dev>` makes one device writable, with confirmation and a
  log entry.

The docs must say that **a software write-block is not a hardware write
blocker**. It is a strong default, not a guarantee that will hold up in court.

### 6.3 Linux chroot recovery is error-prone by hand

A typical repair goes: unlock LUKS, activate LVM, mount the root (and pick the
right btrfs subvolume), read the target's fstab to mount `/boot` and the ESP,
bind-mount the virtual filesystems, `arch-chroot`. Technicians often get this
wrong. A `sanctum chroot` helper (Phase 5) is one of the most useful pieces
Sanctum can add, and it justifies Rust (section 7). `arch-chroot` works for
non-Arch targets too, because inside the chroot the target's own tools are
used (dnf, apt, dracut, update-grub).

### 6.4 Session logging and case notes

Forensic and incident-response work needs a record of what was done:

- timestamped command logs,
- hashes taken at each step,
- the clock offset between the hardware RTC and NTP.

None of this exists, and the brief only implies it. Planned as `sanctum case`
in Phase 5.

### 6.5 Persistence and storing results

A `dd`-written ISO cannot be written to. Logs, reports, ClamAV results and
Warden quarantine stores **disappear at reboot** (they live in tmpfs). The
options are archiso's `cow_label=` persistent overlay, or a separate data
partition. Phase 8 decides. Until then, the docs tell users to write results
to a second USB stick or a network share.

## 7. Rust: where it is justified

The brief says to write down the reason before adding Rust. The user also wants
the project "primarily built in Rust". Both are possible, because the ISO's
**configuration** is not where Rust belongs, but its **tooling** is:

| Component | Language | Reason |
| --- | --- | --- |
| archiso profile, package lists, overlay configs | Data and config files | Declarative. archiso consumes them directly. |
| Build orchestration | Bash (`build-iso.sh`, `in-container.sh`) | Thin glue around mkarchiso. Bash is the brief's stated default. |
| `sanctum` CLI: tool catalog, menus, version, docs viewer | Rust | Typed catalog with validation. The same data drives the CLI menu and the generated `.desktop` and XDG menu files, so it is checked once. |
| Safety-critical helpers: `mount --ro`, `chroot`, `disk rw`, forensic mode, `image`, `hash`, `case` | Rust | Parsing device state (lsblk JSON, blkid, LUKS and LVM state), choosing fs-specific options, and refusing unsafe states. Bash is error-prone here and hard to test. |
| CI checks (package lists, catalog lint) | Rust (`xtask`) or Bash | Start in Bash. Move to `cargo xtask` if it grows. |

Structure, matching Warden and Arsenal: a Cargo workspace under `crates/` with
`rust-toolchain.toml`, `clippy.toml` and `deny.toml`. In v0.1 the `sanctum`
binary is built inside the build container and copied into a **staging copy**
of airootfs (never into the source tree). In Phase 6 this moves to a local
pacman repository with PKGBUILDs, once Warden also needs packaging.

The existing `src/main.rs` and `Cargo.toml` should become
`crates/sanctum-cli` in Phase 0.

## 8. Project and process gaps

### 8.1 Repository layout: changes from the brief

| Brief | Problem | Proposal |
| --- | --- | --- |
| `build/profile/` + `build/overlays/` + `config/{system,network,security,desktop}` | In archiso, `airootfs/` **is** the overlay. Three trees would need a merge step and would split the source of truth. | One `build/profile/` (a complete archiso profile with `airootfs/`). Drop `config/` and `build/overlays/`. |
| `tools/<8 categories>/` | Empty directories. "Tools" here are packages and catalog entries, not source code. | `catalog/<category>.toml` (data) + `crates/` (code). Add `tools/` only for real vendored scripts. |
| `scripts/{install,recovery,diagnostics,security}` | Duplicates the Rust helpers | `scripts/build/` and `scripts/ci/` only, until a real need appears |
| `docs/*` 8 subdirectories | Fine | Keep. Create each when its first page is written. |
| `.github/workflows/{ci,build-iso,release}.yml` | Fine | Keep |

### 8.2 Policy documents the brief needs but does not list

Architecture decision records in `docs/architecture/decisions/`, matching
Warden's style:

1. archiso and the `releng` base
2. Containerised build and snapshot pinning
3. Package inclusion policy (official repos only, exception process,
   rationale format)
4. Non-destructive defaults (section 4)
5. GUI stack
6. Rust components and their boundary with Bash
7. Release integrity (checksums, attestations, future signing)
8. Interfaces to Warden and Arsenal

### 8.3 Legal and compliance

- **GPL source obligations:** distributing an ISO means distributing GPL
  binaries. Each release should publish a source manifest (every package, its
  version, and a link to its source on the Arch archive). Mirroring the source
  tarballs would be better. Most hobby distributions skip this, and Sanctum
  should not. **Decided:** a source manifest with every build, and the
  source tarballs published with every release
  ([ADR-0009](../architecture/decisions/0009-release-integrity.md)).
- **Arch trademark:** "Arch Linux-based" is a factual description and fine.
  Do not use the Arch logo or anything that suggests Arch endorses Sanctum.
- **Project license:** not chosen yet. AGPL matches the siblings, but its
  network clause does nothing for an ISO. GPL-3.0-or-later is the usual
  choice for distribution scripts. Decision needed.
- ClamAV database redistribution terms (section 5.1). The license of any
  bundled YARA rules.

### 8.4 Release integrity without long-lived secrets

- SHA-256 file in `sha256sum -c` format.
- **GitHub artifact attestations** (`actions/attest-build-provenance`, using
  `id-token: write` OIDC): Sigstore-backed build provenance with **no stored
  secret**. Users can check it with `gh attestation verify`.
- minisign or GPG signatures need key custody. Warden already has a minisign
  key process, so reuse it later (Phase 8). A SHA-256 file on the same release
  page only protects against corruption, not tampering. The docs must say
  this.
- The ISO is published outside GitHub (ADR-0005). Attestations are stored
  by GitHub against the file's digest, so `gh attestation verify` works on an
  ISO downloaded from anywhere.
- `release.yml` runs only on `v*.*.*` tags. It checks that the tag matches
  `VERSION`, the workspace version and `profiledef.sh`. It rebuilds from the
  tag; it does not reuse a CI artifact. Only that job gets
  `contents: write`. Actions are pinned by commit SHA.

### 8.5 CI specifics

- `docker run --privileged archlinux@sha256:...` on `ubuntu-latest`.
  Clear disk space first (the runner has about 14 GB free; the archiso work
  directory needs 6-10 GB).
- Cache pacman packages by snapshot date (section 2.2).
- **Boot smoke test in QEMU.** Hosted Linux runners have KVM now. Boot the ISO
  with OVMF (UEFI) and with SeaBIOS. A `sanctum.selftest=1` kernel parameter
  runs read-only checks and prints a pass/fail marker to the serial console:
  - sshd inactive
  - nftables ruleset loaded
  - mdadm and LVM auto-activation off
  - no automount
  - `sanctum version` output correct

  Then it powers off. The same checks are useful to technicians as
  `sanctum selftest`.
- Lint gates: `shellcheck`, `shfmt -d`, `cargo fmt --check`,
  `cargo clippy -D warnings`, `cargo test`, `cargo deny`, `actionlint`,
  `zizmor` (workflow security), markdown lint, link check (`lychee`), and the
  package-list and catalog validators. All of them block merges.

### 8.6 Local dev host prerequisites (Fedora 42)

`sudo dnf install qemu-system-x86 edk2-ovmf ShellCheck shfmt`, plus rootful
podman or docker. Document this in `docs/getting-started/building.md`. Rename
the branch from `master` to `main` before the first commit.

## 9. GUI gaps

The request is "a GUI option similar to startx with SystemRescue". That means
**console first**, with `startx` starting a light desktop when the user wants
it.

| Option | Pros | Cons |
| --- | --- | --- |
| **Xfce on Xorg** (SystemRescue's choice) | Root GUI apps (gparted, wireshark) work without trouble. Light. Well proven on old hardware. Menus can be customised through XDG. | X11 is in maintenance mode |
| Wayland compositor (labwc, sway) | Modern | Running root GUI apps under Wayland is awkward. More work for little gain in a live system. |
| GNOME or KDE | Polished | Heavy. Turns on automount and other background services by default. |

**Recommendation:** Xfce on Xorg, with a minimal component set
(`xfwm4`, `xfce4-panel`, `xfdesktop`, `xfce4-session`, `xfce4-settings`,
`xfce4-terminal`, `thunar`; **no** `thunar-volman`). Started with `startx`.
Category menus (Recovery, Storage, Hardware, ...) are **generated from the same
catalog** as the CLI.

Open decision: whether to include **Firefox** (85 MB). It is useful for
downloading drivers and looking up error messages. Docs can also work without
it: `sanctum docs` in the terminal, and pre-rendered HTML opened in any
browser.

Networking: one stack for CLI and GUI. Use **NetworkManager** (`nmtui` and
`nmcli` on the console, `nm-applet` in Xfce) instead of releng's
networkd + iwd. Two network managers on one system conflict.

## 10. Integration with Warden and Arsenal

### 10.1 Warden: Sanctum overlaps with Warden's Phase 11

Warden's ROADMAP Phase 11 is **"Rescue media and boot integrity (offline scan
and clean from trusted media)"**. Sanctum *is* that rescue media. Agree on this
between the projects so the work is not done twice: Warden provides the
scanner, Sanctum provides the boot environment.

What already exists in Warden: `scan`, `system-check --root` (offline inspection
of a mounted image), minisign-signed content bundles, JSON reports.

What is missing before Sanctum can integrate it:

- No Warden release yet ("software releases wait").
- Building it pulls in yara-x and wasmtime, which is heavy. Sanctum should use
  a **versioned Warden release artifact**, verified by checksum or
  attestation, not build Warden from source in the ISO build.
- On a live system, the **quarantine store sits in tmpfs** and disappears at
  reboot. Remediation from Sanctum needs a quarantine location on the target
  or the USB stick, and it has to be clear which machine's audit log records
  the action.
- `abyssal-wardend` (the service) should **not** run on Sanctum. Only the CLI
  is needed.

Interface: Sanctum's catalog entry `security/warden` calls the
`abyssal-warden` binary. Sanctum only depends on the CLI and the versioned JSON
report format, not on Warden's crates.

### 10.2 Arsenal: optional remote assist, much later

Arsenal is a control plane, and `abyssal-agent` connects out to it. A future
`sanctum arsenal enroll <url> <token>` could let a remote administrator help
with a recovery. That is **remote code execution by design**, so it must be:

- opt-in,
- never started at boot,
- temporary (dropped at reboot),
- clearly shown on screen while connected.

Classed as research (Phase 9). Nothing in v0.1.

## 11. Decisions

Settled by the maintainer on 2026-10-02.

| # | Decision | Outcome |
| --- | --- | --- |
| D1 | Project license | AGPL-3.0-or-later (`LICENSE`), matching the sibling projects |
| D2 | Branch name | `main` |
| D3 | GUI stack | Xfce on Xorg through `startx` |
| D4 | Include Firefox | Yes, if it fits the size budget |
| D5 | Kernel | `linux-hardened` ([ADR-0004](../architecture/decisions/0004-linux-hardened-kernel.md)) |
| D6 | ClamAV databases in the ISO | Only after the redistribution terms are confirmed. Until then, `freshclam` online. |
| D7 | Network stack | NetworkManager |
| D8 | Warden in v0.1 | No. It comes in Phase 6, after Warden's first release. |
| D9 | GitHub location | `AbyssalOath/abyssal-sanctum` |

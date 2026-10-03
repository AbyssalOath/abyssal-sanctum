# Roadmap

Abyssal Sanctum: *a safe environment you boot into for repairs.*

The reasons behind each phase are in the
[gap analysis](docs/development/gap-analysis.md) (referred to below as GA §n).
A phase is done only when its exit criteria pass in CI, or on real hardware
where noted, and its docs are written.

Status legend: **next** (active) · **planned** (scoped) · **research** (needs
investigation before design).

| Phase | Theme | Target | Status |
| --- | --- | --- | --- |
| 0 | Foundations, decisions, containerised builder | v0.1.0 | **implemented, in review** |
| 1 | Smallest bootable branded ISO with safe defaults | v0.1.0 | **implemented, in review** |
| 2 | CI: lint gates, ISO build, QEMU boot tests | v0.1.0 | **implemented, in review** |
| 3 | v0.1 toolkit, `sanctum` CLI, Xfce via `startx`, docs | v0.1.0 | **implemented, in review** |
| 4 | Release pipeline and hardware validation, then tag v0.1.0 | v0.1.0 | **pipeline implemented; hardware testing and tag by the maintainer** |
| 5 | Safety helpers: ro mount, forensic mode, chroot, case log | v0.2.0 | planned |
| 6 | Local package repo and Warden integration | v0.3.0 | planned |
| 7 | Windows recovery depth | v0.4.0 | planned |
| 8 | Persistence, offline content updates, signing, reproducibility check | v0.5.0 | planned |
| 9 | Secure Boot, Arsenal remote assist, netboot, more | -- | research |

**Decided (2026-10-02):** after v0.1.0, the [antimalware track](#antimalware-track)
(AV-1 to AV-7) takes priority over Phases 5 to 8, bringing forward the
parts of them it needs.

---

## Phase 0: Foundations (v0.1.0)

**Goal:** a repository that can build the official `releng` ISO, unchanged
apart from its name, with one command on the Fedora dev host and on CI.

1. Settle decisions D1-D9 (GA §11). Rename the branch to `main`.
2. Add the root files: `README.md`, `LICENSE`, `CHANGELOG.md`,
   `CONTRIBUTING.md`, `SECURITY.md`, `VERSION` (`0.1.0`).
3. Turn the Rust code into a workspace: move the current `src/main.rs` to
   `crates/sanctum-cli` and add `rust-toolchain.toml`, `clippy.toml` and
   `deny.toml` (GA §7).
4. Write ADRs 0001-0004: archiso + releng, containerised + snapshot-pinned
   build, package policy, `linux-hardened` kernel (GA §8.2).
5. Copy `releng` from the pinned archiso version (v91) into `build/profile/`,
   recording the upstream version.
6. Builder:
   - `scripts/build/build-iso.sh`: host wrapper. Checks for docker or podman,
     then runs the archlinux image (pinned by digest) with `--privileged` and
     SELinux-safe mounts.
   - `scripts/build/in-container.sh`: refreshes the keyring, installs the
     pinned archiso, sets `SOURCE_DATE_EPOCH`, and stages the profile into
     `out/work`.
   - Then: `mkarchiso`, `sha256sum`, print the ISO path. Every step runs
     under `set -Eeuo pipefail`, with a clear message on failure.
7. Pin the snapshot (`build/snapshot.conf` → ALA date) and use it from the
   profile's `pacman.conf` (GA §2.2).
8. `docs/getting-started/building.md`, including the Fedora prerequisites
   (GA §8.6).

Status (2026-10-02): implemented. The ISO builds on Fedora 42 with Docker
in about 5 minutes once packages are cached (1560 MiB), and boots to a root
console on `linux-hardened` 7.2.8 under both UEFI (OVMF) and BIOS
(SeaBIOS). The releng name, kernel and Arch logo splash have been replaced.
Everything else is still releng (sshd on, no firewall); that is Phase 1.

**Exit:** `./scripts/build/build-iso.sh` on Fedora produces
`out/abyssal-sanctum-v0.1.0-x86_64.iso` plus `.sha256`, and it boots in QEMU
with both UEFI and BIOS.

## Phase 1: Smallest bootable Sanctum (v0.1.0)

**Goal:** the ISO is recognisably Sanctum, and its defaults keep it from
writing to disks or exposing services on its own.

1. Profile changes (GA §2.4):
   - Remove `cloud-init`, `reflector` and `choose-mirror`.
   - Disable sshd.
   - Strip guest agents down to what boot needs.
   - Remove releng's installer-only pieces (`archinstall`,
     `Installation_guide`, the install-oriented motd) and set the hostname
     (still `archiso`).
   - **Trim for size.** The Phase 0 ISO is already 1560 MiB before any
     Sanctum tools, Xfce or Firefox. The hard limit is 3 GiB (ADR-0005), but
     every MiB costs download time and `copytoram` RAM. Measure
     per-package sizes and cut releng packages Sanctum does not need.
2. Non-destructive defaults (GA §4):
   - md RAID udev auto-assembly masked (not `AUTO -all`, which would also
     break a deliberate `mdadm --assemble --scan`; ADR-0006).
   - LVM `auto_activation_volume_list = []`.
   - `systemd-gpt-auto-generator` masked (inherited from releng).
   - No automount (udisks is not installed; recheck with the GUI in
     Phase 3).
   - nftables default-deny inbound, loaded at boot.
3. Identity:
   - `/etc/os-release` (`NAME="Abyssal Sanctum"`, `ID=abyssal-sanctum`,
     `ID_LIKE=arch`, `VERSION_ID`).
   - `/etc/issue` and the motd banner: "Abyssal Sanctum / Arch Linux-based
     System Recovery Environment", plus the version and the first commands
     to try.
   - `/usr/share/abyssal-sanctum/manifest.json`: version, git commit,
     snapshot date, package list with versions.
4. Boot menu (GA §3.2):
   - Default, nomodeset, copytoram, accessibility, serial.
   - memtest86+ (BIOS and EFI).
   - Branded but restrained (syslinux splash, GRUB/systemd-boot titles).
   - No Plymouth.
5. Live `/etc/pacman.conf` pinned to the same snapshot (GA §2.3).
6. Package lists split by category, with a reason on every line
   (GA §2.5). Start with the core set only: base, kernel, firmware, network,
   storage basics.
7. Review `systemd-loop@.service`. systemd 262 automatically tries to
   loop-attach optical drives (it fails harmlessly on the boot CD in QEMU and
   leaves the system "degraded"). Automatic actions on block devices belong
   in the non-destructive review. Mask it unless there is a reason to keep it.
8. ADR 0006: non-destructive defaults.

**Exit:**

- Boots in QEMU (UEFI and BIOS) to an autologin root console with the banner.
- `systemctl is-active sshd` reports inactive.
- `nft list ruleset` shows the default-deny ruleset.
- Attaching a test disk image that contains an md array and an LVM volume
  group leaves both inactive, and the image's hash is unchanged after boot.

Status (2026-10-02): implemented, and every exit criterion checked by hand
in QEMU (details in ADR-0006, "Verification").

- The ISO is 1150 MiB, down from 1560 MiB. Most of the saving came from
  dropping the `kms` initramfs hook: the initramfs had 150 MB of GPU
  firmware, stored twice on the ISO.
- 80 listed packages (347 with dependencies).
- The serial entry was boot-tested; nomodeset and copytoram were not
  (they only add standard kernel parameters).

Also done beyond the list:

- releng's `script=` remote-script boot feature removed.
- mDNS and LLMNR off.
- NetworkManager's connectivity check off.
- PXE boot configs and initramfs hooks removed (Phase 9).

Found during testing, carried forward:

- `nftables.service` loads the rules and exits, so it reads "inactive".
  Self-tests must check the ruleset (Phase 2).
- The `hostname` command is missing (it lives in `inetutils`). Add it with
  the Phase 3 toolkit.
- `clonezilla` pulls in Python, `partimage`, `screen`, `sshfs` and more.
  Review its weight against `partclone` and `fsarchiver` in Phase 3.

## Phase 2: CI (v0.1.0)

**Goal:** every change is linted, built and boot-tested automatically.

1. `ci.yml` runs on push and PR. All of these block merges:
   - shellcheck and `shfmt -d`
   - `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`,
     `cargo deny`
   - actionlint and zizmor
   - markdown lint and lychee
   - the package-list validator (reason present, no duplicates, exists in
     the snapshot, `core`/`extra` only)
   - `profiledef.sh` / `VERSION` consistency
2. `build-iso.yml` runs on `main` pushes, on PRs that touch `build/` or
   `crates/`, and on manual dispatch:
   - Free runner disk space, then build with the privileged container.
   - Cache pacman packages by snapshot date.
   - Size budget: warn at 2.5 GiB, fail at 3 GiB (GA §2.6, ADR-0005).
   - Upload the ISO as a short-lived workflow artifact.
3. QEMU smoke test (GA §8.5):
   - Boot with KVM, OVMF and SeaBIOS, using `sanctum.selftest=1`.
   - Read the serial log and look for the `SANCTUM-SELFTEST: PASS` marker.
   - Time out after 5 minutes.
   - Automate the Phase 1 disk-safety test: create RAID1 and LVM test
     images, boot with `scripts/test/run-vm.sh --writable-disk`, and compare
     the image hashes before and after.
4. Pin every action by commit SHA. Give each job the least permissions it
   needs (default `contents: read`).
5. `docs/architecture/ci-cd.md`.

**Exit:**

- A PR that breaks shell formatting, adds a package with no reason, or
  re-enables sshd fails CI.
- A clean PR produces a boot-tested ISO artifact.

Status (2026-10-02): implemented and verified locally. The workflows have
not run on GitHub yet, because the repository does not exist there.
actionlint and zizmor pass on them, and every job's script was run locally:

- `scripts/ci/check-all.sh` passes all gates.
- Each gate was shown to fail when it should:
  - shfmt rejects unformatted shell (`profiledef.sh` failed before it was
    formatted).
  - The package checker rejects a missing reason, an empty reason and a
    duplicate.
  - `check-profile.sh` rejects an enabled sshd.
  - `check-version.sh` rejects a version mismatch and a wrong tag.
  - lychee rejects a missing file and a missing heading anchor.
- `boot-test.sh` passes for UEFI and BIOS (about 70 seconds each) with the
  RAID1 and LVM fixtures attached. It fails, and stops QEMU, on a timeout.
- `sanctum-selftest` reported 7 failures when sshd was started, RAID
  assembled, LVM activated, a volume mounted and port 22 opened.

Found and fixed during testing:

- `mdmonitor.service` failed after a deliberate `mdadm --assemble`, which
  left the system "degraded". It is now masked (ADR-0006).
- The snapshot's shellcheck 0.11 reports a check that Fedora's 0.10 does
  not have, which is why CI lints with the pinned version.

Still to confirm on GitHub: hosted runners build within the time and disk
limits, KVM works, and the SMBIOS credential reaches systemd there as it
does locally.

## Phase 3: v0.1 toolkit, CLI and GUI (v0.1.0)

**Goal:** the feature set the brief asks for in v0.1, in the smallest form
that is useful.

1. Package sets with reasons (GA §5.5). Use only the confirmed official
   packages:
   - **Storage:** gptfdisk, parted, smartmontools, nvme-cli, hdparm, sdparm,
     ddrescue, partclone, fsarchiver, rsync, cryptsetup, lvm2, mdadm, all
     common filesystem progs, ntfs-3g/ntfsprogs.
   - **Hardware:** lshw, dmidecode, hwinfo, inxi, pciutils, usbutils,
     lm_sensors, stress-ng, memtester.
   - **Network:** networkmanager, openssh (disabled), nftables, iw, ethtool,
     bind, mtr, iperf3, tcpdump, wireshark-cli, nmap.
   - **Security / forensics:** clamav, yara, rkhunter, unhide, sleuthkit,
     libewf, testdisk/photorec, foremost, perl-image-exiftool, ssdeep,
     binwalk.
   - **Recovery:** arch-install-scripts, grub, efibootmgr, os-prober, hivex,
     chntpw.
2. `sanctum` CLI (Rust, ADR 0008):
   - `sanctum`: the category menu. The categories are Recovery, Storage,
     Hardware, Networking, Security, Forensics, Windows, Linux,
     Documentation and Terminal.
   - `sanctum tools [category]`: list tools, marked read-only, modifies or
     destructive.
   - `sanctum version`.
   - `sanctum docs [topic]`.
   - `sanctum disks`: read-only overview showing fs type, encryption, md and
     LVM membership, hibernation flag and SMART summary.
   - `sanctum ssh enable|disable`: sets a password or authorized key, starts
     sshd, opens port 22 in nftables, and prints the fingerprints.
   - `sanctum selftest`.
   - The catalog lives in `catalog/*.toml` and is validated in CI against
     the package lists.
3. GUI (ADR 0007, GA §9):
   - Xfce on Xorg with the minimal component set, started by `startx` (the
     motd says so).
   - Category menus and `.desktop` files generated from the catalog.
   - Gparted, gsmartcontrol and wireshark-qt.
   - Firefox if D4 says yes.
   - Dark, restrained Abyssal theme.
   - No thunar-volman.
4. ClamAV: a decision based on D6 (GA §5.1). Either bundled databases fetched
   through a cached `cvdupdate` step with their hashes in the manifest, or
   `freshclam` online only, documented.
5. Docs (the brief's minimum set):
   - **getting-started:** building, VM, USB (dd, Ventoy), booting,
     navigation, the Secure Boot and BitLocker warning.
   - **recovery:** Linux, Windows (including the GA §6.1 limits),
     filesystems, disk diagnostics, backup and restore.
   - **security:** offline ClamAV, YARA, offline rootkit investigation
     (GA §5.3), basic forensic workflow, evidence preservation.
   - **architecture:** repo layout, profile, package selection, overlay,
     startup, networking, configuration, CI/CD, releases.
   - **operations:** add or remove packages, bump the snapshot, change
     config, add tools to the catalog, update the ISO, cut a release.

**Exit:**

- Every catalog entry starts on a booted ISO (checked with a selftest).
- `startx` reaches a working Xfce desktop in QEMU.
- The docs build and pass link checks.

Status (2026-10-02): implemented and tested in QEMU.

- The ISO is 1565 MiB (627 packages). Both boot tests pass with 17
  self-test checks. One of them runs `sanctum catalog verify`: all 82
  catalog tools are installed and their version checks succeed.
- Desktop, checked by hand:
  - `startx` reaches Xfce with the Sanctum background, the dark theme and
    the catalog menu.
  - A terminal tool opened from the menu shows its entry and a shell.
  - Firefox runs as root on `linux-hardened` and opens the offline guides.
- Console, checked by hand: `sanctum disks`, `sanctum ssh enable --key`,
  `status` and `disable`.
- ADR-0007 (GUI) and ADR-0008 (Rust) written. Guides and operations docs
  pass markdown lint and the link check.

Changes from the plan:

- `sanctum disks` does not show a hibernation flag yet. Reliable detection
  means reading NTFS structures, and the obvious tool (`ntfs-3g.probe
  --readwrite`) may write to the volume. Moved to Phase 5, with the
  read-only mount helper. NTFS-3G itself still refuses read-write mounts of
  hibernated volumes.
- Clonezilla stays (about 80 MiB of its own dependencies). Technicians know
  its guided workflow, and the size fits the budget.
- The ClamAV databases are not bundled (D6); `freshclam` online, or loading
  them from USB, is documented.
- `cifs-utils` and `librsvg` were added. SMB mounting had only worked
  through Clonezilla's dependencies, and SVG menu icons rendered blank
  without librsvg.

Found and fixed during testing:

- xfdesktop 4.20 ignores background settings added while it runs, so the
  desktop settings are now written from `.xinitrc` before Xfce starts.
- `mc --version` fails without a terminal; its catalog check was removed
  (the command's presence is still checked).
- Rust's default panic on a closed pipe (`sanctum tools | head`) is
  replaced by a quiet exit, without `unsafe`.
- `sanctum disks` now leaves out zram devices and shows swap in use
  correctly.

## Phase 4: Release and v0.1.0

**Goal:** publishing a tag is the only step needed to ship a release.

1. `release.yml` runs on `v*.*.*` tags only:
   - Check tag = `VERSION` = workspace version = `iso_version`.
   - Check that the tagged commit is on `main`.
   - Rebuild from the tag and boot-test.
   - Produce `abyssal-sanctum-vX.Y.Z-x86_64.iso` and `.iso.sha256`.
   - Create a build provenance attestation with no stored secrets
     (GA §8.4).
   - `gh release create` with notes from `CHANGELOG.md`. The GitHub
     Release holds the `.sha256`, the source manifest and the download
     link. The ISO itself is hosted on SourceForge or the project site
     (ADR-0005).
   - Publish the ISO to the download host. Decided: the maintainer
     downloads the CI-built ISO, checks it with `gh attestation verify`, and
     uploads it by hand (no stored secret).
   - Only this job gets `contents: write`, `id-token: write` and
     `attestations: write`.
2. GPL source manifest attached to each release (GA §8.3).
3. Hardware validation checklist (`docs/operations/release-checklist.md`):
   - At least one UEFI laptop, one legacy-BIOS machine and one NVMe system.
   - USB via dd and via Ventoy.
   - Wi-Fi association.
   - Mounting an NTFS volume that is hibernated (it must refuse read-write).
4. ADR-0009: release integrity. Tag v0.1.0.

**Exit:** a GitHub Release with the checksum, source manifest and a download
link; the ISO on the download host, passing `sha256sum -c` and
`gh attestation verify`; and a signed-off hardware checklist.

Status (2026-10-02): the pipeline is implemented and checked locally. What
is left needs the maintainer: the GitHub repository, real hardware and the
download host.

Done:

- `release.yml`: verify the tag (versions, dated changelog section, commit
  on `main`), build and boot-test through `build-iso.yml` with
  `release: true` (no caches, 30-day artifacts), then in the `release`
  environment attest the ISO and manifests and draft the release.
  actionlint and zizmor pass.
- Every build writes `*.sources.txt` (571 package bases, with Arch source
  and packaging-repository links) and `*.manifest.json` next to the ISO.
- `scripts/release/`:
  - `prepare.sh`, `check-release.sh` and `release-notes.sh`, tested in a
    scratch repository, including the failure cases (wrong tag, undated
    changelog, commit not on `main`, preparing twice).
  - `fetch-sources.sh`, tested on four packages covering a direct download,
    `makepkg --allsource`, an epoch version and a `+` in the name.
- ADR-0009, `docs/operations/release-checklist.md`, and a rewritten
  `docs/operations/releasing.md`.

Found along the way:

- Arch's source server had only 56 of the 571 package bases, so linking to
  it would not meet the GPL. Sources are collected and published with each
  release instead (ADR-0009).
- Avahi arrives as a dependency and can be started over D-Bus to announce
  the machine. It is now masked, and the profile check and the self-test
  check the mask.
- GitHub's newer `uses: $/...` form for local workflows is not accepted by
  actionlint 1.7.12 yet; `./` is used with a documented zizmor exception.

Left for the maintainer, in order (docs/operations/releasing.md):

1. Create `AbyssalOath/abyssal-sanctum` and push `main`. Then check the
   first CI and Build ISO runs on hosted runners (time, disk, KVM, the
   self-test credential).
2. Set up the `release` environment with a required reviewer, a `v*` tag
   ruleset, and branch protection for `main`.
3. Work through the release checklist on real hardware.
4. `scripts/release/prepare.sh 0.1.0`, merge, tag `v0.1.0`.
5. Upload the ISO and sources, then publish the draft release.

---

## Antimalware track

Goal: a flagship offline antivirus, malware-removal and rootkit-hunting
system for compromised Windows (and Linux) machines and accounts. The full
analysis, including what open-source engines can and cannot match against
commercial scanners, is in the
[antimalware gap analysis](docs/development/antimalware-gap-analysis.md)
(AGA below). Decisions (AGA section 10): AV-D1 to AV-D5 and AV-D8 yes;
AV-D6 no (open engines and content only); AV-D7 (where to keep and run a
malware test corpus) still open.

| Step | Theme | Target | Brings forward |
| --- | --- | --- | --- |
| AV-1 | Scan foundation: `sanctum targets`, `mount`, `update`, `scan` with multi-threaded ClamAV and alternate data streams, USB data partition, case reports | v0.2.0 | Phase 5 mount helper, Phase 8 data partition |
| AV-2 | Quarantine and review: reversible, NTFS-aware, persistent, audited; GUI scan wizard | v0.2.0 | |
| AV-3 | Detection content: Warden with signed ESET and ReversingLabs rules, signature-base, hash reputation, vulnerable drivers | v0.3.0 | Phase 6 Warden integration |
| AV-4 | Offline Windows persistence analysis and persistence-aware remediation with undo | v0.3.0 | |
| AV-5 | Compromised-account investigation: event logs (Hayabusa, Chainsaw), per-account timeline, capa | v0.4.0 | Phase 6 local package repository |
| AV-6 | Rootkits and boot integrity: driver signatures, MBR/VBR/EFI, Linux `system-check --root`, shadow copies | v0.4.0 | |
| AV-7 | Measured detection rates and false positives, published per release | v0.3.0 onwards | |

Secure Boot research (Phase 9) should start alongside AV-1: turning Secure
Boot off on BitLocker machines is a real obstacle for fleet scanning
(AGA section 9).

## Phase 5: Safety helpers (v0.2.0)

- `sanctum mount <dev> [--rw]`: read-only by default, `blockdev --setro`,
  journal replay suppressed (`noload`/`norecovery`). Detects NTFS
  hibernation and refuses read-write. Unlocks BitLocker and LUKS (GA §4,
  §6.1).
- Forensic boot entry (`sanctum.forensic=1`): a udev rule makes all non-boot
  devices read-only, a banner shows the mode, and `sanctum disk rw <dev>`
  overrides it with confirmation (GA §6.2).
- `sanctum chroot`: finds the root, follows the target's fstab, handles LUKS,
  LVM and btrfs subvolumes, then bind-mounts and runs `arch-chroot`
  (GA §6.3).
- `sanctum image` / `sanctum hash`: ddrescue acquisition with an inline
  SHA-256 and a mapfile, and recursive hash manifests. These replace the
  unavailable dc3dd and hashdeep (GA §5.5).
- `sanctum case`: case directory, timestamped session log, RTC/NTP offset
  record, and a hash of every action's output (GA §6.4).
- A shared confirmation flow: destructive actions require typing the device
  name.

## Phase 6: Local repository and Warden (v0.3.0)

- A local pacman repository built during the ISO build, with PKGBUILDs for
  `abyssal-sanctum` (the CLI and catalog) and `abyssal-warden`. The staging
  copy into airootfs goes away.
- Warden is consumed as a versioned release artifact and verified. Only the
  CLI is included; wardend is not. Agree with the Warden project that this
  covers its Phase 11 (GA §10.1).
- Quarantine store on the target or a USB data location, with an explicit
  audit-log location.
- The catalog's `security/warden` entry runs `scan` and `system-check --root`
  against mounted targets. Warden's signed content bundles provide the
  curated YARA rules (GA §5.2).

## Phase 7: Windows recovery depth (v0.4.0)

- Guided ESP rebuild from `Windows\Boot\EFI` plus `efibootmgr`.
- Read-only BCD and registry inspection with hivex.
- Fast Startup and hibernation detection, with guidance.
- `chntpw` workflow limited to local accounts, with the documented limits.
- Documented `chkdsk`-from-WinRE handoff.
- Evaluate non-official tools (regripper and others) case by case, each with
  an ADR justifying the exception.

## Phase 8: Persistence, content and integrity (v0.5.0)

- Choose between an archiso `cow_label` persistent overlay and a separate
  data partition (GA §6.5). Results, case logs and quarantine survive a
  reboot.
- Offline content updates: load ClamAV databases and Warden bundles from a
  USB data partition, verified before use.
- minisign signatures for releases, using Warden's key custody process
  (GA §8.4).
- Reproducibility check: two independent builds of the same tag, compared
  with `diffoscope`. Results are published.

## Phase 9: Research

- **Secure Boot:** a MOK-enrolled chain with sbctl and mokutil, against a
  signed shim. This is the most important research item (GA §3.1).
- **Arsenal remote assist:** an opt-in, temporary `abyssal-agent` session,
  never started at boot (GA §10.2).
- A `linux` fallback boot entry if `linux-hardened` blocks real recovery
  work, depending on the size budget (ADR-0004).
- PXE/HTTP netboot (archiso supports it).
- Memory image analysis workflows with volatility3 (GA §5.4).
- aarch64.

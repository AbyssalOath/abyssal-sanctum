# Changelog

All notable changes to Abyssal Sanctum are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Changed

- Docs: release attestations need a public repository; CI timings measured
  on GitHub's runners; v0.1.0 hardware test results recorded in the
  roadmap.

## [0.1.0] - 2026-10-03

### Added

- Containerised ISO build (`scripts/build/build-iso.sh`), pinned to an Arch
  Linux Archive snapshot, a builder image digest and an archiso version.
- archiso profile derived from archiso 91's `releng`, renamed to Abyssal
  Sanctum, using the `linux-hardened` kernel.
- SHA-256 checksum and size budget check for every build.
- BIOS boot splash built from the Sanctum emblem, replacing the Arch Linux
  logo (`scripts/dev/make-boot-splash.py`).
- QEMU test runner (`scripts/test/run-vm.sh`) for UEFI and BIOS.
- `sanctum` CLI skeleton (`sanctum version`).
- Roadmap, gap analysis and architecture decision records 0001-0008.
- Non-destructive defaults (ADR-0006): no RAID assembly or LVM activation
  at boot, default-deny nftables firewall, sshd off, no mDNS/LLMNR, no
  NetworkManager connectivity checks.
- Sanctum identity: hostname, `/etc/os-release`, `/etc/issue`, login banner,
  and a build manifest at `/usr/share/abyssal-sanctum/manifest.json`.
- Boot entries for safe graphics, copy to RAM and serial console.
- Package lists by category with a reason for every package
  (`build/packages/*.list`), checked by `scripts/ci/check-package-lists.sh`.
- The live system's pacman uses the same archive snapshot as the build.
- `scripts/test/run-vm.sh --writable-disk` for disk-safety tests.
- Booting guide (`docs/getting-started/booting.md`).
- CI (`.github/workflows/ci.yml`): shell lint with pinned shellcheck and
  shfmt, package-list, profile and version checks, Rust fmt/clippy/tests,
  cargo-deny, markdown lint, offline link check, actionlint and zizmor.
  All actions pinned by commit SHA; read-only permissions; Dependabot.
- ISO build workflow (`.github/workflows/build-iso.yml`) with a pacman
  cache, artifact upload and QEMU boot tests for UEFI and BIOS.
- `sanctum-selftest`: read-only check of identity and safe defaults, run at
  boot in CI through a systemd credential, or by hand.
- Boot test (`scripts/test/boot-test.sh`) with RAID1 and LVM disk fixtures
  that must stay byte-for-byte unchanged.
- `scripts/ci/check-all.sh` runs every CI gate locally.
- CI/CD documentation (`docs/architecture/ci-cd.md`).
- v0.1 toolkit: ClamAV, YARA, rkhunter, unhide, The Sleuth Kit, libewf,
  foremost, ExifTool, ssdeep, binwalk, chntpw, hivex, NTFS-3G, os-prober,
  inxi, lshw, hwinfo, lm_sensors, stress-ng, memtester, mtr, iperf3,
  tshark, SMB mounting and `hostname`.
- Tool catalog (`catalog/*.toml`): 82 tools in 10 categories, each with a
  risk level (read-only, modifies, destructive), notes and examples,
  checked against the package lists and guides.
- `sanctum` CLI: interactive menu, `tools`, `tool`, `docs`, `disks`
  (read-only disk overview with SMART health), `ssh enable|disable|status`,
  `selftest`, `catalog check|verify`, `version` (ADR-0008).
- Xfce desktop via `startx` (ADR-0007): dark theme, the Sanctum menu
  generated from the catalog, the Sanctum background, GParted,
  GSmartControl, Wireshark and Firefox (no telemetry or update checks; the
  offline guides as home page).
- Offline guides in the ISO, as Markdown for `sanctum docs` and as HTML for
  the browser.
- Guides: navigation, writing to USB, running in a VM; Linux and Windows
  repair, filesystems, disk diagnostics, backup and restore; offline
  malware scanning, YARA, rootkit investigation, forensic workflow,
  evidence preservation; architecture overview; operations (packages,
  updating the base, configuration, catalog, releasing).
- Self-test checks that every catalog tool is installed and starts, and
  that no automount software is installed.
- Release workflow (`.github/workflows/release.yml`, ADR-0009): tags on
  `main` are verified, built and boot-tested without caches, attested
  (build provenance, no stored secrets), and drafted as a GitHub release
  with the checksum and manifests.
- Each build writes a source manifest (`*.sources.txt`) and a package
  manifest (`*.manifest.json`) next to the ISO.
- Release scripts: `prepare.sh` (set the version, date the changelog),
  `check-release.sh`, `release-notes.sh`, and `fetch-sources.sh` (collect
  the GPL corresponding source of every package, with `makepkg
  --allsource` where Arch no longer serves it).
- Release hardware checklist (`docs/operations/release-checklist.md`).

### Changed

- NetworkManager replaces releng's systemd-networkd and iwd.
- `mdmonitor.service` is masked: it failed after a deliberate RAID assembly
  because a live system has no mail address for its alerts.
- `profiledef.sh` formatted with shfmt; ADR headers and Markdown tables
  normalised for markdownlint.
- `scripts/dev/make-boot-splash.py` is now `scripts/dev/make-branding.py`
  and also makes the panel icon.
- The login banner points to `sanctum`, `startx` and `sanctum ssh enable`.
- Avahi (pulled in by desktop libraries) is masked, so it cannot be started
  on demand and announce the machine on the network.
- `build-iso.yml` can be called by the release workflow, with caches off.
- ISO size reduced from 1560 MiB to 1150 MiB, mainly by dropping the `kms`
  initramfs hook (GPU firmware was being copied into the initramfs).

### Removed

- From releng: archinstall, cloud-init, reflector and mirror selection, VM
  guest agents, ModemManager and dial-up/VPN clients, smartcard daemon,
  PXE boot support, and the `script=` boot-time script download.

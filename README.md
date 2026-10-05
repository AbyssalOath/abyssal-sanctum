# Abyssal Sanctum

Arch Linux-based System Recovery Environment.

> A safe environment you boot into for repairs.

Abyssal Sanctum is a bootable live environment for system recovery,
diagnostics, repair, administration, security investigation and malware
remediation. It boots from USB or in a virtual machine, runs without
installation, and works offline.

> **Status: v0.1.0, the first release.** The ISO has the v0.1 toolkit, the
> `sanctum` command, an optional Xfce desktop and offline guides, with safe
> defaults checked in CI and on real hardware. Next: the antimalware track
> (v0.2.0). See [ROADMAP.md](ROADMAP.md).

## Download

ISOs are on [GitHub Releases](https://github.com/AbyssalOath/abyssal-sanctum/releases),
together with their checksums. The source code of every package in the ISO
(GPL) is on [SourceForge](https://sourceforge.net/projects/abyssal-sanctum/files/sources/),
one folder per release. See
[writing the ISO to USB](docs/getting-started/writing-to-usb.md) for how to
verify a download and write it to a USB stick.

## Building

You need Docker (or root Podman). The build runs in a pinned Arch Linux
container, so it works on any Linux host.

```bash
./scripts/build/build-iso.sh
```

The result is `out/abyssal-sanctum-v<version>-x86_64.iso` and its `.sha256`
file. To try it in QEMU:

```bash
./scripts/test/run-vm.sh          # UEFI
./scripts/test/run-vm.sh --bios   # legacy BIOS
```

Full instructions: [docs/getting-started/building.md](docs/getting-started/building.md).

## Known limitations

- **No Secure Boot support in v0.1.0.** Secure Boot has to be turned off to
  boot it. On Windows machines with BitLocker, changing the Secure Boot
  setting makes Windows ask for the BitLocker recovery key on its next boot.
  Have the key before you change it. Signed releases that boot with Secure
  Boot on are planned for v0.2.0
  ([ADR-0011](docs/architecture/decisions/0011-secure-boot-shim-mok.md)).
- Sanctum does not activate RAID or LVM, mount disks, or start network
  services by itself ([ADR-0006](docs/architecture/decisions/0006-non-destructive-defaults.md)).
  Tools you run yourself, such as `fdisk` or `mkfs`, can still destroy data.

## Documentation

- [Writing the ISO to USB](docs/getting-started/writing-to-usb.md)
- [Booting and first steps](docs/getting-started/booting.md)
- [Finding your way around](docs/getting-started/navigation.md)
- Recovery: [Linux](docs/recovery/linux.md),
  [Windows](docs/recovery/windows.md),
  [filesystems](docs/recovery/filesystems.md),
  [disk diagnostics](docs/recovery/disk-diagnostics.md),
  [backup and restore](docs/recovery/backup-and-restore.md)
- Security: [malware scanning](docs/security/offline-malware-scanning.md),
  [YARA](docs/security/yara-scanning.md),
  [rootkits](docs/security/rootkit-investigation.md),
  [forensic workflow](docs/security/forensic-workflow.md),
  [evidence preservation](docs/security/evidence-preservation.md)
- [Architecture overview](docs/architecture/overview.md)
- [Roadmap](ROADMAP.md)
- [Gap analysis](docs/development/gap-analysis.md)
- [Antimalware gap analysis and plan](docs/development/antimalware-gap-analysis.md)
- [Architecture decisions](docs/architecture/decisions/README.md)
- [CI/CD](docs/architecture/ci-cd.md)
- Operations: [packages](docs/operations/packages.md),
  [updating the base](docs/operations/updating-the-base.md),
  [configuration](docs/operations/configuration.md),
  [tool catalog](docs/operations/catalog.md),
  [releasing](docs/operations/releasing.md),
  [release checklist](docs/operations/release-checklist.md)

## Related projects

Abyssal Sanctum is part of the Abyssal family of projects. It is developed
independently and will integrate others through their public interfaces:

- [Abyssal Warden](https://github.com/AbyssalOath/abyssal-warden): malware
  and rootkit detection and remediation.
- [Abyssal Arsenal](https://github.com/AbyssalOath/abyssal-arsenal): server
  management and IT operations.

## License

[GNU Affero General Public License v3.0 or later](LICENSE). The ISO also
contains Arch Linux packages under their own licenses.

Abyssal Sanctum is based on Arch Linux. It is not affiliated with or endorsed
by the Arch Linux project.

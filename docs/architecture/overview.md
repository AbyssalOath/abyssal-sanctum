# Architecture overview

Abyssal Sanctum is an archiso profile, a small amount of Bash build glue, a
Rust command-line tool, a tool catalog and documentation. This page shows how
they fit together. The reasons behind each choice are in the
[decision records](decisions/README.md).

## Repository layout

| Path | Contents |
| --- | --- |
| `build/build.conf` | Pinned build inputs: archive snapshot, builder image, archiso version, size budget |
| `build/packages/*.list` | Packages in the ISO, by category, each with a reason ([ADR-0003](decisions/0003-package-policy.md)) |
| `build/profile/` | The archiso profile, derived from releng ([ADR-0001](decisions/0001-archiso-releng-base.md)) |
| `build/profile/airootfs/` | Files copied into the live system: the overlay |
| `catalog/*.toml` | The tool catalog: one file per menu category |
| `crates/sanctum-cli/` | The `sanctum` command ([ADR-0008](decisions/0008-rust-components.md)) |
| `scripts/build/` | `build-iso.sh` (host side) and `in-container.sh` (the build itself) |
| `scripts/test/` | `run-vm.sh` and `boot-test.sh` |
| `scripts/ci/` | The checks CI runs; `check-all.sh` runs them all locally |
| `scripts/dev/` | Developer helpers, such as regenerating branding images |
| `tests/fixtures/` | Disk images for the boot test |
| `assets/` | Branding source images |
| `docs/` | These guides; also shipped in the ISO |

## The build

`scripts/build/build-iso.sh` starts the pinned Arch builder container
([ADR-0002](decisions/0002-containerised-pinned-build.md)), which runs
`scripts/build/in-container.sh`:

1. Pin the container's pacman to the archive snapshot; install archiso and
   rustup.
2. Validate `VERSION`, the package lists and the profile.
3. Stage the profile in a work directory:
   - combine `build/packages/*.list` into `packages.x86_64`,
   - fill in placeholders (`@SANCTUM_VERSION@`, `@SNAPSHOT_DATE@`,
     `@GIT_COMMIT@`),
   - pin the live system's pacman to the snapshot,
   - resolve every package against the snapshot and write the build
     manifest with exact versions.
4. Add Sanctum's own files to the staged overlay:
   - build `sanctum` with the pinned Rust toolchain,
   - check the catalog against the package lists and guides,
   - generate the Xfce menu entries from the catalog,
   - copy the guides and render them to HTML,
   - add the desktop background.
5. Run `mkarchiso`, then write the SHA-256 checksum and check the size
   budget ([ADR-0005](decisions/0005-iso-hosting-and-size-budget.md)).

The repository is mounted read-only; generated files exist only in the
staged copy.

## The archiso profile

| File | Purpose |
| --- | --- |
| `profiledef.sh` | ISO name, version, label, boot modes, compression, file permissions |
| `pacman.conf` | Package source for the build: the archive snapshot only |
| `syslinux/` | BIOS boot menu |
| `efiboot/` | UEFI boot menu (systemd-boot) |
| `grub/` | Menus used when the ISO is booted through GRUB loopback, for example from Ventoy |
| `airootfs/` | The overlay |

All boot menus offer the same entries: normal, safe graphics, copy to RAM,
serial console, speech, and Memtest86+.

## The overlay

`airootfs/` is copied into the live root filesystem before packages are
installed. Files that packages mark as configuration (for example
`/etc/nftables.conf`) keep Sanctum's version; other package files cannot be
replaced this way.

| Area | Files |
| --- | --- |
| Identity | `/etc/os-release`, `/etc/issue`, `/etc/motd`, `/etc/hostname` |
| Safety defaults ([ADR-0006](decisions/0006-non-destructive-defaults.md)) | `/etc/nftables.conf`, `/etc/lvm/lvm.conf`, masks in `/etc/udev/rules.d` and `/etc/systemd/system`, `sshd_config.d`, `resolved.conf.d`, NetworkManager `conf.d` |
| Services | Enable links under `/etc/systemd/system/*.wants/` |
| Desktop ([ADR-0007](decisions/0007-gui-stack.md)) | `/root/.xinitrc`, Xfce settings in `/root/.config/xfce4/`, `/etc/xdg/autostart/`, Firefox policies |
| Scripts | `/usr/local/bin/sanctum-selftest`, `sanctum-desktop-setup` |

Added by the build: `/usr/local/bin/sanctum`, the catalog in
`/usr/share/abyssal-sanctum/catalog/`, the manifest
`/usr/share/abyssal-sanctum/manifest.json`, menu files, and the guides in
`/usr/share/doc/abyssal-sanctum/`.

## Startup

1. The boot loader loads the `linux-hardened` kernel and the initramfs.
2. archiso's initramfs hooks find the boot medium by its UUID, mount the
   compressed root image (`airootfs.sfs`), and put a RAM-backed writable
   layer on top. With `copytoram`, the image is copied into RAM first.
3. systemd starts the enabled services: NetworkManager, systemd-resolved,
   nftables (loads the firewall and exits), timesyncd, and pacman's keyring
   setup. Nothing activates disks.
4. Root is logged in automatically on the first console, and sees the login
   banner.
5. With the `sanctum.selftest` credential or kernel parameter (CI), the
   self-test runs, reports to the serial port, and powers off.

## Networking

NetworkManager manages wired and Wi-Fi connections, and connects wired
networks with DHCP. systemd-resolved resolves names, without mDNS or LLMNR.
The nftables table `inet sanctum` drops all incoming connections except
replies, ICMP and DHCPv6, plus any ports the user adds to its `tcp_open`
set. SSH is installed but off ([navigation guide](../getting-started/navigation.md#remote-access-ssh)).

## Further reading

- [CI/CD](ci-cd.md)
- [Releasing](../operations/releasing.md)
- [Changing configuration](../operations/configuration.md)

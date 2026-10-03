# ADR-0001: archiso with a profile derived from `releng`

- **Status:** Accepted
- **Date:** 2026-10-02

## Context

Abyssal Sanctum is a live environment built on Arch Linux. It needs a
maintained way to turn a package list and a configuration overlay into a
bootable hybrid ISO for BIOS and UEFI, from USB or optical media.

## Options considered

1. **archiso.** Arch's official tool, used for the Arch installation ISO.
   Declarative profiles: `profiledef.sh`, a package list, `pacman.conf`, an
   `airootfs/` overlay, and boot loader configs.
2. **A custom pipeline** (pacstrap + mksquashfs + xorriso). Full control, but
   it would re-implement archiso and its boot-loader handling, which is a lot
   of fragile work.
3. **Other live-system builders** (for example mkosi). Capable, but not the
   Arch-supported route for live ISOs. Arch-specific help and examples are
   thinner.

archiso ships two profiles. `baseline` is minimal and has no networking
setup, accessibility or memtest. `releng` is the official installation ISO,
tested on a wide range of real hardware.

## Decision

Use archiso. Copy the `releng` profile of the pinned archiso version into
`build/profile/` and change it there. `ARCHISO_VERSION` in `build/build.conf`
records which archiso version the profile tracks.

There is one profile tree. In archiso, `airootfs/` already is the overlay on
top of the installed packages, so the brief's separate `config/` and
`build/overlays/` trees would only add a merge step (gap analysis, section
8.1).

## Consequences

- When archiso is updated, the new `releng` must be diffed against
  `build/profile/`, and relevant changes ported by hand. Boot-mode names and
  `profiledef.sh` keys have changed between archiso releases before.
- Everything `releng` enables is inherited and has to be reviewed. The
  security-related removals (sshd, cloud-init, reflector, guest agents) were
  made in Phase 1 (ADR-0006).
- Only x86_64 is built. `packages.aarch64` was dropped from the copy.

# ADR-0002: Containerised build pinned to an archive snapshot

- **Status:** Accepted
- **Date:** 2026-10-02

## Context

The ISO must be buildable with one command, independently of the
developer's machine, and the same commit should build the same ISO. Two
things stand in the way:

- `mkarchiso` needs an Arch userland and root. The main development host runs
  Fedora, and CI runs on Ubuntu.
- Arch is a rolling distribution. Building the same commit two days apart
  normally installs different packages.

## Decision

**Container.** `scripts/build/build-iso.sh` runs
`scripts/build/in-container.sh` in an Arch Linux image pinned by digest. The
container runs `--privileged`, because mkarchiso mounts filesystems and uses
namespaces. Rootless podman cannot do this, so the script refuses it and asks
for docker or `sudo`. The source tree is mounted read-only; the build only
writes to `out/`. On an Arch host, `--native` runs the inner script directly
and leaves the host's pacman configuration alone.

**Pinned inputs**, all in `build/build.conf`:

| Input | Pinned by |
| --- | --- |
| Packages in the ISO and in the builder | `SNAPSHOT_DATE`: an [Arch Linux Archive](https://archive.archlinux.org/) snapshot |
| Builder image | `BUILDER_IMAGE` digest |
| archiso | `ARCHISO_VERSION`, installed from the snapshot and checked |
| File times, ISO UUID | `SOURCE_DATE_EPOCH` from the last commit's timestamp |
| ISO version | `VERSION`, passed to `profiledef.sh` (which fails without it) |
| Rust toolchain (from Phase 3, when `sanctum` joins the ISO) | `rust-toolchain.toml` |

The build `pacman.conf` points only at the snapshot. The snapshot date is
written in one place and substituted when the profile is staged. The build
fails if any placeholder is left.

Before the long build starts, every package in `build/packages/*.list` is resolved
against the snapshot, so typos and packages Arch has dropped fail quickly.

## Consequences

- "Updating the base" means changing `SNAPSHOT_DATE` (and usually the image
  digest) in a reviewed commit, then building and boot-testing.
- The Arch Linux Archive is rate-limited and not meant for heavy use. Local
  builds keep a package cache in `out/cache/pacman`. CI must cache it too,
  keyed by the snapshot date (Phase 2).
- The pinned inputs make builds **repeatable**. Bit-for-bit reproducibility
  is not yet claimed. It will be measured by comparing independent builds in
  Phase 8.
- The ClamAV signature databases (if they are ever bundled) change daily, and
  will need to be recorded as a separate input.

# ADR-0003: Package inclusion policy

- **Status:** Accepted
- **Date:** 2026-10-02

## Context

A recovery environment can easily turn into a pile of every tool that
exists. That makes the ISO bigger, gives it more to keep updated, and makes
it harder for a technician to find the right tool. Packages from outside
Arch's official repositories also bypass Arch's signing and maintenance.

## Decision

1. **Official repositories only** (`core` and `extra`). A package from
   anywhere else (AUR, a third-party repository, a vendored binary) needs its
   own ADR. The ADR has to say why no official package will do, who
   maintains the package, and how its integrity is checked.
2. **Every package has a written reason.** Packages are listed
   per category in `build/packages/<category>.list` with a `# reason`
   comment on each line. The build combines them into `packages.x86_64`. CI
   rejects any of these:
   - a missing reason,
   - a duplicate,
   - a package that is not in the pinned snapshot,
   - a package from outside `core`/`extra`.
3. **Prefer one good tool per job.** Add a second tool for the same job only
   if it covers cases the first cannot.
4. **Destructive tools are labelled.** Each tool's catalog entry says whether
   it is read-only, modifies data, or is destructive (Phase 3).
5. **Size is a cost.** The size budget in `build/build.conf` is a hard limit,
   and any large addition has to justify its size.

`scripts/ci/check-package-lists.sh` checks the format and duplicates
without a container. The build also resolves every package against the
snapshot, which proves it exists in `core` or `extra` (the only repositories
in `build/profile/pacman.conf`). A `build/profile/packages.x86_64` committed
by mistake fails the build.

## Consequences

- Some well-known tools are left out because they are not in the official
  repos (for example chkrootkit, dc3dd, hashdeep). The gap analysis, section
  5.5, lists what replaces each one.
- Adding a package takes a small amount of paperwork. That is intentional.

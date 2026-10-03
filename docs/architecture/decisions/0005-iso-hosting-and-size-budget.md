# ADR-0005: ISO hosting outside GitHub and a 3 GiB size budget

- **Status:** Accepted
- **Date:** 2026-10-02

## Context

GitHub Release assets must each be under 2 GiB. The Phase 0 ISO, which is
still close to releng, is already 1560 MiB. Sanctum's v0.1 toolkit, Xfce,
Firefox and possibly the ClamAV databases (gap analysis, section 2.6) could
push it past 2 GiB.

## Options considered

1. **Stay on GitHub, under 2 GiB.** One host, but a hard ceiling that would
   force tool choices for the wrong reason.
2. **Split the ISO into parts.** Users would have to join the parts before
   writing the ISO to USB. That is a poor experience for a recovery tool.
3. **Host the ISO elsewhere.** SourceForge or the project's own site, with
   the GitHub Release holding the checksum, attestation and a download link.

## Decision

Option 3. The ISO is downloaded from SourceForge or the project site. Each
GitHub Release holds the `.sha256` file, the source manifest and a download
link. Build provenance attestations are stored by GitHub against the ISO's
digest, so `gh attestation verify` works on a copy downloaded from anywhere.

The size budget in `build/build.conf` becomes: **warn at 2560 MiB, fail at
3072 MiB**. This is a budget, not a technical limit. ISO 9660 and USB sticks
have no such limit. The budget exists because size costs:

- **download time**, often on a poor connection next to a broken machine,
- **RAM** when booting with `copytoram`,
- **build time and CI cache space.**

The limit can be raised in a reviewed commit when there is a real need.

## Consequences

- Publishing a release involves a second host. Phase 4 decides between two
  approaches:
  - CI uploads the ISO with a narrowly scoped credential (SourceForge
    accepts SSH keys for its file release system), or
  - the maintainer downloads the CI-built ISO, verifies its attestation, and
    uploads it by hand. This needs no stored secret.
- Because the checksum comes from GitHub and the ISO from another host, a
  matching checksum confirms the download host served the right file. The
  attestation additionally ties the file to the commit and workflow run that
  built it.
- The size budget still matters. Phase 1 trims releng packages Sanctum does
  not need, so the tools that matter have room.

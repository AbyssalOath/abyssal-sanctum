# Security Policy

## Supported versions

Only the latest release is supported. Arch Linux is a rolling
distribution, so old ISOs contain old packages: always use the newest
release.

## Reporting a vulnerability

Please report vulnerabilities privately through GitHub's
[private vulnerability reporting](https://github.com/AbyssalOath/abyssal-sanctum/security/advisories/new).
Do not open a public issue.

Include the Sanctum version (`sanctum version` or the ISO file name), what
you found, and how to reproduce it. You can expect an acknowledgement within
a week.

Vulnerabilities in Arch Linux packages themselves should go to the
[Arch Linux security team](https://security.archlinux.org/). Sanctum picks up
their fixes when it moves to a newer package snapshot.

## Scope

In scope:

- Sanctum's own scripts, configuration, CLI and build pipeline.
- Insecure defaults in the ISO, such as services exposed without the user
  asking, or anything that writes to a disk without an explicit action.
- Weaknesses in how releases are built, checksummed or published.

## Verifying downloads

ISOs are attached to the GitHub Release for each version, or, if larger
than GitHub allows, downloaded from SourceForge or the project site with the
`.sha256` file and the download link on the GitHub Release (ADR-0005). When
the ISO comes from another host, a matching checksum shows that host served
the right file.

From v0.2.0, releases also include an ISO signed for Secure Boot by the
maintainer (ADR-0011). Its certificate's fingerprints are in the release
notes; check them in MokManager before enrolling the key on a machine. A
report about the Secure Boot signing key or its use is a security issue:
report it as described above.

Releases carry GitHub build provenance attestations, which tie the ISO to
the release workflow run and the tagged commit that built it, wherever it
was downloaded from (ADR-0009):

```bash
gh attestation verify abyssal-sanctum-vX.Y.Z-x86_64.iso --repo AbyssalOath/abyssal-sanctum
```

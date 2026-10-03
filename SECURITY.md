# Security Policy

## Supported versions

Abyssal Sanctum has not had a release yet. Once it has, only the latest
release is supported. Arch Linux is a rolling distribution, so old ISOs
contain old packages: always use the newest release.

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

ISOs are downloaded from SourceForge or the project site, not from GitHub.
The GitHub Release for each version holds the `.sha256` file and the download
link (ADR-0005). Because the checksum comes from GitHub and the ISO from
somewhere else, a match shows the download host served the right file.

Releases carry GitHub build provenance attestations, which tie the ISO to
the release workflow run and the tagged commit that built it, wherever it
was downloaded from (ADR-0009):

```bash
gh attestation verify abyssal-sanctum-vX.Y.Z-x86_64.iso --repo AbyssalOath/abyssal-sanctum
```

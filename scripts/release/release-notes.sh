#!/usr/bin/env bash
# Print the GitHub release notes for VERSION: its CHANGELOG.md section, then
# how to download and verify the ISO.
#
# Usage: scripts/release/release-notes.sh X.Y.Z

set -Eeuo pipefail

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
# shellcheck source=../build/lib.sh
source "${repo_root}/scripts/build/lib.sh"

[[ $# -eq 1 ]] || die "usage: $0 X.Y.Z"
version=$1
iso="abyssal-sanctum-v${version}-x86_64.iso"

section=$(awk -v v="${version}" '
	index($0, "## [" v "]") == 1 { on = 1; next }
	/^## \[/ { on = 0 }
	on' "${repo_root}/CHANGELOG.md")
[[ -n ${section//[[:space:]]/} ]] || die "CHANGELOG.md has no [${version}] section"

cat <<EOT
Abyssal Sanctum v${version}: Arch Linux-based System Recovery Environment.

## Download

The ISOs are attached to this release. An ISO over GitHub's 2 GiB limit is
on SourceForge instead: https://sourceforge.net/projects/abyssal-sanctum/files/v${version}/
(ADR-0005).

- **${iso%.iso}-secureboot.iso**: signed for Secure Boot; boots with Secure
  Boot on (after a one-time key enrolment, see
  docs/getting-started/secure-boot.md) or off. Recommended.
- **${iso}**: the same system unsigned, as built and attested by the release
  workflow. Needs Secure Boot off.

Secure Boot certificate (compare the SHA-1 in MokManager before enrolling):

- SHA-1: FINGERPRINT (from ${iso%.iso}-secureboot.txt)
- SHA-256: FINGERPRINT

The maintainer removes the Secure Boot lines for a release that is not
signed.

Release assets here:

- \`${iso}.sha256\`, \`${iso%.iso}-secureboot.iso.sha256\`: checksums
- \`${iso%.iso}-secureboot.txt\`: what was signed, and the unsigned ISO it came from
- \`abyssal-sanctum-v${version}-x86_64.sources.txt\`: where to get the source of every package (GPL)
- \`abyssal-sanctum-v${version}-x86_64.manifest.json\`: every package and version in the ISO

## Sources

Corresponding source for every package in the ISO (GPL):
https://sourceforge.net/projects/abyssal-sanctum/files/sources/v${version}/
with \`SHA256SUMS\`, and \`SOURCE-NOTES.txt\` for any package whose
source needed special handling. Sanctum's own source is tag v${version} in
this repository.

## Verify

\`\`\`bash
sha256sum -c ${iso}.sha256
gh attestation verify ${iso} --repo AbyssalOath/abyssal-sanctum
\`\`\`

The attestation shows the unsigned ISO was built by this repository's
release workflow from tag v${version}. The signed ISO is made from it by the
maintainer (ADR-0011); \`${iso%.iso}-secureboot.txt\` records the unsigned
ISO's SHA-256. See docs/getting-started/writing-to-usb.md.

## Changes
${section}
EOT

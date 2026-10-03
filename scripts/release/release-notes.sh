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

The ISO is hosted outside GitHub (ADR-0005):

- **${iso}**: DOWNLOAD LINK (the maintainer adds this before publishing)

Release assets here:

- \`${iso}.sha256\`: checksum
- \`abyssal-sanctum-v${version}-x86_64.sources.txt\`: where to get the source of every package (GPL)
- \`abyssal-sanctum-v${version}-x86_64.manifest.json\`: every package and version in the ISO

## Verify

\`\`\`bash
sha256sum -c ${iso}.sha256
gh attestation verify ${iso} --repo AbyssalOath/abyssal-sanctum
\`\`\`

The attestation shows the ISO was built by this repository's release
workflow from tag v${version}. See docs/getting-started/writing-to-usb.md.

## Changes
${section}
EOT

#!/usr/bin/env bash
# Prepare a release: set the version everywhere and date the changelog.
# Nothing is committed or tagged; the script prints the next steps.
#
# Usage: scripts/release/prepare.sh X.Y.Z

set -Eeuo pipefail

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
# shellcheck source=../build/lib.sh
source "${repo_root}/scripts/build/lib.sh"
cd -- "${repo_root}"

[[ $# -eq 1 && $1 =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || die "usage: $0 X.Y.Z"
new=$1
old=$(read_version "${repo_root}")
today=$(date -u +%Y-%m-%d)

grep -q '^## \[Unreleased\]$' CHANGELOG.md || die "CHANGELOG.md has no '## [Unreleased]' section"
! grep -q "^## \[${new//./\\.}\]" CHANGELOG.md || die "CHANGELOG.md already has a [${new}] section"
# The Unreleased section must say what is in the release.
notes=$(awk '/^## \[Unreleased\]$/ { on = 1; next } /^## \[/ { on = 0 } on && /[^[:space:]]/' CHANGELOG.md)
[[ -n ${notes} ]] || die "the [Unreleased] section of CHANGELOG.md is empty"

msg "Setting version ${old} -> ${new}"
printf '%s\n' "${new}" >VERSION
sed -i "/^\[workspace.package\]/,/^\[/ s/^version = \".*\"/version = \"${new}\"/" Cargo.toml
sed -i "/^name = \"sanctum-cli\"$/{n;s/^version = \".*\"/version = \"${new}\"/}" Cargo.lock
sed -i "s/^## \[Unreleased\]$/## [Unreleased]\n\n## [${new}] - ${today}/" CHANGELOG.md

"${repo_root}/scripts/ci/check-version.sh"

cat <<EOT

Next steps (docs/operations/releasing.md):
  1. Review: git diff
  2. Commit through a pull request to main, so CI and the boot tests run.
  3. Work through docs/operations/release-checklist.md on real hardware.
  4. Tag the merged commit:  git tag -s v${new} -m "Abyssal Sanctum v${new}" && git push origin v${new}
EOT

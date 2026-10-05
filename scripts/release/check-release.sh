#!/usr/bin/env bash
# Check that a tag can be released: it is v<VERSION>, every version field
# agrees, CHANGELOG.md has a dated section for it, and (with --on-main) the
# tagged commit is on origin/main.
#
# Usage: scripts/release/check-release.sh TAG [--on-main]

set -Eeuo pipefail

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
# shellcheck source=../build/lib.sh
source "${repo_root}/scripts/build/lib.sh"
cd -- "${repo_root}"

[[ $# -ge 1 ]] || die "usage: $0 TAG [--on-main]"
tag=$1
on_main=0
[[ ${2:-} == --on-main ]] && on_main=1

version=$(read_version "${repo_root}")
[[ ${tag} == "v${version}" ]] || die "tag ${tag} does not match VERSION ${version} (expected v${version})"

GITHUB_REF_TYPE=tag GITHUB_REF_NAME=${tag} "${repo_root}/scripts/ci/check-version.sh"

heading=$(grep -m1 -E "^## \[${version//./\\.}\]" CHANGELOG.md || true)
[[ ${heading} =~ ^##\ \[${version//./\\.}\]\ -\ [0-9]{4}-[0-9]{2}-[0-9]{2}$ ]] ||
	die "CHANGELOG.md needs a '## [${version}] - YYYY-MM-DD' section (scripts/release/prepare.sh adds it)"

if ((on_main)); then
	# CI checks out with full history (fetch-depth: 0), so origin/main is
	# already present, and the checkout keeps no credentials to fetch with.
	# Fetch only when the ref is missing, as in some local clones.
	git rev-parse --verify --quiet origin/main >/dev/null || git fetch --quiet origin main
	git merge-base --is-ancestor HEAD origin/main ||
		die "the tagged commit is not on main; release tags must point to merged commits"
fi
msg "Tag ${tag} is releasable"

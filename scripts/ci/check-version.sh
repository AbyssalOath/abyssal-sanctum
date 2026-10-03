#!/usr/bin/env bash
# Check that every place holding the project version agrees with VERSION.
# On a tag build (GITHUB_REF_TYPE=tag), the tag must be v<VERSION> too.

set -Eeuo pipefail

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
# shellcheck source=../build/lib.sh
source "${repo_root}/scripts/build/lib.sh"
version=$(read_version "${repo_root}")
load_build_conf "${repo_root}"
errors=0
error() {
	printf '==> ERROR: %s\n' "$*" >&2
	errors=$((errors + 1))
}

cargo_version=$(sed -n '/^\[workspace.package\]/,/^\[/s/^version = "\(.*\)"/\1/p' "${repo_root}/Cargo.toml")
[[ ${cargo_version} == "${version}" ]] ||
	error "Cargo.toml workspace version is '${cargo_version}', VERSION is '${version}'"

lock_version=$(sed -n '/^name = "sanctum-cli"$/{n;s/^version = "\(.*\)"/\1/p}' "${repo_root}/Cargo.lock")
[[ ${lock_version} == "${version}" ]] ||
	error "Cargo.lock has sanctum-cli '${lock_version}', VERSION is '${version}' (run cargo update -w)"

# shellcheck disable=SC2016 # a literal ${...} is what we look for
grep -qF 'iso_version="v${SANCTUM_VERSION' "${repo_root}/build/profile/profiledef.sh" ||
	error "profiledef.sh must take iso_version from SANCTUM_VERSION"

grep -qE "^## \[(Unreleased|${version//./\\.})\]" "${repo_root}/CHANGELOG.md" ||
	error "CHANGELOG.md has neither an [Unreleased] nor a [${version}] section"

# The workflows must use the same builder image as the build.
while IFS= read -r image; do
	[[ ${image} == "${BUILDER_IMAGE}" ]] ||
		error "a workflow uses ${image}, build.conf pins ${BUILDER_IMAGE}"
done < <(grep -rhoE 'docker\.io/library/archlinux@sha256:[0-9a-f]{64}' "${repo_root}/.github" 2>/dev/null || true)

if [[ ${GITHUB_REF_TYPE:-} == tag ]]; then
	[[ ${GITHUB_REF_NAME:-} == "v${version}" ]] ||
		error "tag ${GITHUB_REF_NAME:-} does not match VERSION (expected v${version})"
fi

((errors == 0)) || die "version check failed (${errors} errors)"
msg "Version ${version} is consistent"

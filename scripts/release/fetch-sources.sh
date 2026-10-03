#!/usr/bin/env bash
# Collect the corresponding source of every package in a release's source
# manifest (*.sources.txt), to publish next to the ISO (ADR-0009).
#
# For each package base, the Arch source tarball is downloaded if Arch's
# source server still has it. Otherwise the package's Arch packaging
# repository is cloned at the release's version tag, and `makepkg
# --allsource` builds the source tarball: the PKGBUILD plus every upstream
# source, checked against the PKGBUILD's checksums. Upstream PGP signatures
# are not checked (that needs every upstream key); the checksums are.
#
# Runs in the pinned builder container (build/build.conf). Files already in
# OUT_DIR are kept, so an interrupted run can be resumed. Expect several GB
# and a few hours for a full release.
#
# Usage: scripts/release/fetch-sources.sh SOURCES.txt OUT_DIR [PKGBASE...]
#
# With PKGBASE arguments, only those package bases are collected.
# Failures are listed in OUT_DIR/MISSING.txt.

set -Eeuo pipefail

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
# shellcheck source=../build/lib.sh
source "${repo_root}/scripts/build/lib.sh"
load_build_conf "${repo_root}"

[[ $# -ge 2 && -f $1 ]] || die "usage: $0 SOURCES.txt OUT_DIR [PKGBASE...]"
manifest=$(cd -- "$(dirname -- "$1")" && pwd)/$(basename -- "$1")
mkdir -p -- "$2"
out=$(cd -- "$2" && pwd)
shift 2

engine=${SANCTUM_ENGINE:-docker}
cache_dir="${repo_root}/out/cache/pacman"
mkdir -p -- "${cache_dir}"

msg "Collecting sources into ${out}"
"${engine}" run --rm \
	--security-opt label=disable \
	-v "${repo_root}:/src:ro" \
	-v "${manifest}:/sources.txt:ro" \
	-v "${out}:/out" \
	-v "${cache_dir}:/var/cache/pacman/pkg" \
	-e SNAPSHOT_DATE="${SNAPSHOT_DATE}" \
	-e HOST_UID="$(id -u)" \
	-e HOST_GID="$(id -g)" \
	"${BUILDER_IMAGE}" \
	/src/scripts/release/sources-in-container.sh "$@"

#!/usr/bin/env bash
# Run a command inside the pinned builder image (build/build.conf), with
# extra packages installed from the same Arch Linux Archive snapshot. CI uses
# this so lint tools have the same versions everywhere.
#
# Usage: scripts/ci/in-builder.sh [PACKAGE...] -- COMMAND [ARG...]
# Example: scripts/ci/in-builder.sh shellcheck shfmt -- scripts/ci/lint-shell.sh
#
# The repository is mounted read-only at /src, which is the working
# directory. Needs docker (or set SANCTUM_ENGINE=podman).

set -Eeuo pipefail

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
# shellcheck source=../build/lib.sh
source "${repo_root}/scripts/build/lib.sh"
load_build_conf "${repo_root}"

packages=()
while (($#)) && [[ $1 != -- ]]; do
	packages+=("$1")
	shift
done
[[ ${1:-} == -- && $# -ge 2 ]] || die "usage: $0 [PACKAGE...] -- COMMAND [ARG...]"
shift

engine=${SANCTUM_ENGINE:-docker}
cache_dir="${repo_root}/out/cache/pacman"
mkdir -p -- "${cache_dir}"

# The setup script receives the snapshot date and packages as arguments, so
# nothing from the command line is interpreted by the outer shell.
# shellcheck disable=SC2016 # expanded inside the container
setup='
set -euo pipefail
printf "Server = https://archive.archlinux.org/repos/%s/\$repo/os/\$arch\n" "$1" >/etc/pacman.d/mirrorlist
shift
n=$1
shift
if ((n > 0)); then
	pacman -Syyuu --noconfirm --needed "${@:1:n}" >/dev/null
fi
shift "${n}"
exec "$@"
'
"${engine}" run --rm \
	--security-opt label=disable \
	-v "${repo_root}:/src:ro" \
	-v "${cache_dir}:/var/cache/pacman/pkg" \
	-w /src \
	"${BUILDER_IMAGE}" \
	bash -c "${setup}" bash "${SNAPSHOT_DATE}" "${#packages[@]}" "${packages[@]}" "$@"

#!/usr/bin/env bash
# Lint every shell script in the repository with shellcheck and check its
# formatting with shfmt. Both are mandatory (CONTRIBUTING.md).
#
# Scripts copied unchanged from archiso's releng profile are skipped, so
# they stay identical to upstream and can be diffed on archiso updates
# (ADR-0001).

set -Eeuo pipefail

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
cd -- "${repo_root}"

upstream=(
	build/profile/airootfs/usr/local/bin/livecd-sound
)

# shfmt -f finds shell scripts by extension and shebang. Only source
# directories are searched (out/ and target/ hold build output).
mapfile -t found < <(shfmt -f build scripts tests crates | sort)
files=()
for f in "${found[@]}"; do
	[[ " ${upstream[*]} " == *" ${f} "* ]] || files+=("${f}")
done
# Sourced Bash files without a shebang or .sh extension.
files+=(build/build.conf)

printf '==> shellcheck (%s): %d files\n' "$(shellcheck --version | sed -n 's/^version: //p')" "${#files[@]}"
shellcheck -x "${files[@]}"
printf '==> shfmt (%s)\n' "$(shfmt --version)"
shfmt -d "${files[@]}"
printf '==> Shell lint OK\n'

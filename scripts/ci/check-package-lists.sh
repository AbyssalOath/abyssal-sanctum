#!/usr/bin/env bash
# Check the format of build/packages/*.list (ADR-0003). Whether each package
# exists in the pinned snapshot is checked by the build itself, which needs
# the Arch container.

set -Eeuo pipefail

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
# shellcheck source=../build/lib.sh
source "${repo_root}/scripts/build/lib.sh"

check_package_lists "${repo_root}/build/packages" || die "package lists have errors (see above)"
msg "Package lists OK: $(package_names "${repo_root}/build/packages" | wc -l) packages"

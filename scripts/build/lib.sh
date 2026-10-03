# shellcheck shell=bash
# Shared helpers for the build scripts. Sourced, never executed.

msg() {
	printf '==> %s\n' "$*" >&2
}

warn() {
	printf '==> WARNING: %s\n' "$*" >&2
}

die() {
	printf '==> ERROR: %s\n' "$*" >&2
	exit 1
}

# Read and validate the repository's VERSION file. Prints the version.
read_version() {
	local repo_root=$1 version
	[[ -f "${repo_root}/VERSION" ]] || die "VERSION file not found in ${repo_root}"
	version=$(<"${repo_root}/VERSION")
	[[ "${version}" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] ||
		die "VERSION must be MAJOR.MINOR.PATCH, got '${version}'"
	printf '%s\n' "${version}"
}

# Source build/build.conf and check that every required value is present.
load_build_conf() {
	local repo_root=$1 var
	# shellcheck source=../../build/build.conf
	source "${repo_root}/build/build.conf"
	for var in SNAPSHOT_DATE BUILDER_IMAGE ARCHISO_VERSION ISO_SIZE_WARN_MIB ISO_SIZE_FAIL_MIB; do
		[[ -n "${!var:-}" ]] || die "build/build.conf does not set ${var}"
	done
	[[ "${SNAPSHOT_DATE}" =~ ^[0-9]{4}/[0-9]{2}/[0-9]{2}$ ]] ||
		die "SNAPSHOT_DATE must be YYYY/MM/DD, got '${SNAPSHOT_DATE}'"
	[[ "${BUILDER_IMAGE}" == *@sha256:* ]] ||
		die "BUILDER_IMAGE must be pinned by digest (name@sha256:...)"
}

# Check build/packages/*.list against the format in ADR-0003: each entry is
# "name  # reason", no package appears twice. Prints every problem; returns 1
# if there were any.
check_package_lists() {
	local dir=$1 file line name lineno errors=0
	local -A seen=()
	local -a lists=("${dir}"/*.list)
	[[ -e ${lists[0]} ]] || {
		printf '%s: no package lists found\n' "${dir}" >&2
		return 1
	}
	for file in "${lists[@]}"; do
		lineno=0
		while IFS= read -r line || [[ -n ${line} ]]; do
			lineno=$((lineno + 1))
			[[ ${line} =~ ^[[:space:]]*(#.*)?$ ]] && continue
			if [[ ! ${line} =~ ^([a-z0-9@._+-]+)[[:space:]]+#.*[^[:space:]] ]]; then
				printf '%s:%d: expected "package  # reason": %s\n' "${file}" "${lineno}" "${line}" >&2
				errors=$((errors + 1))
				continue
			fi
			name=${BASH_REMATCH[1]}
			if [[ -n ${seen[${name}]:-} ]]; then
				printf '%s:%d: %s is already listed in %s\n' "${file}" "${lineno}" "${name}" "${seen[${name}]}" >&2
				errors=$((errors + 1))
			fi
			seen[${name}]="${file##*/}"
		done <"${file}"
	done
	((errors == 0))
}

# Print the package names from build/packages/*.list, one per line, in file
# order. Assumes check_package_lists passed.
package_names() {
	sed -E '/^[[:space:]]*(#.*)?$/d;s/[[:space:]]*#.*$//;s/^[[:space:]]+//' "$1"/*.list
}

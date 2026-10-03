#!/usr/bin/env bash
# Build the Abyssal Sanctum ISO.
#
# This is the host-side entry point. It runs scripts/build/in-container.sh
# inside the pinned Arch Linux builder image (build/build.conf), so the build
# does not depend on the host distribution or its configuration. On an Arch
# host, --native runs the same script directly instead.
#
# See docs/getting-started/building.md.

set -Eeuo pipefail

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
# shellcheck source=lib.sh
source "${repo_root}/scripts/build/lib.sh"

usage() {
	cat <<'EOF'
Usage: scripts/build/build-iso.sh [options]

Build the Abyssal Sanctum ISO and its SHA-256 checksum.

Options:
  -o, --out DIR        Output directory (default: <repo>/out)
  -e, --engine NAME    Container engine: docker or podman (default: docker
                       if usable, otherwise podman when run as root)
      --native         Build on this Arch host without a container
                       (requires root and archiso installed)
  -h, --help           Show this help

Environment:
  SOURCE_DATE_EPOCH    Build timestamp. Defaults to the last commit's time.
EOF
}

out_dir="${repo_root}/out"
engine=""
native=0

while (($#)); do
	case $1 in
	-o | --out)
		[[ $# -ge 2 ]] || die "$1 needs a directory"
		out_dir=$2
		shift 2
		;;
	-e | --engine)
		[[ $# -ge 2 ]] || die "$1 needs an engine name"
		engine=$2
		shift 2
		;;
	--native)
		native=1
		shift
		;;
	-h | --help)
		usage
		exit 0
		;;
	*)
		usage >&2
		die "unknown option: $1"
		;;
	esac
done

version=$(read_version "${repo_root}")
load_build_conf "${repo_root}"
iso_file="abyssal-sanctum-v${version}-x86_64.iso"

# The build timestamp drives every file time and the ISO UUID inside the
# image, so it must come from the source, not the clock (ADR-0002).
if [[ -z "${SOURCE_DATE_EPOCH:-}" ]]; then
	if git -C "${repo_root}" rev-parse --verify -q HEAD >/dev/null 2>&1; then
		SOURCE_DATE_EPOCH=$(git -C "${repo_root}" log -1 --format=%ct)
	else
		warn "no git commit yet: using the current time, so this build is not reproducible"
		SOURCE_DATE_EPOCH=$(date +%s)
	fi
fi
export SOURCE_DATE_EPOCH
# Recorded in os-release (BUILD_ID) and the manifest.
SANCTUM_GIT_COMMIT=none
if git -C "${repo_root}" rev-parse --verify -q HEAD >/dev/null 2>&1; then
	SANCTUM_GIT_COMMIT=$(git -C "${repo_root}" rev-parse --short=12 HEAD)
fi
if [[ -n "$(git -C "${repo_root}" status --porcelain 2>/dev/null)" ]]; then
	warn "uncommitted changes are included in this build"
	[[ ${SANCTUM_GIT_COMMIT} == none ]] || SANCTUM_GIT_COMMIT+="-dirty"
fi
export SANCTUM_GIT_COMMIT

mkdir -p -- "${out_dir}"
out_dir=$(cd -- "${out_dir}" && pwd)
log_file="${out_dir}/${iso_file%.iso}.build.log"

# Who should own the results: the invoking user, also under sudo.
host_uid=${SUDO_UID:-$(id -u)}
host_gid=${SUDO_GID:-$(id -g)}

if ((native)); then
	((EUID == 0)) || die "--native must run as root (mkarchiso needs it)"
	[[ -f /etc/arch-release ]] || die "--native only works on Arch Linux; use the container build"
	msg "Building ${iso_file} natively (log: ${log_file})"
	SANCTUM_SRC="${repo_root}" SANCTUM_OUT="${out_dir}" SANCTUM_GIT_COMMIT="${SANCTUM_GIT_COMMIT}" \
		HOST_UID="${host_uid}" HOST_GID="${host_gid}" \
		"${repo_root}/scripts/build/in-container.sh" 2>&1 | tee -- "${log_file}"
else
	if [[ -z "${engine}" ]]; then
		if command -v docker >/dev/null && docker info >/dev/null 2>&1; then
			engine=docker
		elif command -v podman >/dev/null && ((EUID == 0)); then
			engine=podman
		else
			die "no usable container engine: need docker (with access to its daemon) or root podman (sudo)"
		fi
	fi
	case ${engine} in
	docker | podman) command -v "${engine}" >/dev/null || die "${engine} is not installed" ;;
	*) die "unsupported engine '${engine}' (use docker or podman)" ;;
	esac
	# mkarchiso mounts filesystems and creates device nodes, which rootless
	# podman cannot do even with --privileged.
	if [[ ${engine} == podman ]] && [[ "$(podman info --format '{{.Host.Security.Rootless}}')" == true ]]; then
		die "rootless podman cannot run mkarchiso; use 'sudo $0' or docker"
	fi

	cache_dir="${out_dir}/cache/pacman"
	mkdir -p -- "${cache_dir}"

	msg "Building ${iso_file} with ${engine} (log: ${log_file})"
	# --privileged: mkarchiso needs mounts and namespaces.
	# label=disable: no SELinux relabelling of the source tree on Fedora.
	# The source is mounted read-only; the build only writes to /out.
	"${engine}" run --rm --privileged \
		--security-opt label=disable \
		-v "${repo_root}:/src:ro" \
		-v "${out_dir}:/out" \
		-v "${cache_dir}:/var/cache/pacman/pkg" \
		-e SANCTUM_IN_CONTAINER=1 \
		-e SOURCE_DATE_EPOCH \
		-e SANCTUM_GIT_COMMIT \
		-e HOST_UID="${host_uid}" \
		-e HOST_GID="${host_gid}" \
		"${BUILDER_IMAGE}" \
		/src/scripts/build/in-container.sh 2>&1 | tee -- "${log_file}"
fi

[[ -f "${out_dir}/${iso_file}" && -f "${out_dir}/${iso_file}.sha256" ]] ||
	die "build finished but ${iso_file} or its checksum is missing"
chown "${host_uid}:${host_gid}" -- "${log_file}" 2>/dev/null || true

msg "ISO:      ${out_dir}/${iso_file}"
msg "SHA-256:  ${out_dir}/${iso_file}.sha256"

#!/usr/bin/env bash
# Make a Secure Boot capable copy of a built Abyssal Sanctum ISO (ADR-0011).
#
# Run by the maintainer at release time, on the ISO that CI built and
# attested. The signing key never enters CI. The result boots:
#
#   firmware -> shim (Fedora's, signed by Microsoft)
#            -> systemd-boot (signed with the Sanctum key)
#            -> linux-hardened, Memtest86+ (signed with the Sanctum key)
#
# and carries sanctum.cer for technicians to enrol once per machine in
# MokManager. Nothing else in the ISO changes: the same files, boot layout
# and volume UUID, so the result also boots with Secure Boot off. Signing
# the same ISO with the same key twice gives the same file.
#
# Runs in the pinned builder container (build/build.conf). The key file is
# mounted read-only; when it is passphrase-protected, it is decrypted once
# into memory inside the container and deleted when the container exits.
#
# Usage:
#   scripts/release/sign-secureboot.sh --key KEY --cert CERT [--out DIR] ISO
#
# KEY and CERT come from scripts/release/make-secureboot-key.sh. Writes, to
# DIR (default: the ISO's folder), for abyssal-sanctum-vX.Y.Z-x86_64.iso:
#   abyssal-sanctum-vX.Y.Z-x86_64-secureboot.iso         the signed ISO
#   abyssal-sanctum-vX.Y.Z-x86_64-secureboot.iso.sha256  its checksum
#   abyssal-sanctum-vX.Y.Z-x86_64-secureboot.txt         what was signed, with
#                                                        the certificate's fingerprints

set -Eeuo pipefail

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
# shellcheck source=../build/lib.sh
source "${repo_root}/scripts/build/lib.sh"
load_build_conf "${repo_root}"
# shellcheck source=../../build/secureboot.conf
source "${repo_root}/build/secureboot.conf"

usage() {
	sed -n 's/^# \{0,1\}//; /^Usage:/,/^$/p' "${BASH_SOURCE[0]}" | sed '$d'
}

key="" cert="" out="" iso=""
while (($#)); do
	case $1 in
	--key)
		[[ $# -ge 2 ]] || die "--key needs a file"
		key=$2
		shift
		;;
	--cert)
		[[ $# -ge 2 ]] || die "--cert needs a file"
		cert=$2
		shift
		;;
	--out)
		[[ $# -ge 2 ]] || die "--out needs a directory"
		out=$2
		shift
		;;
	-h | --help)
		usage
		exit 0
		;;
	-*) die "unknown option: $1" ;;
	*)
		[[ -z ${iso} ]] || die "only one ISO at a time"
		iso=$1
		;;
	esac
	shift
done
[[ -n ${key} && -n ${cert} && -n ${iso} ]] || {
	usage >&2
	die "--key, --cert and an ISO are required"
}
for f in "${key}" "${cert}" "${iso}"; do
	[[ -f ${f} ]] || die "not found: ${f}"
done
command -v curl >/dev/null || die "missing dependency: curl"
command -v sha256sum >/dev/null || die "missing dependency: sha256sum"

abs() { printf '%s/%s\n' "$(cd -- "$(dirname -- "$1")" && pwd)" "$(basename -- "$1")"; }
key=$(abs "${key}")
cert=$(abs "${cert}")
iso=$(abs "${iso}")
[[ ${iso} == *.iso && ${iso} != *-secureboot.iso ]] || die "expected an unsigned *.iso, got ${iso}"
[[ -n ${out} ]] || out=$(dirname -- "${iso}")
mkdir -p -- "${out}"
out=$(cd -- "${out}" && pwd)
out_name="$(basename -- "${iso%.iso}")-secureboot.iso"

# The shim package, cached and checked against its pinned hash.
shim_cache="${repo_root}/out/cache/shim"
shim_rpm="${shim_cache}/${SHIM_RPM_URL##*/}"
mkdir -p -- "${shim_cache}"
if [[ ! -f ${shim_rpm} ]]; then
	msg "Downloading ${SHIM_RPM_URL##*/}"
	curl -fsSL --proto '=https' --tlsv1.2 -o "${shim_rpm}.part" -- "${SHIM_RPM_URL}"
	mv -- "${shim_rpm}.part" "${shim_rpm}"
fi
printf '%s  %s\n' "${SHIM_RPM_SHA256}" "${shim_rpm}" | sha256sum --check --quiet ||
	die "${shim_rpm} does not match SHIM_RPM_SHA256 in build/secureboot.conf"

cache_dir="${repo_root}/out/cache/pacman"
mkdir -p -- "${cache_dir}"
tty=()
[[ -t 0 ]] && tty=(-t)

engine=${SANCTUM_ENGINE:-docker}
msg "Signing $(basename -- "${iso}") into ${out}"
"${engine}" run --rm -i "${tty[@]}" \
	--security-opt label=disable \
	-v "${repo_root}:/src:ro" \
	-v "$(dirname -- "${iso}"):/in:ro" \
	-v "${key}:/keys/sanctum-sb.key:ro" \
	-v "${cert}:/keys/sanctum-sb.crt:ro" \
	-v "${shim_rpm}:/shim/shim.rpm:ro" \
	-v "${out}:/out" \
	-v "${cache_dir}:/var/cache/pacman/pkg" \
	-e ISO_NAME="$(basename -- "${iso}")" \
	-e OUT_NAME="${out_name}" \
	-e HOST_UID="$(id -u)" \
	-e HOST_GID="$(id -g)" \
	"${BUILDER_IMAGE}" \
	/src/scripts/release/sign-secureboot-in-container.sh
msg "Signed ISO: ${out}/${out_name}"

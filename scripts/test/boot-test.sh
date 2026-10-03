#!/usr/bin/env bash
# Boot-test an Abyssal Sanctum ISO in QEMU.
#
# The ISO boots with sanctum-selftest enabled and the disk-safety fixtures
# (an md RAID1 pair and an LVM volume group, tests/fixtures/disk-safety)
# attached as writable disks. The test passes when:
#   1. the self-test prints "SANCTUM-SELFTEST: PASS" on the serial console,
#      which includes "no RAID arrays assembled", "no LVM ... active" and
#      "no disks mounted" with those disks present, and
#   2. the guest powers off within the time limit, and
#   3. every fixture is byte-for-byte unchanged afterwards (ADR-0006).

set -Eeuo pipefail

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
# shellcheck source=../build/lib.sh
source "${repo_root}/scripts/build/lib.sh"

usage() {
	cat <<'EOF'
Usage: scripts/test/boot-test.sh [options] [ISO]

Boot ISO (default: out/abyssal-sanctum-v<VERSION>-x86_64.iso) with the
self-test and the disk-safety fixtures, and check the results.

Options:
  --uefi             Boot with OVMF firmware (default)
  --bios             Boot with legacy BIOS (SeaBIOS)
  --timeout SECONDS  Give up after this long (default: 300)
  --log FILE         Serial log path (default: out/boot-test-<firmware>.log)
  -h, --help         Show this help
EOF
}

firmware=uefi
timeout_s=300
log=""
iso=""

while (($#)); do
	case $1 in
	--uefi) firmware=uefi ;;
	--bios) firmware=bios ;;
	--timeout)
		[[ $# -ge 2 && $2 =~ ^[0-9]+$ ]] || die "--timeout needs a number of seconds"
		timeout_s=$2
		shift
		;;
	--log)
		[[ $# -ge 2 ]] || die "--log needs a file path"
		log=$2
		shift
		;;
	-h | --help)
		usage
		exit 0
		;;
	-*)
		usage >&2
		die "unknown option: $1"
		;;
	*) iso=$1 ;;
	esac
	shift
done

[[ -n ${iso} ]] || iso="${repo_root}/out/abyssal-sanctum-v$(read_version "${repo_root}")-x86_64.iso"
[[ -f ${iso} ]] || die "ISO not found: ${iso}"
[[ -n ${log} ]] || log="${repo_root}/out/boot-test-${firmware}.log"
mkdir -p -- "$(dirname -- "${log}")"
for tool in xz sha256sum timeout; do
	command -v "${tool}" >/dev/null || die "missing dependency: ${tool}"
done

fixtures_dir="${repo_root}/tests/fixtures/disk-safety"
tmp=$(mktemp -d)
trap 'rm -rf -- "${tmp}"' EXIT

disk_args=()
for fixture in "${fixtures_dir}"/*.img.xz; do
	image="${tmp}/$(basename -- "${fixture}" .xz)"
	xz -dc -- "${fixture}" >"${image}"
	disk_args+=(--writable-disk "${image}")
done
((${#disk_args[@]})) || die "no fixtures found in ${fixtures_dir}"
(cd -- "${tmp}" && sha256sum -- *.img >before.sha256)

rm -f -- "${log}"
msg "Boot test (${firmware^^}): $(basename -- "${iso}"), up to ${timeout_s}s"
vm_status=0
# Without --foreground, timeout kills the whole process group, QEMU included.
timeout "${timeout_s}" \
	"${repo_root}/scripts/test/run-vm.sh" "--${firmware}" --selftest "${log}" \
	"${disk_args[@]}" "${iso}" >"${tmp}/qemu.out" 2>&1 || vm_status=$?

failed=0
if ((vm_status == 124)); then
	warn "the VM did not power off within ${timeout_s}s"
	failed=1
elif ((vm_status != 0)); then
	warn "QEMU exited with status ${vm_status}:"
	cat -- "${tmp}/qemu.out" >&2
	failed=1
fi

# Self-test output, without the rest of the boot log. The serial console
# carries terminal escape codes, so strip carriage returns first.
results=$(tr -d '\r' <"${log}" 2>/dev/null | grep -aE '^(ok|not ok) - |^SANCTUM-SELFTEST:' || true)
if [[ -n ${results} ]]; then
	printf '%s\n' "${results}"
fi
if ! grep -q '^SANCTUM-SELFTEST: PASS$' <<<"${results}"; then
	warn "the self-test did not pass (full serial log: ${log})"
	failed=1
fi

if ! (cd -- "${tmp}" && sha256sum --quiet -c before.sha256); then
	warn "a disk-safety fixture was modified during the boot"
	failed=1
else
	msg "Disk-safety fixtures unchanged"
fi

((failed == 0)) || die "boot test failed (${firmware^^})"
msg "Boot test passed (${firmware^^})"

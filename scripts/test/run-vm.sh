#!/usr/bin/env bash
# Boot an Abyssal Sanctum ISO in QEMU for manual or scripted testing.
#
# The VM gets no disks unless --disk is given, and any disk image given is
# attached with snapshot=on, so testing never modifies it. --writable-disk
# attaches an image directly, for tests that must see every write (for
# example, checking that Sanctum does not modify disks on its own).

set -Eeuo pipefail

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
# shellcheck source=../build/lib.sh
source "${repo_root}/scripts/build/lib.sh"

usage() {
	cat <<'EOF'
Usage: scripts/test/run-vm.sh [options] [ISO]

Boot ISO (default: out/abyssal-sanctum-v<VERSION>-x86_64.iso) in QEMU.

Options:
  --uefi             Boot with OVMF firmware (default)
  --bios             Boot with legacy BIOS (SeaBIOS)
  --disk IMAGE       Attach a disk image; writes go to a throwaway snapshot
  --writable-disk IMAGE
                     Attach a disk image directly; writes reach the file
  --memory MIB       Guest memory (default: 4096)
  --headless         No window. Serial on stdio, QEMU monitor on
                     <out>/vm-monitor.sock (for screendump and quit)
  --selftest LOG     Run sanctum-selftest at boot (passed as a systemd
                     credential), write the serial console to LOG, no
                     window; QEMU exits when the guest powers off
  -h, --help         Show this help
EOF
}

firmware=uefi
memory=4096
headless=0
selftest_log=""
disks=()
writable_disks=()
iso=""

while (($#)); do
	case $1 in
	--uefi) firmware=uefi ;;
	--bios) firmware=bios ;;
	--disk)
		[[ $# -ge 2 ]] || die "--disk needs an image path"
		[[ -f $2 ]] || die "disk image not found: $2"
		disks+=("$2")
		shift
		;;
	--writable-disk)
		[[ $# -ge 2 ]] || die "--writable-disk needs an image path"
		[[ -f $2 ]] || die "disk image not found: $2"
		writable_disks+=("$2")
		shift
		;;
	--memory)
		[[ $# -ge 2 && $2 =~ ^[0-9]+$ ]] || die "--memory needs a size in MiB"
		memory=$2
		shift
		;;
	--headless) headless=1 ;;
	--selftest)
		[[ $# -ge 2 ]] || die "--selftest needs a log file path"
		selftest_log=$2
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

if [[ -z ${iso} ]]; then
	iso="${repo_root}/out/abyssal-sanctum-v$(read_version "${repo_root}")-x86_64.iso"
fi
[[ -f ${iso} ]] || die "ISO not found: ${iso} (build it with scripts/build/build-iso.sh)"
command -v qemu-system-x86_64 >/dev/null || die "qemu-system-x86_64 is not installed"

# shellcheck disable=SC2054 # commas are QEMU option syntax
args=(
	-machine q35
	-m "${memory}"
	-smp 2
	-device virtio-net-pci,netdev=net0
	-netdev user,id=net0
	-drive "file=${iso},media=cdrom,readonly=on,if=none,id=cd0"
	-device ide-cd,drive=cd0,bootindex=0
)
if [[ -w /dev/kvm ]]; then
	args+=(-accel kvm -cpu host)
else
	warn "/dev/kvm is not writable: falling back to slow software emulation"
fi

for i in "${!disks[@]}"; do
	args+=(-drive "file=${disks[i]},if=virtio,snapshot=on,id=disk${i}")
done
for i in "${!writable_disks[@]}"; do
	args+=(-drive "file=${writable_disks[i]},format=raw,if=virtio,id=wdisk${i}")
done

if [[ ${firmware} == uefi ]]; then
	# OVMF locations on Fedora, Arch and Debian/Ubuntu.
	code="" vars_template=""
	for pair in \
		/usr/share/edk2/ovmf/OVMF_CODE.fd:/usr/share/edk2/ovmf/OVMF_VARS.fd \
		/usr/share/edk2/x64/OVMF_CODE.4m.fd:/usr/share/edk2/x64/OVMF_VARS.4m.fd \
		/usr/share/OVMF/OVMF_CODE_4M.fd:/usr/share/OVMF/OVMF_VARS_4M.fd; do
		if [[ -f ${pair%%:*} && -f ${pair##*:} ]]; then
			code=${pair%%:*}
			vars_template=${pair##*:}
			break
		fi
	done
	[[ -n ${code} ]] || die "OVMF firmware not found (Fedora: dnf install edk2-ovmf)"
	vars=$(mktemp --suffix=.fd)
	trap 'rm -f -- "${vars}"' EXIT
	cp -- "${vars_template}" "${vars}"
	args+=(
		-drive "if=pflash,format=raw,unit=0,readonly=on,file=${code}"
		-drive "if=pflash,format=raw,unit=1,file=${vars}"
	)
fi

if [[ -n ${selftest_log} ]]; then
	# systemd imports SMBIOS type 11 strings as system credentials in VMs;
	# sanctum-selftest.service starts when this one is present.
	args+=(
		-display none
		-serial "file:${selftest_log}"
		-no-reboot
		-smbios "type=11,value=io.systemd.credential:sanctum.selftest=1"
	)
elif ((headless)); then
	monitor="${repo_root}/out/vm-monitor.sock"
	args+=(-display none -serial stdio -monitor "unix:${monitor},server,nowait")
	msg "Monitor socket: ${monitor}"
fi

msg "Booting $(basename -- "${iso}") (${firmware^^}, ${memory} MiB)"
qemu-system-x86_64 "${args[@]}"

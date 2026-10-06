#!/usr/bin/env bash
# The container side of scripts/release/sign-secureboot.sh. Not run directly.
#
# Inputs (mounted by the host script):
#   /in/$ISO_NAME            the unsigned ISO (read-only)
#   /keys/sanctum-sb.key     the signing key (read-only)
#   /keys/sanctum-sb.crt     the signing certificate (read-only)
#   /shim/shim.rpm           Fedora's shim package, hash already checked
#   /out                     where the signed ISO ($OUT_NAME) goes

set -Eeuo pipefail

src=/src
# shellcheck source=../build/lib.sh
source "${src}/scripts/build/lib.sh"
load_build_conf "${src}"
# shellcheck source=../../build/secureboot.conf
source "${src}/build/secureboot.conf"

[[ -n ${ISO_NAME:-} && -n ${OUT_NAME:-} ]] || die "ISO_NAME and OUT_NAME must be set"
[[ ${ISO_NAME} != "${OUT_NAME}" ]] || die "the signed ISO must not replace the unsigned one"
in_iso="/in/${ISO_NAME}"
out_iso="/out/${OUT_NAME}"
summary="${out_iso%.iso}.txt"
work=/var/tmp/sanctum-sign
# The decrypted key lives only in memory, and only while this script runs.
key=/dev/shm/sanctum-sb.key
cert=/keys/sanctum-sb.crt

cleanup() {
	rm -f -- "${key}"
	rm -rf -- "${work}"
	if [[ -n ${HOST_UID:-} && -n ${HOST_GID:-} ]]; then
		chown -R "${HOST_UID}:${HOST_GID}" -- /out 2>/dev/null || true
	fi
}
trap cleanup EXIT
trap 'die "signing failed at line ${LINENO}: ${BASH_COMMAND}"' ERR

# 1. Tools, from the same snapshot as the ISO --------------------------------

msg "Installing signing tools from the ${SNAPSHOT_DATE} snapshot"
# shellcheck disable=SC2016 # $repo and $arch are pacman variables.
printf 'Server = https://archive.archlinux.org/repos/%s/$repo/os/$arch\n' "${SNAPSHOT_DATE}" \
	>/etc/pacman.d/mirrorlist
pacman -Syy --noconfirm --needed archlinux-keyring >/dev/null
pacman -S --noconfirm --needed libisoburn mtools sbsigntools openssl libfaketime >/dev/null

# 2. The key ------------------------------------------------------------------

[[ $(openssl x509 -in "${cert}" -noout -subject -nameopt RFC2253) == "subject=CN=${SIGNING_CERT_CN}" ]] ||
	die "the certificate's subject is not CN=${SIGNING_CERT_CN}"
msg "Unlocking the signing key (asks for its passphrase if it has one)"
(
	umask 077
	openssl pkey -in /keys/sanctum-sb.key -out "${key}"
)
[[ $(openssl pkey -in "${key}" -pubout) == "$(openssl x509 -in "${cert}" -noout -pubkey)" ]] ||
	die "the key does not belong to the certificate"

# 3. The shim -----------------------------------------------------------------

mkdir -p -- "${work}/shim"
bsdtar -xf /shim/shim.rpm -C "${work}/shim"
shim="${work}/shim/${SHIM_EFI_PATH}"
mokmanager="${work}/shim/${MOKMANAGER_EFI_PATH}"
printf '%s  %s\n%s  %s\n' "${SHIM_EFI_SHA256}" "${shim}" "${MOKMANAGER_EFI_SHA256}" "${mokmanager}" |
	sha256sum --check --quiet || die "shim or MokManager does not match build/secureboot.conf"

# 4. The ISO's EFI system partition ----------------------------------------------

# mkarchiso appends the FAT image that UEFI boots from as partition 2, and
# points the El Torito UEFI entry at it. Its sectors come from xorriso's own
# description of the boot setup.
report=$(xorriso -indev "${in_iso}" -report_el_torito as_mkisofs 2>/dev/null)
read -r part_start part_end < <(sed -n \
	"s/^-append_partition 2 0xef --interval:local_fs:\([0-9]*\)d-\([0-9]*\)d::.*/\1 \2/p" <<<"${report}")
[[ -n ${part_start:-} && -n ${part_end:-} ]] || die "no EFI system partition found in ${ISO_NAME}"
grep -q "^-e '--interval:appended_partition_2_" <<<"${report}" ||
	die "the UEFI El Torito entry does not point at the EFI system partition"
iso_date=$(sed -n "s/^--modification-date='\([0-9]\{16\}\)'$/\1/p" <<<"${report}")
[[ -n ${iso_date} ]] || die "cannot read the ISO's modification date (its UUID)"
# Every new file and signature gets the ISO's own date, and xorriso derives
# its GPT GUIDs from it (as in the build), so signing twice gives the same ISO.
iso_time="${iso_date:0:4}-${iso_date:4:2}-${iso_date:6:2} ${iso_date:8:2}:${iso_date:10:2}:${iso_date:12:2}"
stamp=$(date -u -d "${iso_time}" +%Y%m%d%H%M.%S)
SOURCE_DATE_EPOCH=$(date -u -d "${iso_time}" +%s)
export SOURCE_DATE_EPOCH

esp="${work}/efiboot.img"
dd if="${in_iso}" of="${esp}" bs=512 skip="${part_start}" count=$((part_end - part_start + 1)) status=none
export MTOOLS_SKIP_CHECK=1

if mdir -i "${esp}" ::/EFI/BOOT/mmx64.efi >/dev/null 2>&1; then
	die "${ISO_NAME} already contains MokManager: it is signed already"
fi

# 5. Sign ---------------------------------------------------------------------

mkdir -p -- "${work}/files"
maps=()

sign() { # sign FAT_PATH OUT_FILE: sign a file from the ESP into OUT_FILE
	local unsigned="${work}/files/unsigned.efi"
	mcopy -n -i "${esp}" "::$1" "${unsigned}"
	if sbverify --list "${unsigned}" 2>/dev/null | grep -q '^signature [0-9]'; then
		die "$1 is already signed"
	fi
	# sbsign records the signing time; pinning it makes signing repeatable.
	TZ=UTC faketime -f "@${iso_time}" \
		sbsign --key "${key}" --cert "${cert}" --output "$2" "${unsigned}" >/dev/null
	sbverify --cert "${cert}" "$2" >/dev/null || die "the signature on $1 does not verify"
	rm -f -- "${unsigned}"
}

# Every kernel and EFI program the boot menu starts.
mapfile -t entries < <(mdir -b -i "${esp}" ::/loader/entries/ | grep '\.conf$')
((${#entries[@]})) || die "no boot menu entries in the EFI system partition"
mapfile -t targets < <(
	for entry in "${entries[@]}"; do
		mtype -i "${esp}" "${entry}"
	done | sed -n 's/^\(linux\|efi\)[[:space:]]\+\(\/[^[:space:]]*\)[[:space:]]*$/\2/p' | sort -u
)
((${#targets[@]})) || die "no kernels found in the boot menu entries"
for path in "${targets[@]}"; do
	[[ ${path} =~ ^/[A-Za-z0-9._+/-]+$ && ${path} != *..* ]] || die "unexpected path in a boot entry: ${path}"
	msg "Signing ${path}"
	signed="${work}/files/$(basename -- "${path}")"
	sign "${path}" "${signed}"
	touch -t "${stamp}" -- "${signed}"
	mcopy -o -m -i "${esp}" "${signed}" "::${path}"
	maps+=(-map "${signed}" "${path}")
done

# shim starts grubx64.efi from its own folder: that is systemd-boot.
msg "Signing systemd-boot (as EFI/BOOT/grubx64.efi)"
sign /EFI/BOOT/BOOTx64.EFI "${work}/files/grubx64.efi"
cp -- "${shim}" "${work}/files/BOOTx64.EFI"
cp -- "${mokmanager}" "${work}/files/mmx64.efi"
cp -- "${cert}" "${work}/files/sanctum.crt"
openssl x509 -in "${cert}" -outform DER -out "${work}/files/sanctum.cer"
touch -t "${stamp}" -- "${work}/files/"{grubx64.efi,BOOTx64.EFI,mmx64.efi,sanctum.cer}
mcopy -o -m -i "${esp}" "${work}/files/BOOTx64.EFI" ::/EFI/BOOT/BOOTx64.EFI
mcopy -o -m -i "${esp}" "${work}/files/grubx64.efi" "${work}/files/mmx64.efi" ::/EFI/BOOT/
mcopy -o -m -i "${esp}" "${work}/files/sanctum.cer" ::/sanctum.cer
maps+=(
	-map "${work}/files/BOOTx64.EFI" /EFI/BOOT/BOOTx64.EFI
	-map "${work}/files/grubx64.efi" /EFI/BOOT/grubx64.efi
	-map "${work}/files/mmx64.efi" /EFI/BOOT/mmx64.efi
	-map "${work}/files/sanctum.cer" /sanctum.cer
)

# 6. Write the signed ISO -----------------------------------------------------

msg "Writing ${OUT_NAME}"
rm -f -- "${out_iso}" "${out_iso}.sha256" "${summary}"
# replay keeps the boot setup (BIOS, El Torito, hybrid MBR/GPT); the new
# FAT image replaces partition 2, and the volume date (archiso's UUID) stays.
xorriso -report_about SORRY \
	-indev "${in_iso}" -outdev "${out_iso}" \
	-boot_image any replay \
	-append_partition 2 0xef "${esp}" \
	"${maps[@]}" \
	-volume_date uuid "${iso_date}"

# 7. Check the result ---------------------------------------------------------

msg "Checking the signed ISO"
new_report=$(xorriso -indev "${out_iso}" -report_el_torito as_mkisofs 2>/dev/null)
# Positions and file names may change; everything else must not.
# shellcheck disable=SC2001 # a regular expression, not a fixed string
strip() { sed -e 's/--interval:[^ ]*//g' <<<"$1"; }
[[ "$(strip "${report}")" == "$(strip "${new_report}")" ]] || die "the boot setup changed"
read -r new_start new_end < <(sed -n \
	"s/^-append_partition 2 0xef --interval:local_fs:\([0-9]*\)d-\([0-9]*\)d::.*/\1 \2/p" <<<"${new_report}")
check_esp="${work}/check.img"
dd if="${out_iso}" of="${check_esp}" bs=512 skip="${new_start}" count=$((new_end - new_start + 1)) status=none
for path in "${targets[@]}" /EFI/BOOT/grubx64.efi; do
	mcopy -n -i "${check_esp}" "::${path}" "${work}/check.efi"
	sbverify --cert "${cert}" "${work}/check.efi" >/dev/null || die "${path} is not signed in the written ISO"
done
mcopy -n -i "${check_esp}" ::/EFI/BOOT/BOOTx64.EFI "${work}/check.efi"
printf '%s  %s\n' "${SHIM_EFI_SHA256}" "${work}/check.efi" | sha256sum --check --quiet ||
	die "the written ISO does not boot the pinned shim"

(cd /out && sha256sum -- "${OUT_NAME}" >"${OUT_NAME}.sha256")
{
	printf 'Abyssal Sanctum Secure Boot signing summary\n\n'
	printf 'Signed ISO:    %s\n' "${OUT_NAME}"
	printf 'SHA-256:       %s\n' "$(cut -d' ' -f1 <"${out_iso}.sha256")"
	printf 'Unsigned ISO:  %s\n' "${ISO_NAME}"
	printf 'SHA-256:       %s\n' "$(sha256sum -- "${in_iso}" | cut -d' ' -f1)"
	printf '\nSigning certificate (compare in MokManager before enrolling):\n'
	printf '  Subject:     %s\n' "$(openssl x509 -in "${cert}" -noout -subject -nameopt RFC2253 | sed 's/^subject=//')"
	printf '  SHA-1:       %s\n' "$(openssl x509 -in "${cert}" -noout -fingerprint -sha1 | sed 's/^.*=//')"
	printf '  SHA-256:     %s\n' "$(openssl x509 -in "${cert}" -noout -fingerprint -sha256 | sed 's/^.*=//')"
	printf '\nshim:          %s (%s)\n' "${SHIM_RPM_URL##*/}" "${SHIM_EFI_SHA256}"
	printf 'MokManager:    %s\n' "${MOKMANAGER_EFI_SHA256}"
	printf '\nSigned with the Sanctum key:\n'
	printf '  %s\n' /EFI/BOOT/grubx64.efi "${targets[@]}"
} >"${summary}"
cat -- "${summary}" >&2
msg "Done"

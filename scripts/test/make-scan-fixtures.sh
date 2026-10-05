#!/usr/bin/env bash
# Build the scan-test disk images in tests/fixtures/scan/ (AV-1).
#
#   windows.img  NTFS, label WINTEST: a Windows folder layout, hibernated
#                (hiberfil.sys starts with "hibr"), the EICAR test file in a
#                Downloads folder and hidden in an alternate data stream
#   linux.img    ext4, label LINUXTEST: an os-release file and one EICAR file
#   data.img     ext4, label SANCTUM_DATA: a Sanctum data partition holding
#                a one-line ClamAV test signature for EICAR, so scan tests
#                need no real (and large) ClamAV databases
#
# EICAR is the industry-standard antivirus test file: harmless, but
# detected by every scanner. It is stored base64-encoded here, so this
# script itself is not flagged. The images are written xz-compressed.
#
# Runs in the pinned builder container (needs docker or root podman, and
# FUSE for NTFS-3G). Regenerate only when the scan test needs new content.

set -Eeuo pipefail

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
# shellcheck source=../build/lib.sh
source "${repo_root}/scripts/build/lib.sh"
load_build_conf "${repo_root}"

if [[ ${1:-} != --in-container ]]; then
	out="${repo_root}/tests/fixtures/scan"
	mkdir -p -- "${out}"
	msg "Building scan fixtures in the builder container"
	exec "${SANCTUM_ENGINE:-docker}" run --rm --privileged \
		--security-opt label=disable \
		-v "${repo_root}:/src:ro" \
		-v "${out}:/out" \
		-e SNAPSHOT_DATE="${SNAPSHOT_DATE}" \
		-e HOST_UID="$(id -u)" -e HOST_GID="$(id -g)" \
		"${BUILDER_IMAGE}" /src/scripts/test/make-scan-fixtures.sh --in-container
fi

# --- inside the container --------------------------------------------------------

# shellcheck disable=SC2016 # $repo and $arch are pacman variables.
printf 'Server = https://archive.archlinux.org/repos/%s/$repo/os/$arch\n' "${SNAPSHOT_DATE}" >/etc/pacman.d/mirrorlist
pacman -Syy --noconfirm --needed ntfs-3g ntfsprogs e2fsprogs xz >/dev/null

eicar_b64='WDVPIVAlQEFQWzRcUFpYNTQoUF4pN0NDKTd9JEVJQ0FSLVNUQU5EQVJELUFOVElWSVJVUy1URVNULUZJTEUhJEgrSCo='
eicar() { base64 -d <<<"${eicar_b64}"; }
work=$(mktemp -d)
trap 'umount "${work}/mnt" 2>/dev/null; rm -rf -- "${work}"' EXIT
mkdir -p "${work}/mnt"
export E2FSPROGS_FAKE_TIME=1767225600 # 2026-01-01, for repeatable ext4 images

# Windows (NTFS).
truncate -s 48M "${work}/windows.img"
mkntfs --quiet --fast --force --label WINTEST "${work}/windows.img"
ntfs-3g -o streams_interface=windows "${work}/windows.img" "${work}/mnt"
w="${work}/mnt"
mkdir -p "${w}/Windows/System32/config" "${w}/Users/test/Downloads" "${w}/Users/test/Documents"
: >"${w}/Windows/System32/config/SOFTWARE"
{
	printf 'hibr'
	head -c 4092 /dev/zero
} >"${w}/hiberfil.sys"
eicar >"${w}/Users/test/Downloads/invoice.com"
printf 'An ordinary document.\n' >"${w}/Users/test/Documents/readme.txt"
# The same document, carrying EICAR in a named stream (readme.txt:payload).
eicar >"${w}/Users/test/Documents/readme.txt:payload"
printf 'Clean file.\n' >"${w}/Users/test/Documents/notes.txt"
sync
umount "${w}"

# Linux (ext4), populated without mounting.
mkdir -p "${work}/linux/etc" "${work}/linux/home/user" "${work}/linux/tmp"
printf 'NAME="Test Linux"\nPRETTY_NAME="Test Linux 1.0"\nID=testlinux\n' >"${work}/linux/etc/os-release"
eicar >"${work}/linux/tmp/.cache-update"
printf 'hello\n' >"${work}/linux/home/user/notes.txt"
mkfs.ext4 -q -L LINUXTEST -U 6b7a0c1e-0000-4000-8000-000000000001 \
	-d "${work}/linux" "${work}/linux.img" 32M

# Sanctum data partition (ext4) with a test signature: MD5:size:name.
mkdir -p "${work}/data/clamav"
printf '%s:%s:Sanctum.Test.EICAR\n' "$(eicar | md5sum | cut -d' ' -f1)" "$(eicar | wc -c)" \
	>"${work}/data/clamav/sanctum-test.hdb"
mkfs.ext4 -q -L SANCTUM_DATA -U 6b7a0c1e-0000-4000-8000-000000000002 \
	-d "${work}/data" "${work}/data.img" 64M

for img in windows linux data; do
	xz -9e -c "${work}/${img}.img" >"/out/${img}.img.xz"
done
chown "${HOST_UID}:${HOST_GID}" /out/*.img.xz
msg "Wrote windows.img.xz, linux.img.xz and data.img.xz"

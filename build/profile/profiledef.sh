#!/usr/bin/env bash
# shellcheck disable=SC2034

#
# Abyssal Sanctum archiso profile. Derived from archiso's releng profile; the
# archiso version it tracks is ARCHISO_VERSION in build/build.conf.
#
# SANCTUM_VERSION comes from the repository's VERSION file and is exported by
# scripts/build/in-container.sh. Building without it is an error, so the ISO
# version can never silently drift from VERSION.

iso_name="abyssal-sanctum"
iso_version="v${SANCTUM_VERSION:?SANCTUM_VERSION must be set (see scripts/build/build-iso.sh)}"
# ISO 9660 volume ID: uppercase, digits and underscores, at most 32 chars.
# v0.1.0 -> SANCTUM_0_1_0
iso_label="SANCTUM_${SANCTUM_VERSION//./_}"
iso_publisher="Abyssal Sanctum <https://github.com/AbyssalOath/abyssal-sanctum>"
iso_application="Abyssal Sanctum - Arch Linux-based System Recovery Environment"
install_dir="sanctum"
bootmodes=('bios.syslinux'
	'uefi.systemd-boot')
pacman_conf="pacman.conf"
airootfs_image_type="squashfs"
airootfs_image_tool_options=('-comp' 'xz' '-Xbcj' 'x86' '-b' '1M' '-Xdict-size' '1M')
bootstrap_tarball_compression=('zstd' '-c' '-T0' '--auto-threads=logical' '--long' '-19')
file_permissions=(
	["/etc/shadow"]="0:0:400"
	["/root"]="0:0:750"
	["/root/.gnupg"]="0:0:700"
	["/usr/local/bin/livecd-sound"]="0:0:755"
	["/usr/local/bin/sanctum"]="0:0:755"
	["/usr/local/bin/sanctum-desktop-setup"]="0:0:755"
	["/usr/local/bin/sanctum-selftest"]="0:0:755"
)

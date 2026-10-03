#!/usr/bin/env bash
# The container side of scripts/release/fetch-sources.sh. Reads
# /sources.txt, writes source tarballs to /out.

set -Eeuo pipefail

# shellcheck source=../build/lib.sh
source /src/scripts/build/lib.sh

give_back_output() {
	chown -R "${HOST_UID}:${HOST_GID}" -- /out 2>/dev/null || true
}
trap give_back_output EXIT

# shellcheck disable=SC2016 # $repo and $arch are pacman variables.
printf 'Server = https://archive.archlinux.org/repos/%s/$repo/os/$arch\n' "${SNAPSHOT_DATE}" >/etc/pacman.d/mirrorlist
pacman -Syy --noconfirm --needed archlinux-keyring git >/dev/null

# makepkg refuses to run as root.
useradd --create-home builder 2>/dev/null || true
work=/var/tmp/sources-work

declare -A wanted=()
for pkgbase in "$@"; do
	wanted[${pkgbase}]=1
done

missing=()
done_count=0
while read -r pkgbase pkgver tarball repo_url; do
	[[ -z ${pkgbase} || ${pkgbase} == \#* ]] && continue
	if ((${#wanted[@]})) && [[ -z ${wanted[${pkgbase}]:-} ]]; then
		continue
	fi
	file="/out/${pkgbase}-${pkgver}.src.tar.gz"
	if [[ -s ${file} ]]; then
		done_count=$((done_count + 1))
		continue
	fi

	# 1. Arch's source server, if it still has this version.
	if curl -sfL --retry 3 -o "${file}.part" -- "${tarball}"; then
		mv -- "${file}.part" "${file}"
		msg "${pkgbase} ${pkgver}: downloaded"
		done_count=$((done_count + 1))
		continue
	fi
	rm -f -- "${file}.part"

	# 2. The packaging repository at this version, with makepkg --allsource.
	git_url=${repo_url%%/-/tree/*}.git
	tag=${repo_url##*/-/tree/}
	rm -rf -- "${work}"
	mkdir -p -- "${work}"
	chown builder: "${work}"
	if runuser -u builder -- git -c advice.detachedHead=false clone --quiet --depth 1 --branch "${tag}" -- "${git_url}" "${work}/pkg" &&
		(cd -- "${work}/pkg" && runuser -u builder -- makepkg --allsource --skippgpcheck --nocolor >"${work}/makepkg.log" 2>&1); then
		built=("${work}"/pkg/*.src.tar.gz)
		mv -- "${built[0]}" "${file}"
		msg "${pkgbase} ${pkgver}: built with makepkg --allsource"
		done_count=$((done_count + 1))
	else
		warn "${pkgbase} ${pkgver}: failed (see the packaging repository: ${repo_url})"
		[[ -f ${work}/makepkg.log ]] && tail -n 5 -- "${work}/makepkg.log" >&2
		missing+=("${pkgbase} ${pkgver} ${repo_url}")
	fi
done </sources.txt
rm -rf -- "${work}"

rm -f -- /out/MISSING.txt
if ((${#missing[@]})); then
	printf '%s\n' "${missing[@]}" >/out/MISSING.txt
	die "${#missing[@]} package sources could not be collected (listed in MISSING.txt); ${done_count} done"
fi
(cd /out && sha256sum -- *.src.tar.gz >SHA256SUMS)
msg "Collected ${done_count} source tarballs, with SHA256SUMS"

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
# git and mercurial: some packages build from version control sources.
pacman -Syy --noconfirm --needed archlinux-keyring git mercurial >/dev/null

# Upstream files move, vanish or get regenerated after Arch builds a package.
# makepkg checks every downloaded file against the PKGBUILD's checksums, so a
# copy from somewhere else is accepted only if it is the exact file Arch
# used. This download agent falls back to the Internet Archive's copy of the
# same URL; with SANCTUM_ARCHIVE_FIRST=1 (after a checksum mismatch) it tries
# the archived copy first.
cat >/usr/local/bin/sanctum-fetch <<'AGENT'
#!/usr/bin/env bash
out=$1 url=$2
direct() { /usr/bin/curl -qgb "" -fLC - --retry 3 --retry-delay 3 -o "${out}" -- "${url}"; }
# "2" asks for the capture closest to the year 2000: the earliest one, made
# before any upstream change.
archived() {
	rm -f -- "${out}"
	printf '  -> trying the Internet Archive copy of %s\n' "${url}" >&2
	/usr/bin/curl -qgfL --retry 3 --retry-delay 5 -o "${out}" -- "https://web.archive.org/web/2id_/${url}"
}
if [[ ${SANCTUM_ARCHIVE_FIRST:-0} == 1 ]]; then
	archived || { rm -f -- "${out}" && direct; }
else
	direct || archived
fi
AGENT
chmod 0755 /usr/local/bin/sanctum-fetch
cat >/etc/makepkg.conf.d/sanctum-fetch.conf <<'CONF'
DLAGENTS=('file::/usr/bin/curl -qgC - -o %o %u'
          'ftp::/usr/bin/curl -qgfC - --ftp-pasv --retry 3 --retry-delay 3 -o %o %u'
          'http::/usr/local/bin/sanctum-fetch %o %u'
          'https::/usr/local/bin/sanctum-fetch %o %u'
          'rsync::/usr/bin/rsync --no-motd -z %u %o'
          'scp::/usr/bin/scp -C %u %o')
CONF

# makepkg refuses to run as root.
useradd --create-home builder 2>/dev/null || true
work=/var/tmp/sources-work

declare -A wanted=()
for pkgbase in "$@"; do
	wanted[${pkgbase}]=1
done

# makepkg --allsource in DIR, logging to LOG. When files fail their
# checksums (regenerated upstream), delete them and fetch them again,
# archived copies first.
allsource() {
	local dir=$1 log=$2 failed
	(cd -- "${dir}" && runuser -u builder -- makepkg --allsource --skippgpcheck --nocolor >"${log}" 2>&1) && return 0
	mapfile -t failed < <(sed -n 's/^ *\([^ ]*\) \.\.\. FAILED$/\1/p' "${log}")
	((${#failed[@]})) || return 1
	local f
	for f in "${failed[@]}"; do
		[[ ${f} != */* && ${f} != .* ]] || return 1
		rm -rf -- "${dir:?}/${f}"
	done
	(cd -- "${dir}" && runuser -u builder -- env SANCTUM_ARCHIVE_FIRST=1 \
		makepkg --allsource --skippgpcheck --nocolor >>"${log}" 2>&1)
}

missing=()
done_count=0
while read -r pkgbase pkgver tarball repo_url; do
	[[ -z ${pkgbase} || ${pkgbase} == \#* ]] && continue
	if ((${#wanted[@]})) && [[ -z ${wanted[${pkgbase}]:-} ]]; then
		continue
	fi
	# ":" (epochs) and "+" become "_": download hosts reject or rewrite them.
	name="${pkgbase}-${pkgver}"
	name=${name//:/_}
	file="/out/${name//+/_}.src.tar.gz"
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
		allsource "${work}/pkg" "${work}/makepkg.log"; then
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

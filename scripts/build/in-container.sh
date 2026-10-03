#!/usr/bin/env bash
# Build the Abyssal Sanctum ISO on an Arch Linux system.
#
# Normally run by scripts/build/build-iso.sh inside the pinned builder
# container (SANCTUM_IN_CONTAINER=1), where it may reconfigure pacman freely.
# With --native it runs on an Arch host, and then it never touches the host's
# pacman configuration or packages.
#
# Steps: validate dependencies, validate configuration, stage the profile,
# build the ISO, write the checksum, check the size budget.

set -Eeuo pipefail

src=${SANCTUM_SRC:-/src}
out=${SANCTUM_OUT:-/out}
work=${SANCTUM_WORK:-/var/tmp/sanctum-work}
in_container=${SANCTUM_IN_CONTAINER:-0}

# shellcheck source=lib.sh
source "${src}/scripts/build/lib.sh"

# Files created as root inside the container belong to the invoking user,
# whether the build succeeds or fails.
give_back_output() {
	if [[ -n "${HOST_UID:-}" && -n "${HOST_GID:-}" ]]; then
		chown -R "${HOST_UID}:${HOST_GID}" -- "${out}" 2>/dev/null || true
	fi
}
trap give_back_output EXIT
trap 'die "build failed at line ${LINENO}: ${BASH_COMMAND}"' ERR

((EUID == 0)) || die "must run as root"

version=$(read_version "${src}")
load_build_conf "${src}"
export SANCTUM_VERSION="${version}"
[[ -n "${SOURCE_DATE_EPOCH:-}" ]] || die "SOURCE_DATE_EPOCH is not set"
export SOURCE_DATE_EPOCH

profile_src="${src}/build/profile"
profile="${work}/profile"
iso_file="abyssal-sanctum-v${version}-x86_64.iso"
snapshot_url="https://archive.archlinux.org/repos/${SNAPSHOT_DATE}"

msg "Abyssal Sanctum v${version}: snapshot ${SNAPSHOT_DATE}, archiso ${ARCHISO_VERSION}"

# 1. Dependencies -------------------------------------------------------------

if [[ "${in_container}" == 1 ]]; then
	msg "Pinning the builder to the ${SNAPSHOT_DATE} snapshot"
	# shellcheck disable=SC2016 # $repo and $arch are pacman variables.
	printf 'Server = %s/$repo/os/$arch\n' "${snapshot_url}" >/etc/pacman.d/mirrorlist
	# Keyring first, so packages signed by newer keys verify. -uu also allows
	# downgrades when the image is newer than the snapshot.
	pacman -Syy --noconfirm --needed archlinux-keyring
	pacman -Suu --noconfirm
	# rustup builds the sanctum CLI with the toolchain pinned in
	# rust-toolchain.toml (ADR-0008).
	pacman -S --noconfirm --needed archiso rustup
fi

for tool in mkarchiso pacman sha256sum cargo; do
	command -v "${tool}" >/dev/null || die "missing dependency: ${tool}"
done
installed_archiso=$(pacman -Q archiso | cut -d' ' -f2)
if [[ "${installed_archiso}" != "${ARCHISO_VERSION}" ]]; then
	if [[ "${in_container}" == 1 ]]; then
		die "snapshot provides archiso ${installed_archiso}, build.conf expects ${ARCHISO_VERSION}"
	fi
	warn "host has archiso ${installed_archiso}, build.conf expects ${ARCHISO_VERSION}"
fi

# 2. Configuration ------------------------------------------------------------

msg "Validating the profile and package lists"
for f in profiledef.sh pacman.conf airootfs; do
	[[ -e "${profile_src}/${f}" ]] || die "build/profile/${f} is missing"
done
[[ ! -e "${profile_src}/packages.x86_64" ]] ||
	die "build/profile/packages.x86_64 must not exist; packages belong in build/packages/*.list"
check_package_lists "${src}/build/packages" || die "package lists have errors (see above)"
bash -n "${profile_src}/profiledef.sh" || die "build/profile/profiledef.sh has syntax errors"
# shellcheck disable=SC2154 # iso_name and iso_version come from profiledef.sh.
profile_ids=$(
	# mkarchiso declares this associative array before sourcing the profile.
	# shellcheck disable=SC2034 # read by profiledef.sh, not here
	declare -A file_permissions
	# shellcheck source=../../build/profile/profiledef.sh
	source "${profile_src}/profiledef.sh"
	printf '%s-%s' "${iso_name}" "${iso_version}"
)
[[ "${profile_ids}-x86_64.iso" == "${iso_file}" ]] ||
	die "profiledef.sh would produce ${profile_ids}-x86_64.iso, expected ${iso_file}"
git_commit=${SANCTUM_GIT_COMMIT:-none}
[[ "${git_commit}" =~ ^([0-9a-f]{7,40}(-dirty)?|none)$ ]] ||
	die "SANCTUM_GIT_COMMIT must be a commit hash, got '${git_commit}'"

# 3. Stage the profile --------------------------------------------------------

msg "Staging the profile in ${work}"
rm -rf -- "${work}"
mkdir -p -- "${work}"
cp -a -- "${profile_src}" "${profile}"

{
	printf '# Generated from build/packages/*.list by scripts/build/in-container.sh.\n'
	package_names "${src}/build/packages"
} >"${profile}/packages.x86_64"

# Fill in build values. Placeholders look like @NAME@; any left over fail the
# build, so a typo cannot ship.
grep -rIlZ '@[A-Z_]*@' -- "${profile}" | xargs -0 -r sed -i \
	-e "s#@SANCTUM_VERSION@#${version}#g" \
	-e "s#@SNAPSHOT_DATE@#${SNAPSHOT_DATE}#g" \
	-e "s#@GIT_COMMIT@#${git_commit}#g"
if grep -rIl '@[A-Z_]*@' -- "${profile}"; then
	die "unreplaced placeholders in the staged profile (files listed above)"
fi

# The live system installs packages from the same snapshot as the ISO, so
# `pacman -S` cannot cause a partial upgrade (gap analysis, section 2.3).
install -D -m 0644 -- "${profile}/pacman.conf" "${profile}/airootfs/etc/pacman.conf"
mkdir -p -- "${profile}/airootfs/etc/pacman.d"
{
	printf '# Abyssal Sanctum: packages come from the snapshot this ISO was built from.\n'
	# shellcheck disable=SC2016 # $repo and $arch are pacman variables.
	printf 'Server = %s/$repo/os/$arch\n' "${snapshot_url}"
} >"${profile}/airootfs/etc/pacman.d/mirrorlist"

# Resolve every package against the snapshot before the long build, so a
# typo or a package dropped from Arch fails in seconds. The same resolution
# gives the exact versions pacstrap will install, for the manifest.
msg "Resolving the package list against the snapshot"
mkdir -p -- "${work}/db"
pacman --config "${profile}/pacman.conf" --dbpath "${work}/db" -Sy >/dev/null
mapfile -t packages < <(package_names "${src}/build/packages")
resolved=$(pacman --config "${profile}/pacman.conf" --dbpath "${work}/db" \
	-Sp --print-format '%n %v %e' -- "${packages[@]}") ||
	die "some packages in build/packages are not in the ${SNAPSHOT_DATE} snapshot"

msg "Writing the build manifest"
manifest="${profile}/airootfs/usr/share/abyssal-sanctum/manifest.json"
mkdir -p -- "${manifest%/*}"
{
	printf '{\n'
	printf '  "name": "Abyssal Sanctum",\n'
	printf '  "version": "%s",\n' "${version}"
	printf '  "git_commit": "%s",\n' "${git_commit}"
	printf '  "snapshot_date": "%s",\n' "${SNAPSHOT_DATE}"
	printf '  "archiso_version": "%s",\n' "${ARCHISO_VERSION}"
	printf '  "source_date_epoch": %d,\n' "${SOURCE_DATE_EPOCH}"
	printf '  "packages": [\n'
	sep=""
	while read -r name pkgver pkgbase; do
		# Arch package names and versions never need JSON escaping; refuse
		# anything that would.
		[[ "${name} ${pkgver} ${pkgbase}" =~ ^[a-zA-Z0-9@._+-]+\ [a-zA-Z0-9.:_+~-]+\ [a-zA-Z0-9@._+-]+$ ]] ||
			die "unexpected package name, version or base: ${name} ${pkgver} ${pkgbase}"
		printf '%s    {"name": "%s", "version": "%s"}' "${sep}" "${name}" "${pkgver}"
		sep=$',\n'
	done < <(sort <<<"${resolved}")
	printf '\n  ]\n}\n'
} >"${manifest}"
msg "Manifest lists $(wc -l <<<"${resolved}") packages"

# Where the source code of every package comes from, published with each
# release for GPL compliance (ADR-0009). One line per package base.
msg "Writing the source manifest"
sources="${work}/sources.txt"
{
	cat <<EOT
# Abyssal Sanctum v${version}: corresponding source
#
# Sanctum itself: https://github.com/AbyssalOath/abyssal-sanctum/tree/v${version}
# (Rust crate sources are listed in Cargo.lock there.)
# Arch Linux packages, from the ${SNAPSHOT_DATE} Arch Linux Archive snapshot.
# Columns: package base, version, Arch source tarball, packaging repository.
# scripts/release/fetch-sources.sh collects the source of every package here.
EOT
	while read -r pkgbase pkgver; do
		# Git tags cannot contain ":", so Arch tags epochs as "1-"; and
		# repository names spell "+" as "plus" (libsigc++: libsigcplusplus).
		tag=${pkgver/:/-}
		repo=${pkgbase//+/plus}
		printf '%s %s https://sources.archlinux.org/sources/packages/%s-%s.src.tar.gz https://gitlab.archlinux.org/archlinux/packaging/packages/%s/-/tree/%s\n' \
			"${pkgbase}" "${pkgver}" "${pkgbase}" "${pkgver}" "${repo}" "${tag}"
	done < <(awk '{ print $3, $2 }' <<<"${resolved}" | sort -u)
} >"${sources}"

# Sanctum's own files -----------------------------------------------------------

# The sanctum CLI, built with the pinned toolchain. The toolchain and crates
# are cached in out/cache, so later builds work offline.
msg "Building the sanctum CLI"
export CARGO_TARGET_DIR="${work}/target"
(
	cd -- "${src}"
	if [[ "${in_container}" == 1 ]]; then
		# Native builds use the host's own rustup and cargo setup.
		export RUSTUP_HOME="${out}/cache/rustup" CARGO_HOME="${out}/cache/cargo"
		rustup toolchain install
	fi
	cargo build --release --locked --quiet
)
sanctum="${CARGO_TARGET_DIR}/release/sanctum"
install -D -m 0755 -- "${sanctum}" "${profile}/airootfs/usr/local/bin/sanctum"

# The catalog must match the package lists and guides before it ships.
"${sanctum}" catalog check \
	--catalog "${src}/catalog" --packages "${src}/build/packages" --docs "${src}/docs"
install -d -- "${profile}/airootfs/usr/share/abyssal-sanctum/catalog"
install -m 0644 -- "${src}"/catalog/*.toml "${profile}/airootfs/usr/share/abyssal-sanctum/catalog/"

# Xfce menu entries generated from the catalog (ADR-0007).
"${sanctum}" render desktop --catalog "${src}/catalog" --root "${profile}/airootfs"

# Offline guides: the Markdown sources for `sanctum docs`, and HTML for the
# browser.
doc_root="${profile}/airootfs/usr/share/doc/abyssal-sanctum"
install -d -- "${doc_root}"
cp -r -- "${src}/docs" "${doc_root}/docs"
install -m 0644 -- "${src}"/{README,ROADMAP,CHANGELOG,SECURITY}.md "${doc_root}/"
"${sanctum}" render docs --src "${doc_root}" --out "${doc_root}/html"

# Desktop background.
install -D -m 0644 -- "${src}/assets/abyssal-sanctum_portal-of-midnight_background.png" \
	"${profile}/airootfs/usr/share/backgrounds/abyssal-sanctum/portal-of-midnight.png"

# 4-5. Build the filesystem and the ISO ---------------------------------------

mkdir -p -- "${out}"
release_base="${out}/${iso_file%.iso}"
rm -f -- "${out}/${iso_file}" "${out}/${iso_file}.sha256" \
	"${release_base}.sources.txt" "${release_base}.manifest.json"
msg "Running mkarchiso"
mkarchiso -v -r -w "${work}/build" -o "${out}" "${profile}"
[[ -f "${out}/${iso_file}" ]] || die "mkarchiso did not produce ${iso_file}"

# 6. Checksum -----------------------------------------------------------------

msg "Writing ${iso_file}.sha256"
(cd -- "${out}" && sha256sum -- "${iso_file}" >"${iso_file}.sha256")

# Release companions: the source manifest and the package manifest.
install -m 0644 -- "${sources}" "${release_base}.sources.txt"
install -m 0644 -- "${manifest}" "${release_base}.manifest.json"

# Size budget -----------------------------------------------------------------

size_mib=$(($(stat -c %s -- "${out}/${iso_file}") / 1024 / 1024))
if ((size_mib > ISO_SIZE_FAIL_MIB)); then
	die "ISO is ${size_mib} MiB, over the ${ISO_SIZE_FAIL_MIB} MiB limit"
elif ((size_mib > ISO_SIZE_WARN_MIB)); then
	warn "ISO is ${size_mib} MiB, over the ${ISO_SIZE_WARN_MIB} MiB warning level"
fi

rm -rf -- "${work}"
msg "Built ${iso_file} (${size_mib} MiB)"

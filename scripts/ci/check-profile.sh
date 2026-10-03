#!/usr/bin/env bash
# Static checks of the archiso profile that do not need the Arch container.
# The build repeats the essential ones; these catch mistakes in seconds.

set -Eeuo pipefail

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
# shellcheck source=../build/lib.sh
source "${repo_root}/scripts/build/lib.sh"
profile="${repo_root}/build/profile"
errors=0
error() {
	printf '==> ERROR: %s\n' "$*" >&2
	errors=$((errors + 1))
}

[[ ! -e "${profile}/packages.x86_64" ]] ||
	error "build/profile/packages.x86_64 must not exist; packages belong in build/packages/*.list"
bash -n "${profile}/profiledef.sh" || error "profiledef.sh has syntax errors"

# Every path given permissions in profiledef.sh must exist in airootfs, or
# be one the build adds (scripts/build/in-container.sh).
generated=(/usr/local/bin/sanctum)
mapfile -t perm_paths < <(sed -n 's/^[[:space:]]*\["\([^"]*\)"\]=.*/\1/p' "${profile}/profiledef.sh")
for path in "${perm_paths[@]}"; do
	[[ -e "${profile}/airootfs${path}" || " ${generated[*]} " == *" ${path} "* ]] ||
		error "profiledef.sh sets permissions on ${path}, which is not in airootfs"
done

# Every executable shipped in airootfs/usr/local/bin needs an entry, or it
# ends up non-executable (mkarchiso does not preserve modes).
for f in "${profile}"/airootfs/usr/local/bin/*; do
	path=${f#"${profile}/airootfs"}
	printf '%s\n' "${perm_paths[@]}" | grep -qxF -- "${path}" ||
		error "${path} has no file_permissions entry in profiledef.sh"
done

# Enabled units must point at units that exist, either in airootfs or as
# absolute paths into packages (checked at boot by the self-test).
while IFS= read -r link; do
	target=$(readlink -- "${link}")
	case ${target} in
	/dev/null | /usr/lib/* | /etc/systemd/system/*) ;;
	../*) [[ -e "$(dirname -- "${link}")/${target}" ]] || error "${link#"${profile}/"} points to missing ${target}" ;;
	*) error "${link#"${profile}/"} has an unexpected target ${target}" ;;
	esac
done < <(find "${profile}/airootfs/etc/systemd" -type l)

# The safety-critical defaults of ADR-0006 must stay in place.
[[ "$(readlink -- "${profile}/airootfs/etc/udev/rules.d/64-md-raid-assembly.rules")" == /dev/null ]] ||
	error "md RAID auto-assembly must stay masked (ADR-0006)"
grep -q '^[[:space:]]*auto_activation_volume_list = \[\]' "${profile}/airootfs/etc/lvm/lvm.conf" ||
	error "LVM auto_activation_volume_list must stay empty (ADR-0006)"
[[ ! -e "${profile}/airootfs/etc/systemd/system/multi-user.target.wants/sshd.service" ]] ||
	error "sshd must not be enabled at boot (ADR-0006)"
for unit in avahi-daemon.service avahi-daemon.socket; do
	[[ "$(readlink -- "${profile}/airootfs/etc/systemd/system/${unit}")" == /dev/null ]] ||
		error "${unit} must stay masked: it announces the machine on the network (ADR-0006)"
done
grep -q 'policy drop' "${profile}/airootfs/etc/nftables.conf" ||
	error "the firewall input policy must be drop (ADR-0006)"

# Placeholders must be ones the build knows how to fill.
while IFS= read -r placeholder; do
	case ${placeholder} in
	@SANCTUM_VERSION@ | @SNAPSHOT_DATE@ | @GIT_COMMIT@) ;;
	*) error "unknown placeholder ${placeholder}" ;;
	esac
done < <(grep -rIoh '@[A-Z_]*@' -- "${profile}" | sort -u)

((errors == 0)) || die "profile check failed (${errors} errors)"
msg "Profile OK"

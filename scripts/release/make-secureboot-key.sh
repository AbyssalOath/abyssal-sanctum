#!/usr/bin/env bash
# Create Sanctum's Secure Boot signing key and certificate (ADR-0011).
#
# Run once, by the maintainer, on a trusted machine. The private key is the
# trust anchor on every machine where a technician enrolled the certificate:
# keep it offline (an encrypted USB stick, a password manager's file vault),
# never in the repository and never in CI.
#
# Usage: scripts/release/make-secureboot-key.sh DIR [--no-passphrase]
#
# Writes into DIR (which must not exist yet):
#   sanctum-sb.key  the private key, PEM (PKCS#8), passphrase-protected (AES-256)
#   sanctum-sb.crt  the certificate, PEM (for sbsign)
#   sanctum.cer     the same certificate, DER (shipped on the ISO for MokManager)
#
# --no-passphrase leaves the key unencrypted. Use it only for throwaway test
# keys.

set -Eeuo pipefail

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
# shellcheck source=../build/lib.sh
source "${repo_root}/scripts/build/lib.sh"
# shellcheck source=../../build/secureboot.conf
source "${repo_root}/build/secureboot.conf"

[[ $# -ge 1 ]] || die "usage: $0 DIR [--no-passphrase]"
dir=$1
no_passphrase=0
[[ ${2:-} == --no-passphrase ]] && no_passphrase=1
[[ $# -le 2 && (${2:-} == "" || ${2:-} == --no-passphrase) ]] || die "usage: $0 DIR [--no-passphrase]"
[[ ! -e ${dir} ]] || die "${dir} already exists; refusing to overwrite a signing key"
command -v openssl >/dev/null || die "missing dependency: openssl"

umask 077
mkdir -p -- "${dir}"
# A failed run leaves nothing behind, so it can simply be run again.
trap 'rm -rf -- "${dir}"; die "key creation failed; nothing was kept"' ERR

key="${dir}/sanctum-sb.key"
# RSA 2048 is what shim and MokManager are tested with on every firmware.
if ((no_passphrase)); then
	msg "Creating the signing key (no passphrase)"
	openssl genpkey -algorithm RSA -pkeyopt rsa_keygen_bits:2048 -out "${key}"
else
	msg "Creating the signing key: choose a passphrase (asked twice)"
	openssl genpkey -algorithm RSA -pkeyopt rsa_keygen_bits:2048 -aes-256-cbc -out "${key}"
	msg "Creating the certificate: enter the passphrase again"
fi

# The validity period is informational: shim does not check expiry.
openssl req -new -x509 -sha256 -days 7300 -key "${key}" \
	-subj "/CN=${SIGNING_CERT_CN}/" \
	-addext "basicConstraints=critical,CA:FALSE" \
	-addext "keyUsage=critical,digitalSignature" \
	-addext "extendedKeyUsage=codeSigning" \
	-out "${dir}/sanctum-sb.crt"
openssl x509 -in "${dir}/sanctum-sb.crt" -outform DER -out "${dir}/sanctum.cer"
chmod 0644 -- "${dir}/sanctum-sb.crt" "${dir}/sanctum.cer"
trap - ERR

msg "Created in ${dir}:"
openssl x509 -in "${dir}/sanctum-sb.crt" -noout -subject >&2
openssl x509 -in "${dir}/sanctum-sb.crt" -noout -fingerprint -sha1 >&2
openssl x509 -in "${dir}/sanctum-sb.crt" -noout -fingerprint -sha256 >&2
msg "Keep sanctum-sb.key offline, with a backup. Publish both fingerprints with each release;"
msg "technicians compare the SHA-1 in MokManager."

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
#   sanctum-sb.key  the private key, PEM, passphrase-protected (AES-256)
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
encrypt=(-aes256)
[[ ${2:-} == --no-passphrase ]] && encrypt=(-noenc)
[[ $# -le 2 && (${2:-} == "" || ${2:-} == --no-passphrase) ]] || die "usage: $0 DIR [--no-passphrase]"
[[ ! -e ${dir} ]] || die "${dir} already exists; refusing to overwrite a signing key"
command -v openssl >/dev/null || die "missing dependency: openssl"

umask 077
mkdir -p -- "${dir}"

# RSA 2048 is what shim and MokManager are tested with on every firmware.
# The validity period is informational: shim does not check expiry.
msg "Creating the signing key (you will be asked for a passphrase)"
openssl req -new -x509 -sha256 -days 7300 \
	-newkey rsa:2048 "${encrypt[@]}" \
	-subj "/CN=${SIGNING_CERT_CN}/" \
	-addext "basicConstraints=critical,CA:FALSE" \
	-addext "keyUsage=critical,digitalSignature" \
	-addext "extendedKeyUsage=codeSigning" \
	-keyout "${dir}/sanctum-sb.key" -out "${dir}/sanctum-sb.crt"
openssl x509 -in "${dir}/sanctum-sb.crt" -outform DER -out "${dir}/sanctum.cer"
chmod 0644 -- "${dir}/sanctum-sb.crt" "${dir}/sanctum.cer"

msg "Created in ${dir}:"
openssl x509 -in "${dir}/sanctum-sb.crt" -noout -subject -fingerprint -sha256 >&2
msg "Keep sanctum-sb.key offline. Publish the SHA-256 fingerprint above with each release."

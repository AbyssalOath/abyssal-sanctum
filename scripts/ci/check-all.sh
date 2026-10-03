#!/usr/bin/env bash
# Run every CI lint and test gate locally, the same way ci.yml does.
# The ISO build and boot tests are separate (scripts/build/build-iso.sh,
# scripts/test/boot-test.sh) because they take minutes.
#
# Needs docker for the pinned tool images; Rust gates need cargo (rustup
# installs the pinned toolchain) and cargo-deny.

set -Eeuo pipefail

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
# shellcheck source=../build/lib.sh
source "${repo_root}/scripts/build/lib.sh"
cd -- "${repo_root}"

# Same images as .github/workflows/ci.yml (markdown and links run there
# through their official actions, which bundle the same tools).
actionlint_image="rhysd/actionlint@sha256:b1934ee5f1c509618f2508e6eb47ee0d3520686341fec936f3b79331f9315667"
zizmor_image="ghcr.io/zizmorcore/zizmor@sha256:a2eb396d886c053073405c7a980f2139ba2248ec172243cfa3841e57196e8101"
markdownlint_image="davidanson/markdownlint-cli2@sha256:ea33f1f6a0f062f88a3dddfc49f6d6b5621648a93a0ff49a58bf8ac5a15330b9"
lychee_image="lycheeverse/lychee@sha256:eaff3e0a13603c9a701accfcc84f44158bb77bf36ecfa4622b626056c3463892"

failed=()
gate() {
	local name=$1
	shift
	msg "${name}"
	"$@" || failed+=("${name}")
}

gate "Shell (pinned shellcheck and shfmt)" scripts/ci/in-builder.sh shellcheck shfmt -- scripts/ci/lint-shell.sh
gate "Package lists" scripts/ci/check-package-lists.sh
gate "archiso profile" scripts/ci/check-profile.sh
gate "Version consistency" scripts/ci/check-version.sh
gate "Rust format" cargo fmt --all -- --check
gate "Rust clippy" cargo clippy --all-targets --locked -- -D warnings
gate "Rust tests" cargo test --locked
gate "cargo-deny" cargo deny check
gate "Tool catalog" cargo run --locked --quiet -- catalog check
gate "Markdown lint" docker run --rm -v "${repo_root}:/workdir:ro" -w /workdir "${markdownlint_image}"
gate "Links" docker run --rm -v "${repo_root}:/input:ro" -w /input "${lychee_image}" --config lychee.toml --no-progress .
gate "actionlint" docker run --rm -v "${repo_root}:/repo:ro" -w /repo "${actionlint_image}"
gate "zizmor" docker run --rm -v "${repo_root}:/repo:ro" -w /repo "${zizmor_image}" --offline .github/workflows

if ((${#failed[@]})); then
	die "failed: $(printf '%s; ' "${failed[@]}")"
fi
msg "All checks passed"

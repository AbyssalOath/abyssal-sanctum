# CI/CD

Abyssal Sanctum's checks run on GitHub Actions, and every check is a script
in the repository that runs the same way locally. The workflows only install
tools and call those scripts.

```bash
scripts/ci/check-all.sh           # every lint and test gate (a few minutes)
scripts/build/build-iso.sh        # build the ISO
scripts/test/boot-test.sh --uefi  # boot-test it (about 1-2 minutes each)
scripts/test/boot-test.sh --bios
```

## Workflows

### `ci.yml`: lint and test gates

Runs on every push to `main` and every pull request. All jobs must pass.

| Job | What it checks | Script or tool |
| --- | --- | --- |
| Shell scripts | shellcheck and shfmt on every shell script | `scripts/ci/lint-shell.sh`, run inside the pinned builder image |
| Packages, profile and versions | Package-list format and reasons; profile structure and the ADR-0006 safety defaults; `VERSION` agreeing with Cargo and the workflows | `check-package-lists.sh`, `check-profile.sh`, `check-version.sh` |
| Rust | `cargo fmt --check`, `clippy -D warnings`, `cargo test`, and `sanctum catalog check` (tool catalog against the package lists and guides) | rustup installs the toolchain pinned in `rust-toolchain.toml` |
| Dependencies | Advisories, licenses, sources | cargo-deny with `deny.toml` |
| Documentation | Markdown style; links and `#fragments` inside the repository | markdownlint-cli2 (`.markdownlint-cli2.yaml`), lychee (`lychee.toml`) |
| Workflows | Workflow syntax; workflow security | actionlint, zizmor |

Shell lint runs inside the pinned Arch builder image
(`scripts/ci/in-builder.sh`), so shellcheck and shfmt are the versions from
the snapshot both locally and in CI. Distribution versions differ: during
setup, the snapshot's shellcheck reported a warning that Fedora's older
version did not know about.

The link check is offline on purpose. It checks file links and heading
anchors, but does not fetch external sites, so CI does not fail when a
third-party site is down.

### `build-iso.yml`: build and boot test

Runs on pushes to `main` and pull requests that change anything the ISO is
built from (`build/`, `scripts/build/`, `scripts/test/`, `tests/fixtures/`,
`crates/`, `catalog/`, `docs/`, `assets/`, Cargo files, `VERSION`), and on
manual dispatch.

1. **Build** (the first run, without caches, took about 31 minutes
   including both boot tests):
   - Free disk space.
   - Restore the pacman package cache (keyed by snapshot date and package
     lists) and the Rust toolchain and crate cache (keyed by
     `rust-toolchain.toml` and `Cargo.lock`).
   - Run `scripts/build/build-iso.sh`.
   - Upload the ISO, checksum and build log as the `iso` artifact (kept 7
     days). The size budget (ADR-0005) is enforced by the build itself.
2. **Boot test** (UEFI and BIOS in parallel):
   - Download the ISO and verify its checksum.
   - Enable KVM and install QEMU and OVMF.
   - Run `scripts/test/boot-test.sh`.
   - Upload the serial log as an artifact, even on failure.

### `release.yml`: tagged releases

Runs when a tag `vX.Y.Z` is pushed ([ADR-0009](decisions/0009-release-integrity.md)):

1. **Verify:** `scripts/release/check-release.sh` checks the tag against
   `VERSION`, Cargo and the changelog, and that the commit is on `main`.
2. **Build:** calls `build-iso.yml` with `release: true`. That disables
   the caches and keeps the artifacts for 30 days. Both boot tests run.
3. **Publish**, in the `release` environment: attests the ISO and the
   manifests, and drafts the GitHub release with the checksum, the source
   manifest, the package manifest and notes from the changelog.

The ISO is not attached to the release. The maintainer uploads it to the
download host ([releasing](../operations/releasing.md)).

## The boot test

`scripts/test/boot-test.sh` boots the ISO's **default menu entry**, through
the real boot loader (systemd-boot on UEFI, syslinux on BIOS), with:

- **the self-test switched on** through a systemd credential. QEMU passes
  `io.systemd.credential:sanctum.selftest=1` as an SMBIOS type 11 string;
  systemd imports such strings as system credentials when it runs in a VM,
  and `sanctum-selftest.service` starts only when the credential (or the
  `sanctum.selftest=1` kernel parameter) is present. No boot menu editing is
  needed. On real hardware nothing passes this credential, so the self-test
  never runs by surprise.
- **the disk-safety fixtures attached as writable disks**
  (`tests/fixtures/disk-safety/`): a two-disk md RAID1 array and an LVM
  volume group, each with ext4.

`sanctum-selftest` (in the ISO at `/usr/local/bin/sanctum-selftest`) is
read-only. It checks:

- identity: os-release, manifest, hostname, kernel,
- no failed units,
- sshd off, firewall default-deny with no open ports, no network listeners,
- no RAID assembled, no LVM, LUKS or other device-mapper devices active, no
  disks mounted, no swap,
- auto-activation masks in place,
- no mDNS, LLMNR or connectivity checks,
- pacman pinned to the snapshot,
- no automount software installed,
- every catalog tool installed, with its version check succeeding
  (`sanctum catalog verify`).

It writes the results to the serial console, ending with
`SANCTUM-SELFTEST: PASS` or `FAIL`, then powers off.

The test passes only if all of these hold:

1. the self-test passed,
2. the VM powered off within the time limit (default 300 s),
3. every fixture's SHA-256 is unchanged.

Technicians can run `sanctum-selftest` by hand on a booted system. It
changes nothing.

### Disk-safety fixtures

The three images in `tests/fixtures/disk-safety/` are 64 MiB raw disks,
xz-compressed to about 12 KB each. They were created inside a booted
Sanctum VM attached to three blank images:

```bash
mdadm --create /dev/md/sanctumtest --run --level=1 --raid-devices=2 \
  --metadata=1.2 --assume-clean /dev/vda /dev/vdb
mkfs.ext4 -q /dev/md/sanctumtest
mdadm --stop /dev/md/sanctumtest
pvcreate /dev/vdc
vgcreate sanctumvg /dev/vdc
lvcreate -y -n data -l 100%FREE sanctumvg
mkfs.ext4 -q /dev/sanctumvg/data
vgchange -an sanctumvg
```

Then each image was compressed with `xz -9e`. They only need regenerating
if the test needs a new kind of storage.

## Security of the workflows

- **Least privilege.** Every workflow sets `permissions: {}`, and each job
  asks for `contents: read` only. The one exception is the release
  workflow's publishing job (`contents: write`, `id-token: write`,
  `attestations: write`), which runs in the `release` environment and can
  require a reviewer's approval.
- **No caches in releases.** Release builds restore nothing from earlier
  runs.
- **Pinned actions.** Every action is pinned to a full commit SHA, with the
  version in a comment. Docker images used by jobs are pinned by digest.
  Dependabot (`.github/dependabot.yml`) proposes updates weekly as pull
  requests.
- **No stored secrets.** No workflow uses a repository secret. Checkouts do
  not keep the token (`persist-credentials: false`).
- **No untrusted input in scripts.** Matrix values and tag names reach
  shell steps through environment variables, not `${{ }}` expansion inside
  `run:`.
- **zizmor** checks all of this on every change to the workflows.

## Keeping things in sync

- The builder image digest lives in `build/build.conf`.
  `scripts/ci/check-version.sh` fails if a workflow references a different
  Arch image.
- The actionlint image digest appears in both `ci.yml` and
  `scripts/ci/check-all.sh`; update them together. zizmor, markdownlint and
  lychee run through their official actions in CI and through pinned images
  locally, so their versions can differ slightly. Keep them close when
  Dependabot updates the actions.
- `build-iso.yml` lists its trigger paths twice (push and pull request).
  YAML anchors would avoid that, but they limit what zizmor can check.

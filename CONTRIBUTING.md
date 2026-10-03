# Contributing to Abyssal Sanctum

## Ground rules

- **Never destroy data silently.** Nothing in Sanctum may modify a disk,
  repair a filesystem or delete a file without an explicit user action. See
  the [gap analysis](docs/development/gap-analysis.md), section 4.
- **Official Arch repositories only.** A package from anywhere else needs an
  ADR explaining why (ADR-0003).
- **Every package has a reason.** Say why it is in the ISO and what it
  replaces or adds.
- **Decisions are written down.** A change to the architecture, the package
  policy or a security default comes with an ADR in
  `docs/architecture/decisions/`.
- No em dashes in comments, notes or docs. Use `-` or `--`.

## Checks

Formatting and linting are mandatory, and CI blocks any change that fails
them. Run the same gates locally before sending a change:

```bash
scripts/ci/check-all.sh
```

It runs shellcheck and shfmt (pinned versions, in the builder container),
the package-list, profile and version checks, `cargo fmt`, `clippy`, `cargo
test`, `cargo deny`, markdown lint, the link check, actionlint and zizmor.
See [docs/architecture/ci-cd.md](docs/architecture/ci-cd.md).

Shell scripts use Bash, tabs for indentation (shfmt's default), and
`set -Eeuo pipefail`.

## Building and testing

See [docs/getting-started/building.md](docs/getting-started/building.md).
Any change to `build/` must be built and boot-tested with both UEFI and
BIOS before review:

```bash
scripts/build/build-iso.sh
scripts/test/boot-test.sh --uefi
scripts/test/boot-test.sh --bios
```

CI does the same for every pull request that touches the ISO.

## Commits

Write commit messages in the imperative ("Add X", "Fix Y"). Keep unrelated
changes in separate commits. Add user-visible changes to `CHANGELOG.md`.

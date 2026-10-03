# ADR-0008: Rust components

- **Status:** Accepted
- **Date:** 2026-10-02

## Context

The brief says to use Bash where shell scripting fits, and to document why
before introducing Rust or another language. The project is also meant to
be "primarily built in Rust". Sanctum needs:

- a typed tool catalog, read by the menu, the desktop and the self-test,
- an interactive menu and tool listings,
- a disk overview that parses device data,
- an SSH on/off command,
- build-time generation of desktop menus and HTML guides,
- later, safety helpers (read-only mounting, chroot setup, forensic mode)
  that must parse device state carefully and refuse unsafe situations.

## Decision

| Component | Language | Why |
| --- | --- | --- |
| archiso profile, package lists, overlay | Configuration files | Declarative; archiso reads them directly |
| Build orchestration (`scripts/build/`) | Bash | Thin glue around docker and mkarchiso |
| CI checks (`scripts/ci/`) | Bash | Short checks of files |
| Self-test (`sanctum-selftest`) | Bash | Runs system commands and checks their output; easy to read next to the commands a technician would type |
| `sanctum` CLI | Rust | Parses TOML, JSON (lsblk, smartctl) and Markdown; validates the catalog; generates files; holds the future safety helpers, where Bash is error-prone and hard to test |

The `sanctum` command lives in `crates/sanctum-cli`, a Cargo workspace like
the other Abyssal projects. Settings: edition 2024, `unsafe_code = "forbid"`,
Clippy with `-D warnings`, and the toolchain pinned in
`rust-toolchain.toml`.

Dependencies are few and widely used: `clap` (arguments), `serde`, `toml`
and `serde_json` (catalog, lsblk and smartctl output, manifest), and
`pulldown-cmark` (guides to HTML). `cargo deny` checks their advisories,
licences and sources.

The ISO build compiles `sanctum` in the builder container with the pinned
toolchain (`cargo build --release --locked`) and installs it as
`/usr/local/bin/sanctum`. The toolchain and crates are cached in
`out/cache/`, so later builds work offline.

## Consequences

- The first build downloads the Rust toolchain and crates (several hundred
  MB). CI caches them.
- Building a Sanctum package for a local pacman repository (ROADMAP Phase 6)
  will replace the direct install.
- The CLI never needs `unsafe`. Where that would have been convenient (for
  example restoring the default SIGPIPE behaviour), it uses a safe
  alternative instead: output helpers that exit quietly when a pipe closes.

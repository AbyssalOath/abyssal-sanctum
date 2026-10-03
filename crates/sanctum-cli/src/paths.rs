//! Where Sanctum's data lives on the live system. Each location can be
//! overridden with an environment variable, which tests and development
//! runs use.

use std::path::PathBuf;

fn from_env(var: &str, default: &str) -> PathBuf {
    std::env::var_os(var).map_or_else(|| PathBuf::from(default), PathBuf::from)
}

/// The tool catalog (`catalog/*.toml` in the repository).
pub(crate) fn catalog_dir() -> PathBuf {
    from_env("SANCTUM_CATALOG_DIR", "/usr/share/abyssal-sanctum/catalog")
}

/// The documentation tree: README.md, docs/, and html/.
pub(crate) fn docs_dir() -> PathBuf {
    from_env("SANCTUM_DOCS_DIR", "/usr/share/doc/abyssal-sanctum")
}

/// The build manifest written by scripts/build/in-container.sh.
pub(crate) fn manifest() -> PathBuf {
    from_env(
        "SANCTUM_MANIFEST",
        "/usr/share/abyssal-sanctum/manifest.json",
    )
}

/// The read-only self-test script.
pub(crate) fn selftest() -> PathBuf {
    from_env("SANCTUM_SELFTEST", "/usr/local/bin/sanctum-selftest")
}

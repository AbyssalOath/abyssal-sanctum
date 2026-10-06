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

/// Helper programs shipped with Sanctum (probe-fs).
pub(crate) fn libexec_dir() -> PathBuf {
    from_env("SANCTUM_LIBEXEC", "/usr/lib/abyssal-sanctum")
}

/// Where the SANCTUM_DATA partition is mounted (ADR-0010).
pub(crate) fn data_mount() -> PathBuf {
    from_env("SANCTUM_DATA_MOUNT", "/sanctum")
}

/// Used for databases, cases and quarantine when no data partition is
/// present. It is in RAM: everything there is lost at reboot.
pub(crate) fn ram_workspace() -> PathBuf {
    from_env("SANCTUM_RAM_WORKSPACE", "/root/sanctum")
}

/// Where `sanctum mount` mounts targets: /mnt/sanctum/<device name>.
pub(crate) fn mount_base() -> PathBuf {
    from_env("SANCTUM_MOUNT_BASE", "/mnt/sanctum")
}

/// Runtime files: generated ClamAV configuration, the clamd socket,
/// extracted alternate data streams.
pub(crate) fn run_dir() -> PathBuf {
    from_env("SANCTUM_RUN_DIR", "/run/sanctum")
}

/// The root that /sys is read from for firmware state (Secure Boot, MOK
/// variables, kernel lockdown). Tests point it at a fake tree.
pub(crate) fn sys_root() -> PathBuf {
    from_env("SANCTUM_SYS_ROOT", "/")
}

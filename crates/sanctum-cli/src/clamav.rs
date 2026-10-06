//! ClamAV signature databases (AV-1): status, online updates, and offline
//! update packs.
//!
//! Databases live in the workspace (`/sanctum/clamav` on the data
//! partition, otherwise in RAM). Online updates use freshclam with
//! `ScriptedUpdates no`, so only complete, digitally signed .cvd files are
//! downloaded. Imported .cvd and .cud files must pass ClamAV's own signature
//! check (`sigtool --info`) before they are used.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Serialize;

use crate::data::{self, Workspace};
use crate::paths;
use crate::sys;
use crate::term::{self, outln};

/// Signed database containers.
const SIGNED: [&str; 2] = ["cvd", "cud"];
/// Incrementally updated containers: valid, but their signature cannot be
/// checked after updating.
const INCREMENTAL: [&str; 1] = ["cld"];
/// Plain signature files a technician may add (local or third-party rules).
const PLAIN: [&str; 22] = [
    "hdb", "hsb", "hdu", "hsu", "mdb", "msb", "mdu", "msu", "ndb", "ndu", "ldb", "ldu", "cdb",
    "idb", "fp", "sfp", "ign", "ign2", "cbc", "pdb", "wdb", "yar",
];

/// Signatures older than this many days are flagged.
pub(crate) const STALE_DAYS: i64 = 7;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct Database {
    pub(crate) file: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) build_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) age_days: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) signatures: Option<u64>,
    /// signed, incremental or local
    pub(crate) kind: String,
}

fn extension(path: &Path) -> String {
    path.extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

pub(crate) fn is_database(path: &Path) -> bool {
    let ext = extension(path);
    SIGNED.contains(&ext.as_str())
        || INCREMENTAL.contains(&ext.as_str())
        || PLAIN.contains(&ext.as_str())
}

pub(crate) fn database_dir(ws: &Workspace) -> Result<PathBuf, String> {
    ws.dir("clamav")
}

/// Fields of `sigtool --info` output.
pub(crate) fn parse_sigtool(output: &str) -> (Option<String>, Option<String>, Option<u64>, bool) {
    let field = |name: &str| {
        output
            .lines()
            .find_map(|l| l.strip_prefix(name))
            .map(|v| v.trim().to_owned())
    };
    (
        field("Version:"),
        field("Build time:"),
        field("Signatures:").and_then(|s| s.parse().ok()),
        output.contains("Verification OK"),
    )
}

fn days_since(build_time: &str) -> Option<i64> {
    let then: i64 = sys::output("date", &["-d", build_time, "+%s"])
        .ok()?
        .trim()
        .parse()
        .ok()?;
    let now: i64 = sys::output("date", &["+%s"]).ok()?.trim().parse().ok()?;
    Some((now - then) / 86_400)
}

fn describe(path: &Path) -> Database {
    let ext = extension(path);
    let file = path
        .file_name()
        .map(|f| f.to_string_lossy().into_owned())
        .unwrap_or_default();
    if PLAIN.contains(&ext.as_str()) {
        let signatures = std::fs::read_to_string(path)
            .map(|t| {
                t.lines()
                    .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
                    .count() as u64
            })
            .ok();
        return Database {
            file,
            version: None,
            build_time: None,
            age_days: None,
            signatures,
            kind: "local".to_owned(),
        };
    }
    let info = sys::output_or_empty("sigtool", &["--info", &path.to_string_lossy()]);
    let (version, build_time, signatures, _) = parse_sigtool(&info);
    let age_days = build_time.as_deref().and_then(days_since);
    let kind = if INCREMENTAL.contains(&ext.as_str()) {
        "incremental"
    } else {
        "signed"
    };
    Database {
        file,
        version,
        build_time,
        age_days,
        signatures,
        kind: kind.to_owned(),
    }
}

/// The databases in the workspace, sorted by file name.
pub(crate) fn databases(ws: &Workspace) -> Result<Vec<Database>, String> {
    let dir = database_dir(ws)?;
    let mut paths: Vec<PathBuf> = std::fs::read_dir(&dir)
        .map_err(|e| format!("cannot read {}: {e}", dir.display()))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file() && is_database(p))
        .collect();
    paths.sort();
    Ok(paths.iter().map(|p| describe(p)).collect())
}

/// Days since the newest signed or incremental database was built.
///
/// New signatures arrive in `daily`; `main` and `bytecode` are rebuilt
/// rarely, so their own age says nothing about how current the set is.
pub(crate) fn freshness(dbs: &[Database]) -> Option<i64> {
    dbs.iter()
        .filter(|db| db.kind != "local")
        .filter_map(|db| db.age_days)
        .min()
}

/// A warning when the newest signatures are older than [`STALE_DAYS`].
pub(crate) fn stale_warning(dbs: &[Database]) -> Option<String> {
    freshness(dbs)
        .filter(|d| *d > STALE_DAYS)
        .map(|d| format!("the newest ClamAV signatures are {d} days old; newer malware will be missed. Run `sanctum update`."))
}

pub(crate) fn status() -> Result<(), String> {
    let ws = data::workspace();
    data::warn_if_volatile(&ws);
    let dbs = databases(&ws)?;
    let dir = database_dir(&ws)?;
    if dbs.is_empty() {
        outln!("No ClamAV databases in {}.", dir.display());
        outln!(
            "Get them with `sanctum update` (online) or `sanctum update --import PACK` (offline)."
        );
        return Ok(());
    }
    outln!("ClamAV databases in {}:", dir.display());
    for db in &dbs {
        let age = db
            .age_days
            .map_or_else(String::new, |d| format!("built {d} days ago"));
        outln!(
            "  {:<22} {:<12} {:>10} signatures  {}",
            db.file,
            db.version
                .as_deref()
                .map_or_else(|| db.kind.clone(), |v| format!("version {v}")),
            db.signatures
                .map_or_else(|| "?".to_owned(), |s| s.to_string()),
            age
        );
    }
    match (stale_warning(&dbs), freshness(&dbs)) {
        (Some(warning), _) => outln!("{}", term::caution(&warning)),
        (None, Some(d)) => outln!(
            "{}",
            term::good(&format!("Signatures are current ({d} days old)."))
        ),
        (None, None) => {}
    }
    Ok(())
}

/// Download current databases with freshclam.
pub(crate) fn update_online() -> Result<(), String> {
    sys::require_root()?;
    let ws = data::workspace();
    data::warn_if_volatile(&ws);
    let dir = database_dir(&ws)?;
    let run = paths::run_dir();
    std::fs::create_dir_all(&run).map_err(|e| e.to_string())?;
    let conf = run.join("freshclam.conf");
    std::fs::write(
        &conf,
        format!(
            "# Generated by sanctum update.\n\
             DatabaseDirectory {}\n\
             DatabaseOwner root\n\
             DatabaseMirror database.clamav.net\n\
             # Complete, digitally signed .cvd files only (no incremental .cld).\n\
             ScriptedUpdates no\n\
             LogTime yes\n",
            dir.display()
        ),
    )
    .map_err(|e| format!("cannot write {}: {e}", conf.display()))?;
    outln!(
        "Downloading ClamAV databases into {} (over 100 MB)...",
        dir.display()
    );
    sys::run("freshclam", &[&format!("--config-file={}", conf.display())])?;
    status()
}

/// Import databases from an update pack (.tar) or a directory.
pub(crate) fn import(source: &Path) -> Result<(), String> {
    sys::require_root()?;
    let ws = data::workspace();
    data::warn_if_volatile(&ws);
    let dir = database_dir(&ws)?;
    let staging = paths::run_dir().join("import");
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(&staging).map_err(|e| e.to_string())?;

    let from = if source.is_dir() {
        source.to_path_buf()
    } else {
        sys::run(
            "tar",
            &[
                "--extract",
                "--no-same-owner",
                "--no-same-permissions",
                "--file",
                &source.to_string_lossy(),
                "--directory",
                &staging.to_string_lossy(),
            ],
        )?;
        staging.clone()
    };

    // A pack's SHA256SUMS must match, if present.
    let sums = from.join("SHA256SUMS");
    let expected: Vec<(String, String)> = std::fs::read_to_string(&sums)
        .map(|t| {
            t.lines()
                .filter_map(|l| l.split_once("  "))
                .map(|(h, f)| (f.trim().to_owned(), h.trim().to_owned()))
                .collect()
        })
        .unwrap_or_default();

    let mut candidates: Vec<PathBuf> = std::fs::read_dir(&from)
        .map_err(|e| format!("cannot read {}: {e}", from.display()))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file() && is_database(p))
        .collect();
    candidates.sort();
    if candidates.is_empty() {
        return Err(format!("no ClamAV database files in {}", source.display()));
    }

    // Verify everything first: one bad file means the pack was damaged or
    // tampered with, and nothing from it is trusted.
    let mut problems = Vec::new();
    let mut verified = Vec::new();
    for path in &candidates {
        let name = path
            .file_name()
            .map(|f| f.to_string_lossy().into_owned())
            .unwrap_or_default();
        if !expected.is_empty() {
            match expected.iter().find(|(f, _)| *f == name) {
                None => {
                    problems.push(format!("{name}: not listed in SHA256SUMS"));
                    continue;
                }
                Some((_, hash)) if &sys::sha256_file(path)? != hash => {
                    problems.push(format!("{name}: does not match SHA256SUMS"));
                    continue;
                }
                Some(_) => {}
            }
        }
        let ext = extension(path);
        if SIGNED.contains(&ext.as_str()) {
            let info = sys::output_or_empty("sigtool", &["--info", &path.to_string_lossy()]);
            if !parse_sigtool(&info).3 {
                problems.push(format!("{name}: ClamAV signature verification failed"));
                continue;
            }
        }
        verified.push((path, name, ext));
    }
    if !problems.is_empty() {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(format!(
            "nothing was imported; {} failed verification:\n  {}",
            source.display(),
            problems.join("\n  ")
        ));
    }

    for (path, name, ext) in verified {
        let target = dir.join(&name);
        let partial = dir.join(format!(".{name}.partial"));
        std::fs::copy(path, &partial)
            .and_then(|_| std::fs::rename(&partial, &target))
            .map_err(|e| {
                let _ = std::fs::remove_file(&partial);
                format!("cannot copy {name}: {e}")
            })?;
        let note = if SIGNED.contains(&ext.as_str()) {
            "signature verified"
        } else if INCREMENTAL.contains(&ext.as_str()) {
            "incremental database; its signature cannot be checked"
        } else {
            "local signatures, not signed"
        };
        outln!("  {name}: imported ({note})");
    }
    let _ = std::fs::remove_dir_all(&staging);
    Ok(())
}

/// Write an update pack (tar with SHA256SUMS and INFO) of the current
/// databases, for machines without Internet access.
pub(crate) fn export(destination: Option<&Path>) -> Result<(), String> {
    let ws = data::workspace();
    let dir = database_dir(&ws)?;
    let dbs = databases(&ws)?;
    if dbs.is_empty() {
        return Err("no databases to export; run `sanctum update` first".to_owned());
    }
    let date = sys::output("date", &["-u", "+%Y%m%d"])?.trim().to_owned();
    let pack = match destination {
        Some(p) if p.is_dir() => p.join(format!("sanctum-clamav-{date}.tar")),
        Some(p) => p.to_path_buf(),
        None => ws
            .dir("updates")?
            .join(format!("sanctum-clamav-{date}.tar")),
    };
    let mut sums = String::new();
    let mut info = format!("Abyssal Sanctum ClamAV update pack, created {date} (UTC).\n\n");
    for db in &dbs {
        let _ = writeln!(
            sums,
            "{}  {}",
            sys::sha256_file(&dir.join(&db.file))?,
            db.file
        );
        let _ = writeln!(
            info,
            "{}: {} {}",
            db.file,
            db.version
                .as_deref()
                .map_or_else(|| db.kind.clone(), |v| format!("version {v}")),
            db.build_time.as_deref().unwrap_or("")
        );
    }
    std::fs::write(dir.join("SHA256SUMS"), sums).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("INFO"), info).map_err(|e| e.to_string())?;
    let mut args: Vec<String> = vec![
        "--create".into(),
        "--file".into(),
        pack.to_string_lossy().into_owned(),
        "--directory".into(),
        dir.to_string_lossy().into_owned(),
        "SHA256SUMS".into(),
        "INFO".into(),
    ];
    args.extend(dbs.iter().map(|d| d.file.clone()));
    let status = Command::new("tar")
        .args(&args)
        .status()
        .map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(dir.join("SHA256SUMS"));
    let _ = std::fs::remove_file(dir.join("INFO"));
    if !status.success() {
        return Err(format!("tar failed ({status})"));
    }
    outln!("Wrote {} ({} databases).", pack.display(), dbs.len());
    outln!(
        "On another machine: sanctum update --import {}",
        pack.file_name()
            .map(|f| f.to_string_lossy())
            .unwrap_or_default()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_sigtool_info() {
        let out = "File: daily.cvd\nBuild time: 04 Oct 2026 08:22 -0400\nVersion: 27782\nSignatures: 2070123\n\
                   Functionality level: 90\nVerification OK.\n";
        let (version, build, sigs, ok) = parse_sigtool(out);
        assert_eq!(version.as_deref(), Some("27782"));
        assert_eq!(build.as_deref(), Some("04 Oct 2026 08:22 -0400"));
        assert_eq!(sigs, Some(2_070_123));
        assert!(ok);
        assert!(!parse_sigtool("File: x.cvd\nERROR: Can't verify database integrity\n").3);
    }

    #[test]
    fn recognises_database_files() {
        assert!(is_database(Path::new("main.cvd")));
        assert!(is_database(Path::new("daily.cld")));
        assert!(is_database(Path::new("sanctum-test.hdb")));
        assert!(!is_database(Path::new("notes.txt")));
        assert!(!is_database(Path::new("SHA256SUMS")));
    }

    fn db(file: &str, kind: &str, age: Option<i64>) -> Database {
        Database {
            file: file.to_owned(),
            version: None,
            build_time: None,
            age_days: age,
            signatures: None,
            kind: kind.to_owned(),
        }
    }

    #[test]
    fn freshness_follows_the_newest_signed_database() {
        let dbs = vec![
            db("bytecode.cvd", "signed", Some(389)),
            db("daily.cvd", "signed", Some(0)),
            db("main.cvd", "signed", Some(292)),
            db("local.hdb", "local", None),
        ];
        assert_eq!(freshness(&dbs), Some(0));
        assert!(stale_warning(&dbs).is_none());
        let old = vec![
            db("daily.cld", "incremental", Some(30)),
            db("main.cvd", "signed", Some(292)),
        ];
        assert!(stale_warning(&old).unwrap().contains("30 days"));
        assert_eq!(freshness(&[db("local.hdb", "local", None)]), None);
    }
}

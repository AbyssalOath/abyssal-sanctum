//! `sanctum`: the Abyssal Sanctum command-line interface.
//!
//! Run without arguments for the interactive menu. See `sanctum --help`
//! and docs/getting-started/navigation.md. Why this is in Rust: ADR-0008.

mod blockdev;
mod catalog;
mod clamav;
mod data;
mod disks;
mod docs;
mod menu;
mod mount;
mod paths;
mod render;
mod scan;
mod secureboot;
mod ssh;
mod sys;
mod targets;
mod term;

use crate::term::{out, outln};
use std::io::IsTerminal;
use std::os::unix::process::CommandExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use clap::{Parser, Subcommand};

use crate::catalog::Catalog;

const NAME: &str = "Abyssal Sanctum";
const DESCRIPTION: &str = "Arch Linux-based System Recovery Environment";
const TAGLINE: &str = "A safe environment you boot into for repairs.";
const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Parser)]
#[command(
    name = "sanctum",
    version,
    about = "Abyssal Sanctum - Arch Linux-based System Recovery Environment",
    long_about = "Abyssal Sanctum - Arch Linux-based System Recovery Environment.\n\
                  Run without a command for the interactive tool menu."
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Cmd>,
}

#[derive(Debug, Subcommand)]
enum Cmd {
    /// Show the Sanctum version and build information
    Version,
    /// List tools, all or in one category
    Tools {
        /// Category id, for example storage or forensics
        category: Option<String>,
    },
    /// Show one tool: what it does, how risky it is, examples
    Tool {
        /// Tool id, as shown by `sanctum tools`
        id: String,
        /// Start an interactive shell afterwards (used by the desktop menu)
        #[arg(long)]
        shell: bool,
    },
    /// List the guides, or read one
    Docs {
        /// Topic, for example recovery/linux
        topic: Option<String>,
    },
    /// Read-only overview of disks, partitions and SMART health
    Disks {
        /// Skip the SMART health query
        #[arg(long)]
        no_smart: bool,
    },
    /// Turn remote SSH access on or off
    Ssh {
        #[command(subcommand)]
        action: SshAction,
    },
    /// Check that Sanctum's safe defaults are in effect (read-only)
    Selftest,
    /// Find the installed systems (Windows, Linux) on this machine's disks
    Targets {
        /// Print JSON instead of a table
        #[arg(long)]
        json: bool,
    },
    /// Mount a target's filesystem: read-only unless --rw (BitLocker and
    /// LUKS are unlocked first)
    Mount {
        /// The partition, for example /dev/sda3
        device: String,
        /// Mount read-write (asks for confirmation; refused for a
        /// hibernated Windows volume)
        #[arg(long)]
        rw: bool,
        /// Where to mount it (default: /mnt/sanctum/<device name>)
        #[arg(long, value_name = "DIR")]
        at: Option<PathBuf>,
        /// Do not ask for confirmation (for scripts)
        #[arg(long)]
        yes: bool,
    },
    /// Unmount what `sanctum mount` mounted, and lock encrypted volumes again
    Umount {
        /// A mount point or device; may be left out when only one is mounted
        what: Option<String>,
        /// Unmount everything Sanctum mounted
        #[arg(long)]
        all: bool,
    },
    /// Scan mounted targets for malware with ClamAV and write a case report
    Scan {
        /// Directories to scan, for example /mnt/sanctum/sda3
        #[arg(required = true)]
        paths: Vec<PathBuf>,
        /// A name for the case (default: scan)
        #[arg(long)]
        case: Option<String>,
        /// Do not check NTFS alternate data streams
        #[arg(long)]
        no_streams: bool,
    },
    /// ClamAV databases: download, import or export an offline update pack
    Update {
        /// Show the installed databases and their age
        #[arg(long, conflicts_with_all = ["import", "export"])]
        status: bool,
        /// Import an update pack (.tar) or a folder of database files
        #[arg(long, value_name = "PACK", conflicts_with = "export")]
        import: Option<PathBuf>,
        /// Write an update pack of the current databases (default: the
        /// data partition's updates folder)
        // A String, not a PathBuf: clap's path parser rejects the empty
        // value that stands for "no path given".
        #[arg(long, value_name = "PATH", num_args = 0..=1, default_missing_value = "")]
        export: Option<String>,
    },
    /// Secure Boot state, and removing the Sanctum key from a machine
    Secureboot {
        #[command(subcommand)]
        action: SecurebootAction,
    },
    /// The Sanctum data partition (label SANCTUM_DATA)
    Data {
        #[command(subcommand)]
        action: DataAction,
    },
    /// Check or verify the tool catalog
    Catalog {
        #[command(subcommand)]
        action: CatalogAction,
    },
    /// Generate menu entries or HTML guides (used by the ISO build)
    #[command(hide = true)]
    Render {
        #[command(subcommand)]
        what: RenderAction,
    },
}

#[derive(Debug, Subcommand)]
enum DataAction {
    /// Show whether a data partition is mounted, and what it holds
    Status,
    /// ERASE a disk or partition and make it the Sanctum data partition
    Init {
        /// For example /dev/sdb (a whole USB stick) or /dev/sdb1
        device: String,
        /// Do not ask for confirmation (for scripts)
        #[arg(long)]
        yes: bool,
    },
}

#[derive(Debug, Subcommand)]
enum SshAction {
    /// Set a way to log in, start sshd and open port 22
    Enable {
        /// Authorize this OpenSSH public key instead of setting a password
        #[arg(long, value_name = "FILE")]
        key: Option<PathBuf>,
    },
    /// Stop sshd and close port 22
    Disable,
    /// Show whether SSH is on
    Status,
}

#[derive(Debug, Subcommand)]
enum SecurebootAction {
    /// Show Secure Boot, the boot chain and whether the Sanctum key is enrolled
    Status,
    /// Ask shim to remove the Sanctum key from this machine at the next boot
    Forget,
}

#[derive(Debug, Subcommand)]
enum CatalogAction {
    /// Check the catalog's structure, packages and guides (repository)
    Check {
        #[arg(long, default_value = "catalog")]
        catalog: PathBuf,
        #[arg(long, default_value = "build/packages")]
        packages: PathBuf,
        #[arg(long, default_value = "docs")]
        docs: PathBuf,
    },
    /// Check that every tool is installed and starts (live system)
    Verify,
}

#[derive(Debug, Subcommand)]
enum RenderAction {
    /// Write .desktop, .directory and menu files under ROOT
    Desktop {
        #[arg(long)]
        catalog: PathBuf,
        #[arg(long)]
        root: PathBuf,
    },
    /// Render the Markdown guides under SRC to HTML in OUT
    Docs {
        #[arg(long)]
        src: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
}

fn version_text() -> String {
    let mut text = format!("{NAME} v{VERSION}\n{DESCRIPTION}\n");
    if let Ok(raw) = std::fs::read_to_string(paths::manifest())
        && let Ok(manifest) = serde_json::from_str::<serde_json::Value>(&raw)
    {
        let field = |k: &str| {
            manifest
                .get(k)
                .and_then(serde_json::Value::as_str)
                .unwrap_or("unknown")
                .to_owned()
        };
        text.push_str(&format!(
            "Build: commit {}, Arch Linux packages from the {} snapshot\n",
            field("git_commit"),
            field("snapshot_date")
        ));
    }
    text
}

fn load_catalog() -> Result<Catalog, String> {
    Catalog::load(&paths::catalog_dir())
}

fn catalog_check(catalog_dir: &Path, packages: &Path, docs: &Path) -> Result<(), String> {
    let catalog = Catalog::load(catalog_dir)?;
    let packages = catalog::read_package_lists(packages)?;
    let mut problems = catalog.validate();
    problems.extend(catalog.check_packages(&packages));
    problems.extend(catalog.check_docs(docs));
    if problems.is_empty() {
        outln!(
            "Catalog OK: {} tools in {} categories",
            catalog.tools().count(),
            catalog.sections.len()
        );
        Ok(())
    } else {
        Err(format!("catalog problems:\n  {}", problems.join("\n  ")))
    }
}

/// Run the command; the result is the process exit code.
fn run(cli: Cli) -> Result<u8, String> {
    match cli.command {
        Some(Cmd::Scan {
            paths,
            case,
            no_streams,
        }) => scan::run(&paths, case.as_deref(), !no_streams),
        other => run_command(other).map(|()| 0),
    }
}

fn run_command(command: Option<Cmd>) -> Result<(), String> {
    match command {
        None => {
            let catalog = load_catalog()?;
            if std::io::stdin().is_terminal() {
                let header = format!(
                    "{}  {}\n{}",
                    term::accent(&format!("{NAME} v{VERSION}")),
                    DESCRIPTION,
                    term::dim(TAGLINE)
                );
                menu::interactive(&catalog, &header);
                Ok(())
            } else {
                menu::list(&catalog, None)
            }
        }
        Some(Cmd::Version) => {
            out!("{}", version_text());
            Ok(())
        }
        Some(Cmd::Tools { category }) => menu::list(&load_catalog()?, category.as_deref()),
        Some(Cmd::Tool { id, shell }) => {
            let catalog = load_catalog()?;
            let (category, tool) = catalog
                .find(&id)
                .ok_or_else(|| format!("no tool '{id}' (run `sanctum tools` for the list)"))?;
            menu::print_tool(&category.name, tool);
            if shell {
                outln!();
                let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_owned());
                let err = Command::new(&shell).exec();
                return Err(format!("cannot start {shell}: {err}"));
            }
            Ok(())
        }
        Some(Cmd::Docs { topic }) => docs::run(&paths::docs_dir(), topic.as_deref()),
        Some(Cmd::Disks { no_smart }) => disks::run(!no_smart),
        Some(Cmd::Ssh { action }) => match action {
            SshAction::Enable { key } => ssh::enable(key.as_deref()),
            SshAction::Disable => ssh::disable(),
            SshAction::Status => ssh::status(),
        },
        Some(Cmd::Secureboot { action }) => match action {
            SecurebootAction::Status => secureboot::status(),
            SecurebootAction::Forget => secureboot::forget(),
        },
        Some(Cmd::Targets { json }) => targets::run(json),
        Some(Cmd::Mount {
            device,
            rw,
            at,
            yes,
        }) => mount::mount(&device, rw, at.as_deref(), yes),
        Some(Cmd::Umount { what, all }) => mount::umount(what.as_deref(), all),
        Some(Cmd::Scan { .. }) => Err("internal error: scan is handled by run()".to_owned()),
        Some(Cmd::Update {
            status,
            import,
            export,
        }) => match (status, import, export) {
            (true, _, _) => clamav::status(),
            (_, Some(pack), _) => clamav::import(&pack).and_then(|()| clamav::status()),
            (_, _, Some(path)) => clamav::export((!path.is_empty()).then(|| Path::new(&path))),
            _ => clamav::update_online(),
        },
        Some(Cmd::Data { action }) => match action {
            DataAction::Status => data::status(),
            DataAction::Init { device, yes } => data::init(&device, yes),
        },
        Some(Cmd::Selftest) => {
            let script = paths::selftest();
            let err = Command::new(&script).exec();
            Err(format!("cannot run {}: {err}", script.display()))
        }
        Some(Cmd::Catalog { action }) => match action {
            CatalogAction::Check {
                catalog,
                packages,
                docs,
            } => catalog_check(&catalog, &packages, &docs),
            CatalogAction::Verify => {
                let catalog = load_catalog()?;
                let failures = catalog.verify();
                if failures.is_empty() {
                    outln!(
                        "All {} catalog tools are installed and start",
                        catalog.tools().count()
                    );
                    Ok(())
                } else {
                    Err(format!(
                        "catalog tools failing:\n  {}",
                        failures.join("\n  ")
                    ))
                }
            }
        },
        Some(Cmd::Render { what }) => match what {
            RenderAction::Desktop { catalog, root } => {
                let catalog = Catalog::load(&catalog)?;
                let count = render::write_desktop(&catalog, &root)?;
                outln!("Wrote {count} menu entries under {}", root.display());
                Ok(())
            }
            RenderAction::Docs { src, out } => {
                let count = docs::render_html(&src, &out)?;
                outln!("Wrote {count} HTML pages to {}", out.display());
                Ok(())
            }
        },
    }
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(code) => ExitCode::from(code),
        Err(message) => {
            eprintln!("sanctum: {message}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn version_identifies_the_environment() {
        let text = version_text();
        assert!(text.starts_with("Abyssal Sanctum v"));
        assert!(text.contains("Arch Linux-based System Recovery Environment"));
    }

    #[test]
    fn subcommands_parse() {
        let cli =
            Cli::try_parse_from(["sanctum", "ssh", "enable", "--key", "k.pub"]).expect("parses");
        assert!(matches!(
            cli.command,
            Some(Cmd::Ssh {
                action: SshAction::Enable { key: Some(_) }
            })
        ));
        assert!(Cli::try_parse_from(["sanctum", "bogus"]).is_err());
    }

    fn export_arg(args: &[&str]) -> Option<String> {
        match Cli::try_parse_from(args).expect("parses").command {
            Some(Cmd::Update { export, .. }) => export,
            _ => panic!("not an update command"),
        }
    }

    #[test]
    fn update_export_path_is_optional() {
        assert_eq!(
            export_arg(&["sanctum", "update", "--export"]).as_deref(),
            Some("")
        );
        assert_eq!(
            export_arg(&["sanctum", "update", "--export", "/tmp/x"]).as_deref(),
            Some("/tmp/x")
        );
        assert_eq!(export_arg(&["sanctum", "update"]), None);
    }
}

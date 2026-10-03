//! `sanctum`: the Abyssal Sanctum command-line interface.
//!
//! Run without arguments for the interactive menu. See `sanctum --help`
//! and docs/getting-started/navigation.md. Why this is in Rust: ADR-0008.

mod catalog;
mod disks;
mod docs;
mod menu;
mod paths;
mod render;
mod ssh;
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

fn run(cli: Cli) -> Result<(), String> {
    match cli.command {
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
        Ok(()) => ExitCode::SUCCESS,
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
}

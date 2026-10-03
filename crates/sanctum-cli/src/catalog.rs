//! The tool catalog: `catalog/<category>.toml`, one file per category.
//!
//! The catalog drives the `sanctum` menu and tool listings, the Xfce menu
//! entries, and the self-test's check that every listed tool is present.
//! The format is documented in docs/operations/catalog.md.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::Deserialize;

/// The categories, in menu order. Every one must have a catalog file.
pub(crate) const CATEGORY_ORDER: [&str; 10] = [
    "recovery",
    "storage",
    "hardware",
    "networking",
    "security",
    "forensics",
    "windows",
    "linux",
    "documentation",
    "terminal",
];

/// Package name for tools that Sanctum itself provides.
pub(crate) const BUILTIN_PACKAGE: &str = "sanctum";

/// What a tool can do to data on disks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Risk {
    /// Never changes existing data.
    ReadOnly,
    /// Changes data or settings in a controlled way.
    Modifies,
    /// Can erase or overwrite disk contents with one command.
    Destructive,
}

impl Risk {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Risk::ReadOnly => "read-only",
            Risk::Modifies => "modifies",
            Risk::Destructive => "destructive",
        }
    }

    pub(crate) fn meaning(self) -> &'static str {
        match self {
            Risk::ReadOnly => "never changes existing data",
            Risk::Modifies => "changes data or settings when you tell it to",
            Risk::Destructive => "can erase or overwrite disk contents with one command",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Category {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) summary: String,
    pub(crate) icon: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Tool {
    pub(crate) id: String,
    pub(crate) name: String,
    /// The Arch package that provides it, or "sanctum".
    pub(crate) package: String,
    /// The executable that must exist on the live system.
    pub(crate) command: String,
    pub(crate) risk: Risk,
    pub(crate) summary: String,
    /// A graphical application, started from the Xfce menu.
    #[serde(default)]
    pub(crate) gui: bool,
    /// Command line for graphical tools, if not just `command`.
    pub(crate) exec: Option<String>,
    pub(crate) icon: Option<String>,
    pub(crate) note: Option<String>,
    #[serde(default)]
    pub(crate) usage: Vec<String>,
    /// A harmless command (usually a version query) that must succeed.
    pub(crate) check: Option<Vec<String>>,
    /// A documentation topic, such as "recovery/linux".
    pub(crate) docs: Option<String>,
    /// Further categories the tool is listed in.
    #[serde(default)]
    pub(crate) also: Vec<String>,
}

impl Tool {
    /// The command line a graphical tool is started with.
    pub(crate) fn gui_exec(&self) -> String {
        self.exec.clone().unwrap_or_else(|| self.command.clone())
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CategoryFile {
    category: Category,
    #[serde(default, rename = "tool")]
    tools: Vec<Tool>,
}

#[derive(Debug)]
pub(crate) struct Section {
    pub(crate) category: Category,
    pub(crate) tools: Vec<Tool>,
}

#[derive(Debug)]
pub(crate) struct Catalog {
    pub(crate) sections: Vec<Section>,
}

impl Catalog {
    /// Load every category file from `dir`, in menu order.
    pub(crate) fn load(dir: &Path) -> Result<Catalog, String> {
        let mut sections = Vec::new();
        for id in CATEGORY_ORDER {
            let path = dir.join(format!("{id}.toml"));
            let text = fs::read_to_string(&path)
                .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
            let file: CategoryFile =
                toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
            if file.category.id != id {
                return Err(format!(
                    "{}: category id is '{}', expected '{id}'",
                    path.display(),
                    file.category.id
                ));
            }
            sections.push(Section {
                category: file.category,
                tools: file.tools,
            });
        }
        let entries =
            fs::read_dir(dir).map_err(|e| format!("cannot read {}: {e}", dir.display()))?;
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if let Some(stem) = name.strip_suffix(".toml")
                && !CATEGORY_ORDER.contains(&stem)
            {
                return Err(format!("{name}: '{stem}' is not a known category"));
            }
        }
        Ok(Catalog { sections })
    }

    pub(crate) fn tools(&self) -> impl Iterator<Item = &Tool> {
        self.sections.iter().flat_map(|s| s.tools.iter())
    }

    pub(crate) fn section(&self, id: &str) -> Option<&Section> {
        self.sections.iter().find(|s| s.category.id == id)
    }

    pub(crate) fn find(&self, id: &str) -> Option<(&Category, &Tool)> {
        self.sections.iter().find_map(|s| {
            s.tools
                .iter()
                .find(|t| t.id == id)
                .map(|t| (&s.category, t))
        })
    }

    /// The tools listed in a category: its own, then those from other
    /// categories that name it in `also`.
    pub(crate) fn tools_in(&self, category: &str) -> Vec<&Tool> {
        let mut tools: Vec<&Tool> = self
            .section(category)
            .map(|s| s.tools.iter().collect())
            .unwrap_or_default();
        tools.extend(
            self.sections
                .iter()
                .filter(|s| s.category.id != category)
                .flat_map(|s| s.tools.iter())
                .filter(|t| t.also.iter().any(|a| a == category)),
        );
        tools
    }

    /// Structural problems in the catalog itself.
    pub(crate) fn validate(&self) -> Vec<String> {
        let mut problems = Vec::new();
        let mut seen: BTreeMap<&str, &str> = BTreeMap::new();
        for section in &self.sections {
            let cat = section.category.id.as_str();
            if section.tools.is_empty() {
                problems.push(format!("{cat}: the category has no tools"));
            }
            for tool in &section.tools {
                let at = format!("{cat}/{}", tool.id);
                if !valid_id(&tool.id) {
                    problems.push(format!(
                        "{at}: id must be lowercase letters, digits and dashes"
                    ));
                }
                if let Some(other) = seen.insert(&tool.id, cat) {
                    problems.push(format!("{at}: id is already used in {other}"));
                }
                for field in [&tool.name, &tool.summary, &tool.command, &tool.package] {
                    if field.trim().is_empty() {
                        problems.push(format!(
                            "{at}: name, summary, command and package are required"
                        ));
                        break;
                    }
                }
                if tool.command.contains('/') || tool.command.contains(' ') {
                    problems.push(format!("{at}: command must be a bare executable name"));
                }
                if tool.gui {
                    if !tool.usage.is_empty() {
                        problems.push(format!("{at}: graphical tools have no usage examples"));
                    }
                } else {
                    if tool.usage.is_empty() {
                        problems.push(format!("{at}: give at least one usage example"));
                    }
                    if tool.exec.is_some() {
                        problems.push(format!("{at}: exec is only for graphical tools"));
                    }
                }
                if let Some(exec) = &tool.exec
                    && exec
                        .chars()
                        .any(|c| matches!(c, '"' | '\'' | '`' | '$' | '\\' | '%'))
                {
                    problems.push(format!("{at}: exec must not contain quotes, $, \\ or %"));
                }
                if tool.check.as_ref().is_some_and(Vec::is_empty) {
                    problems.push(format!("{at}: check must not be empty"));
                }
                let mut also_seen = BTreeSet::new();
                for also in &tool.also {
                    if !CATEGORY_ORDER.contains(&also.as_str()) {
                        problems.push(format!("{at}: also names unknown category '{also}'"));
                    } else if also == cat {
                        problems.push(format!("{at}: also names its own category"));
                    } else if !also_seen.insert(also) {
                        problems.push(format!("{at}: also names '{also}' twice"));
                    }
                }
            }
        }
        problems
    }

    /// Tools whose package is not in the ISO's package lists.
    pub(crate) fn check_packages(&self, packages: &BTreeSet<String>) -> Vec<String> {
        self.tools()
            .filter(|t| t.package != BUILTIN_PACKAGE && !packages.contains(&t.package))
            .map(|t| {
                format!(
                    "{}: package '{}' is not in build/packages/*.list",
                    t.id, t.package
                )
            })
            .collect()
    }

    /// Tools that refer to documentation topics that do not exist.
    pub(crate) fn check_docs(&self, docs_dir: &Path) -> Vec<String> {
        self.tools()
            .filter_map(|t| t.docs.as_ref().map(|d| (t, d)))
            .filter(|(_, topic)| !docs_dir.join(format!("{topic}.md")).is_file())
            .map(|(t, topic)| format!("{}: docs topic '{topic}' does not exist", t.id))
            .collect()
    }

    /// On the live system: every tool's command exists, and its check
    /// command (if any) succeeds. Returns one line per failure.
    pub(crate) fn verify(&self) -> Vec<String> {
        let mut failures = Vec::new();
        for tool in self.tools() {
            if find_executable(&tool.command).is_none() {
                failures.push(format!("{}: command '{}' not found", tool.id, tool.command));
                continue;
            }
            if let Some(check) = &tool.check
                && let Err(e) = run_check(check)
            {
                let mut line = format!("{}: '{}' failed: ", tool.id, check.join(" "));
                let _ = write!(line, "{e}");
                failures.push(line);
            }
        }
        failures
    }
}

fn valid_id(id: &str) -> bool {
    let mut chars = id.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// Directories searched in addition to PATH. Perl tools such as exiftool
/// live in vendor_perl, which only login shells add to PATH.
const EXTRA_BIN_DIRS: [&str; 4] = [
    "/usr/local/bin",
    "/usr/bin",
    "/usr/bin/vendor_perl",
    "/usr/bin/core_perl",
];

pub(crate) fn find_executable(name: &str) -> Option<PathBuf> {
    use std::os::unix::fs::PermissionsExt as _;
    let path_var = std::env::var_os("PATH").unwrap_or_default();
    std::env::split_paths(&path_var)
        .chain(EXTRA_BIN_DIRS.iter().map(PathBuf::from))
        .map(|dir| dir.join(name))
        .find(|p| {
            fs::metadata(p)
                .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
                .unwrap_or(false)
        })
}

/// Run a check command with no input or output, for at most 30 seconds.
fn run_check(argv: &[String]) -> Result<(), String> {
    let (program, args) = argv.split_first().ok_or("empty command")?;
    let program = find_executable(program).ok_or("not found")?;
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        match child.try_wait().map_err(|e| e.to_string())? {
            Some(status) if status.success() => return Ok(()),
            Some(status) => return Err(status.to_string()),
            None if Instant::now() > deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("timed out".to_owned());
            }
            None => std::thread::sleep(Duration::from_millis(50)),
        }
    }
}

/// Package names from build/packages/*.list (ADR-0003 format).
pub(crate) fn read_package_lists(dir: &Path) -> Result<BTreeSet<String>, String> {
    let mut packages = BTreeSet::new();
    let entries = fs::read_dir(dir).map_err(|e| format!("cannot read {}: {e}", dir.display()))?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "list") {
            let text = fs::read_to_string(&path)
                .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
            for line in text.lines() {
                let name = line.split('#').next().unwrap_or("").trim();
                if !name.is_empty() {
                    packages.insert(name.to_owned());
                }
            }
        }
    }
    Ok(packages)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo_dir(rel: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(rel)
    }

    fn repo_catalog() -> Catalog {
        Catalog::load(&repo_dir("catalog")).expect("the repository catalog loads")
    }

    #[test]
    fn repository_catalog_is_valid() {
        let catalog = repo_catalog();
        assert_eq!(catalog.sections.len(), CATEGORY_ORDER.len());
        assert_eq!(catalog.validate(), Vec::<String>::new());
    }

    #[test]
    fn repository_catalog_matches_package_lists_and_docs() {
        let catalog = repo_catalog();
        let packages = read_package_lists(&repo_dir("build/packages")).expect("package lists");
        assert_eq!(catalog.check_packages(&packages), Vec::<String>::new());
        assert_eq!(catalog.check_docs(&repo_dir("docs")), Vec::<String>::new());
    }

    #[test]
    fn also_lists_tools_in_other_categories() {
        let catalog = repo_catalog();
        let forensics: Vec<&str> = catalog
            .tools_in("forensics")
            .iter()
            .map(|t| t.id.as_str())
            .collect();
        assert!(forensics.contains(&"sleuthkit"));
        assert!(
            forensics.contains(&"photorec"),
            "photorec is listed via also"
        );
    }

    #[test]
    fn validation_reports_problems() {
        let text = r#"
            [category]
            id = "recovery"
            name = "Recovery"
            summary = "s"
            icon = "i"

            [[tool]]
            id = "Bad_Id"
            name = "n"
            package = "p"
            command = "/usr/bin/x"
            risk = "read-only"
            summary = "s"
            also = ["recovery", "nowhere"]

            [[tool]]
            id = "dup"
            name = "n"
            package = "p"
            command = "x"
            risk = "modifies"
            summary = "s"
            usage = ["x"]

            [[tool]]
            id = "dup"
            name = "n"
            package = "p"
            command = "x"
            gui = true
            exec = "x $HOME"
            risk = "destructive"
            summary = "s"
        "#;
        let file: CategoryFile = toml::from_str(text).expect("parses");
        let catalog = Catalog {
            sections: vec![Section {
                category: file.category,
                tools: file.tools,
            }],
        };
        let problems = catalog.validate().join("\n");
        for expected in [
            "id must be lowercase",
            "bare executable",
            "at least one usage",
            "own category",
            "unknown category 'nowhere'",
            "already used",
            "must not contain quotes",
        ] {
            assert!(
                problems.contains(expected),
                "missing '{expected}' in:\n{problems}"
            );
        }
    }

    #[test]
    fn unknown_fields_are_rejected() {
        let text = r#"
            [category]
            id = "recovery"
            name = "Recovery"
            summary = "s"
            icon = "i"
            colour = "blue"
        "#;
        assert!(toml::from_str::<CategoryFile>(text).is_err());
    }
}

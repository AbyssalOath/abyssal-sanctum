//! Offline documentation: `sanctum docs` lists and pages the guides in the
//! terminal, and `sanctum render docs` turns them into HTML at build time
//! for the browser.

use crate::term::{out, outln};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::process::Command;

use pulldown_cmark::{CowStr, Event, Options, Parser, Tag, TagEnd};

use crate::term;

/// Sections of docs/, in the order they are listed. Others follow.
const SECTION_ORDER: [&str; 6] = [
    "getting-started",
    "recovery",
    "security",
    "operations",
    "architecture",
    "development",
];

#[derive(Debug)]
pub(crate) struct Topic {
    /// "recovery/linux" for docs/recovery/linux.md.
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) path: PathBuf,
}

fn section_rank(id: &str) -> usize {
    let section = id.split('/').next().unwrap_or("");
    SECTION_ORDER
        .iter()
        .position(|s| *s == section)
        .unwrap_or(SECTION_ORDER.len())
}

/// The first level-one heading of a Markdown file, or its file name.
fn title_of(text: &str, fallback: &str) -> String {
    text.lines()
        .find_map(|l| l.strip_prefix("# "))
        .map_or_else(|| fallback.to_owned(), |t| t.trim().to_owned())
}

fn markdown_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            markdown_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "md") {
            out.push(path);
        }
    }
}

/// Every guide under `<root>/docs`, grouped by section.
pub(crate) fn topics(root: &Path) -> Vec<Topic> {
    let docs = root.join("docs");
    let mut files = Vec::new();
    markdown_files(&docs, &mut files);
    let mut topics: Vec<Topic> = files
        .into_iter()
        .filter_map(|path| {
            let rel = path.strip_prefix(&docs).ok()?.with_extension("");
            let id = rel.to_string_lossy().into_owned();
            let text = fs::read_to_string(&path).ok()?;
            Some(Topic {
                title: title_of(&text, &id),
                id,
                path,
            })
        })
        .collect();
    topics.sort_by(|a, b| {
        section_rank(&a.id)
            .cmp(&section_rank(&b.id))
            .then_with(|| a.id.cmp(&b.id))
    });
    topics
}

/// `sanctum docs [TOPIC]`.
pub(crate) fn run(root: &Path, topic: Option<&str>) -> Result<(), String> {
    let all = topics(root);
    if all.is_empty() {
        return Err(format!("no documentation found in {}", root.display()));
    }
    let Some(wanted) = topic else {
        let mut current = "";
        for t in &all {
            let section = t.id.split('/').next().unwrap_or("");
            if section != current {
                outln!("\n{}", term::bold(section));
                current = section;
            }
            outln!("  {:<38} {}", t.id, t.title);
        }
        outln!(
            "\nRead one with: sanctum docs TOPIC\nIn the browser: {}",
            root.join("html/index.html").display()
        );
        return Ok(());
    };
    let found = all
        .iter()
        .find(|t| t.id == wanted)
        .or_else(|| {
            let matches: Vec<&Topic> = all
                .iter()
                .filter(|t| t.id.ends_with(&format!("/{wanted}")))
                .collect();
            (matches.len() == 1).then(|| matches[0])
        })
        .ok_or_else(|| format!("no topic '{wanted}' (run `sanctum docs` for the list)"))?;
    page(&found.path)
}

fn page(path: &Path) -> Result<(), String> {
    if std::io::stdout().is_terminal() {
        let pager = std::env::var("PAGER").unwrap_or_else(|_| "less".to_owned());
        let status = Command::new(&pager)
            .arg(path)
            .status()
            .map_err(|e| format!("cannot start {pager}: {e}"))?;
        if !status.success() {
            return Err(format!("{pager} exited with {status}"));
        }
        Ok(())
    } else {
        let text =
            fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        out!("{text}");
        Ok(())
    }
}

/// GitHub-style heading anchor: lowercase, spaces to dashes, punctuation
/// dropped. Matches the #fragments used in the Markdown sources.
pub(crate) fn slug(text: &str) -> String {
    text.chars()
        .filter_map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                Some(c.to_lowercase().next().unwrap_or(c))
            } else if c == ' ' {
                Some('-')
            } else {
                None
            }
        })
        .collect()
}

/// Rewrite a relative link to a Markdown file so it points to the HTML page.
pub(crate) fn rewrite_link(dest: &str) -> String {
    if dest.contains("://") || dest.starts_with('#') || dest.starts_with("mailto:") {
        return dest.to_owned();
    }
    let (path, fragment) = dest
        .split_once('#')
        .map_or((dest, None), |(p, f)| (p, Some(f)));
    match path.strip_suffix(".md") {
        Some(stem) => match fragment {
            Some(f) => format!("{stem}.html#{f}"),
            None => format!("{stem}.html"),
        },
        None => dest.to_owned(),
    }
}

/// Render one Markdown document to an HTML body.
pub(crate) fn markdown_to_html(text: &str) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    let events: Vec<Event<'_>> = Parser::new_ext(text, options).collect();

    // Give every heading an id, as GitHub does, so #fragments work.
    let mut used: BTreeMap<String, usize> = BTreeMap::new();
    let mut out: Vec<Event<'_>> = Vec::with_capacity(events.len());
    let mut i = 0;
    while i < events.len() {
        match &events[i] {
            Event::Start(Tag::Heading {
                level,
                id: None,
                classes,
                attrs,
            }) => {
                let mut text = String::new();
                for e in &events[i + 1..] {
                    match e {
                        Event::End(TagEnd::Heading(_)) => break,
                        Event::Text(t) | Event::Code(t) => text.push_str(t),
                        _ => {}
                    }
                }
                let base = slug(&text);
                let n = used.entry(base.clone()).or_insert(0);
                let id = if *n == 0 { base } else { format!("{base}-{n}") };
                *n += 1;
                out.push(Event::Start(Tag::Heading {
                    level: *level,
                    id: Some(CowStr::from(id)),
                    classes: classes.clone(),
                    attrs: attrs.clone(),
                }));
            }
            Event::Start(Tag::Link {
                link_type,
                dest_url,
                title,
                id,
            }) => out.push(Event::Start(Tag::Link {
                link_type: *link_type,
                dest_url: CowStr::from(rewrite_link(dest_url)),
                title: title.clone(),
                id: id.clone(),
            })),
            other => out.push(other.clone()),
        }
        i += 1;
    }
    let mut html = String::new();
    pulldown_cmark::html::push_html(&mut html, out.into_iter());
    html
}

const STYLE: &str = "\
:root{color-scheme:dark;--bg:#0b0e14;--fg:#d6dbe4;--muted:#8b93a3;--accent:#4fa8f5;--line:#232a36;--code:#141922}
body{margin:0;background:var(--bg);color:var(--fg);font:16px/1.6 'DejaVu Sans',sans-serif}
main{max-width:52rem;margin:0 auto;padding:1.5rem 1rem 4rem}
header{border-bottom:1px solid var(--line);padding:.75rem 1rem;font-size:.9rem;color:var(--muted)}
header a{color:var(--accent);text-decoration:none;font-weight:bold}
a{color:var(--accent)}
h1,h2,h3{line-height:1.25}h1{font-size:1.8rem}h2{border-bottom:1px solid var(--line);padding-bottom:.25rem;margin-top:2rem}
code,pre{font-family:'DejaVu Sans Mono',monospace;font-size:.9em;background:var(--code)}
code{padding:.1em .3em;border-radius:3px}pre{padding:.75rem 1rem;overflow-x:auto;border:1px solid var(--line);border-radius:4px}pre code{padding:0}
table{border-collapse:collapse;width:100%;font-size:.95em}th,td{border:1px solid var(--line);padding:.4rem .6rem;text-align:left;vertical-align:top}th{background:var(--code)}
blockquote{margin:1rem 0;padding:.25rem 1rem;border-left:3px solid var(--accent);color:var(--muted)}
";

fn html_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn page_html(title: &str, index_href: &str, body: &str) -> String {
    format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <title>{title} - Abyssal Sanctum</title>\n<style>\n{STYLE}</style>\n</head>\n<body>\n\
         <header><a href=\"{index_href}\">Abyssal Sanctum</a> &middot; guides (offline)</header>\n\
         <main>\n{body}</main>\n</body>\n</html>\n",
        title = html_escape(title),
    )
}

/// Render every Markdown file under `src` (README.md, docs/, ...) into
/// `out`, mirroring the layout, plus an index page. Returns the page count.
pub(crate) fn render_html(src: &Path, out: &Path) -> Result<usize, String> {
    let mut files = Vec::new();
    markdown_files(src, &mut files);
    files.retain(|p| !p.starts_with(out));
    let mut count = 0;
    for path in &files {
        let rel = path
            .strip_prefix(src)
            .map_err(|e| e.to_string())?
            .with_extension("html");
        let text =
            fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        let depth = rel.components().count() - 1;
        let index_href = format!("{}index.html", "../".repeat(depth));
        let title = title_of(&text, &rel.to_string_lossy());
        let target = out.join(&rel);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
        }
        fs::write(
            &target,
            page_html(&title, &index_href, &markdown_to_html(&text)),
        )
        .map_err(|e| format!("cannot write {}: {e}", target.display()))?;
        count += 1;
    }

    let mut body = String::from(
        "<h1>Abyssal Sanctum guides</h1>\n<p>Arch Linux-based System Recovery Environment. \
         These pages are on the boot medium and work offline. In a terminal, \
         <code>sanctum docs</code> lists the same guides.</p>\n",
    );
    let mut current = String::new();
    for topic in topics(src) {
        let section = topic.id.split('/').next().unwrap_or("").to_owned();
        if section != current {
            if !current.is_empty() {
                body.push_str("</ul>\n");
            }
            let _ = writeln!(body, "<h2>{}</h2>\n<ul>", html_escape(&section));
            current = section;
        }
        let _ = writeln!(
            body,
            "<li><a href=\"docs/{}.html\">{}</a></li>",
            topic.id,
            html_escape(&topic.title)
        );
    }
    if !current.is_empty() {
        body.push_str("</ul>\n");
    }
    body.push_str(
        "<h2>Project</h2>\n<ul>\n<li><a href=\"README.html\">README</a></li>\n\
         <li><a href=\"ROADMAP.html\">Roadmap</a></li>\n<li><a href=\"CHANGELOG.html\">Changelog</a></li>\n\
         <li><a href=\"SECURITY.html\">Security policy</a></li>\n</ul>\n",
    );
    let index = out.join("index.html");
    fs::write(&index, page_html("Guides", "index.html", &body))
        .map_err(|e| format!("cannot write {}: {e}", index.display()))?;
    Ok(count + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs_match_github_anchors() {
        assert_eq!(
            slug("2.4 Profile base: copy `releng`, then remove"),
            "24-profile-base-copy-releng-then-remove"
        );
        assert_eq!(
            slug("Phase 1: Smallest bootable Sanctum (v0.1.0)"),
            "phase-1-smallest-bootable-sanctum-v010"
        );
    }

    #[test]
    fn markdown_links_become_html_links() {
        assert_eq!(
            rewrite_link("../recovery/linux.md"),
            "../recovery/linux.html"
        );
        assert_eq!(rewrite_link("guide.md#step-2"), "guide.html#step-2");
        assert_eq!(rewrite_link("#local"), "#local");
        assert_eq!(
            rewrite_link("https://example.com/a.md"),
            "https://example.com/a.md"
        );
        assert_eq!(rewrite_link("../../LICENSE"), "../../LICENSE");
    }

    #[test]
    fn headings_get_ids_and_tables_render() {
        let html = markdown_to_html(
            "# Title\n\n## Same\n\n## Same\n\n| a | b |\n| --- | --- |\n| 1 | 2 |\n\n[x](other.md)\n",
        );
        assert!(html.contains("<h1 id=\"title\">"));
        assert!(html.contains("<h2 id=\"same\">"));
        assert!(html.contains("<h2 id=\"same-1\">"));
        assert!(html.contains("<table>"));
        assert!(html.contains("href=\"other.html\""));
    }

    #[test]
    fn repository_docs_have_topics_in_section_order() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let all = topics(&root);
        assert!(all.iter().any(|t| t.id == "getting-started/booting"));
        let first = all.first().map(|t| t.id.as_str()).unwrap_or("");
        assert!(
            first.starts_with("getting-started/"),
            "first topic is {first}"
        );
    }
}

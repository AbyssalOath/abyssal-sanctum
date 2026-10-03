//! The interactive `sanctum` menu and the tool listings.
//!
//! The menu is a guide: it shows what each tool is for, how risky it is
//! and example commands. It never runs a tool itself, so choosing a menu
//! item cannot change a disk.

use crate::term::{out, outln};
use std::io::{BufRead, Write};

use crate::catalog::{Catalog, Risk, Tool};
use crate::term;

pub(crate) fn print_tool(category_name: &str, tool: &Tool) {
    outln!(
        "{}  {}",
        term::accent(&tool.name),
        term::dim(&format!("({})", tool.id))
    );
    outln!("{}", tool.summary);
    outln!();
    outln!("  Category:  {category_name}");
    outln!(
        "  Risk:      {} - {}",
        term::risk(tool.risk),
        tool.risk.meaning()
    );
    outln!(
        "  Command:   {}{}",
        tool.command,
        if tool.gui {
            " (graphical: run `startx` first)"
        } else {
            ""
        }
    );
    if tool.package != crate::catalog::BUILTIN_PACKAGE {
        outln!("  Package:   {}", tool.package);
    }
    if let Some(note) = &tool.note {
        outln!();
        outln!("  {}", wrap(note, 74, "  "));
    }
    if !tool.usage.is_empty() {
        outln!();
        outln!("  Examples:");
        for line in &tool.usage {
            outln!("    {line}");
        }
    }
    if let Some(topic) = &tool.docs {
        outln!();
        outln!("  Guide:     sanctum docs {topic}");
    }
    if !tool.gui {
        outln!("  Manual:    man {}", tool.command);
    }
}

/// Wrap text at `width` columns, indenting continuation lines.
fn wrap(text: &str, width: usize, indent: &str) -> String {
    let mut out = String::new();
    let mut line_len = 0;
    for word in text.split_whitespace() {
        if line_len > 0 && line_len + 1 + word.len() > width {
            out.push('\n');
            out.push_str(indent);
            line_len = 0;
        } else if line_len > 0 {
            out.push(' ');
            line_len += 1;
        }
        out.push_str(word);
        line_len += word.len();
    }
    out
}

fn tool_line(index: Option<usize>, tool: &Tool) -> String {
    let number = index.map_or_else(String::new, |i| format!("{i:>3}. "));
    let risk_label = tool.risk.label();
    // Pad on the plain label so colour codes do not break alignment.
    let pad = " ".repeat(12usize.saturating_sub(risk_label.len()));
    format!(
        "{number}{:<24} {}{pad}{}{}",
        tool.id,
        term::risk(tool.risk),
        tool.summary,
        if tool.gui {
            term::dim(" [GUI]")
        } else {
            String::new()
        }
    )
}

/// `sanctum tools [CATEGORY]`.
pub(crate) fn list(catalog: &Catalog, category: Option<&str>) -> Result<(), String> {
    match category {
        Some(id) => {
            let section = catalog
                .section(id)
                .ok_or_else(|| format!("no category '{id}' (run `sanctum tools` for the list)"))?;
            outln!(
                "{} - {}",
                term::bold(&section.category.name),
                section.category.summary
            );
            for tool in catalog.tools_in(id) {
                outln!("  {}", tool_line(None, tool));
            }
        }
        None => {
            for section in &catalog.sections {
                outln!(
                    "\n{} ({}) - {}",
                    term::bold(&section.category.name),
                    section.category.id,
                    section.category.summary
                );
                for tool in catalog.tools_in(&section.category.id) {
                    outln!("  {}", tool_line(None, tool));
                }
            }
            outln!();
            print_legend();
        }
    }
    Ok(())
}

fn print_legend() {
    for risk in [Risk::ReadOnly, Risk::Modifies, Risk::Destructive] {
        outln!("  {:<12} {}", risk.label(), term::dim(risk.meaning()));
    }
}

fn prompt(input: &mut impl BufRead, text: &str) -> Option<String> {
    out!("{text}");
    let _ = std::io::stdout().flush();
    let mut line = String::new();
    match input.read_line(&mut line) {
        Ok(0) | Err(_) => None,
        Ok(_) => Some(line.trim().to_owned()),
    }
}

/// The interactive menu: categories, then the tools in one, then details.
pub(crate) fn interactive(catalog: &Catalog, header: &str) {
    let stdin = std::io::stdin();
    let mut input = stdin.lock();
    loop {
        outln!("\n{header}\n");
        for (i, section) in catalog.sections.iter().enumerate() {
            outln!(
                "  {:>2}. {:<14} {}",
                i + 1,
                section.category.name,
                term::dim(&section.category.summary)
            );
        }
        outln!("\n  Also: sanctum disks | sanctum docs | sanctum ssh enable | startx (desktop)");
        let Some(choice) = prompt(&mut input, "\nCategory number, or q to quit: ") else {
            return;
        };
        if choice.eq_ignore_ascii_case("q") {
            return;
        }
        let Some(section) = choice
            .parse::<usize>()
            .ok()
            .and_then(|n| n.checked_sub(1))
            .and_then(|i| catalog.sections.get(i))
        else {
            outln!("Choose a number from the list.");
            continue;
        };
        let tools = catalog.tools_in(&section.category.id);
        loop {
            outln!(
                "\n{} - {}\n",
                term::bold(&section.category.name),
                section.category.summary
            );
            for (i, tool) in tools.iter().enumerate() {
                outln!("{}", tool_line(Some(i + 1), tool));
            }
            let Some(choice) = prompt(
                &mut input,
                "\nTool number for details, b to go back, q to quit: ",
            ) else {
                return;
            };
            match choice.as_str() {
                "q" | "Q" => return,
                "b" | "B" | "" => break,
                _ => match choice
                    .parse::<usize>()
                    .ok()
                    .and_then(|n| n.checked_sub(1))
                    .and_then(|i| tools.get(i))
                {
                    Some(tool) => {
                        outln!();
                        let home = catalog
                            .find(&tool.id)
                            .map_or(section.category.name.as_str(), |(c, _)| c.name.as_str());
                        print_tool(home, tool);
                        let _ = prompt(&mut input, "\nPress Enter to continue...");
                    }
                    None => outln!("Choose a number from the list."),
                },
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraps_long_notes() {
        let text = "one two three four five six seven eight nine ten";
        let wrapped = wrap(text, 14, "  ");
        assert_eq!(
            wrapped,
            "one two three\n  four five six\n  seven eight\n  nine ten"
        );
    }
}

//! Build-time generation of the Xfce menu from the catalog: one `.desktop`
//! file per tool, one `.directory` file per category, and the XDG menu
//! file that arranges them (ADR-0007).
//!
//! Graphical tools start directly. Command-line tools open a terminal that
//! shows the tool's catalog entry (`sanctum tool ID --shell`) and leaves a
//! shell open, so the menu never runs a tool with arguments the user has
//! not chosen.

use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use crate::catalog::{Catalog, Category, Tool};

/// Write the menu files under `root`, which is the airootfs being built.
pub(crate) fn write_desktop(catalog: &Catalog, root: &Path) -> Result<usize, String> {
    let apps = root.join("usr/share/applications");
    let dirs = root.join("usr/share/desktop-directories");
    let menus = root.join("root/.config/menus");
    for dir in [&apps, &dirs, &menus] {
        fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    }
    let mut count = 0;
    for section in &catalog.sections {
        write(
            &dirs.join(format!("sanctum-{}.directory", section.category.id)),
            &directory_entry(&section.category),
        )?;
        for tool in &section.tools {
            write(
                &apps.join(format!("sanctum-{}.desktop", tool.id)),
                &desktop_entry(&section.category, tool),
            )?;
            count += 1;
        }
    }
    write(&menus.join("xfce-applications.menu"), &menu_file(catalog))?;
    Ok(count)
}

fn write(path: &Path, content: &str) -> Result<(), String> {
    fs::write(path, content).map_err(|e| format!("cannot write {}: {e}", path.display()))
}

/// The XDG category name for a Sanctum category.
fn xdg_category(id: &str) -> String {
    let mut name = String::from("X-Sanctum-");
    let mut upper = true;
    for c in id.chars() {
        if upper {
            name.extend(c.to_uppercase());
            upper = false;
        } else {
            name.push(c);
        }
    }
    name
}

/// Quote one argument for a desktop entry's Exec key (Desktop Entry
/// Specification, "The Exec key"), including the string-level escaping
/// of backslashes.
fn exec_arg(arg: &str) -> String {
    let arg = arg.replace('%', "%%");
    if !arg.is_empty()
        && arg
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_./:=+,@".contains(c))
    {
        return arg;
    }
    let mut quoted = String::from("\"");
    for c in arg.chars() {
        match c {
            '"' | '`' | '$' => {
                quoted.push('\\');
                quoted.push(c);
            }
            '\\' => quoted.push_str("\\\\\\\\"),
            _ => quoted.push(c),
        }
    }
    quoted.push('"');
    quoted
}

/// Values in desktop entries cannot contain newlines.
fn one_line(text: &str) -> String {
    text.replace(['\n', '\r'], " ")
}

pub(crate) fn desktop_entry(category: &Category, tool: &Tool) -> String {
    let exec = if tool.gui {
        tool.gui_exec()
            .split_whitespace()
            .map(exec_arg)
            .collect::<Vec<_>>()
            .join(" ")
    } else {
        [
            "xfce4-terminal".to_owned(),
            format!("--title={}", tool.name),
            "-x".to_owned(),
            "sanctum".to_owned(),
            "tool".to_owned(),
            tool.id.clone(),
            "--shell".to_owned(),
        ]
        .iter()
        .map(|a| exec_arg(a))
        .collect::<Vec<_>>()
        .join(" ")
    };
    let mut categories = format!("{};", xdg_category(&category.id));
    for also in &tool.also {
        let _ = write!(categories, "{};", xdg_category(also));
    }
    let icon = tool.icon.as_deref().unwrap_or(&category.icon);
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name={name}\n\
         Comment={summary} ({risk})\n\
         Exec={exec}\n\
         Icon={icon}\n\
         Terminal=false\n\
         Categories={categories}\n\
         X-Sanctum-Risk={risk}\n",
        name = one_line(&tool.name),
        summary = one_line(&tool.summary),
        risk = tool.risk.label(),
    )
}

fn directory_entry(category: &Category) -> String {
    format!(
        "[Desktop Entry]\n\
         Type=Directory\n\
         Name={}\n\
         Comment={}\n\
         Icon={}\n",
        one_line(&category.name),
        one_line(&category.summary),
        category.icon
    )
}

fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// The Xfce applications menu: Sanctum's categories first, then Xfce's own
/// settings and session entries. Other applications' own menu entries are
/// not included, so every item comes from the catalog.
fn menu_file(catalog: &Catalog) -> String {
    let mut menus = String::new();
    let mut layout = String::new();
    for section in &catalog.sections {
        let c = &section.category;
        let _ = write!(
            menus,
            "  <Menu>\n    <Name>{name}</Name>\n    <Directory>sanctum-{id}.directory</Directory>\n    \
             <Include>\n      <Category>{xdg}</Category>\n    </Include>\n  </Menu>\n",
            name = xml_escape(&c.name),
            id = c.id,
            xdg = xdg_category(&c.id),
        );
        let _ = writeln!(layout, "    <Menuname>{}</Menuname>", xml_escape(&c.name));
    }
    format!(
        r#"<!DOCTYPE Menu PUBLIC "-//freedesktop//DTD Menu 1.0//EN"
  "http://www.freedesktop.org/standards/menu-spec/1.0/menu.dtd">
<!-- Generated by `sanctum render desktop` from catalog/*.toml. Do not edit. -->
<Menu>
  <Name>Xfce</Name>
  <DefaultAppDirs/>
  <DefaultDirectoryDirs/>

{menus}
  <Menu>
    <Name>Settings</Name>
    <Directory>xfce-settings.directory</Directory>
    <Include>
      <Category>X-XFCE-SettingsDialog</Category>
    </Include>
  </Menu>

  <Layout>
{layout}    <Separator/>
    <Menuname>Settings</Menuname>
    <Separator/>
    <Filename>xfce4-session-logout.desktop</Filename>
  </Layout>
</Menu>
"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::Risk;

    fn tool(gui: bool) -> Tool {
        Tool {
            id: "smartctl".to_owned(),
            name: "SMART tool".to_owned(),
            package: "smartmontools".to_owned(),
            command: "smartctl".to_owned(),
            risk: Risk::ReadOnly,
            summary: "Disk health".to_owned(),
            gui,
            exec: None,
            icon: None,
            note: None,
            usage: vec![],
            check: None,
            docs: None,
            also: vec!["hardware".to_owned()],
        }
    }

    fn category() -> Category {
        Category {
            id: "storage".to_owned(),
            name: "Storage".to_owned(),
            summary: "Disks".to_owned(),
            icon: "drive-harddisk".to_owned(),
        }
    }

    #[test]
    fn exec_arguments_are_quoted() {
        assert_eq!(exec_arg("sanctum"), "sanctum");
        assert_eq!(exec_arg("--title=SMART tool"), "\"--title=SMART tool\"");
        assert_eq!(exec_arg("a$b\"c"), "\"a\\$b\\\"c\"");
        assert_eq!(exec_arg("100%"), "\"100%%\"");
    }

    #[test]
    fn terminal_tools_open_their_catalog_entry() {
        let entry = desktop_entry(&category(), &tool(false));
        assert!(entry.contains(
            "Exec=xfce4-terminal \"--title=SMART tool\" -x sanctum tool smartctl --shell\n"
        ));
        assert!(entry.contains("Categories=X-Sanctum-Storage;X-Sanctum-Hardware;\n"));
        assert!(entry.contains("Comment=Disk health (read-only)\n"));
        assert!(entry.contains("Icon=drive-harddisk\n"));
    }

    #[test]
    fn graphical_tools_start_directly() {
        let mut t = tool(true);
        t.exec = Some("firefox /usr/share/doc/index.html".to_owned());
        let entry = desktop_entry(&category(), &t);
        assert!(entry.contains("Exec=firefox /usr/share/doc/index.html\n"));
    }

    #[test]
    fn xdg_category_names() {
        assert_eq!(xdg_category("documentation"), "X-Sanctum-Documentation");
    }
}

//! `sanctum disks`: a read-only overview of every disk.
//!
//! Information comes from `lsblk` (the udev database, no device access
//! beyond what udev already did) and `smartctl -H` (the SMART status
//! command, which only reads). Nothing is mounted, assembled or activated.

use crate::term::{out, outln};
use std::fmt::Write as _;
use std::process::Command;

use serde_json::Value;

use crate::term;

pub(crate) fn run(smart: bool) -> Result<(), String> {
    let output = Command::new("lsblk")
        .args([
            "--json",
            "--bytes",
            "--output",
            "NAME,PATH,TYPE,SIZE,FSTYPE,LABEL,MOUNTPOINTS,MODEL,TRAN,RM,RO",
        ])
        .output()
        .map_err(|e| format!("cannot run lsblk: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "lsblk failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let tree: Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("cannot parse lsblk output: {e}"))?;
    let disks = disks(&tree);
    if disks.is_empty() {
        outln!("No disks found.");
        return Ok(());
    }
    for disk in disks {
        let health = if smart && !disk.boot_medium {
            Some(smart_health(&disk.path))
        } else {
            None
        };
        out!("{}", format_disk(&disk, health.as_deref()));
        outln!();
    }
    outln!(
        "{}",
        term::dim(
            "Nothing above is mounted or activated by Sanctum. See `sanctum docs recovery/disk-diagnostics`."
        )
    );
    Ok(())
}

#[derive(Debug, Default)]
pub(crate) struct Device {
    pub(crate) path: String,
    pub(crate) kind: String,
    pub(crate) size: u64,
    pub(crate) fstype: String,
    pub(crate) label: String,
    pub(crate) mountpoints: Vec<String>,
    pub(crate) model: String,
    pub(crate) transport: String,
    pub(crate) removable: bool,
    pub(crate) read_only: bool,
    pub(crate) children: Vec<Device>,
    /// The disk Sanctum booted from.
    pub(crate) boot_medium: bool,
}

fn text(v: &Value, key: &str) -> String {
    v.get(key)
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_owned()
}

/// lsblk prints booleans as true/false, or as "1"/"0" in older versions.
fn flag(v: &Value, key: &str) -> bool {
    match v.get(key) {
        Some(Value::Bool(b)) => *b,
        Some(Value::String(s)) => s == "1",
        Some(Value::Number(n)) => n.as_u64() == Some(1),
        _ => false,
    }
}

fn parse(v: &Value) -> Device {
    let size = match v.get("size") {
        Some(Value::Number(n)) => n.as_u64().unwrap_or(0),
        Some(Value::String(s)) => s.parse().unwrap_or(0),
        _ => 0,
    };
    let mountpoints = v
        .get("mountpoints")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    let children: Vec<Device> = v
        .get("children")
        .and_then(Value::as_array)
        .map(|a| a.iter().map(parse).collect())
        .unwrap_or_default();
    let mut device = Device {
        path: text(v, "path"),
        kind: text(v, "type"),
        size,
        fstype: text(v, "fstype"),
        label: text(v, "label"),
        mountpoints,
        model: text(v, "model"),
        transport: text(v, "tran"),
        removable: flag(v, "rm"),
        read_only: flag(v, "ro"),
        children,
        boot_medium: false,
    };
    device.boot_medium = is_boot(&device);
    device
}

fn is_boot(d: &Device) -> bool {
    d.mountpoints.iter().any(|m| m.starts_with("/run/archiso/")) || d.children.iter().any(is_boot)
}

/// Physical disks from lsblk's JSON. Loop devices (the live system's
/// image) and RAM-backed devices (zram) are left out; optical drives are
/// kept only if Sanctum booted from one.
pub(crate) fn disks(tree: &Value) -> Vec<Device> {
    tree.get("blockdevices")
        .and_then(Value::as_array)
        .map(|a| a.iter().map(parse).collect::<Vec<_>>())
        .unwrap_or_default()
        .into_iter()
        .filter(|d| {
            let virtual_ram = d.path.starts_with("/dev/zram") || d.path.starts_with("/dev/ram");
            (d.kind == "disk" && !virtual_ram) || (d.kind == "rom" && d.boot_medium)
        })
        .collect()
}

pub(crate) fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 6] = ["B", "KiB", "MiB", "GiB", "TiB", "PiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

/// What a filesystem type means for the technician.
fn note(d: &Device) -> Option<String> {
    let note = match d.fstype.as_str() {
        "crypto_LUKS" => "encrypted (LUKS): cryptsetup open --readonly",
        "BitLocker" => "encrypted (BitLocker): needs the recovery key",
        "linux_raid_member" => "RAID member: mdadm --examine",
        "LVM2_member" => "LVM physical volume: pvs, vgs",
        "swap" if d.mountpoints.iter().any(|m| m == "[SWAP]") => "swap, in use",
        "swap" => "swap (not activated)",
        "ntfs" => "Windows NTFS",
        "vfat" if d.label.eq_ignore_ascii_case("efi") || d.label.eq_ignore_ascii_case("esp") => {
            "EFI system partition"
        }
        "zfs_member" => "ZFS pool member",
        "bcache" => "bcache device",
        _ => return None,
    };
    Some(note.to_owned())
}

fn format_device(d: &Device, depth: usize, out: &mut String) {
    let indent = "  ".repeat(depth);
    let mut extras = Vec::new();
    if let Some(n) = note(d) {
        extras.push(n);
    }
    if d.kind != "part" && d.kind != "disk" {
        extras.push(format!("active {}", d.kind));
    }
    for m in d.mountpoints.iter().filter(|m| *m != "[SWAP]") {
        extras.push(format!("mounted at {m}"));
    }
    let _ = writeln!(
        out,
        "{indent}{:<16} {:>10}  {:<12} {:<16} {}",
        d.path,
        human_size(d.size),
        if d.fstype.is_empty() { "-" } else { &d.fstype },
        if d.label.is_empty() { "" } else { &d.label },
        term::dim(&extras.join("; "))
    );
    for child in &d.children {
        format_device(child, depth + 1, out);
    }
}

pub(crate) fn format_disk(disk: &Device, health: Option<&str>) -> String {
    let mut out = String::new();
    let mut facts = Vec::new();
    if !disk.model.is_empty() {
        facts.push(disk.model.clone());
    }
    if !disk.transport.is_empty() {
        facts.push(disk.transport.clone());
    }
    if disk.removable {
        facts.push("removable".to_owned());
    }
    if disk.read_only {
        facts.push("read-only".to_owned());
    }
    let _ = writeln!(
        out,
        "{}  {}  {}",
        term::bold(&disk.path),
        human_size(disk.size),
        facts.join(", ")
    );
    if disk.boot_medium {
        let _ = writeln!(out, "  {}", term::accent("Sanctum boot medium"));
    } else if let Some(h) = health {
        let _ = writeln!(out, "  SMART: {h}");
    }
    if !disk.fstype.is_empty() {
        format_device(
            &Device {
                children: Vec::new(),
                ..clone_shallow(disk)
            },
            1,
            &mut out,
        );
    }
    for child in &disk.children {
        format_device(child, 1, &mut out);
    }
    out
}

/// A disk formatted directly (no partition table) is shown as one line.
fn clone_shallow(d: &Device) -> Device {
    Device {
        path: d.path.clone(),
        kind: "part".to_owned(),
        size: d.size,
        fstype: d.fstype.clone(),
        label: d.label.clone(),
        mountpoints: d.mountpoints.clone(),
        ..Device::default()
    }
}

/// SMART overall health via `smartctl -H --json`.
fn smart_health(path: &str) -> String {
    let Ok(output) = Command::new("smartctl")
        .args(["-H", "--json=c", path])
        .output()
    else {
        return "smartctl not available".to_owned();
    };
    let Ok(json) = serde_json::from_slice::<Value>(&output.stdout) else {
        return "not available".to_owned();
    };
    smart_verdict(&json)
}

pub(crate) fn smart_verdict(json: &Value) -> String {
    match json
        .get("smart_status")
        .and_then(|s| s.get("passed"))
        .and_then(Value::as_bool)
    {
        Some(true) => term::good("PASSED"),
        Some(false) => format!(
            "{} - back up this disk now (smartctl -a for details)",
            term::bad("FAILING")
        ),
        None => {
            let reason = json
                .pointer("/smartctl/messages/0/string")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|m| !m.is_empty());
            match reason {
                Some(r) => format!("not available ({r})"),
                None => "not available for this device".to_owned(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{"blockdevices":[
      {"name":"loop0","path":"/dev/loop0","type":"loop","size":900000000,"fstype":"squashfs","label":null,
       "mountpoints":["/run/archiso/airootfs"],"model":null,"tran":null,"rm":false,"ro":true},
      {"name":"sda","path":"/dev/sda","type":"disk","size":256060514304,"fstype":null,"label":null,
       "mountpoints":[null],"model":"Samsung SSD 860","tran":"sata","rm":false,"ro":false,
       "children":[
         {"name":"sda1","path":"/dev/sda1","type":"part","size":536870912,"fstype":"vfat","label":"EFI","mountpoints":[null],"rm":false,"ro":false},
         {"name":"sda2","path":"/dev/sda2","type":"part","size":255000000000,"fstype":"crypto_LUKS","label":null,"mountpoints":[null],"rm":false,"ro":false}
       ]},
      {"name":"sr0","path":"/dev/sr0","type":"rom","size":1200000000,"fstype":"iso9660","label":"SANCTUM_0_1_0",
       "mountpoints":["/run/archiso/bootmnt"],"model":"QEMU DVD","tran":"sata","rm":true,"ro":false},
      {"name":"vda","path":"/dev/vda","type":"disk","size":"67108864","fstype":"linux_raid_member","label":"sanctum:test",
       "mountpoints":[null],"rm":"0","ro":"0"}
    ]}"#;

    #[test]
    fn lists_disks_and_the_boot_medium() {
        let tree: Value = serde_json::from_str(SAMPLE).expect("sample parses");
        let disks = disks(&tree);
        let paths: Vec<&str> = disks.iter().map(|d| d.path.as_str()).collect();
        assert_eq!(paths, ["/dev/sda", "/dev/sr0", "/dev/vda"]);
        assert!(disks[1].boot_medium);
        assert!(!disks[0].boot_medium);
        assert_eq!(disks[2].size, 67_108_864);
    }

    #[test]
    fn notes_explain_encryption_and_raid() {
        let tree: Value = serde_json::from_str(SAMPLE).expect("sample parses");
        let disks = disks(&tree);
        let sda = format_disk(&disks[0], Some("PASSED"));
        assert!(sda.contains("encrypted (LUKS)"));
        assert!(sda.contains("EFI system partition"));
        assert!(sda.contains("SMART: PASSED"));
        let vda = format_disk(&disks[2], None);
        assert!(vda.contains("RAID member"), "{vda}");
    }

    #[test]
    fn sizes_are_human_readable() {
        assert_eq!(human_size(512), "512 B");
        assert_eq!(human_size(67_108_864), "64.0 MiB");
        assert_eq!(human_size(256_060_514_304), "238.5 GiB");
    }

    #[test]
    fn smart_verdicts() {
        let pass: Value =
            serde_json::from_str(r#"{"smart_status":{"passed":true}}"#).expect("json");
        let fail: Value =
            serde_json::from_str(r#"{"smart_status":{"passed":false}}"#).expect("json");
        assert!(smart_verdict(&pass).contains("PASSED"));
        assert!(smart_verdict(&fail).contains("FAILING"));
        assert!(smart_verdict(&Value::Null).contains("not available"));
        let usb: Value = serde_json::from_str(
            r#"{"smartctl":{"messages":[{"string":"/dev/sdb: Unknown USB bridge","severity":"error"}]}}"#,
        )
        .expect("json");
        assert_eq!(
            smart_verdict(&usb),
            "not available (/dev/sdb: Unknown USB bridge)"
        );
    }
}

//! Block devices as lsblk reports them, flattened: disks, partitions, and
//! the device-mapper and RAID devices stacked on them.

use serde_json::Value;

use crate::sys;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Volume {
    /// /dev/sda3, /dev/mapper/sanctum-sda3, /dev/md127 ...
    pub(crate) path: String,
    /// The kernel name: sda3, dm-0, md127.
    pub(crate) kname: String,
    /// disk, part, crypt, lvm, raid1, rom, loop ...
    pub(crate) kind: String,
    pub(crate) fstype: String,
    pub(crate) label: String,
    pub(crate) size: u64,
    pub(crate) mountpoints: Vec<String>,
    /// The kernel name of the parent device.
    pub(crate) parent: String,
}

fn text(v: &Value, key: &str) -> String {
    v.get(key)
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_owned()
}

fn collect(v: &Value, parent: &str, out: &mut Vec<Volume>) {
    let volume = Volume {
        path: text(v, "path"),
        kname: text(v, "kname"),
        kind: text(v, "type"),
        fstype: text(v, "fstype"),
        label: text(v, "label"),
        size: match v.get("size") {
            Some(Value::Number(n)) => n.as_u64().unwrap_or(0),
            Some(Value::String(s)) => s.parse().unwrap_or(0),
            _ => 0,
        },
        mountpoints: v
            .get("mountpoints")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default(),
        // lsblk prints a tree only when it shows NAME; the flat list it
        // prints otherwise carries the parent in PKNAME.
        parent: match text(v, "pkname") {
            p if p.is_empty() => parent.to_owned(),
            p => p,
        },
    };
    let kname = volume.kname.clone();
    // A device stacked on several parents (RAID, LVM) appears once.
    if !out.iter().any(|existing| existing.path == volume.path) {
        out.push(volume);
    }
    if let Some(children) = v.get("children").and_then(Value::as_array) {
        for child in children {
            collect(child, &kname, out);
        }
    }
}

/// Parse `lsblk --json` output into a flat list.
pub(crate) fn parse(json: &str) -> Result<Vec<Volume>, String> {
    let tree: Value =
        serde_json::from_str(json).map_err(|e| format!("cannot parse lsblk output: {e}"))?;
    let mut out = Vec::new();
    for device in tree
        .get("blockdevices")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        collect(device, "", &mut out);
    }
    Ok(out)
}

/// Every block device on the system.
pub(crate) fn volumes() -> Result<Vec<Volume>, String> {
    parse(&sys::output(
        "lsblk",
        &[
            "--json",
            "--bytes",
            "--output",
            "PATH,KNAME,PKNAME,TYPE,SIZE,FSTYPE,LABEL,MOUNTPOINTS",
        ],
    )?)
}

/// Find a device by path, following symlinks such as /dev/disk/by-label.
pub(crate) fn find(path: &str) -> Result<Volume, String> {
    let real = std::fs::canonicalize(path)
        .map_err(|e| format!("{path}: {e}"))?
        .to_string_lossy()
        .into_owned();
    volumes()?
        .into_iter()
        .find(|v| {
            v.path == real
                || std::fs::canonicalize(&v.path).is_ok_and(|p| p.to_string_lossy() == real)
        })
        .ok_or_else(|| format!("{path} is not a block device known to lsblk"))
}

/// True when this volume holds the running Sanctum (the boot medium or its
/// image).
pub(crate) fn is_boot_medium(v: &Volume) -> bool {
    v.mountpoints.iter().any(|m| m.starts_with("/run/archiso/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flattens_nested_devices() {
        let json = r#"{"blockdevices":[
          {"path":"/dev/sda","kname":"sda","type":"disk","size":1000,"fstype":null,"label":null,"mountpoints":[null],
           "children":[
             {"path":"/dev/sda1","kname":"sda1","type":"part","size":100,"fstype":"vfat","label":"EFI","mountpoints":[null]},
             {"path":"/dev/sda2","kname":"sda2","type":"part","size":900,"fstype":"BitLocker","label":null,"mountpoints":[null],
              "children":[{"path":"/dev/mapper/sanctum-sda2","kname":"dm-0","type":"crypt","size":890,"fstype":"ntfs","label":"OS","mountpoints":["/mnt/sanctum/sda2"]}]}
           ]}
        ]}"#;
        let v = parse(json).expect("parses");
        let paths: Vec<&str> = v.iter().map(|x| x.path.as_str()).collect();
        assert_eq!(
            paths,
            [
                "/dev/sda",
                "/dev/sda1",
                "/dev/sda2",
                "/dev/mapper/sanctum-sda2"
            ]
        );
        assert_eq!(v[3].parent, "sda2");
        assert_eq!(v[3].mountpoints, ["/mnt/sanctum/sda2"]);
        assert_eq!(v[1].label, "EFI");
    }

    #[test]
    fn flat_output_takes_parent_from_pkname() {
        // What `lsblk --json --output PATH,KNAME,PKNAME,...` prints: no tree.
        let json = r#"{"blockdevices":[
          {"path":"/dev/vdb","kname":"vdb","pkname":null,"type":"disk","size":2147483648,"fstype":null,"label":null,"mountpoints":[null]},
          {"path":"/dev/vdb1","kname":"vdb1","pkname":"vdb","type":"part","size":2146108928,"fstype":"ext4","label":"SANCTUM_DATA","mountpoints":["/sanctum"]}
        ]}"#;
        let v = parse(json).expect("parses");
        assert_eq!(v[0].parent, "");
        assert_eq!(v[1].parent, "vdb");
        assert_eq!(v[1].mountpoints, ["/sanctum"]);
    }
}

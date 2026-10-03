# The tool catalog

`catalog/*.toml` lists the tools technicians see in `sanctum`, `sanctum
tools` and the desktop's Sanctum menu. There is one file per category,
and every category file must exist:

`recovery`, `storage`, `hardware`, `networking`, `security`, `forensics`,
`windows`, `linux`, `documentation`, `terminal`.

## Format

```toml
[category]
id = "storage"                # must match the file name
name = "Storage"
summary = "Disks, partitions, filesystems, RAID, LVM, encryption and imaging"
icon = "drive-harddisk"       # icon name from the icon theme

[[tool]]
id = "smartctl"               # unique; lowercase letters, digits, dashes
name = "smartctl"
package = "smartmontools"     # must be in build/packages/*.list, or "sanctum"
command = "smartctl"          # executable that must exist on the live system
risk = "read-only"            # read-only | modifies | destructive
summary = "SMART health, error logs and self-tests for SATA, SAS and NVMe disks"
note = "Optional: a warning or tip shown with the tool."
usage = ["smartctl -a /dev/sdX", "smartctl -t short /dev/sdX"]
check = ["smartctl", "--version"]   # optional: must exit 0 on the live system
docs = "recovery/disk-diagnostics"  # optional: a guide under docs/
also = ["hardware"]                 # optional: also list it in these categories
```

Graphical tools set `gui = true`, have no `usage`, and may set `exec` (the
command line, if not just `command`) and `icon`.

## Risk levels

Risk describes what a tool can do to data on disks:

| Level | Meaning | Examples |
| --- | --- | --- |
| `read-only` | Never changes existing data. Writing new output files to a destination the user chooses is fine | `smartctl`, `lsblk`, `photorec` |
| `modifies` | Changes data or settings, in a controlled way, when told to | `e2fsck`, `rsync`, `chntpw` |
| `destructive` | Can erase or overwrite disk contents with one command | `gdisk`, `ddrescue`, `cryptsetup` |

Rate a tool by the worst thing it does with an ordinary command, and use
`note` to say which of its uses are safe.

## Checks

```bash
cargo run --locked --quiet -- catalog check
```

This runs in CI, in `scripts/ci/check-all.sh` and in every ISO build. It
fails when:

- a category file is missing or unknown, or a field is unknown or missing,
- an id is reused or badly formed,
- a package is not in the package lists,
- a `docs` topic does not exist,
- a command-line tool has no usage example, or a graphical one has,
- `also` names an unknown category or the tool's own.

On the live system, `sanctum catalog verify` (part of the self-test and
every boot test) checks that every `command` exists and every `check`
command succeeds within 30 seconds.

## The desktop menu

The build turns the catalog into the Xfce menu
([ADR-0007](../architecture/decisions/0007-gui-stack.md)):

- one `sanctum-ID.desktop` file per tool, in the categories
  `X-Sanctum-<Category>`,
- one `.directory` file per category,
- `/root/.config/menus/xfce-applications.menu`, with the categories in
  the order above.

Graphical tools start directly. Command-line tools open a terminal running
`sanctum tool ID --shell`, which shows the catalog entry and leaves a shell
open. The menu never runs a command-line tool with arguments.

## Adding a tool

1. Make sure its package is in `build/packages/` ([adding packages](packages.md)).
2. Add a `[[tool]]` entry to the right category file.
3. Run `catalog check`, then build and boot-test. The boot test's self-test
   runs `catalog verify`, so a wrong `command` or failing `check` shows up
   there.

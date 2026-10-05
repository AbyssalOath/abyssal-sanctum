# Finding your way around

Sanctum starts at a root console. Everything works from the command line,
and a graphical desktop is one command away.

## The console

After boot you are logged in as `root` on the first console. The login
banner lists the most useful commands. Further consoles are on
**Alt+F2** to **Alt+F6** (log in as
`root`, no password).

### The `sanctum` command

| Command | What it does |
| --- | --- |
| `sanctum` | Interactive menu: categories, the tools in each, and how to use them |
| `sanctum tools [CATEGORY]` | List the tools, all or in one category |
| `sanctum tool ID` | One tool: purpose, risk, example commands, guide |
| `sanctum disks` | Read-only overview of every disk, its partitions and SMART health |
| `sanctum targets` | Find the installed systems: Windows (and whether it is hibernated), Linux, encrypted volumes |
| `sanctum mount DEVICE` | Mount a partition read-only (BitLocker and LUKS are unlocked first); `--rw` to write |
| `sanctum umount [--all]` | Unmount what `sanctum mount` mounted |
| `sanctum update` | ClamAV databases: download, `--import` or `--export` an offline pack, `--status` |
| `sanctum scan DIR...` | Malware scan with a case report ([guide](../security/offline-malware-scanning.md)) |
| `sanctum data status` | The [data partition](data-partition.md), which keeps databases and reports across reboots |
| `sanctum secureboot status` | Secure Boot state, and whether the Sanctum key is enrolled; `forget` removes it ([guide](secure-boot.md)) |
| `sanctum docs [TOPIC]` | List the guides, or read one |
| `sanctum ssh enable` | Allow remote SSH logins (see below) |
| `sanctum selftest` | Check that the safe defaults are in effect |
| `sanctum version` | Version and build information |

The menu is a guide. It never runs a tool for you, so browsing it cannot
change anything.

The categories are Recovery, Storage, Hardware, Networking, Security,
Forensics, Windows, Linux, Documentation and Terminal.

### Risk levels

Every tool is labelled by what it can do to data on disks:

| Label | Meaning |
| --- | --- |
| read-only | Never changes existing data |
| modifies | Changes data or settings when you tell it to |
| destructive | Can erase or overwrite disk contents with one command |

A "destructive" tool is not dangerous to look at: `gdisk -l` only lists.
The label says what the tool is capable of, so read its notes before you
write anything.

### Keyboard layout

The console starts with a US keyboard. Change it with `loadkeys`, for
example `loadkeys de` or `loadkeys uk`. On the desktop, use
`setxkbmap de` in a terminal, or the Keyboard settings.

## The desktop

Run `startx` to start the Xfce desktop. The **Sanctum** menu in the top left
has the same categories as the `sanctum` command:

- Graphical tools (GParted, GSmartControl, Wireshark, Firefox, the file
  manager and editor) start directly.
- Command-line tools open a terminal that shows the tool's description,
  risk and examples, and leaves a shell open for you to run it.
- Each menu entry's tooltip shows the risk level.

The guides open in Firefox from Documentation, or from Firefox's home page.

The file manager does not mount disks automatically. Mount what you need in
a terminal first (see the [filesystem guide](../recovery/filesystems.md)).

To leave the desktop, use the log-out button in the top right. You return
to the console.

## Network

Wired networks with DHCP connect automatically. For Wi-Fi, a static
address or other settings:

- console: `nmtui`, or `nmcli device wifi connect SSID --ask`
- desktop: the network icon in the panel

Nothing on the network can reach Sanctum: the firewall drops all incoming
connections unless you open a port.

## Remote access (SSH)

SSH is off by default. To let someone log in remotely:

```bash
sanctum ssh enable                       # asks for a root password
sanctum ssh enable --key /mnt/usb/id.pub # or authorise a public key
```

This starts `sshd`, opens port 22 in the firewall, and prints the host key
fingerprints and the addresses to connect to. `sanctum ssh status` shows the
current state, and `sanctum ssh disable` turns it off again.

## Shutting down

Run `poweroff` or `reboot`, or use the log-out button on the desktop.
Unmount anything you mounted first, so the target's filesystems are closed
cleanly:

```bash
umount -R /mnt
```

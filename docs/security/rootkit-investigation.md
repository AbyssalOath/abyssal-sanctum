# Rootkit investigation

A rootkit hides itself from the system it runs on. Booting Sanctum takes
that system offline: its kernel and processes are not running, so nothing
on the disk can hide from you. The flip side is that tools which inspect
the **running** system, such as `rkhunter` and `unhide`, inspect Sanctum
itself when run from Sanctum, and tell you nothing about the target.

## Linux targets

Mount the target read-only at `/mnt` (see the
[filesystem guide](../recovery/filesystems.md#mounting-read-only-properly)).
Always use Sanctum's tools, never binaries from the target, which may be
modified.

### 1. Verify installed files against the package database

| Target | Command (read-only) |
| --- | --- |
| Arch | `pacman --sysroot /mnt -Qkk 2>&1 \| grep -v ' 0 altered files'` |
| Fedora, RHEL | `rpm --root /mnt -Va` |
| Debian, Ubuntu | `chroot /mnt dpkg --verify` (this runs the target's dpkg) |

Changed binaries in `/usr/bin`, `/usr/lib` or `/usr/lib/modules` that are
not configuration files deserve a closer look. A rootkit that also
rewrote the package database will not show up here.

### 2. Look where persistence lives

```bash
cat /mnt/etc/ld.so.preload                       # should not exist
ls -la /mnt/etc/systemd/system /mnt/usr/lib/systemd/system | less
ls -la /mnt/etc/cron* /mnt/var/spool/cron
cat /mnt/root/.ssh/authorized_keys /mnt/home/*/.ssh/authorized_keys
ls -la /mnt/etc/profile.d /mnt/etc/update-motd.d 2>/dev/null
grep -r . /mnt/etc/modules-load.d /mnt/etc/modprobe.d
```

### 3. Kernel modules not owned by any package

```bash
# Arch
find /mnt/usr/lib/modules -name '*.ko*' | sed 's|^/mnt||' |
  while read -r f; do pacman --sysroot /mnt -Qo "$f" >/dev/null 2>&1 || echo "$f"; done
```

### 4. Scan

Run [ClamAV](offline-malware-scanning.md) and [YARA](yara-scanning.md) over
the target.

## Windows targets

1. Mount the Windows volume read-only.
2. Scan it with ClamAV and YARA.
3. Inspect autostart locations in the registry with `hivexsh`: the `Run` and
   `RunOnce` keys in `SOFTWARE` and in each user's `NTUSER.DAT`, and the
   services under `SYSTEM\ControlSet001\Services`. See the
   [Windows guide](../recovery/windows.md#inspecting-the-registry).
4. Check `Windows/System32/drivers` for recently changed drivers:
   `find /mnt/windows/Windows/System32/drivers -newermt 2026-01-01 -ls`.

## The live tools

`rkhunter` and `unhide` are included for checking a **live** suspect system,
for example when Sanctum's tools are copied onto it, and for checking that
Sanctum itself is clean (`rkhunter --check --sk --rwo`). They are not
offline scanners.

## What comes next

Abyssal Warden's `system-check --root` will automate much of this for mounted
Linux systems (ROADMAP Phase 6).

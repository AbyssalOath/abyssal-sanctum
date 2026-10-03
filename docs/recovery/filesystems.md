# Filesystems, partitions, RAID and LVM

## Before you repair

1. **Is the disk healthy?** Check with `sanctum disks` and `smartctl -a`
   ([disk diagnostics](disk-diagnostics.md)). Repairing a filesystem on a
   failing disk makes the disk work harder and can lose more data. Image it
   with `ddrescue` first, then repair the image.
2. **Look before you write.** Every tool below has a mode that only reports.
   Run that first.
3. **Copy what matters first** if the filesystem still mounts read-only.

## Mounting read-only, properly

A plain `mount -o ro` can still write to the disk: ext4 and XFS replay their
journal on mount. To leave the disk untouched:

| Filesystem | Command |
| --- | --- |
| ext4 | `mount -o ro,noload /dev/sdX2 /mnt` |
| XFS | `mount -o ro,norecovery /dev/sdX2 /mnt` |
| Btrfs | `mount -o ro,rescue=nologreplay /dev/sdX2 /mnt` |
| NTFS | `mount -t ntfs-3g -o ro /dev/sdX3 /mnt` |
| FAT, exFAT | `mount -o ro /dev/sdX1 /mnt` |

You can also make the whole device read-only in the kernel first:
`blockdev --setro /dev/sdX` (and each partition). This is a software
safeguard, not a hardware write blocker.

## Checking and repairing

| Filesystem | Report only | Repair |
| --- | --- | --- |
| ext2/3/4 | `e2fsck -fn /dev/sdX2` | `e2fsck -f /dev/sdX2` (asks before each fix) |
| XFS | `xfs_repair -n /dev/sdX2` | `xfs_repair /dev/sdX2` |
| Btrfs | `btrfs check /dev/sdX2` | see below |
| FAT/ESP | `fsck.fat -n /dev/sdX1` | `fsck.fat -a /dev/sdX1` |
| exFAT | `fsck.exfat -n /dev/sdX1` | `fsck.exfat /dev/sdX1` |
| F2FS | `fsck.f2fs /dev/sdX1` | `fsck.f2fs -f /dev/sdX1` |
| NTFS | `ntfsinfo -m /dev/sdX3` | Not from Linux: see the [Windows guide](windows.md) |

The filesystem must not be mounted while you repair it.

**XFS** refuses to repair a filesystem with a dirty log. Mount it once
(read-write) so the log replays, unmount, then run `xfs_repair`. Only if it
cannot be mounted at all, `xfs_repair -L` discards the log. That can lose the
most recent changes.

**Btrfs** repairs are risky. Prefer, in order:

1. `mount -o ro,rescue=all /dev/sdX2 /mnt` and copy the data off.
2. `btrfs restore /dev/sdX2 /mnt/usb/restored/` copies files from an
   unmountable filesystem without changing it.
3. `btrfs rescue super-recover` or `btrfs rescue zero-log` for specific
   damage.
4. `btrfs check --repair` only as a last resort, after copying the data off.

## Lost partitions

1. If the disk uses GPT, `gdisk -l /dev/sdX` reports a damaged primary
   table. The backup copy at the end of the disk can rebuild it: in `gdisk`,
   use `r` (recovery), then `b` (use backup header) or `c` (load backup
   table), check with `p`, and write with `w`.
2. `testdisk /dev/sdX` searches for lost partitions and shows them before
   writing anything. Choose Analyse, then Quick Search (or Deeper Search),
   check the listed partitions, and only then Write.
3. `gpart /dev/sdX` guesses partitions from a raw scan, read-only.

## Deleted files

`photorec` recovers files by content, even from a damaged or reformatted
filesystem. Recovered files must go to a **different disk**:

```bash
photorec /log /d /mnt/usb/recovered /dev/sdX
```

File names and folders are usually lost; files come back sorted by type.

## RAID (mdadm)

Sanctum never assembles arrays by itself.

```bash
mdadm --examine /dev/sdX1              # read the member's RAID metadata
mdadm --assemble --scan --readonly     # assemble everything found, read-only
cat /proc/mdstat
```

Assembling a degraded array read-write can start a resync, which writes to
every member. Use `--readonly` until you have decided. `mdadm --readwrite
/dev/md127` switches later.

## LVM

Sanctum never activates volumes by itself.

```bash
pvs; vgs; lvs                          # what exists (read-only)
vgchange -ay VGNAME                    # activate one volume group
vgchange -an VGNAME                    # deactivate before you finish
```

LVM keeps metadata backups in `/etc/lvm/archive` on the installed system.
`vgcfgrestore` can roll back to one, but read `man vgcfgrestore` first.

## Encryption

```bash
cryptsetup luksDump /dev/sdX2                      # header details (read-only)
cryptsetup open --readonly /dev/sdX2 data          # unlock without allowing writes
cryptsetup close data
```

A damaged LUKS header cannot be recovered without a header backup
(`cryptsetup luksHeaderBackup`). Before any risky work on a LUKS volume,
back up its header to another disk.

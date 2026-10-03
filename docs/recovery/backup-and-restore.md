# Backup and restore

Sanctum runs from RAM and keeps nothing between boots. Everything you want
to keep (copied files, disk images, logs) has to go to another disk or a
network share.

## Where to put backups

Mount the destination first:

| Destination | Command |
| --- | --- |
| A USB disk | `mount /dev/sdY1 /mnt/usb` |
| An NFS share | `mount -t nfs server:/export /mnt/backup` |
| A Windows (SMB) share | `mount -t cifs //server/share /mnt/backup -o username=NAME` |
| Another machine over SSH | `rsync -a ... user@host:/path/` (no mount needed) |

Connect to the network first if needed (`nmtui`). Outgoing connections are
allowed by the firewall.

## Copying files

The safest first step on a system that still mounts. Mount the source
read-only ([how](filesystems.md#mounting-read-only-properly)):

```bash
rsync -aHAX --info=progress2 /mnt/source/home/ /mnt/usb/home/
```

`-aHAX` keeps permissions, hard links, ACLs and extended attributes, which
is right for Linux filesystems. From NTFS or FAT, use `-rt` instead.

Run the same command again to resume an interrupted copy.

## Imaging a failing disk

Use `ddrescue`, never `dd`, on a disk that may be failing. It copies the
readable areas first and records progress in a map file, so it can stop and
resume:

```bash
ddrescue -n /dev/sdX /mnt/usb/disk.img /mnt/usb/disk.map    # fast first pass
ddrescue -r3 /dev/sdX /mnt/usb/disk.img /mnt/usb/disk.map   # retry bad areas
```

The destination needs as much free space as the whole source disk. Then
work on the image, not the disk:

```bash
losetup -r -P -f --show /mnt/usb/disk.img    # read-only loop device, e.g. /dev/loop1
lsblk /dev/loop1                              # its partitions: /dev/loop1p1, ...
```

## Imaging a healthy disk or partition

| Tool | Best for | Back up | Restore (overwrites the target) |
| --- | --- | --- | --- |
| partclone | One filesystem, only used blocks | `partclone.ext4 -c -s /dev/sdX2 -o /mnt/usb/root.img` | `partclone.ext4 -r -s /mnt/usb/root.img -o /dev/sdX2` |
| fsarchiver | Restoring to a different-sized partition | `fsarchiver savefs /mnt/usb/root.fsa /dev/sdX2` | `fsarchiver restfs /mnt/usb/root.fsa id=0,dest=/dev/sdX2` |
| Clonezilla | Whole disks, guided | `clonezilla`, then device-image | `clonezilla`, then image-device |
| ddrescue | Exact raw copies | as above | `ddrescue -f /mnt/usb/disk.img /dev/sdX` |

partclone has one command per filesystem: `partclone.ext4`, `partclone.xfs`,
`partclone.btrfs`, `partclone.ntfs`, `partclone.vfat`, and so on.

## Verifying

Record a hash of every image, and check it before restoring:

```bash
sha256sum /mnt/usb/disk.img > /mnt/usb/disk.img.sha256
sha256sum -c /mnt/usb/disk.img.sha256
```

## Restoring

Every restore overwrites its target. Before you press Enter:

1. Check the target device with `lsblk -o NAME,SIZE,MODEL,SERIAL`.
2. Make sure nothing on the target is mounted (`lsblk` shows mount points).
3. Read the command again.

# Repairing an installed Linux system

This guide covers the usual repair: mount the installed system, enter it
with `arch-chroot`, and fix it with its own tools. It works for any
distribution, not only Arch, because inside the chroot you use the
target's own package manager, initramfs and boot loader tools.

If the disk itself may be failing, read
[disk diagnostics](disk-diagnostics.md) first and image the disk before
repairing anything.

## 1. Find the system

```bash
sanctum disks
lsblk -f
```

Look for the root filesystem (ext4, xfs or btrfs), an EFI system partition
(vfat, about 100 MB to 1 GB) and possibly a separate `/boot`.

Check how Sanctum itself was booted. Repairing a UEFI installation's boot
entries only works if Sanctum was also booted in UEFI mode:

```bash
[ -d /sys/firmware/efi ] && echo UEFI || echo BIOS
```

## 2. Unlock and activate storage

Only what you need, and only when you choose.

| Layer | Command |
| --- | --- |
| LUKS encryption | `cryptsetup open /dev/sdX2 cryptroot` |
| LVM | `vgchange -ay` (or `vgchange -ay VGNAME`) |
| Software RAID | `mdadm --assemble --scan` |

Add `--readonly` to `cryptsetup open` (and `--readonly` to `mdadm
--assemble`) if you only want to look.

## 3. Mount the system

```bash
mount /dev/mapper/cryptroot /mnt          # or /dev/sdX2, /dev/VG/root ...
cat /mnt/etc/fstab                         # see what else belongs where
mount /dev/sdX1 /mnt/boot/efi             # the ESP, if fstab has it there
mount /dev/sdX3 /mnt/boot                 # a separate /boot, if any
```

The ESP is mounted at `/boot/efi` on Debian, Ubuntu and Fedora, and often at
`/boot` or `/efi` on Arch. Follow the target's fstab.

**Btrfs** systems keep the root in a subvolume. List them and mount the
right one:

```bash
mount -o subvolid=5 /dev/sdX2 /mnt && btrfs subvolume list /mnt && umount /mnt
mount -o subvol=@ /dev/sdX2 /mnt          # @ on Ubuntu/Arch, root on Fedora
```

## 4. Enter the system

```bash
arch-chroot /mnt
```

`arch-chroot` mounts `/proc`, `/sys`, `/dev` and `/run`, and makes DNS work
inside, so the package manager can download. Type `exit` to leave.

## 5. Common repairs

### Rebuild the initramfs

| Distribution | Command |
| --- | --- |
| Arch | `mkinitcpio -P` |
| Debian, Ubuntu | `update-initramfs -u -k all` |
| Fedora, RHEL | `dracut -f --regenerate-all` |

### Reinstall the boot loader

GRUB on UEFI:

| Distribution | Commands |
| --- | --- |
| Arch | `grub-install --target=x86_64-efi --efi-directory=/boot/efi --bootloader-id=GRUB` then `grub-mkconfig -o /boot/grub/grub.cfg` |
| Debian, Ubuntu | `grub-install` then `update-grub` |
| Fedora | `dnf reinstall shim-* grub2-efi-*` then `grub2-mkconfig -o /boot/grub2/grub.cfg` |

GRUB on BIOS: `grub-install /dev/sdX` (the disk, not a partition), then
regenerate the configuration as above. systemd-boot: `bootctl install`.

After a UEFI repair, check the firmware's boot entries from outside the
chroot with `efibootmgr -v`.

### Finish an interrupted update

| Distribution | Commands |
| --- | --- |
| Arch | `rm /var/lib/pacman/db.lck` (only if no pacman is running), then `pacman -Syu` |
| Debian, Ubuntu | `dpkg --configure -a`, then `apt -f install` |
| Fedora | `dnf distro-sync`; for a damaged database, `rpm --rebuilddb` |

### Reset a password

```bash
passwd USERNAME
```

### Read the logs of the failed boot

From outside the chroot, without running anything from the target:

```bash
journalctl -D /mnt/var/log/journal --list-boots
journalctl -D /mnt/var/log/journal -b -1 -p err
```

## 6. Clean up

```bash
exit                      # leave the chroot
umount -R /mnt
vgchange -an              # if you activated LVM
cryptsetup close cryptroot
```

Then reboot and remove the Sanctum medium.

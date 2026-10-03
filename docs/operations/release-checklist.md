# Release hardware checklist

CI proves the ISO builds and boots in QEMU. This checklist covers what only
real hardware shows. Work through it with the ISO built from the release
commit, before tagging. Record the results in the release notes (the
"Tested on" section).

Copy the table below into the release pull request and fill it in.

## Machines

At least one of each. One machine can count for several rows.

| Requirement | Machine (make, model) | Result |
| --- | --- | --- |
| UEFI laptop | | |
| Legacy BIOS (or UEFI with CSM) machine | | |
| System with an NVMe drive | | |
| Machine with Wi-Fi | | |

## Boot

| Check | Result |
| --- | --- |
| USB written with `dd` boots (UEFI) | |
| USB written with `dd` boots (BIOS) | |
| Ventoy boots the ISO in normal mode | |
| "safe graphics" entry boots on a machine with a discrete GPU | |
| "copy to RAM" boots, and the USB can be removed afterwards | |
| The console shows the Sanctum banner; `sanctum selftest` passes | |

## Hardware

| Check | Result |
| --- | --- |
| `sanctum disks` lists every internal disk, with SMART health for SATA and NVMe | |
| `inxi -Fxz` shows CPU, graphics, network and sensors | |
| Wired Ethernet gets an address automatically | |
| Wi-Fi connects with `nmtui` | |
| Memtest86+ starts from the boot menu (BIOS and UEFI) | |

## Desktop

| Check | Result |
| --- | --- |
| `startx` reaches the desktop at the screen's native resolution | |
| The Sanctum menu opens a graphical tool (GParted) and a terminal tool (smartctl) | |
| Firefox opens the guides | |
| Keyboard and touchpad or mouse work | |

## Safety

| Check | Result |
| --- | --- |
| A Windows volume shut down with Fast Startup refuses a read-write NTFS-3G mount (`mount -t ntfs-3g /dev/sdX3 /mnt` fails with "hibernated" or "unclean") | |
| No internal disk is mounted after boot or after `startx` (`findmnt`) | |
| A BitLocker volume unlocks read-only with the recovery key (if one is available) | |

## Sign-off

| | |
| --- | --- |
| Version | |
| Commit | |
| Tested by | |
| Date | |
| Problems found | |

Problems that block the release get an issue and a fix before tagging. Other
problems go into the release notes as known issues.

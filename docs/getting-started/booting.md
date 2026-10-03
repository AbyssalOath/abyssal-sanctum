# Booting Abyssal Sanctum

## Before you boot

- **Secure Boot must be off.** Sanctum is not signed for Secure Boot yet.
- **BitLocker:** changing the Secure Boot setting on a Windows machine with
  BitLocker makes Windows ask for the **BitLocker recovery key** on its next
  boot. Make sure you have the key before changing anything in the firmware
  settings.
- Sanctum does not mount, repair, assemble or activate anything on the
  machine's disks by itself (ADR-0006).

## Boot menu

The menu looks the same on UEFI (systemd-boot) and BIOS (syslinux) machines.
It boots the first entry after 10-15 seconds.

| Entry | Use it when |
| --- | --- |
| Abyssal Sanctum | Normal start |
| safe graphics | The screen stays blank or garbled. Adds `nomodeset`, so the display runs at a basic resolution without GPU drivers. |
| copy to RAM | You want to remove the boot USB after starting, or the USB is slow. Needs about 4 GiB of RAM. |
| serial console | The machine has no screen. The console is also sent to the first serial port (ttyS0, 115200 baud). Log in as `root` with no password. |
| with speech | You need the speakup screen reader |
| Memtest86+ | Testing RAM |
| Hardware Information (HDT), BIOS only | Looking at hardware without starting Linux |
| EFI Shell, UEFI only | Inspecting firmware, devices and EFI variables |
| Boot existing OS, BIOS only | Starting the installed system instead |

## After boot

You are logged in as `root` on the first console. The login banner lists the
most useful commands:

- **Tools:** `sanctum` opens a menu of every tool by category, with its risk
  level and examples. `sanctum disks` gives a read-only overview of the
  disks. See [finding your way around](navigation.md).
- **Desktop:** `startx`.
- **Network:** wired networks connect automatically with DHCP. For Wi-Fi or
  other settings, run `nmtui`.
- **RAID and LVM** are not activated. Run `mdadm --assemble --scan` or
  `vgchange -ay` when you decide to.
- **Repairing an installed Linux system:** see the
  [Linux recovery guide](../recovery/linux.md).
- **SSH** is off, and the firewall blocks it. `sanctum ssh enable` turns it
  on.
- **Check the environment:** `sanctum selftest` confirms the safe defaults
  are in effect (firewall, no SSH, nothing assembled or mounted). It changes
  nothing.
- **Version:** `sanctum version`. The full build manifest (versions of every
  package, snapshot date, commit) is in
  `/usr/share/abyssal-sanctum/manifest.json`.

Installing extra packages with `pacman -S` works when online. Packages come
from the same Arch Linux Archive snapshot the ISO was built from, so they
match the installed system. Anything installed lives in RAM and is gone after
a reboot.

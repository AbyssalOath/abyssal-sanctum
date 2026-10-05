# Running Sanctum in a virtual machine

A virtual machine is the quickest way to try Sanctum, and the way the
project tests every build.

## With the repository's script

From a clone of the repository, after [building](building.md):

```bash
./scripts/test/run-vm.sh            # UEFI
./scripts/test/run-vm.sh --bios     # legacy BIOS
./scripts/test/run-vm.sh --disk broken-disk.img
```

The VM gets 4 GiB of RAM (`--memory` changes it), KVM acceleration when
available, and user-mode networking. Disk images given with `--disk` are
attached with `snapshot=on`: Sanctum sees them as writable, but every write
is discarded when the VM stops, so the image file never changes.

To try a signed ISO with Secure Boot on (OVMF with Microsoft's keys, as on
most PCs), keep the UEFI variables in a file so that an enrolled key
survives the reboot:

```bash
./scripts/test/run-vm.sh --secureboot --vars sb-vars.fd out/abyssal-sanctum-v*-secureboot.iso
```

To run the automated boot test, which also checks the safe defaults:

```bash
./scripts/test/boot-test.sh --uefi
```

## With virt-manager, GNOME Boxes or VirtualBox

- **Firmware:** UEFI (OVMF) or BIOS both work. Secure Boot must be off,
  unless the ISO is a signed release
  ([Secure Boot](secure-boot.md)).
- **Memory:** 4 GiB. 2 GiB is enough for the console only.
- **CD drive:** the Sanctum ISO.
- **Disks to repair:** attach them read-only, or attach a copy, unless you
  mean to change them.

Sanctum does not install guest agents, so clipboard sharing and automatic
screen resizing are not available. Use `xrandr` or the Display settings to
change the resolution on the desktop.

## Practising on a damaged system

To practise a repair and keep the result, work on a copy of a disk image
and attach it with `--writable-disk`, so changes reach the file:

```bash
cp --sparse=always original.img practice.img
./scripts/test/run-vm.sh --writable-disk practice.img
```

# Writing the ISO to USB

Any USB stick of 4 GB or more works. **Everything on the stick is
overwritten.**

## 1. Verify the download

Download the ISO and its `.sha256` file, and check them:

```bash
sha256sum -c abyssal-sanctum-vX.Y.Z-x86_64.iso.sha256
```

The `.sha256` file comes from the GitHub release, while the ISO comes from
the download site, so a match also shows the download site served the
right file. To check that the ISO was built by the project's own workflow
from the tagged source:

```bash
gh attestation verify abyssal-sanctum-vX.Y.Z-x86_64.iso --repo AbyssalOath/abyssal-sanctum
```

## 2. Write it

### Linux

Find the stick's device name. Plug it in and compare:

```bash
lsblk -o NAME,SIZE,MODEL,TRAN
```

Then write the ISO to the **whole device** (`/dev/sdX`, not a partition
such as `/dev/sdX1`):

```bash
sudo dd if=abyssal-sanctum-vX.Y.Z-x86_64.iso of=/dev/sdX bs=4M status=progress oflag=sync
```

Double-check the device name. `dd` overwrites whatever it is given without
asking.

### Windows

Use [Rufus](https://rufus.ie/) in **DD image mode** (Rufus asks when you
select the ISO), or balenaEtcher. ISO mode in Rufus rewrites the boot
setup and is not supported.

### macOS

```bash
diskutil list
diskutil unmountDisk /dev/diskN
sudo dd if=abyssal-sanctum-vX.Y.Z-x86_64.iso of=/dev/rdiskN bs=4m
```

## Ventoy

[Ventoy](https://www.ventoy.net/) sticks hold several ISOs. Copy the Sanctum
ISO onto the Ventoy partition and choose it from Ventoy's menu, in normal
mode. Ventoy is part of the release test checklist, but if a Ventoy boot
fails, try a stick written with `dd` before reporting a problem.

## Next

[Booting Abyssal Sanctum](booting.md).

# The data partition

Sanctum runs from RAM: everything it creates disappears at shutdown. A
**data partition** keeps what matters between boots:

| Folder | Contents |
| --- | --- |
| `clamav/` | ClamAV signature databases, so you do not download them every time |
| `updates/` | Offline update packs (`sanctum update --export`) |
| `cases/` | Scan reports, one folder per case (`sanctum scan`) |
| `quarantine/` | Quarantined files (from v0.2.0) |

A data partition is an **ext4 filesystem labelled `SANCTUM_DATA`**, on a
second USB stick or any disk you choose. Sanctum mounts it at `/sanctum`
automatically when it appears, at boot or when plugged in later. It is
mounted so that nothing stored on it can run (`noexec`, `nosuid`, `nodev`).
Nothing else is ever mounted automatically.

## Create one

Plug in the stick, find its name, and let Sanctum format it. **This erases
the whole stick.**

```bash
lsblk -o NAME,SIZE,MODEL,TRAN
sanctum data init /dev/sdX
```

Sanctum lists any filesystems already on the device, then asks you to type
the device name to confirm. `sanctum data init` refuses the Sanctum boot
stick and mounted disks. In scripts, `--yes` skips the question only for
blank media: a device that holds any filesystem always needs the typed
confirmation.

From another Linux machine, the same result:

```bash
sudo mkfs.ext4 -L SANCTUM_DATA /dev/sdX1
```

8 GB is enough for the databases and many cases. More space helps if you
quarantine large files or store disk images.

## Check it

```bash
sanctum data status
```

shows where it is mounted, the free space, and how much each folder holds.

## Without a data partition

Everything still works, in RAM under `/root/sanctum`. Sanctum warns that
results will be lost at reboot. Copy the case folders somewhere before you
shut down.

The live system's writable space is 256 MiB by default. The ClamAV
databases take over 100 MB of it. On a machine with enough memory, add
`cow_spacesize=2G` (or another size) to the kernel command line at the boot
menu for more room.

## Keeping it unmounted

The label is the only thing Sanctum checks. If you ever need Sanctum to
leave a disk labelled `SANCTUM_DATA` alone, for example when examining one
as evidence, add `sanctum.nodata` to the kernel command line at the boot
menu (press **Tab** in the BIOS menu, **e** in the UEFI menu).

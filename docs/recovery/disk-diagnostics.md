# Disk and hardware diagnostics

The first question in any repair: is this a hardware fault or a software
fault? This guide helps answer it.

## Overview

```bash
sanctum disks
```

It lists every disk with its model, connection, SMART health, partitions,
filesystems, and whether they are encrypted or part of a RAID array or LVM.
It only reads.

## SMART

SATA and SAS disks:

```bash
smartctl -a /dev/sdX
```

Attributes that point to a failing disk:

| Attribute | Meaning | Worry when |
| --- | --- | --- |
| Reallocated_Sector_Ct | Bad sectors replaced by spares | Above 0, and especially if it grows |
| Current_Pending_Sector | Sectors that could not be read | Any |
| Offline_Uncorrectable | Sectors that failed the offline scan | Any |
| Reported_Uncorrect | Errors the disk could not correct | Any |
| UDMA_CRC_Error_Count | Transfer errors between disk and computer | Growing: usually the cable or port, not the disk |

NVMe drives:

```bash
smartctl -a /dev/nvme0
nvme smart-log /dev/nvme0
```

Look at `critical_warning` (should be 0), `media_errors`, `available_spare`
(compared to `available_spare_threshold`) and `percentage_used` (wear; can
exceed 100).

### Self-tests

Self-tests run inside the disk and do not change data:

```bash
smartctl -t short /dev/sdX       # about 2 minutes
smartctl -t long /dev/sdX        # hours on large disks
smartctl -l selftest /dev/sdX    # results
```

### USB enclosures

Many USB adapters hide SMART. Try `smartctl -d sat -a /dev/sdX`. If that
fails too, connect the disk directly if you can.

## Kernel messages

Errors the kernel saw while talking to the hardware:

```bash
journalctl -k -p warning
journalctl -k | grep -iE 'ata[0-9]|nvme|i/o error|reset'
```

Repeated link resets or I/O errors on one disk point to the disk, its cable
or its port.

## Memory

- **Memtest86+** in the boot menu is the most thorough RAM test. Let it run
  at least one full pass; any error means faulty RAM (or wrong memory
  settings in the firmware).
- From the running system: `memtester 2G 1`.

## CPU, temperature and power

```bash
inxi -Fxz                       # overview, including temperatures
sensors                         # temperatures, fans, voltages
stress-ng --cpu 0 --timeout 10m --metrics-brief
```

Watch `sensors` in a second console while `stress-ng` runs. A machine that
crashes or throttles heavily under load, but is stable at idle, usually has
a cooling or power problem.

## Hardware or software?

| Observation | Likely cause |
| --- | --- |
| Sanctum runs stable for hours, the installed system crashes | Software (drivers, updates, configuration) |
| Memtest86+ reports errors | RAM |
| Crashes under `stress-ng`, high temperatures | Cooling, power supply |
| SMART pending or uncorrectable sectors; I/O errors in the log | Disk: copy the data off now |
| Growing CRC errors only | Cable or port |
| Device missing from `lspci` / `lsusb` | Hardware, or disabled in firmware settings |

## Identifying hardware

```bash
inxi -Fxz
lspci -nnk          # PCI devices and their drivers
lsusb
dmidecode -t system # model and serial number
hwinfo --short
```

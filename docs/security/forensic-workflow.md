# Basic forensic workflow

This is a starting workflow for incident response and investigations with
Sanctum. It does not replace your organisation's procedures or legal
advice. If the evidence may end up in court, follow your jurisdiction's
rules and use a hardware write blocker.

## Before you boot Sanctum

**Capture memory first, if it matters.** Booting Sanctum restarts the
machine and erases its RAM: running processes, network connections and
encryption keys are gone. Memory has to be captured from the running suspect
system (with a tool such as AVML or LiME on Linux, or WinPmem on Windows)
before you shut it down. Sanctum can analyse a memory image afterwards with
Volatility 3 (ROADMAP Phase 9), but cannot capture it.

Note how the machine was running, what was on screen, and when you shut it
down.

## 1. Start a record

Boot Sanctum, mount a disk for your case files, and log the session:

```bash
mkdir -p /mnt/case && mount /dev/sdY1 /mnt/case
script -a /mnt/case/session-$(date -u +%Y%m%dT%H%M%SZ).log
```

`script` records everything you type and see until you `exit`.

Record the time difference between the machine's clock and real time:

```bash
date -u
hwclock --show --utc
```

## 2. Identify the evidence without mounting it

```bash
sanctum disks --no-smart
lsblk -o NAME,SIZE,MODEL,SERIAL,FSTYPE
smartctl -i /dev/sdX            # model and serial number for your notes
```

## 3. Protect it

```bash
blockdev --setro /dev/sdX
for p in /dev/sdX[0-9]*; do blockdev --setro "$p"; done
blockdev --getro /dev/sdX        # 1 means read-only
```

This blocks writes in the kernel. It is a strong safeguard but not a
hardware write blocker. See [evidence preservation](evidence-preservation.md).

## 4. Acquire an image

Expert Witness format (E01), with hashes and case notes inside the image:

```bash
ewfacquire -t /mnt/case/evidence -C CASE -E ITEM -e "Your name" /dev/sdX
ewfverify /mnt/case/evidence.E01
```

Or a raw image, with a map file in case the disk has bad areas:

```bash
ddrescue /dev/sdX /mnt/case/evidence.raw /mnt/case/evidence.map
sha256sum /dev/sdX /mnt/case/evidence.raw | tee /mnt/case/evidence.sha256
```

The two hashes match only if every sector was read. If `ddrescue` reported
errors, record that the image is incomplete and where.

## 5. Analyse the image, not the disk

```bash
losetup -r -P -f --show /mnt/case/evidence.raw      # e.g. /dev/loop1
mmls /mnt/case/evidence.raw                         # partition layout
fls -r -o 2048 /mnt/case/evidence.raw | less        # files, including deleted (*)
icat -o 2048 /mnt/case/evidence.raw 1234 > /mnt/case/file-1234
```

`-o` is the partition's start sector from `mmls`. E01 images work directly
with The Sleuth Kit.

### A timeline

```bash
fls -r -m / -o 2048 /mnt/case/evidence.raw > /mnt/case/body.txt
mactime -b /mnt/case/body.txt -d > /mnt/case/timeline.csv
```

### Carving and metadata

```bash
photorec /log /d /mnt/case/carved /mnt/case/evidence.raw
foremost -i /mnt/case/evidence.raw -o /mnt/case/foremost
exiftool -r -csv /mnt/case/extracted > /mnt/case/metadata.csv
```

### Logs

Linux systems: `journalctl -D /path/to/var/log/journal`, and the files in
`var/log`. Windows event logs are in `Windows/System32/winevt/Logs`; copy
them out for analysis on a machine with Windows log tools.

## 6. Close the record

Hash everything you produced, then `exit` the `script` session:

```bash
sha256sum /mnt/case/* > /mnt/case/SHA256SUMS
```

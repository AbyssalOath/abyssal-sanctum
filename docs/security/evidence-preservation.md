# Evidence preservation

How to keep evidence unchanged while you work, and how to show that it is.
The [forensic workflow](forensic-workflow.md) puts these steps in order.

## What Sanctum already does

By default ([ADR-0006](../architecture/decisions/0006-non-destructive-defaults.md)),
Sanctum does not:

- mount any filesystem,
- assemble RAID arrays or activate LVM volumes,
- use swap partitions on the machine's disks,
- run any network service.

`sanctum selftest` confirms these defaults are in effect.

## What can still change evidence

| Action | What it changes | Avoid it by |
| --- | --- | --- |
| Mounting ext4 or XFS with plain `-o ro` | Replays the journal: writes to the disk | `-o ro,noload` (ext4), `-o ro,norecovery` (XFS), or image first |
| Mounting a hibernated NTFS volume read-write | Destroys `hiberfil.sys` (memory evidence) and the hibernated session | Mount read-only, or image first |
| Assembling a degraded RAID array read-write | Starts a resync | `mdadm --assemble --readonly` |
| `fsck` and other repair tools | Rewrite filesystem structures | Never run them on evidence |
| Running `os-prober` | Mounts every partition read-only (with journal replay) | Do not run it during an investigation |
| Time synchronisation | Sanctum's clock syncs over the network, and the kernel then updates the machine's hardware clock | Record `hwclock` before connecting to a network (a forensic boot mode without time sync is planned, ROADMAP Phase 5) |
| Booting the machine's own OS | Many writes, log rotation, updates | Do not boot it again before imaging |

## Write protection

Set the device and all its partitions read-only in the kernel before
anything else:

```bash
blockdev --setro /dev/sdX
for p in /dev/sdX[0-9]*; do blockdev --setro "$p"; done
```

This stops writes from Linux, including from tools started by mistake.
It does not stop the disk's own firmware, and it is not equivalent to a
hardware write blocker in court. Use a hardware blocker where your
procedures require one.

## Hashing

Hash the source before and after imaging, and hash the image:

```bash
sha256sum /dev/sdX
```

For E01 images, `ewfacquire` stores MD5 and optionally SHA hashes inside the
image, and `ewfverify` checks them.

For files, keep a manifest:

```bash
find /mnt/case -type f -exec sha256sum {} + > /mnt/case/SHA256SUMS
```

## Documentation

Write down, as you go:

- who handled the evidence, when, and what was done,
- make, model and serial number of every disk (`smartctl -i`),
- the time offset between the machine's clock and real time,
- the Sanctum version (`sanctum version`) and the commands you ran
  (`script` records them),
- every hash, and where each copy of the evidence is stored.

## Storing evidence

- Keep the original disk and at least one verified image.
- Work on copies of the image.
- Store case files on a disk that is not the evidence disk, and hash them
  when you finish.

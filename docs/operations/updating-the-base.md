# Updating the base

Sanctum's packages come from a dated Arch Linux Archive snapshot
([ADR-0002](../architecture/decisions/0002-containerised-pinned-build.md)).
Updating the base means moving to a newer snapshot. Do this for every
release, and sooner when Arch publishes important security fixes.

## Steps

1. **Choose the snapshot date.** Normally yesterday's date, so the snapshot
   is complete:

   ```bash
   curl -sI https://archive.archlinux.org/repos/YYYY/MM/DD/core/os/x86_64/core.db | head -1
   ```

2. **Update the builder image.** Pull the current image and record its
   digest:

   ```bash
   docker pull archlinux:base-devel
   docker image inspect archlinux:base-devel --format '{{index .RepoDigests 0}}'
   ```

3. **Edit `build/build.conf`:** `SNAPSHOT_DATE`, `BUILDER_IMAGE` (and the
   comment with its date).
4. **Check the archiso version.** Build once. If the build stops with
   "snapshot provides archiso X, build.conf expects Y", archiso was
   updated:
   - Read archiso's changelog for the new version.
   - Diff the new `releng` profile against the previous one, and port the
     relevant changes into `build/profile/`. You can extract releng with:

     ```bash
     docker run --rm -v "$PWD/out:/out" "$BUILDER_IMAGE" bash -c '
       echo "Server = https://archive.archlinux.org/repos/YYYY/MM/DD/\$repo/os/\$arch" >/etc/pacman.d/mirrorlist
       pacman -Syy --noconfirm archiso >/dev/null && cp -r /usr/share/archiso/configs/releng /out/releng-new'
     ```

   - Set `ARCHISO_VERSION`.
5. **Check `linux-hardened`.** It sometimes trails a new mainline release by
   a few days ([ADR-0004](../architecture/decisions/0004-linux-hardened-kernel.md)).
6. **Build, boot-test** (UEFI and BIOS), and run the `startx` check from
   the [release checklist](releasing.md).
7. **Record it** in `CHANGELOG.md`: the new snapshot date, kernel version,
   and anything notable.

## If a package disappeared

A package that Arch dropped makes the build fail at "Resolving the package
list against the snapshot". Find a replacement in the official repositories
(or drop the tool), and update the package list and the catalog together.

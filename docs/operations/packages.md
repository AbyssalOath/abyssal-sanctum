# Adding and removing packages

Packages follow the inclusion policy in
[ADR-0003](../architecture/decisions/0003-package-policy.md): official Arch
repositories only, and a written reason for every package.

## Adding a package

1. Check that it is in `core` or `extra`:
   <https://archlinux.org/packages/>. A package from anywhere else needs its
   own ADR first.
2. Add one line to the matching list in `build/packages/`:

   ```text
   iperf3               # Throughput tests between two machines
   ```

   The reason says what the package is for in Sanctum, not what it is.
3. If technicians should find it, add it to the [tool catalog](catalog.md).
4. Check:

   ```bash
   scripts/ci/check-package-lists.sh
   cargo run --locked --quiet -- catalog check
   ```

5. Build and boot-test. The build resolves every package against the pinned
   snapshot, so a misspelled name fails within seconds:

   ```bash
   scripts/build/build-iso.sh
   scripts/test/boot-test.sh --uefi
   scripts/test/boot-test.sh --bios
   ```

6. Note the ISO size in the build output. The build warns above 2.5 GiB and
   fails above 3 GiB ([ADR-0005](../architecture/decisions/0005-iso-hosting-and-size-budget.md)).
7. Add the change to `CHANGELOG.md`.

### Things to check for new packages

- **Services.** Arch packages do not enable their services. Do not enable
  one unless it is needed, and never one that listens on the network
  ([ADR-0006](../architecture/decisions/0006-non-destructive-defaults.md)).
- **Automatic actions on disks.** udev rules or units that mount, assemble
  or activate storage must be masked. The self-test fails if udisks2, gvfs
  or thunar-volman are installed.
- **Dependencies.** A small package can pull in a large dependency tree.
  Compare the ISO size before and after.

## Removing a package

1. Delete its line from `build/packages/`.
2. Remove its catalog entries. `catalog check` fails while a catalog entry
   still refers to it.
3. Check that no other file depends on it (for example the overlay or the
   self-test): `grep -r NAME build/ catalog/ docs/`.
4. Build, boot-test, and update `CHANGELOG.md`.

A package can still end up in the ISO as another package's dependency.
The manifest (`/usr/share/abyssal-sanctum/manifest.json`) lists everything
that was installed.

# Changing the live system's configuration

The live system's configuration lives in `build/profile/airootfs/`, which is
copied over the installed packages
([architecture overview](../architecture/overview.md#the-overlay)).

## Changing a file

Put the file at its path under `airootfs/`, for example
`airootfs/etc/NetworkManager/conf.d/50-sanctum.conf`.

- **Prefer drop-in directories** (`conf.d`, `*.conf.d`) over replacing a
  whole configuration file. Drop-ins survive package updates.
- **Files owned by a package** can only be replaced if the package marks
  them as configuration. Otherwise the build fails with a file conflict.
  Check with `pacman -Qii PACKAGE` on an Arch system (the "Backup Files"
  list).
- **Executable files** need an entry in `file_permissions` in
  `build/profile/profiledef.sh`. `scripts/ci/check-profile.sh` fails
  without it.

## Build-time values

These placeholders are filled in when the profile is staged, in any text
file under `build/profile/`:

| Placeholder | Value |
| --- | --- |
| `@SANCTUM_VERSION@` | `VERSION`, for example `0.1.0` |
| `@SNAPSHOT_DATE@` | `SNAPSHOT_DATE` from `build/build.conf` |
| `@GIT_COMMIT@` | The commit being built, `-dirty` if there were uncommitted changes |

Any other `@NAME@` fails the build and `check-profile.sh`.

## Services

Enable a service with a symlink, as `systemctl enable` would create it:

```bash
ln -s /usr/lib/systemd/system/NAME.service \
  build/profile/airootfs/etc/systemd/system/multi-user.target.wants/NAME.service
```

Mask a unit with a link to `/dev/null`. Never enable a service that listens
on the network.

## Safety defaults

The defaults in [ADR-0006](../architecture/decisions/0006-non-destructive-defaults.md)
are checked three times:

1. `scripts/ci/check-profile.sh` checks the files (md and LVM settings,
   sshd not enabled, the firewall policy).
2. `sanctum-selftest` checks the running system in every boot test.
3. The boot test attaches RAID and LVM disks and checks they are unchanged.

To change a safety default, write a new ADR first, then update all three.

## Generated files

Do not add these to `airootfs/`: the build generates them.

- `/usr/local/bin/sanctum`
- `/usr/share/applications/sanctum-*.desktop`, the `.directory` files and
  `/root/.config/menus/xfce-applications.menu` (from the [catalog](catalog.md))
- `/usr/share/doc/abyssal-sanctum/` (from `docs/` and the root Markdown files)
- `/usr/share/abyssal-sanctum/manifest.json`
- `/etc/pacman.conf`, `/etc/pacman.d/mirrorlist` (from `build/profile/pacman.conf`)
- the desktop background (from `assets/`)

## Branding images

The BIOS boot splash and the panel icon are generated from
`assets/abyssal-sanctum-emblem.png`:

```bash
python3 scripts/dev/make-branding.py   # needs Pillow
```

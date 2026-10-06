# Working on a Windows installation

Sanctum can read and copy Windows data, inspect the registry, reset local
passwords and rebuild some boot files. It cannot do everything Windows'
own tools do. The limits are listed first so you can decide quickly
whether to reach for Windows installation media instead.

## What Linux cannot do

| Task | From Sanctum | Use instead |
| --- | --- | --- |
| Repair NTFS | No. `ntfsfix` only clears the dirty flag and resets the journal, so Windows runs `chkdsk` at next boot | `chkdsk /f` from Windows RE |
| Create or rebuild the BCD boot store | No | `bcdboot C:\Windows` from Windows RE |
| Reset a Microsoft account password | No, only local accounts | account.microsoft.com |
| Unlock BitLocker with only a TPM | No | The 48-digit recovery key |
| Undo Fast Startup or hibernation safely | No | Boot Windows and shut down fully (Shift + Shut down) |

## BitLocker

Sanctum unlocks BitLocker volumes with the password or the 48-digit
recovery key:

```bash
cryptsetup open --type bitlk --readonly /dev/sdX3 windows
mount -t ntfs-3g -o ro /dev/mapper/windows /mnt/windows
```

Drop `--readonly` and `-o ro` only if you must write. Remember that turning
Secure Boot off to boot Sanctum makes Windows ask for this key on its next
boot; a signed release can boot with Secure Boot on instead
([Secure Boot](../getting-started/secure-boot.md)).

## Mounting the Windows volume

Mount read-only unless you need to write:

```bash
mkdir -p /mnt/windows
mount -t ntfs-3g -o ro /dev/sdX3 /mnt/windows
```

If Windows was hibernated, or shut down with **Fast Startup** (the
default), the volume still holds Windows' cached state. NTFS-3G then
refuses to mount it read-write ("Windows is hibernated, refused to mount").
That refusal protects your data. Writing anyway (`remove_hiberfile`)
throws away the hibernated session, and anything unsaved in it.
`hiberfil.sys` is also memory evidence in an investigation.

## Copying user data

```bash
mount -t ntfs-3g -o ro /dev/sdX3 /mnt/windows
mount /dev/sdY1 /mnt/usb                  # the backup disk
rsync -rt --info=progress2 /mnt/windows/Users/NAME/ /mnt/usb/NAME/
```

`-rt` copies files and times without trying to recreate Windows ownership.
See [backup and restore](backup-and-restore.md) for whole-disk images.

## Resetting a local account password

1. Mount the Windows volume read-write, and back up the SAM hive first:

   ```bash
   mount -t ntfs-3g /dev/sdX3 /mnt/windows
   cp /mnt/windows/Windows/System32/config/SAM /mnt/usb/SAM.backup
   ```

2. List the accounts, then edit one:

   ```bash
   chntpw -l /mnt/windows/Windows/System32/config/SAM
   chntpw -u NAME /mnt/windows/Windows/System32/config/SAM
   ```

3. Choose "Clear (blank) user password" and "Unlock and enable user
   account", then quit and write the hive.

This works for local accounts only. Accounts signed in with a Microsoft
account, and volumes encrypted with BitLocker that you cannot unlock, are
out of reach.

## Inspecting the registry

`hivexsh` browses hive files and is read-only unless started with `-w`:

```bash
hivexsh /mnt/windows/Windows/System32/config/SOFTWARE
```

Inside it:

```text
cd \Microsoft\Windows\CurrentVersion\Run
lsval
```

Programs started at login (the `Run` keys) are a common place to look when
investigating malware. The user's own `Run` key is in
`Users/NAME/NTUSER.DAT`.

## UEFI boot problems

The Windows boot manager lives on the EFI system partition (ESP), as
`\EFI\Microsoft\Boot\bootmgfw.efi`, with its configuration in `BCD` next
to it.

1. Check the firmware's boot entries:

   ```bash
   efibootmgr -v
   ```

2. Mount the ESP and look for the files:

   ```bash
   mount /dev/sdX1 /mnt/esp
   ls /mnt/esp/EFI/Microsoft/Boot/
   ```

3. If only the firmware entry is missing, recreate it (Sanctum must be booted
   in UEFI mode):

   ```bash
   efibootmgr --create --disk /dev/sdX --part 1 \
     --label "Windows Boot Manager" --loader '\EFI\Microsoft\Boot\bootmgfw.efi'
   ```

4. If boot manager files are missing, copy them from the Windows
   installation (`/mnt/windows/Windows/Boot/EFI/`) into
   `/mnt/esp/EFI/Microsoft/Boot/`. If `BCD` itself is missing or damaged,
   Linux cannot rebuild it: run `bcdboot C:\Windows` from Windows RE.

## Malware

See [offline malware scanning](../security/offline-malware-scanning.md). Scan
the Windows volume mounted read-only, and move findings to a USB quarantine
rather than deleting them.

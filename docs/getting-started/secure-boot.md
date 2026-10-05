# Booting with Secure Boot on

Signed Sanctum releases boot with **Secure Boot left on**. This matters on
Windows machines with BitLocker: turning Secure Boot off makes Windows ask
for the BitLocker recovery key at its next start, and leaving it on avoids
that.

Signing starts with the first signed release (planned for v0.2.0). v0.1.0
and builds you make yourself are not signed: for those, turn Secure Boot
off as described in [booting](booting.md).

## How it works

Sanctum starts through **shim**, a small loader signed by Microsoft that
Linux distributions use for Secure Boot (Sanctum uses Fedora's). shim then
checks Sanctum's boot loader and kernel against **Sanctum's own key**. Each
machine has to trust that key once: you **enrol** it in shim's key manager,
MokManager, the first time Sanctum boots there. The key goes into the
machine's list of machine owner keys (MOK), not into the firmware's own key
database, so Windows' BitLocker measurements do not change.

When you are done with a machine, **remove the key again** with
`sanctum secureboot forget` (see below).

## First boot on a machine: enrol the key

You need the Sanctum key's **SHA-1 fingerprint** from the release notes
(it is also in the `-secureboot.txt` file published with each signed ISO).

1. Boot from the Sanctum USB stick with Secure Boot on. A blue screen says
   **Verification failed: (0x1A) Security Violation**. This is expected:
   the machine does not trust Sanctum's key yet. Press **Enter** (OK).
2. **Press any key to perform MOK management** appears for 10 seconds.
   Press a key.
3. Choose **Enroll key from disk**.
4. Choose the volume **ARCHISO_EFI** (the stick's boot partition), then the
   file **sanctum.cer**.
5. Choose **View key 0**. Check that the subject is
   `CN=Abyssal Sanctum Secure Boot Signing` and that the **fingerprint
   matches the one in the release notes**. If it does not match, stop: the
   stick does not hold a genuine Sanctum release. Press a key to go back.
6. Choose **Continue**, then **Yes**, then **Reboot**.
7. Boot from the USB stick again. Sanctum now starts normally, and does so
   on this machine every time until the key is removed.

Check the result in Sanctum:

```bash
sanctum secureboot status
```

```text
Firmware:        UEFI
Secure Boot:     on
Boot chain:      shim (Sanctum's signed boot chain)
Kernel lockdown: none
Sanctum key:     enrolled on this machine (SHA-1 ...)
                 Remove it when you are done: sanctum secureboot forget
```

## When you are done: remove the key

While the key is enrolled, anything signed with it starts on that machine
with Secure Boot on, including any copy of Sanctum, which gives a root
shell. Remove it before you hand the machine back:

```bash
sanctum secureboot forget
```

1. Type a **one-time password** twice. You need it once more, at the next
   boot.
2. Reboot **with the Sanctum USB stick still plugged in** and boot from it.
   The removal is done by shim on the stick, not by Windows.
3. **Press any key to perform MOK management** appears for 10 seconds.
   Press a key. If you miss it, the request is dropped: boot Sanctum and
   run `sanctum secureboot forget` again.
4. Choose **Delete MOK**, **Continue**, **Yes**, type the one-time password,
   then **Reboot**.

The machine is back as it was: with Secure Boot on, Sanctum is refused
again (the blue "Verification failed" screen).

## What does not work with Secure Boot on

- The **EFI Shell** entry. It is deliberately not signed: a trusted shell
  could start any EFI program on every machine that enrolled the key.
- **32-bit UEFI** machines (some old tablets). Turn Secure Boot off there.
- PCs where Microsoft's **third-party UEFI CA** is not trusted, for example
  many Secured-core PCs. The firmware refuses shim before MokManager
  appears. The firmware setting is usually called "Allow Microsoft 3rd party
  UEFI CA". Changing it, like turning Secure Boot off, makes Windows ask for
  the BitLocker recovery key. Have the key first.

Every Sanctum entry in the boot menu works. Memtest86+ is signed too, but
has not yet been tried with Secure Boot on (it does not start under QEMU's
Secure Boot firmware even unsigned); if it does not start on your machine,
turn Secure Boot off for the RAM test.

## Good to know

- Sanctum does not turn on **kernel lockdown** under Secure Boot: some
  recovery and forensic tools need raw access to memory and devices.
  Secure Boot here protects the machine's Windows boot, not Sanctum itself.
- shim also trusts software signed by Fedora. That is true of every PC that
  trusts Microsoft's third-party CA anyway, and enrolling Sanctum's key does
  not change it.
- Enrolling or removing the key changes shim's key list only, which
  Windows' BitLocker does not measure, so it should not trigger a recovery
  prompt. This is still to be confirmed on a real BitLocker machine
  ([ADR-0011](../architecture/decisions/0011-secure-boot-shim-mok.md)).

# Secure Boot research

Status: research report for decision AV-D8, 2026-10-05, with the results of
the QEMU prototype. The decision that follows from it is
[ADR-0011](../architecture/decisions/0011-secure-boot-shim-mok.md).
Statements marked **to verify** still need real hardware.

## Why it matters

Sanctum v0.1.0 is not signed for Secure Boot, so technicians turn Secure
Boot off. On Windows machines with BitLocker bound to PCR 7 (the default),
changing the Secure Boot state makes Windows ask for the **BitLocker
recovery key** at its next boot. When scanning a fleet of compromised
machines, that is a recovery-key hunt for every machine, and a real risk of
locking a user out when the key cannot be found.

Booting Sanctum **with Secure Boot left on** avoids this. Sanctum's own boot
is measured, but the next Windows boot sees the same Secure Boot policy as
before, so its PCR 7 value is unchanged and BitLocker unlocks as usual.
**To verify** on a BitLocker machine.

## How other Linux systems boot with Secure Boot on

Firmware trusts binaries signed by keys in its `db`. On most PCs that
includes Microsoft's third-party UEFI CA. Linux distributions get a small
first-stage loader, **shim**, signed by Microsoft after a public review
(shim-review). shim contains the distribution's own certificate and
verifies the next stage (GRUB or a kernel) against it, or against
**Machine Owner Keys (MOK)** that the machine's owner enrolled.

## Options for Sanctum

| Option | How | For | Against |
| --- | --- | --- | --- |
| 1. Turn Secure Boot off (v0.1.0) | Documented warning | Nothing to build | BitLocker recovery prompts; extra steps on every machine |
| 2. Reuse a distribution's Microsoft-signed shim, plus a Sanctum MOK | Ship e.g. Fedora's signed shim as `BOOTX64.EFI`; sign Sanctum's boot loader and kernel with a Sanctum key; the technician enrols the Sanctum certificate once per machine in MokManager | Works on most PCs without touching firmware settings; used by Ventoy and other tools | One MokManager enrolment per machine (a reboot and a few key presses); the enrolled key stays in the machine's NVRAM unless removed; Sanctum depends on that distribution keeping its shim current |
| 3. Own shim, signed by Microsoft | Pass shim-review with a Sanctum certificate | No enrolment step | Needs an organisation, HSM key custody, kernel lockdown and SBAT policies, and months of review; not realistic for now |
| 4. Enrol Sanctum's key in the firmware `db` | `sbctl` on each machine | No shim | Changes the firmware key database, which changes PCR 7, which triggers BitLocker recovery: the problem this is meant to solve |

Option 2 was chosen (ADR-0011).

## Option 2 in detail

### Boot chain

```text
firmware (Secure Boot on, trusts Microsoft UEFI CA 2011 or 2023)
  -> EFI/BOOT/BOOTx64.EFI   shim 16.1 (Fedora's, signed by Microsoft)
    -> EFI/BOOT/grubx64.efi systemd-boot (signed with the Sanctum key)
      -> linux-hardened, Memtest86+ (signed with the Sanctum key)
         initramfs (not signed; see the caveats)
```

### Signing

- **Not in mkarchiso.** An earlier draft of this report said mkarchiso's
  `-c` option signs EFI binaries for Secure Boot. That was wrong: in
  archiso 91, `-c` is CMS signing of netboot artifacts.
- Instead, `scripts/release/sign-secureboot.sh` turns a built, unsigned ISO
  into a signed one: it signs systemd-boot (installed as `grubx64.efi`, the
  name shim starts) and every kernel and EFI program in the boot menu with
  `sbsign`, adds shim, MokManager and `sanctum.cer`, and rewrites the ISO
  with `xorriso`. It keeps the boot layout (BIOS, El Torito, hybrid MBR/GPT)
  and the volume date, which archiso uses as the ISO's UUID to find its
  files. The result also boots with Secure Boot off.
- The signing time is pinned to the ISO's date (`faketime`), and xorriso
  derives its GPT GUIDs from the same date, so **signing the same ISO with
  the same key twice gives the same file**.
- The signing tools (`sbsigntools`, `libisoburn`, `mtools`, `libfaketime`)
  come from the same Arch snapshot as the ISO, in the pinned builder
  container.

### Which shim

- Arch has no official signed shim. Sanctum takes **Fedora's `shim-x64`
  16.1-7** (stable in Fedora 44 to 46), pinned by the package's SHA-256 and
  the SHA-256 of the two files used (`build/secureboot.conf`).
- Its `shimx64.efi` carries **two Microsoft signatures: UEFI CA 2011 and
  UEFI CA 2023**, so it boots on firmware with either. (16.1-10, a newer
  build, has the same `shimx64.efi`, but was not released when checked.)
- `mmx64.efi` (MokManager) is signed by Fedora, which shim trusts.
- `fbx64.efi` (fallback) is **left out on purpose**: next to shim on a
  removable drive it creates boot entries in the machine's firmware.

### Second stage and SBAT

- shim requires SBAT data in the second stage it starts. Arch's
  systemd-boot 262 has it (`systemd-boot,1` and `systemd-boot.arch,1`), so
  systemd-boot is the second stage, renamed to `grubx64.efi`. Unified kernel
  images were not needed.
- The linux-hardened kernel has **no SBAT section**. That is fine: shim
  requires SBAT only for images it starts itself, not for images verified
  through its protocol, which is how systemd-boot starts kernels. Confirmed
  in the prototype.
- Sanctum must follow Fedora's shim updates: SBAT revocations delivered by
  Windows Update can make an old shim refuse to start. **To verify** which
  SBAT levels the target machines enforce.

### The technician's experience (prototype)

1. Boot Sanctum with Secure Boot on: "Verification failed: (0x1A) Security
   Violation". OK opens MokManager.
2. "Enroll key from disk", volume `ARCHISO_EFI`, file `sanctum.cer`. "View
   key 0" shows the subject and the **SHA-1** fingerprint, so releases
   publish the SHA-1 as well as the SHA-256.
3. Continue, Yes, Reboot. Sanctum boots, and keeps booting on that machine.

Removal (`sanctum secureboot forget`, using `mokutil --delete` with a
one-time password):

1. The request is stored in NVRAM (`MokDel`) and carried out by shim at the
   next boot, so the technician reboots from the Sanctum stick.
2. MokManager shows "Press any key to perform MOK management" for **10
   seconds**. **If no key is pressed, shim drops the request** and the key
   stays enrolled. The command and guide say so, and `sanctum secureboot
   status` shows whether the key is still there.
3. "Delete MOK", Continue, Yes, the password, Reboot. Sanctum is then
   refused again with Secure Boot on.

Also learned: `mokutil --test-key` exits with status 1 when the key **is**
enrolled, so the command reads `mokutil --list-enrolled` instead.

### Caveats

- **Microsoft's third-party CA is disabled by default on some PCs** (for
  example Secured-core PCs). There, no shim boots until the setting "Allow
  Microsoft 3rd party UEFI CA" is turned on. Changing it changes the
  firmware `db` and so PCR 7: BitLocker recovery again. **To verify** how
  common this is on the machines you service.
- **The Sanctum signing key becomes a trust anchor** on every machine where
  it is enrolled: anything signed with it boots there with Secure Boot on.
  The maintainer keeps it offline and signs at release time (ADR-0011), and
  technicians remove it after work.
- **The initramfs and kernel command line are not verified.** With the key
  enrolled, someone with physical access could boot Sanctum's signed kernel
  with their own initramfs. Unified kernel images would close this, at the
  cost of a much larger EFI partition (one image per boot entry). For now
  the answer is removing the key after work.
- **Kernel lockdown** is not enabled (`[none]` in the prototype).
  linux-hardened does not turn it on under Secure Boot, and Sanctum does not
  either: forensic tools need `/dev/mem` and raw device access.
- The **EFI Shell** is deliberately left unsigned. 32-bit UEFI (IA32) is not
  covered: Fedora ships no signed IA32 shim for this purpose.

## Prototype results (QEMU, 2026-10-05)

OVMF with Secure Boot on and Microsoft's keys enrolled
(`scripts/test/run-vm.sh --secureboot --vars FILE`):

| Check | Result |
| --- | --- |
| Unsigned ISO with Secure Boot on | Refused by the firmware (Access Denied), as expected |
| Signed ISO, key not enrolled | shim starts, refuses systemd-boot, opens MokManager |
| Enrolment from `sanctum.cer` on the stick | Works; fingerprint matches the certificate |
| Boot after enrolment | Boot menu, kernel and live system start; `SecureBoot` is 1 |
| Removal with `mokutil --delete` and a reboot | Works when the MokManager prompt is answered; dropped after the 10 s timeout |
| After removal | Signed ISO refused again: the machine is as before |
| Signing twice | Same ISO, byte for byte |
| `sanctum secureboot status` and `forget` | Status correct before and after; forget plus MokManager removes the key |
| Signed ISO with Secure Boot off (UEFI through shim, and BIOS) | Self-test passes |
| Memtest86+ with Secure Boot on | Not tested: Memtest86+ shows a black screen under QEMU's SMM firmware even unsigned, so QEMU cannot tell. To verify on hardware |

Still to test on hardware (ADR-0011): at least two machines, one with
BitLocker, checking that BitLocker unlocks normally after a Sanctum boot,
an enrolment and a removal.

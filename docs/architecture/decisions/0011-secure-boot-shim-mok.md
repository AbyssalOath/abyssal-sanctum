# ADR-0011: Secure Boot through a signed shim and a Sanctum machine owner key

- **Status:** Accepted (2026-10-05). Signed releases ship after the
  hardware test below passes.
- **Date:** 2026-10-05

## Context

Sanctum v0.1.0 cannot boot with Secure Boot on. Turning Secure Boot off
triggers BitLocker recovery prompts on Windows machines, which makes
scanning compromised Windows fleets (the antimalware track) slow and risky.
The options are compared in the
[Secure Boot research report](../../development/secure-boot-research.md).

## Decision

1. **shim:** Sanctum boots through Fedora's Microsoft-signed shim
   (`shim-x64` 16.1-7, signed by both Microsoft UEFI CAs, 2011 and 2023),
   with Fedora's MokManager. This is the one vendored binary in Sanctum, and
   this ADR is the record ADR-0003 asks for:
   - *Why no official package:* Arch Linux has no Microsoft-signed shim.
   - *Who maintains it:* Fedora. Sanctum follows Fedora's stable updates,
     because SBAT revocations can retire old shims.
   - *Integrity:* the package's SHA-256 and the SHA-256 of `shimx64.efi`
     and `mmx64.efi` are pinned in `build/secureboot.conf` and checked
     every time. `fbx64.efi` is never shipped (it writes firmware boot
     entries).
2. **Second stage:** Arch's systemd-boot, signed with the Sanctum key and
   installed as `EFI/BOOT/grubx64.efi`. The kernels and Memtest86+ in the
   boot menu are signed with the same key. The EFI Shell is not signed.
3. **Key and certificate:** one RSA-2048 signing key with the certificate
   `CN=Abyssal Sanctum Secure Boot Signing`, made with
   `scripts/release/make-secureboot-key.sh`. The certificate is on every
   signed ISO as `sanctum.cer`; its SHA-1 and SHA-256 fingerprints are
   published with every release.
4. **Key custody: the maintainer signs at release time.** The private key
   stays offline, passphrase-protected, outside the repository and outside
   CI. CI keeps building and attesting the unsigned ISO; the maintainer runs
   `scripts/release/sign-secureboot.sh` on that ISO. Signing is
   reproducible (the same ISO and key give the same file), and the signed
   ISO's summary records the unsigned ISO's SHA-256, so the published ISO
   can be tied back to the attested one.
5. **Enrolment:** technicians enrol `sanctum.cer` once per machine in
   MokManager, after checking its fingerprint.
6. **Removal:** `sanctum secureboot forget` requests the removal with
   `mokutil --delete`, and the technician confirms it in MokManager at the
   next boot. `sanctum secureboot status` shows whether the key is enrolled.

## Before signed releases ship

- Hardware test on at least two machines, one with BitLocker: boot with
  Secure Boot on, enrol, use Sanctum, remove the key, and check that
  BitLocker unlocks normally afterwards.
- Check how the target machines enforce SBAT (shim's `SbatLevel`).

Done in QEMU (OVMF with Microsoft's keys): the boot chain, enrolment,
removal and reproducible signing (research report, prototype results).
Done on hardware (2026-10-06, one UEFI machine without BitLocker, real
release key): enrolment, boot with Secure Boot on, Memtest86+. Still
needed: the BitLocker machine.

## Consequences

- The release checklist gains a signing step and a Secure Boot boot test;
  the release notes publish the certificate fingerprints.
- Sanctum must follow Fedora's shim updates (`build/secureboot.conf`).
- While the key is enrolled on a machine, anything signed with it boots
  there with Secure Boot on, including the kernel with another initramfs.
  The guide tells technicians to remove the key after work; unified kernel
  images, which would also cover the initramfs, are a possible later step.
- PCs that do not trust Microsoft's third-party CA (some Secured-core PCs)
  still need a firmware change, and so the BitLocker recovery key.
- Kernel lockdown stays off, so forensic tools keep raw access.
- A lost or leaked key means a new key, a new certificate for every
  machine, and removing the old one wherever it was enrolled. The
  maintainer keeps a backup of the key, offline.

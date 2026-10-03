# ADR-0004: `linux-hardened` kernel

- **Status:** Accepted
- **Date:** 2026-10-02

## Context

Sanctum boots on machines that may be compromised, and it may be connected
to untrusted networks during an incident. The kernel options in the official
repos are `linux` (mainline stable), `linux-lts` and `linux-hardened`.

## Options considered

1. **`linux`.** Newest hardware support. This is what Arch's own ISO uses.
2. **`linux-lts`.** More conservative. Can be too old for brand-new laptops.
3. **`linux-hardened`.** Follows mainline stable closely: in the 2026-10-01
   snapshot it is 7.2.8.hardened1, the same base as `linux` 7.2.8. It adds
   hardening patches and stricter defaults.

## Decision

`linux-hardened` is the only kernel in the ISO. This matches Sanctum's
security-first defaults, and because it follows mainline, it keeps the
hardware-support benefit of option 1.

## Consequences

Things that must be checked in Phase 3 and on real hardware in Phase 4,
because they could affect recovery work:

- Unprivileged user namespaces are disabled by default. This can affect
  application sandboxes, for example Firefox's.
- Some debugging and introspection interfaces are restricted. Hardware
  tools that need raw `/dev/mem` access may fail; `dmidecode` normally reads
  `/sys/firmware/dmi` instead.
- Hardening such as zeroing memory on allocation and free has a small
  performance cost. That is acceptable for a recovery system.
- Out-of-tree modules (for example NVIDIA's proprietary driver) are not
  available prebuilt for this kernel. Sanctum relies on in-tree drivers
  (nouveau for NVIDIA), with a `nomodeset` boot entry as a fallback.
- `linux-hardened` sometimes trails a new mainline point release by a few
  days. Snapshot bumps should check that it has caught up.

If any of these blocks real recovery work, the fallback is a second boot
entry with `linux`, subject to the size budget. That would be a new ADR.

The boot loader configs reference `vmlinuz-linux-hardened` and
`initramfs-linux-hardened.img`. Changing the kernel means updating
`syslinux/`, `efiboot/` and `grub/` together.

# ADR-0006: Non-destructive defaults

- **Status:** Accepted
- **Date:** 2026-10-02

## Context

Sanctum boots on machines whose disks may be damaged, encrypted, part of a
RAID array, or evidence in an investigation. The brief requires:

- no automatic disk modification,
- no automatic filesystem repair,
- minimal exposed services,
- SSH off unless explicitly enabled.

A standard Arch live system breaks several of these quietly (gap analysis,
sections 2.4 and 4). This record lists what Sanctum changes and why.

## Decision

### Disks

| Mechanism | Default in Arch | Sanctum | How |
| --- | --- | --- | --- |
| md RAID auto-assembly | udev assembles arrays when member disks appear; a dirty array can start a resync, which writes to the disks | Off | `/etc/udev/rules.d/64-md-raid-assembly.rules` masked (linked to `/dev/null`) |
| LVM auto-activation | udev activates every volume group it finds | Off | `/etc/lvm/lvm.conf`: `auto_activation_volume_list = []` |
| GPT partition auto-discovery | systemd may mount or activate partitions (including swap) by type | Off | `systemd-gpt-auto-generator` masked (inherited from releng) |
| Optical media loop-attach | systemd 262 loop-attaches some optical drives to expose partitions | Off | `systemd-loop@.service` masked |
| RAID monitoring | `mdmonitor.service` starts when an array is assembled, and fails without a mail address | Off | `mdmonitor.service` masked: a live system cannot deliver its alerts, and the failure marked the system "degraded" after a deliberate `mdadm --assemble` |
| Desktop automount | Not installed yet | Must stay off | Checked when the GUI arrives (Phase 3) |

Masking the md udev rule was chosen over `AUTO -all` in `mdadm.conf`.
`AUTO -all` would also make a deliberate `mdadm --assemble --scan` do
nothing, which would confuse technicians. With the rule masked, arrays
assemble only when someone runs `mdadm`, and `--scan` works as documented.

LVM is handled through `auto_activation_volume_list` instead of
`event_activation = 0`. In current lvm2, `event_activation = 0` moves
activation to boot-time services, which still activate everything.

Users activate storage deliberately:

- RAID: `mdadm --assemble --scan`
- LVM: `vgchange -ay`

### Network exposure

- **Firewall:** `nftables` with a default-deny input policy, loaded at boot
  (`/etc/nftables.conf`, table `inet sanctum`). Allowed in: replies to the
  machine's own connections, loopback, ICMP, and DHCPv6 replies. A
  `tcp_open` set lists ports the user opens explicitly:
  `nft add element inet sanctum tcp_open '{ 22 }'`.
  Arch's `nftables.service` loads the rules and exits, so it shows as
  "inactive" afterwards. Checks must inspect the ruleset, not the unit
  state.
- **SSH:** `sshd` is installed but not enabled. Root has no password, and
  `sshd` refuses empty passwords, so starting `sshd` alone lets nobody in.
  Enabling SSH takes three deliberate steps: set a password, start `sshd`,
  open port 22.
- **No local-network name services.** mDNS and LLMNR are off in
  systemd-resolved, so the machine does not announce itself or answer name
  queries.
- **No mDNS announcer.** Avahi is installed as a dependency of desktop
  libraries and can be started on demand over D-Bus. Its service and socket
  are masked.
- **No unsolicited outbound checks.** NetworkManager's connectivity check is
  off. IPv6 privacy addresses are on.
- **Removed from releng:**
  - `cloud-init`: an attached disk labelled as a datasource could configure
    the live system.
  - VM guest agents (QEMU, VMware, VirtualBox, Hyper-V): host control
    channels.
  - `pcscd`, `ModemManager`, `reflector` and `choose-mirror`.
  - releng's `script=` boot parameter, which downloads and runs a script.

## Consequences

- A technician must activate RAID and LVM by hand. The login banner says
  so, and shows the commands.
- The boot environment has no listening network services (checked with
  `ss -tulpn` in testing).
- Tools Sanctum cannot wrap (for example `mkfs`, `fdisk`, `gparted`) remain
  destructive by design. Sanctum's own helpers (Phase 5) will add
  read-only mounting and explicit confirmations.
- `systemd-timesyncd` stays on, because correct time matters for TLS and
  package signatures. When synchronised, the kernel periodically writes the
  time to the machine's hardware clock. That changes machine state, though
  not disk contents. The forensic boot mode (Phase 5) will turn time sync
  off.

## Verification (2026-10-02)

The ISO booted under UEFI with three writable disk images attached:

- a two-disk RAID1 array with ext4,
- an LVM volume group with an ext4 logical volume.

Results:

- No array was assembled; the md module did not even load.
- The logical volume stayed inactive.
- No target filesystem was mounted, and no swap was in use.
- No unit failed.
- No non-loopback socket was listening.

The SHA-256 of every image was identical before and after the boot.

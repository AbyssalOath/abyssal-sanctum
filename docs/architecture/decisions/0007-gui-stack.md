# ADR-0007: GUI stack

- **Status:** Accepted
- **Date:** 2026-10-02

## Context

The brief asks for a graphical option "similar to startx with SystemRescue":
a console-first system where a light desktop starts on request. The command
line stays a first-class interface. Technicians need graphical tools such as
GParted, GSmartControl and Wireshark, a browser, and an easy way to find
tools by task.

## Options considered

| Option | For | Against |
| --- | --- | --- |
| Xfce on Xorg | Root GUI applications (GParted, Wireshark) work without workarounds; light; proven on old hardware; XDG menus are easy to customise | X11 is in maintenance mode |
| A Wayland compositor (labwc, sway) | Modern | Running root GUI applications is awkward; more setup for no gain on a live system |
| GNOME or KDE | Polished | Heavy; enable automounting and other background services by default |

## Decision

**Xfce on Xorg, started with `startx`** (decision D3). The system boots to
the console. `/root/.xinitrc` runs `startxfce4`.

- **Minimal components:** `xfwm4`, `xfce4-panel`, `xfce4-session`,
  `xfce4-settings`, `xfdesktop`, `xfce4-terminal`, `thunar`, `mousepad`,
  and `network-manager-applet`. No `thunar-volman`, `udisks2` or `gvfs`:
  nothing mounts disks automatically. The self-test fails if any of these is
  installed.
- **The menu comes from the tool catalog.** The build generates one
  `.desktop` file per catalog tool and an XDG menu with Sanctum's ten
  categories (`docs/operations/catalog.md`). Other applications' own menu
  entries are left out, so every menu item has a risk label and notes.
  Command-line tools open a terminal showing their catalog entry, and never
  run with arguments the user did not type.
- **Restrained dark theme:** GTK's Adwaita-dark, DejaVu fonts, a dark
  terminal, the Sanctum emblem on the menu button, and the "portal of
  midnight" background on every monitor (set at login, because monitor names
  are only known then). No compositing, which is slow without GPU
  acceleration.
- **Firefox** is included (decision D4): useful for driver downloads and
  error messages, and it shows the offline guides. Policies turn off
  telemetry, studies, Pocket and update checks, and set the guides as the
  home page.
- **No display manager.** Logging out returns to the console.

## Consequences

- The desktop adds about 1.3 GiB of installed packages, mostly Wireshark's
  Qt and Firefox. The ISO stays within the budget in ADR-0005.
- X runs as root, like everything on the live system. That is normal for a
  single-user rescue system.
- Qt applications (Wireshark) use their default light style.
- GPU drivers load after boot (ADR-0006 removed early KMS from the
  initramfs). The "safe graphics" boot entry (`nomodeset`) covers GPUs that
  fail.

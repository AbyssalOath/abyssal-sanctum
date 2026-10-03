# Architecture decision records

Each record explains one decision: the context, the options considered, what
was chosen and what it costs. Records are never deleted. A decision that
changes gets a new record that supersedes the old one.

| ADR | Title | Status |
| --- | --- | --- |
| [0001](0001-archiso-releng-base.md) | archiso with a profile derived from `releng` | Accepted |
| [0002](0002-containerised-pinned-build.md) | Containerised build pinned to an archive snapshot | Accepted |
| [0003](0003-package-policy.md) | Package inclusion policy | Accepted |
| [0004](0004-linux-hardened-kernel.md) | `linux-hardened` kernel | Accepted |
| [0005](0005-iso-hosting-and-size-budget.md) | ISO hosting outside GitHub and a 3 GiB size budget | Accepted |
| [0006](0006-non-destructive-defaults.md) | Non-destructive defaults | Accepted |
| [0007](0007-gui-stack.md) | GUI stack: Xfce on Xorg via startx | Accepted |
| [0008](0008-rust-components.md) | Rust components | Accepted |
| [0009](0009-release-integrity.md) | Release integrity | Accepted |

Planned (see [ROADMAP.md](../../../ROADMAP.md)): Warden and Arsenal
interfaces.

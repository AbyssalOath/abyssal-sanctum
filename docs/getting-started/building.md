# Building the ISO

The ISO is built inside a pinned Arch Linux container, so any Linux host with
Docker or root Podman can build it. The reasoning is in
[ADR-0002](../architecture/decisions/0002-containerised-pinned-build.md).

## Requirements

- About 15 GB of free disk space. The container works under the engine's
  storage directory (`/var/lib/docker` or `/var/lib/containers`); results and
  the package cache go to `out/`.
- Internet access to `archive.archlinux.org`. The first build downloads about
  1 GB of packages. Later builds reuse `out/cache/pacman`.
- One of:
  - **Docker**, with your user in the `docker` group (or run the script with
    `sudo`).
  - **Podman as root** (`sudo`). Rootless Podman cannot run mkarchiso, and
    the script refuses it.
- For testing: QEMU and OVMF.

### Fedora

```bash
# Container engine: Fedora's moby-engine, docker-ce from Docker's own
# repository, or the preinstalled podman (run the build with sudo).
sudo dnf install moby-engine && sudo systemctl enable --now docker
sudo usermod -aG docker "$USER"   # log out and back in afterwards
sudo dnf install qemu-system-x86 edk2-ovmf ShellCheck shfmt
```

### Arch Linux

```bash
sudo pacman -S --needed docker qemu-desktop edk2-ovmf shellcheck shfmt
# or build natively, without a container:
sudo pacman -S --needed archiso
```

### Debian / Ubuntu

```bash
sudo apt install docker.io qemu-system-x86 ovmf shellcheck shfmt
```

## Build

```bash
./scripts/build/build-iso.sh
```

The script:

1. Checks dependencies: the container engine, and inside the container the
   pinned archiso version.
2. Checks the configuration: `VERSION`, `build/build.conf` and the profile.
3. Stages the profile, filling in the snapshot date, and resolves every
   package against the snapshot.
4. Builds the root filesystem, then the ISO, with `mkarchiso`.
5. Writes the SHA-256 checksum.
6. Checks the ISO against the size budget.
7. Prints the paths of the results.

Any failing step stops the build with an error message. Results go to `out/`:

| File | Contents |
| --- | --- |
| `abyssal-sanctum-v<version>-x86_64.iso` | The ISO |
| `abyssal-sanctum-v<version>-x86_64.iso.sha256` | Checksum (`sha256sum -c` format) |
| `abyssal-sanctum-v<version>-x86_64.build.log` | Full build log |
| `cache/pacman/` | Package cache, reused by later builds |

Options:

```text
-o, --out DIR        Output directory (default: out/)
-e, --engine NAME    docker or podman
    --native         Build directly on an Arch host (as root)
```

### Timestamps and reproducibility

File times inside the ISO, and the ISO's UUID, come from `SOURCE_DATE_EPOCH`.
By default this is the timestamp of the last commit. The script warns when
there are uncommitted changes, or when there is no commit yet. In both cases
the ISO cannot be rebuilt identically from the repository.

## Test in a virtual machine

```bash
./scripts/test/run-vm.sh            # UEFI (OVMF)
./scripts/test/run-vm.sh --bios     # legacy BIOS
./scripts/test/run-vm.sh --disk some-disk.img   # attach a disk; writes are discarded
```

The VM gets 4 GiB of RAM, KVM acceleration when `/dev/kvm` is available, and
user-mode networking.

## Verify the checksum

```bash
cd out && sha256sum -c abyssal-sanctum-v*-x86_64.iso.sha256
```

## Troubleshooting

| Problem | Fix |
| --- | --- |
| `no usable container engine` | Start the Docker daemon (`sudo systemctl start docker`) and check that you are in the `docker` group, or run with `sudo`. |
| `rootless podman cannot run mkarchiso` | Run with `sudo`, or use Docker. |
| `some packages ... are not in the snapshot` | A package name in `build/packages/*.list` is wrong, or it does not exist on that snapshot date. |
| `snapshot provides archiso X, build.conf expects Y` | `SNAPSHOT_DATE` was changed without updating `ARCHISO_VERSION`. Update it, and diff the new `releng` profile (ADR-0001). |
| Package downloads fail with HTTP 429 | The Arch Linux Archive is rate-limiting. Wait and re-run. Packages already downloaded stay in the cache. |
| `out/` files owned by root | Only possible if a build was killed before cleanup. Run `sudo chown -R "$USER:" out`. |

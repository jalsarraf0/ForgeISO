# ForgeISO

> Build custom, unattended Linux ISOs on bare metal — no cloud agents, no remote servers, no endpoint configuration.

[![CI](https://github.com/jalsarraf0/ForgeISO/actions/workflows/ci.yml/badge.svg)](https://github.com/jalsarraf0/ForgeISO/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/jalsarraf0/ForgeISO)](https://github.com/jalsarraf0/ForgeISO/releases/latest)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue)](LICENSE)

ForgeISO takes an Ubuntu ISO, injects a fully configured cloud-init autoinstall payload, and produces a new ISO that installs hands-free. It also inspects, verifies, diffs, scans, and smoke-tests ISOs — all from a single binary on your Linux host.

---

## Install

Download the latest release from the **[Releases page](https://github.com/jalsarraf0/ForgeISO/releases/latest)**, then:

### Fedora · RHEL · openSUSE
```bash
sudo rpm -ivh forgeiso-0.3.2-1.x86_64.rpm
```

### Debian · Ubuntu · Linux Mint
```bash
sudo dpkg -i forgeiso_0.3.2-1_amd64.deb
sudo apt-get install -f        # resolve xorriso, squashfs-tools, mtools if missing
```

### Any x86-64 Linux (tarball)
```bash
tar -xzf forgeiso-0.3.2-linux-x86_64.tar.gz
sudo install -m755 forgeiso-0.3.2-linux-x86_64/bin/forgeiso /usr/local/bin/
sudo install -m755 forgeiso-0.3.2-linux-x86_64/bin/forgeiso-tui /usr/local/bin/
```

> **Dependencies:** `xorriso` · `squashfs-tools` · `mtools`
> Optional for smoke testing: `qemu-system-x86_64` · `ovmf`

Verify your download:
```bash
sha256sum -c checksums.txt
```

Check what's available on your host:
```bash
forgeiso doctor
```

---

## Quick start

```bash
# Download an Ubuntu ISO and embed an autoinstall config
forgeiso inject \
  --source ubuntu-24.04-server-amd64.iso \
  --out /tmp/out \
  --hostname bastion \
  --username admin --password secret \
  --group sudo --firewall --allow-port 22 \
  --docker --no-user-interaction
```

Boot the output ISO. It installs Ubuntu completely hands-free with your configuration baked in.

---

## Commands

| Command | What it does |
|---|---|
| [`inject`](#inject) | Embed autoinstall config into an Ubuntu ISO |
| [`verify`](#verify) | Check ISO SHA-256 against official Ubuntu checksums |
| [`inspect`](#inspect) | Read distro/release/arch/hash from an ISO or URL |
| [`build`](#build) | Repack an ISO with a local overlay directory |
| [`diff`](#diff) | Compare two ISOs — added, removed, modified files |
| [`scan`](#scan) | SBOM + CVE + secrets scan on an ISO |
| [`test`](#test) | BIOS/UEFI boot smoke test via QEMU |
| [`report`](#report) | Render a build report as HTML or JSON |
| [`doctor`](#doctor) | Check host prerequisites |

---

## inject

Generates a Ubuntu [cloud-init autoinstall](https://ubuntu.com/server/docs/install/autoinstall) payload and embeds it into the ISO. The installed system is fully configured before the first boot.

```bash
forgeiso inject \
  --source ubuntu-24.04-server-amd64.iso \
  --out /tmp/out \
  [OPTIONS]
```

Pass `--autoinstall user-data.yaml` to merge flags into an existing YAML instead of generating from scratch.

### Identity

| Flag | Description |
|---|---|
| `--hostname NAME` | System hostname |
| `--username NAME` | Primary user login |
| `--password PASS` | Password (auto-hashed to SHA-512-crypt) |
| `--password-file FILE` | Read password from file |
| `--password-stdin` | Read password from stdin |
| `--realname NAME` | User display name |

### SSH

| Flag | Description |
|---|---|
| `--ssh-key KEY` | Authorized public key (repeatable) |
| `--ssh-key-file FILE` | Read public key from file (repeatable) |
| `--ssh-password-auth` | Enable SSH password authentication |

### Networking

| Flag | Description |
|---|---|
| `--static-ip CIDR` | Static IPv4 address, e.g. `10.0.0.5/24` |
| `--gateway IP` | Default route |
| `--dns IP` | DNS server (repeatable) |
| `--ntp-server HOST` | NTP server (repeatable) |
| `--http-proxy URL` | HTTP proxy |
| `--https-proxy URL` | HTTPS proxy |
| `--no-proxy HOST` | Proxy exception (repeatable) |

### User & access

| Flag | Description |
|---|---|
| `--group NAME` | Add user to group, e.g. `sudo`, `docker` (repeatable) |
| `--shell PATH` | Login shell, e.g. `/bin/zsh` |
| `--sudo-nopasswd` | Grant passwordless sudo (`NOPASSWD:ALL`) |
| `--sudo-command CMD` | Restrict sudo to specific command (repeatable) |

### Firewall

| Flag | Description |
|---|---|
| `--firewall` | Enable UFW |
| `--firewall-policy POLICY` | Default incoming policy: `allow` \| `deny` \| `reject` |
| `--allow-port PORT` | Open port, e.g. `22/tcp`, `443` (repeatable) |
| `--deny-port PORT` | Block port (repeatable) |

### Storage

| Flag | Description |
|---|---|
| `--storage-layout NAME` | Partition layout: `lvm` \| `direct` \| `zfs` |
| `--encrypt` | Enable LUKS full-disk encryption |
| `--encrypt-passphrase PASS` | Encryption passphrase |
| `--encrypt-passphrase-file FILE` | Read passphrase from file |
| `--swap-size MB` | Create swap file of this size |
| `--swap-file PATH` | Swap file path (default `/swapfile`) |
| `--swappiness 0-100` | VM swappiness kernel parameter |
| `--mount FSTAB_LINE` | Raw fstab entry (repeatable) |

### System

| Flag | Description |
|---|---|
| `--timezone TZ` | e.g. `America/Chicago` |
| `--locale LOCALE` | e.g. `en_US.UTF-8` |
| `--keyboard-layout CODE` | e.g. `us` |
| `--apt-mirror URL` | Custom APT mirror |
| `--apt-repo REPO` | Add PPA or deb repo (repeatable) |
| `--package NAME` | Extra package to install (repeatable) |

### Services & kernel

| Flag | Description |
|---|---|
| `--enable-service NAME` | Enable systemd service after install (repeatable) |
| `--disable-service NAME` | Disable systemd service after install (repeatable) |
| `--sysctl KEY=VALUE` | Kernel parameter written to `/etc/sysctl.d` (repeatable) |

### Containers

| Flag | Description |
|---|---|
| `--docker` | Install Docker CE |
| `--podman` | Install Podman |
| `--docker-user NAME` | Add user to `docker` group (repeatable) |

### Boot

| Flag | Description |
|---|---|
| `--grub-timeout SEC` | GRUB menu timeout in seconds |
| `--grub-cmdline PARAM` | Append kernel parameter (repeatable) |
| `--grub-default ENTRY` | Default GRUB entry |

### Commands & automation

| Flag | Description |
|---|---|
| `--run-command CMD` | Run command post-install (repeatable) |
| `--late-command CMD` | Cloud-init late-command (repeatable) |
| `--no-user-interaction` | Fully automated install, no prompts |
| `--name NAME` | Output ISO filename (without `.iso`) |
| `--volume-label LABEL` | ISO volume label |
| `--json` | Print result as JSON |

---

## verify

Computes the SHA-256 of a local ISO and checks it against the official Ubuntu checksums file. Auto-detects the checksums URL from the ISO metadata.

```bash
forgeiso verify --source ubuntu-24.04-server-amd64.iso
```

Override the checksums URL:
```bash
forgeiso verify \
  --source ubuntu-24.04-server-amd64.iso \
  --sums-url https://releases.ubuntu.com/24.04/SHA256SUMS
```

---

## inspect

Reads distro, release, architecture, and SHA-256 from a local ISO or a URL (ForgeISO downloads to `~/.cache/forgeiso` first).

```bash
forgeiso inspect --source ubuntu-24.04-server-amd64.iso
forgeiso inspect --source https://releases.ubuntu.com/24.04/ubuntu-24.04-server-amd64.iso
```

---

## build

Repacks an ISO with a local overlay directory merged into the root.

```bash
forgeiso build \
  --source ubuntu-24.04-server-amd64.iso \
  --out ./artifacts \
  --name my-server \
  --overlay ./my-overlay-dir \
  --profile minimal
```

`--profile` is `minimal` (default) or `desktop`.

---

## diff

Compares two ISOs and lists files that were added, removed, or modified, with size deltas.

```bash
forgeiso diff --base original.iso --target custom.iso
```

---

## scan

Runs SBOM generation, CVE scanning, and secrets detection against ISO contents. Uses whichever of `syft`, `trivy`, `grype` are installed.

```bash
forgeiso scan --source custom.iso
```

---

## test

Boots the ISO in QEMU and verifies it reaches the boot menu. Requires `qemu-system-x86_64` and `ovmf`.

```bash
forgeiso test --source custom.iso --bios --uefi
```

---

## report

Renders the build report for an output directory.

```bash
forgeiso report --build ./artifacts --format html
forgeiso report --build ./artifacts --format json
```

---

## doctor

```bash
forgeiso doctor
```

Reports availability of `xorriso`, `squashfs-tools`, `mtools`, `qemu-system-x86_64`, `ovmf`, `trivy`, `syft`, `grype`, and `oscap`.

---

## Logging

```bash
RUST_LOG=debug forgeiso inject --source ubuntu.iso --out /tmp/out --username admin --password secret
```

Valid levels: `error` · `warn` · `info` · `debug` · `trace`

---

## Build from source

Requires Rust 1.75+ and the system tools listed above.

```bash
git clone https://github.com/jalsarraf0/ForgeISO
cd ForgeISO
cargo build --release -p forgeiso-cli
sudo install -m755 target/release/forgeiso /usr/local/bin/
```

Run tests:
```bash
cargo test --workspace
```

GUI (Tauri + React):
```bash
cd gui && npm ci && npm run build
cargo build --release --manifest-path gui/src-tauri/Cargo.toml
```

---

## License

[Apache-2.0](LICENSE)

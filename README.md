# ForgeISO

Build custom Linux ISOs locally. No cloud agents, no endpoint configuration, no remote servers — everything runs on your bare-metal Linux host.

## Install

### RPM — Fedora, RHEL, openSUSE
```bash
sudo rpm -ivh forgeiso-0.3.1-1.x86_64.rpm
```

### DEB — Debian, Ubuntu, Linux Mint
```bash
sudo dpkg -i forgeiso_0.3.1-1_amd64.deb
sudo apt-get install -f   # pull in xorriso, squashfs-tools, mtools if missing
```

### Tarball — any Linux
```bash
tar -xzf forgeiso-0.3.1-linux-x86_64.tar.gz
sudo install -m755 forgeiso-0.3.1-linux-x86_64/bin/forgeiso /usr/local/bin/
sudo install -m755 forgeiso-0.3.1-linux-x86_64/bin/forgeiso-tui /usr/local/bin/
```

Download the latest release from the [Releases page](https://github.com/jalsarraf0/ForgeISO/releases).

### Requirements

| Tool | Purpose |
|---|---|
| `xorriso` | ISO inspection and repacking |
| `squashfs-tools` | rootfs extraction and repacking |
| `mtools` | FAT filesystem access (UEFI boot) |
| `qemu-system-x86_64` + `ovmf` | BIOS/UEFI smoke testing (optional) |

Check what's available on your system:
```bash
forgeiso doctor
```

---

## Commands

### `inject` — Build an unattended install ISO

Embeds a cloud-init autoinstall configuration into an Ubuntu ISO so it installs hands-free.

```bash
forgeiso inject \
  --source ubuntu-24.04-server-amd64.iso \
  --out /tmp/out \
  --hostname bastion \
  --username admin --password secret \
  --group sudo --firewall --allow-port 22 \
  --docker --no-user-interaction
```

The output ISO boots directly into a fully automated Ubuntu install with the configuration baked in.

**Identity**
```
--hostname NAME          System hostname after install
--username NAME          Primary user login name
--password PASS          Password (plaintext, auto-hashed to SHA-512-crypt)
--password-file FILE     Read password from file
--password-stdin         Read password from stdin
--realname NAME          Full display name for the user
```

**SSH**
```
--ssh-key KEY            Add authorized key (repeatable)
--ssh-key-file FILE      Read authorized key from file (repeatable)
--ssh-password-auth      Enable SSH password authentication
```

**Networking**
```
--static-ip CIDR         Static IPv4 address, e.g. 10.0.0.5/24
--gateway IP             Default route
--dns IP                 DNS server (repeatable)
--ntp-server HOST        NTP server (repeatable)
--http-proxy URL         HTTP proxy
--https-proxy URL        HTTPS proxy
--no-proxy HOST          Proxy exception (repeatable)
```

**User & access**
```
--group NAME             Add user to group (repeatable)
--shell PATH             Login shell, e.g. /bin/zsh
--sudo-nopasswd          Grant passwordless sudo (NOPASSWD:ALL)
--sudo-command CMD       Restrict sudo to specific command (repeatable)
```

**Firewall**
```
--firewall               Enable UFW
--firewall-policy POLICY Default incoming policy: allow | deny | reject
--allow-port PORT        Open port (repeatable), e.g. 22/tcp or 443
--deny-port PORT         Block port (repeatable)
```

**Storage**
```
--storage-layout NAME    Partition layout: lvm | direct | zfs
--encrypt                Enable LUKS full-disk encryption
--encrypt-passphrase P   Encryption passphrase
--encrypt-passphrase-file FILE
--swap-size MB           Create swap file of this size
--swap-file PATH         Swap file path (default /swapfile)
--swappiness 0-100       VM swappiness kernel parameter
--mount FSTAB_LINE       Raw fstab entry (repeatable)
```

**System**
```
--timezone TZ            e.g. America/Chicago
--locale LOCALE          e.g. en_US.UTF-8
--keyboard-layout CODE   e.g. us
--apt-mirror URL         Custom APT mirror
--apt-repo REPO          Add PPA or deb repo (repeatable)
--package NAME           Extra package to install (repeatable)
```

**Services & kernel**
```
--enable-service NAME    Enable systemd service after install (repeatable)
--disable-service NAME   Disable systemd service after install (repeatable)
--sysctl KEY=VALUE       Kernel parameter written to sysctl.d (repeatable)
```

**Containers**
```
--docker                 Install Docker CE
--podman                 Install Podman
--docker-user NAME       Add user to docker group (repeatable)
```

**Boot**
```
--grub-timeout SEC       GRUB menu timeout in seconds
--grub-cmdline PARAM     Append kernel parameter (repeatable)
--grub-default ENTRY     Default GRUB entry
```

**Commands & automation**
```
--run-command CMD        Run command post-install (repeatable)
--late-command CMD       Cloud-init late-command (repeatable)
--no-user-interaction    Fully automated install, no prompts
--autoinstall FILE       Merge flags into an existing user-data YAML
--name NAME              Output ISO filename (without .iso)
--volume-label LABEL     ISO volume label
--json                   Print result as JSON
```

---

### `verify` — Check ISO authenticity

Computes the SHA-256 of a local ISO and compares it against the official Ubuntu checksums file. Auto-detects the checksums URL from ISO metadata.

```bash
forgeiso verify --source ubuntu-24.04-server-amd64.iso
forgeiso verify --source ubuntu-24.04-server-amd64.iso --sums-url https://releases.ubuntu.com/24.04/SHA256SUMS
```

---

### `inspect` — Read ISO metadata

```bash
forgeiso inspect --source ubuntu-24.04-server-amd64.iso
forgeiso inspect --source https://releases.ubuntu.com/24.04/ubuntu-24.04-server-amd64.iso
```

Prints distro, release, architecture, and SHA-256. Accepts a local path or URL (ForgeISO downloads to cache).

---

### `build` — Repack an ISO with an overlay

```bash
forgeiso build \
  --source ubuntu-24.04-server-amd64.iso \
  --out ./artifacts \
  --name my-server \
  --overlay ./my-overlay \
  --profile minimal
```

`--overlay DIR` copies files into the ISO root before repacking. `--profile` is `minimal` or `desktop`.

---

### `diff` — Compare two ISOs

```bash
forgeiso diff --base original.iso --target custom.iso
```

Lists files that were added, removed, or modified between the two ISOs with size deltas.

---

### `scan` — Security scan

```bash
forgeiso scan --source custom.iso
```

Runs SBOM generation (syft), vulnerability scanning (trivy/grype), and secrets detection against the ISO contents. `forgeiso doctor` shows which scanners are installed.

---

### `test` — BIOS/UEFI smoke test

```bash
forgeiso test --source custom.iso --bios --uefi
```

Boots the ISO in QEMU and checks that it reaches the boot menu. Requires `qemu-system-x86_64` and `ovmf`.

---

### `report` — Build report

```bash
forgeiso report --build ./artifacts --format html
forgeiso report --build ./artifacts --format json
```

---

### `doctor` — Check prerequisites

```bash
forgeiso doctor
```

---

## Logging

Set `RUST_LOG` to control verbosity:

```bash
RUST_LOG=debug forgeiso inject --source ubuntu.iso --out /tmp/out --username admin --password secret
```

Levels: `error`, `warn`, `info`, `debug`, `trace`

---

## Build from source

Requires Rust 1.75+ and the system tools listed above.

```bash
git clone https://github.com/jalsarraf0/ForgeISO
cd ForgeISO
cargo build --release -p forgeiso-cli
sudo install -m755 target/release/forgeiso /usr/local/bin/
```

**Run tests:**
```bash
cargo test --workspace
```

**GUI (Tauri + React):**
```bash
cd gui && npm ci && npm run build
cargo build --release --manifest-path gui/src-tauri/Cargo.toml
```

---

## License

Apache-2.0 — see [LICENSE](LICENSE)

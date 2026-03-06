# Troubleshooting

Author: Jamal Al-Sarraf

## Tauri build fails on Linux

Install GTK/WebKit dependencies:
- Debian/Ubuntu: `libgtk-3-dev libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev`
- Fedora: `gtk3-devel webkit2gtk4.1-devel libappindicator-gtk3-devel librsvg2-devel`

## Docker unavailable

If Docker is unavailable, set runtime to Podman in config:

```yaml
runtime: podman
```

## VM tests unavailable in CI

If nested virtualization is unavailable, run mocked smoke mode in C6 and offload real BIOS/UEFI tests to a self-hosted runner with KVM support.

## Secrets scan failures

Review `secrets.json`, scrub credentials from modules/files, and rerun with strict mode enabled.

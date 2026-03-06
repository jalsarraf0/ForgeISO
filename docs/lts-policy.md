# Release Policy

Author: Jamal Al-Sarraf

## Enforced distro policy

- Ubuntu: LTS-only versions accepted (`xx.04` LTS cadence)
- Linux Mint: LTS lineage only (Mint releases aligned to Ubuntu LTS base)
- Fedora: latest supported stable policy; non-LTS lifecycle warning always included
- Arch: rolling snapshot format `YYYY.MM.DD`

## Build modes

- `latest`: update package repositories during image build
- `pinned`: use fixed base ISO URL/checksum and record best-effort snapshot metadata

## Report requirements

Each build report includes:
- Base ISO URL and checksum
- Tool versions and runtime selection
- Distro release and policy warnings
- Security scan summary
- Test summary

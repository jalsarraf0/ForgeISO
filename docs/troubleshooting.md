# Troubleshooting

## Build fails on a non-Linux host

ForgeISO only supports local build and VM test flows on Linux.

## ISO inspection is missing distro details

Install `xorriso` so ForgeISO can read files from inside the ISO instead of relying only on the primary volume label.

## Repack fails after rootfs extraction

Install both `unsquashfs` and `mksquashfs`. ForgeISO uses them for Ubuntu, Mint, and Arch root filesystem updates.

## Fedora overlay does not reach the live rootfs

Some Fedora live images use nested filesystem layouts that are not yet rewritten by the local remaster step. ForgeISO will still update top-level ISO content and report the limitation honestly.

## UEFI smoke test fails immediately

Install QEMU and an OVMF firmware package so ForgeISO can boot the ISO locally in UEFI mode.

# ForgeISO RPM spec file — for native rpmbuild (reference)
# The release pipeline uses fpm; this spec is provided for maintainers
# who want to build a source-package-traceable RPM via rpmbuild.
#
# Usage:
#   rpmbuild -ba packaging/forgeiso.spec \
#            --define "_version 0.3.1" \
#            --define "_bindir /path/to/target/release"

%define _version %{?version}%{!?version:0.3.1}

Name:           forgeiso
Version:        %{_version}
Release:        1%{?dist}
Summary:        Linux ISO builder and autoinstall injection tool
License:        Apache-2.0
URL:            https://github.com/jalsarraf0/ForgeISO
ExclusiveArch:  x86_64

Requires:       xorriso
Requires:       squashfs-tools
Requires:       mtools

%description
ForgeISO builds custom Linux ISOs locally on bare metal with no cloud agents
or endpoints. It supports cloud-init autoinstall injection with 60+ configuration
flags covering identity, SSH, networking, firewall, storage, encryption, services,
containers (Docker/Podman), GRUB, and more. Additional capabilities include
SHA-256 verification, ISO filesystem diffing, SBOM/vulnerability scanning, and
QEMU-based BIOS/UEFI smoke testing.

%prep
# Binaries are pre-built; no source compilation in spec build mode.
# Set _bindir to target/release when invoking rpmbuild.

%install
install -Dm755 "%{_bindir}/forgeiso"     "%{buildroot}%{_bindir}/forgeiso"
install -Dm755 "%{_bindir}/forgeiso-tui" "%{buildroot}%{_bindir}/forgeiso-tui"
install -Dm644 "%{_topdir}/README.md"    "%{buildroot}%{_docdir}/%{name}/README.md"

%files
%license LICENSE
%doc README.md
%{_bindir}/forgeiso
%{_bindir}/forgeiso-tui

%changelog
* Thu Mar 06 2025 Jamal Al-Sarraf <jalsarraf0@github.com> - 0.3.1-1
- Wave 2: Inject all 60+ CLI flags, Verify, Diff commands
- GUI overhaul with 4-tab layout (Build/Inject/Verify/Diff)
- Proper RPM/DEB/pacman packaging pipeline

* Fri Jan 01 2025 Jamal Al-Sarraf <jalsarraf0@github.com> - 0.3.0-1
- Initial packaged release

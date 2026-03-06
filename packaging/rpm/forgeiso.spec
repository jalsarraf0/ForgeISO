Name:           forgeiso
Version:        %{?version}%{!?version:0.1.0}
Release:        %{?release}%{!?release:1}%{?dist}
Summary:        Cross-distro ISO customization platform

License:        Apache-2.0
URL:            https://github.com/jalsarraf0/ForgeISO
Source0:        %{name}-%{version}.tar.gz
BuildArch:      x86_64

Requires:       bash
Requires:       docker

%description
ForgeISO provides enterprise ISO customization with CLI, TUI, GUI, and optional remote agent support.
This RPM installs release binaries for the CLI, TUI, and agent.

%prep
%setup -q

%build
# binaries are prebuilt and packaged in Source0

%install
mkdir -p %{buildroot}%{_bindir}
install -m 0755 bin/forgeiso %{buildroot}%{_bindir}/forgeiso
install -m 0755 bin/forgeiso-tui %{buildroot}%{_bindir}/forgeiso-tui
install -m 0755 bin/forgeiso-agent %{buildroot}%{_bindir}/forgeiso-agent

mkdir -p %{buildroot}%{_datadir}/doc/%{name}
install -m 0644 README.md %{buildroot}%{_datadir}/doc/%{name}/README.md
install -m 0644 LICENSE %{buildroot}%{_datadir}/doc/%{name}/LICENSE

%files
%{_bindir}/forgeiso
%{_bindir}/forgeiso-tui
%{_bindir}/forgeiso-agent
%{_datadir}/doc/%{name}/README.md
%license %{_datadir}/doc/%{name}/LICENSE

%changelog
* Fri Mar 06 2026 Jamal Al-Sarraf <19882582+jalsarraf0@users.noreply.github.com> - 0.1.0-1
- Initial ForgeISO release package

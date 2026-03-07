// ── Distro capability model ───────────────────────────────────────────────────
// This is the authoritative definition of what ForgeISO supports per distro.
// Add support for a new distro by adding a DistroFamily entry and capability set.
// The UI reads this at runtime — no UI code changes needed for new distros.

export type SupportLevel = 'supported' | 'beta' | 'coming_soon' | 'not_applicable';

export type DistroCapabilities = {
  build: SupportLevel;
  inject: SupportLevel;
  verify: SupportLevel;
  diff: SupportLevel;
  scan: SupportLevel;
  test: SupportLevel;
};

export type DistroRelease = {
  version: string;
  label: string;
  lts?: boolean;
  recommended?: boolean;
};

export type DistroFamily = {
  id: string;
  label: string;
  description: string;
  iconChar: string;           // emoji/char to use when no SVG icon is available
  accentColor: string;        // CSS color string for branding accents
  capabilities: DistroCapabilities;
  releases: DistroRelease[];
  injectMethod: string;       // human-readable injection method description
  injectNotes?: string;       // any caveats or limitations
};

export const DISTRO_FAMILIES: DistroFamily[] = [
  {
    id: 'ubuntu',
    label: 'Ubuntu',
    description: 'Canonical Ubuntu — full cloud-init autoinstall support.',
    iconChar: '🟠',
    accentColor: '#e95420',
    capabilities: {
      build: 'supported',
      inject: 'supported',
      verify: 'supported',
      diff: 'supported',
      scan: 'supported',
      test: 'supported',
    },
    releases: [
      { version: '24.04', label: '24.04 LTS (Noble)', lts: true, recommended: true },
      { version: '22.04', label: '22.04 LTS (Jammy)', lts: true },
      { version: '23.10', label: '23.10 (Mantic)' },
    ],
    injectMethod: 'Cloud-init autoinstall (nocloud ds)',
    injectNotes: 'Generates a full Ubuntu autoinstall user-data YAML and embeds it into the ISO boot path. 60+ configuration flags are supported.',
  },
  {
    id: 'mint',
    label: 'Linux Mint',
    description: 'Linux Mint — cloud-init autoinstall injection (Ubuntu-based).',
    iconChar: '🟢',
    accentColor: '#86b840',
    capabilities: {
      build: 'supported',
      inject: 'supported',
      verify: 'supported',
      diff: 'supported',
      scan: 'supported',
      test: 'supported',
    },
    releases: [
      { version: '21.3', label: '21.3 (Virginia)', recommended: true },
      { version: '21.2', label: '21.2 (Victoria)' },
    ],
    injectMethod: 'Cloud-init autoinstall (Ubuntu-compatible)',
    injectNotes: 'Linux Mint 21+ is Ubuntu 22.04-based. ForgeISO injects a cloud-init autoinstall user-data YAML compatible with the underlying Ubuntu casper layer.',
  },
  {
    id: 'fedora',
    label: 'Fedora / RHEL',
    description: 'Fedora and RHEL family — Kickstart (ks.cfg) injection.',
    iconChar: '🔵',
    accentColor: '#3c6eb4',
    capabilities: {
      build: 'supported',
      inject: 'supported',
      verify: 'supported',
      diff: 'supported',
      scan: 'supported',
      test: 'supported',
    },
    releases: [
      { version: '41', label: 'Fedora 41', recommended: true },
      { version: '40', label: 'Fedora 40' },
      { version: '9', label: 'RHEL / AlmaLinux / Rocky 9' },
    ],
    injectMethod: 'Kickstart (ks.cfg)',
    injectNotes: 'ForgeISO generates a ks.cfg from your config flags and embeds it at the ISO root. The boot entries are patched to add inst.ks=cdrom:/ks.cfg.',
  },
  {
    id: 'arch',
    label: 'Arch Linux',
    description: 'Arch Linux — archinstall JSON config injection.',
    iconChar: '🔷',
    accentColor: '#1793d1',
    capabilities: {
      build: 'supported',
      inject: 'supported',
      verify: 'supported',
      diff: 'supported',
      scan: 'supported',
      test: 'supported',
    },
    releases: [
      { version: 'rolling', label: 'Rolling Release', recommended: true },
    ],
    injectMethod: 'archinstall JSON config',
    injectNotes: 'ForgeISO injects an archinstall-config.json and a run-archinstall.sh launcher into arch/boot/. The boot entry is patched to execute the script automatically.',
  },
];

export function getDistro(id: string): DistroFamily {
  return DISTRO_FAMILIES.find((d) => d.id === id) ?? DISTRO_FAMILIES[0];
}

export function capabilityLabel(level: SupportLevel): string {
  switch (level) {
    case 'supported':     return 'Supported';
    case 'beta':          return 'Beta';
    case 'coming_soon':   return 'Coming Soon';
    case 'not_applicable': return 'N/A';
  }
}

export function capabilityClass(level: SupportLevel): string {
  switch (level) {
    case 'supported':     return 'cap-supported';
    case 'beta':          return 'cap-beta';
    case 'coming_soon':   return 'cap-soon';
    case 'not_applicable': return 'cap-na';
  }
}

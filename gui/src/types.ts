// ── Domain types mirroring Rust engine structs ────────────────────────────────

export type LogEntry = {
  ts: string;
  phase: string;
  level: string;
  message: string;
  substage?: string | null;
  percent?: number | null;
  bytesDone?: number | null;
  bytesTotal?: number | null;
};

export type Inspection = {
  source_path: string;
  distro?: string | null;
  release?: string | null;
  architecture?: string | null;
  volume_id?: string | null;
  sha256: string;
  warnings: string[];
};

export type BuildResult = {
  output_dir: string;
  report_json: string;
  report_html: string;
  artifacts: string[];
  iso?: Inspection;
};

export type InjectResult = {
  output_dir: string;
  report_json: string;
  report_html: string;
  artifacts: string[];
};

export type VerifyResult = {
  filename: string;
  expected: string;
  actual: string;
  matched: boolean;
};

export type DiffEntry = {
  path: string;
  base_size?: number;
  target_size?: number;
};

export type IsoDiff = {
  added: string[];
  removed: string[];
  modified: DiffEntry[];
  unchanged: number;
};

export type DoctorReport = {
  host_os: string;
  host_arch: string;
  linux_supported: boolean;
  tooling: Record<string, boolean>;
  warnings: string[];
  timestamp: string;
};

// ── Application workflow stages ───────────────────────────────────────────────

export type AppStage = 'build' | 'inject' | 'verify' | 'diff' | 'completion';
export type StageStatus = 'pending' | 'active' | 'running' | 'success' | 'error' | 'skipped';

// ── Job progress model ────────────────────────────────────────────────────────

export type JobStatus = 'idle' | 'running' | 'success' | 'error' | 'cancelled';

export type JobProgress = {
  jobId: string;
  stage: AppStage;
  status: JobStatus;
  currentOperation: string;
  substage: string | null;
  percent: number | null;           // 0–100 or null for indeterminate
  bytesDone: number | null;
  bytesTotal: number | null;
  startedAt: Date;
  updatedAt: Date;
  endedAt: Date | null;
  warnings: string[];
};

// ── App-wide state ────────────────────────────────────────────────────────────

export type AppState = {
  activeStage: AppStage;
  stageStatus: Record<AppStage, StageStatus>;
  isRunning: boolean;
  progress: JobProgress | null;
  doctor: DoctorReport | null;
  logs: LogEntry[];

  // Cross-stage artifacts
  buildResult: BuildResult | null;
  injectResult: InjectResult | null;
  verifyResult: VerifyResult | null;
  diffResult: IsoDiff | null;

  // Remembered source paths (so stages pre-fill intelligently)
  lastSourceIso: string;
  lastOutputDir: string;
  lastInjectedIso: string;
};

// ── Inject form state ─────────────────────────────────────────────────────────

export type InjectState = {
  source: string;
  outputDir: string;
  outName: string;
  outputLabel: string;
  autoinstallYaml: string;
  hostname: string;
  username: string;
  password: string;
  realname: string;
  sshKeys: string;
  sshPasswordAuth: boolean;
  sshInstallServer: boolean;
  dnsServers: string;
  ntpServers: string;
  staticIp: string;
  gateway: string;
  httpProxy: string;
  httpsProxy: string;
  noProxy: string;
  timezone: string;
  locale: string;
  keyboardLayout: string;
  storageLayout: string;
  aptMirror: string;
  groups: string;
  shell: string;
  sudoNopasswd: boolean;
  sudoCommands: string;
  firewallEnabled: boolean;
  firewallPolicy: string;
  allowPorts: string;
  denyPorts: string;
  enableServices: string;
  disableServices: string;
  sysctl: string;
  swapSizeMb: string;
  swapFile: string;
  swappiness: string;
  docker: boolean;
  podman: boolean;
  dockerUsers: string;
  grubTimeout: string;
  grubCmdline: string;
  grubDefault: string;
  encrypt: boolean;
  encryptPassphrase: string;
  mounts: string;
  packages: string;
  aptRepos: string;
  runCommands: string;
  extraLateCommands: string;
  noUserInteraction: boolean;
  distro: string;
};

export const defaultInjectState: InjectState = {
  source: '', outputDir: './artifacts', outName: 'forgeiso-autoinstall',
  outputLabel: '', autoinstallYaml: '',
  hostname: '', username: 'ubuntu', password: '', realname: '',
  sshKeys: '', sshPasswordAuth: false, sshInstallServer: true,
  dnsServers: '1.1.1.1\n8.8.8.8', ntpServers: 'pool.ntp.org',
  staticIp: '', gateway: '', httpProxy: '', httpsProxy: '', noProxy: '',
  timezone: 'America/Chicago', locale: 'en_US.UTF-8', keyboardLayout: 'us',
  storageLayout: 'lvm', aptMirror: '',
  groups: 'sudo', shell: '/bin/bash', sudoNopasswd: false, sudoCommands: '',
  firewallEnabled: false, firewallPolicy: 'deny', allowPorts: '22', denyPorts: '',
  enableServices: '', disableServices: '',
  sysctl: '', swapSizeMb: '', swapFile: '/swapfile', swappiness: '',
  docker: false, podman: false, dockerUsers: '',
  grubTimeout: '5', grubCmdline: '', grubDefault: '0',
  encrypt: false, encryptPassphrase: '',
  mounts: '', packages: 'curl\ngit\nvim', aptRepos: '',
  runCommands: '', extraLateCommands: '', noUserInteraction: true,
  distro: 'ubuntu',
};

// ── Inject preset templates ───────────────────────────────────────────────────

export type InjectPreset = {
  id: string;
  label: string;
  description: string;
  overrides: Partial<InjectState>;
};

export const INJECT_PRESETS: InjectPreset[] = [
  {
    id: 'minimal',
    label: 'Minimal Server',
    description: 'Bare-bones Ubuntu server with SSH only.',
    overrides: {
      packages: 'curl\ngit\nvim', docker: false, podman: false,
      firewallEnabled: true, allowPorts: '22', storageLayout: 'lvm',
      noUserInteraction: true, sshInstallServer: true,
    },
  },
  {
    id: 'docker-host',
    label: 'Docker Host',
    description: 'Docker CE + Compose, UFW open on 22/2375.',
    overrides: {
      packages: 'curl\ngit\nvim\nca-certificates', docker: true, podman: false,
      firewallEnabled: true, allowPorts: '22\n2375', groups: 'sudo\ndocker',
      storageLayout: 'lvm', noUserInteraction: true,
    },
  },
  {
    id: 'vm-guest',
    label: 'VM Guest',
    description: 'Lightweight VM guest with open-vm-tools.',
    overrides: {
      packages: 'open-vm-tools\ncurl\ngit', docker: false, podman: false,
      grubCmdline: 'quiet splash', grubTimeout: '3',
      storageLayout: 'direct', noUserInteraction: true,
    },
  },
  {
    id: 'hardened',
    label: 'Hardened Server',
    description: 'Security-focused: UFW, auditd, fail2ban, no password auth.',
    overrides: {
      packages: 'ufw\nauditd\nfail2ban\ncurl\ngit',
      firewallEnabled: true, firewallPolicy: 'deny', allowPorts: '22',
      sshPasswordAuth: false, sudoNopasswd: false,
      sysctl: 'net.ipv4.ip_forward=0\nkernel.dmesg_restrict=1\nnet.ipv4.conf.all.rp_filter=1',
      enableServices: 'auditd\nfail2ban',
      noUserInteraction: true,
    },
  },
];

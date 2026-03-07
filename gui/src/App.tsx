import { useEffect, useMemo, useReducer, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

// ── Types ─────────────────────────────────────────────────────────────────────

type LogEntry = { ts: string; phase: string; level: string; message: string };

type Inspection = {
  source_path: string;
  distro?: string | null;
  release?: string | null;
  architecture?: string | null;
  volume_id?: string | null;
  sha256: string;
  warnings: string[];
};

type BuildResult = {
  output_dir: string;
  report_json: string;
  report_html: string;
  artifacts: string[];
};

type VerifyResult = {
  filename: string;
  expected: string;
  actual: string;
  matched: boolean;
};

type DiffEntry = { path: string; size_bytes?: number };

type IsoDiff = {
  added: DiffEntry[];
  removed: DiffEntry[];
  modified: DiffEntry[];
  unchanged: DiffEntry[];
};

type InjectResult = {
  output_dir: string;
  report_json: string;
  report_html: string;
  artifacts: string[];
};

// ── Inject form state ─────────────────────────────────────────────────────────

type InjectState = {
  // Basic
  source: string;
  outputDir: string;
  outName: string;
  outputLabel: string;
  autoinstallYaml: string;
  // Identity
  hostname: string;
  username: string;
  password: string;
  realname: string;
  // SSH
  sshKeys: string;
  sshPasswordAuth: boolean;
  sshInstallServer: boolean;
  // Network
  dnsServers: string;
  ntpServers: string;
  staticIp: string;
  gateway: string;
  httpProxy: string;
  httpsProxy: string;
  noProxy: string;
  // System
  timezone: string;
  locale: string;
  keyboardLayout: string;
  // Storage
  storageLayout: string;
  aptMirror: string;
  // User
  groups: string;
  shell: string;
  sudoNopasswd: boolean;
  sudoCommands: string;
  // Firewall
  firewallEnabled: boolean;
  firewallPolicy: string;
  allowPorts: string;
  denyPorts: string;
  // Services
  enableServices: string;
  disableServices: string;
  // Kernel
  sysctl: string;
  // Swap
  swapSizeMb: string;
  swapFile: string;
  swappiness: string;
  // Containers
  docker: boolean;
  podman: boolean;
  dockerUsers: string;
  // GRUB
  grubTimeout: string;
  grubCmdline: string;
  grubDefault: string;
  // Encryption
  encrypt: boolean;
  encryptPassphrase: string;
  // Mounts
  mounts: string;
  // Packages
  packages: string;
  aptRepos: string;
  // Commands
  runCommands: string;
  extraLateCommands: string;
  // Misc
  noUserInteraction: boolean;
};

const defaultInject: InjectState = {
  source: '', outputDir: './artifacts', outName: 'forgeiso-autoinstall',
  outputLabel: '', autoinstallYaml: '',
  hostname: '', username: '', password: '', realname: '',
  sshKeys: '', sshPasswordAuth: false, sshInstallServer: true,
  dnsServers: '', ntpServers: '', staticIp: '', gateway: '',
  httpProxy: '', httpsProxy: '', noProxy: '',
  timezone: '', locale: '', keyboardLayout: '',
  storageLayout: '', aptMirror: '',
  groups: '', shell: '', sudoNopasswd: false, sudoCommands: '',
  firewallEnabled: false, firewallPolicy: 'deny', allowPorts: '', denyPorts: '',
  enableServices: '', disableServices: '', sysctl: '',
  swapSizeMb: '', swapFile: '', swappiness: '',
  docker: false, podman: false, dockerUsers: '',
  grubTimeout: '', grubCmdline: '', grubDefault: '',
  encrypt: false, encryptPassphrase: '',
  mounts: '',
  packages: '', aptRepos: '',
  runCommands: '', extraLateCommands: '',
  noUserInteraction: true,
};

type InjectAction = { key: keyof InjectState; value: InjectState[keyof InjectState] };

function injectReducer(state: InjectState, action: InjectAction): InjectState {
  return { ...state, [action.key]: action.value };
}

// ── Utilities ─────────────────────────────────────────────────────────────────

const lines = (s: string): string[] =>
  s.split('\n').map((l) => l.trim()).filter(Boolean);

const optStr = (s: string): string | null => (s.trim() ? s.trim() : null);

const optNum = (s: string): number | null => {
  const n = parseInt(s, 10);
  return Number.isFinite(n) ? n : null;
};

// ── Accordion section ─────────────────────────────────────────────────────────

function Section({
  id, title, open, onToggle, children,
}: {
  id: string; title: string; open: boolean; onToggle: (id: string) => void; children: React.ReactNode;
}) {
  return (
    <div className="section">
      <div className="section-header" onClick={() => onToggle(id)}>
        <h3>{title}</h3>
        <span className={`chevron ${open ? 'open' : ''}`}>▶</span>
      </div>
      {open && <div className="section-body">{children}</div>}
    </div>
  );
}

// ── Field helpers ─────────────────────────────────────────────────────────────

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return <label className="field">{label}<div>{children}</div></label>;
}

function TextInput({ value, onChange, placeholder }: {
  value: string; onChange: (v: string) => void; placeholder?: string;
}) {
  return <input value={value} onChange={(e) => onChange(e.target.value)} placeholder={placeholder} />;
}

function TextArea({ value, onChange, placeholder, rows = 3 }: {
  value: string; onChange: (v: string) => void; placeholder?: string; rows?: number;
}) {
  return (
    <textarea
      value={value}
      onChange={(e) => onChange(e.target.value)}
      placeholder={placeholder}
      rows={rows}
    />
  );
}

function Checkbox({ label, checked, onChange }: {
  label: string; checked: boolean; onChange: (v: boolean) => void;
}) {
  return (
    <label className="checkbox-row">
      <input type="checkbox" checked={checked} onChange={(e) => onChange(e.target.checked)} />
      {label}
    </label>
  );
}

// ── Tabs ──────────────────────────────────────────────────────────────────────

type Tab = 'build' | 'inject' | 'verify' | 'diff';

// ── Build Tab ─────────────────────────────────────────────────────────────────

function BuildTab({
  busy, status, logs, lastIso, lastBuildDir,
  setBusy, setStatus, setLogs, setLastIso, setLastBuildDir,
}: {
  busy: boolean; status: string; logs: LogEntry[]; lastIso: string; lastBuildDir: string;
  setBusy: (v: boolean) => void; setStatus: (v: string) => void;
  setLogs: (fn: (prev: LogEntry[]) => LogEntry[]) => void;
  setLastIso: (v: string) => void; setLastBuildDir: (v: string) => void;
}) {
  const [source, setSource] = useState('');
  const [outputDir, setOutputDir] = useState('./artifacts');
  const [buildName, setBuildName] = useState('forgeiso-local');
  const [overlayDir, setOverlayDir] = useState('');
  const [outputLabel, setOutputLabel] = useState('');
  const [profile, setProfile] = useState('minimal');
  const [inspection, setInspection] = useState<Inspection | null>(null);

  const canBuild = useMemo(() => source.trim().length > 0 && outputDir.trim().length > 0, [source, outputDir]);

  const inspect = async () => {
    if (!source.trim()) { setStatus('Source is required'); return; }
    setBusy(true); setStatus('Inspecting ISO...');
    try {
      setInspection(await invoke<Inspection>('inspect_source', { source }));
      setStatus('Inspection complete');
    } catch (e) { setStatus(`Inspect failed: ${e}`); }
    finally { setBusy(false); }
  };

  const build = async () => {
    setBusy(true); setStatus('Building ISO...');
    try {
      const r = await invoke<BuildResult>('build_local', {
        request: { source, outputDir, name: buildName, overlayDir: overlayDir || null, outputLabel: outputLabel || null, profile },
      });
      setLastIso(r.artifacts[0] ?? '');
      setLastBuildDir(r.output_dir);
      setStatus(`Build complete: ${r.artifacts[0] ?? r.output_dir}`);
    } catch (e) { setStatus(`Build failed: ${e}`); }
    finally { setBusy(false); }
  };

  const scan = async () => {
    if (!lastIso) { setStatus('Build an ISO first'); return; }
    setBusy(true); setStatus('Scanning...');
    try {
      const r = await invoke<any>('scan_artifact', { artifact: lastIso });
      setStatus(`Scan complete: ${r.report_json}`);
    } catch (e) { setStatus(`Scan failed: ${e}`); }
    finally { setBusy(false); }
  };

  const testIso = async () => {
    if (!lastIso) { setStatus('Build an ISO first'); return; }
    setBusy(true); setStatus('Testing ISO (BIOS + UEFI)...');
    try {
      const r = await invoke<any>('test_iso', { iso: lastIso, bios: true, uefi: true });
      setStatus(`Test complete: passed=${r.passed}`);
    } catch (e) { setStatus(`Test failed: ${e}`); }
    finally { setBusy(false); }
  };

  const report = async (format: 'html' | 'json') => {
    if (!lastBuildDir) { setStatus('Build an ISO first'); return; }
    setBusy(true); setStatus(`Rendering ${format} report...`);
    try {
      const path = await invoke<string>('render_report', { buildDir: lastBuildDir, format });
      setStatus(`Report written: ${path}`);
    } catch (e) { setStatus(`Report failed: ${e}`); }
    finally { setBusy(false); }
  };

  return (
    <div className="tab-content">
      <div className="two-col-layout">
        <div className="panel">
          <h2>Local Build</h2>
          <div className="field-grid">
            <Field label="Source ISO / URL">
              <TextInput value={source} onChange={setSource} placeholder="/path/to/base.iso or https://…/distro.iso" />
            </Field>
            <Field label="Output directory">
              <TextInput value={outputDir} onChange={setOutputDir} placeholder="./artifacts" />
            </Field>
            <Field label="Build name">
              <TextInput value={buildName} onChange={setBuildName} />
            </Field>
            <Field label="Overlay directory">
              <TextInput value={overlayDir} onChange={setOverlayDir} placeholder="Optional file overlay" />
            </Field>
            <Field label="Volume label">
              <TextInput value={outputLabel} onChange={setOutputLabel} placeholder="Optional ISO label (≤32 chars)" />
            </Field>
            <Field label="Profile">
              <select value={profile} onChange={(e) => setProfile(e.target.value)}>
                <option value="minimal">Minimal</option>
                <option value="desktop">Desktop</option>
              </select>
            </Field>
          </div>
          <div className="actions">
            <button className="ghost" onClick={inspect} disabled={busy || !source.trim()}>Inspect</button>
            <button className="primary" onClick={build} disabled={busy || !canBuild}>Build ISO</button>
            <button className="ghost" onClick={scan} disabled={busy || !lastIso}>Scan</button>
            <button className="ghost" onClick={testIso} disabled={busy || !lastIso}>Test</button>
            <button className="ghost" onClick={() => report('html')} disabled={busy || !lastBuildDir}>HTML Report</button>
            <button className="ghost" onClick={() => report('json')} disabled={busy || !lastBuildDir}>JSON Report</button>
          </div>
          <p className="status-line">{status}</p>
        </div>

        <div className="panel">
          <h2>Detected ISO</h2>
          {inspection ? (
            <dl className="inspection-grid">
              <div><dt>Distro</dt><dd>{inspection.distro ?? 'unknown'}</dd></div>
              <div><dt>Release</dt><dd>{inspection.release ?? 'unknown'}</dd></div>
              <div><dt>Architecture</dt><dd>{inspection.architecture ?? 'unknown'}</dd></div>
              <div><dt>Volume ID</dt><dd>{inspection.volume_id ?? 'unknown'}</dd></div>
              <div className="span-2"><dt>SHA-256</dt><dd className="mono">{inspection.sha256}</dd></div>
            </dl>
          ) : (
            <p className="muted">Inspect an ISO to see metadata.</p>
          )}
          {inspection?.warnings?.length ? (
            <div className="warnings">
              {inspection.warnings.map((w) => <p key={w}>{w}</p>)}
            </div>
          ) : null}
          {lastIso && <p className="status-line">Last ISO: {lastIso}</p>}
        </div>
      </div>
    </div>
  );
}

// ── Inject Tab ────────────────────────────────────────────────────────────────

function InjectTab({ busy, setBusy, setStatus, status }: {
  busy: boolean; status: string;
  setBusy: (v: boolean) => void; setStatus: (v: string) => void;
}) {
  const [inj, dispatch] = useReducer(injectReducer, defaultInject);
  const set = <K extends keyof InjectState>(key: K) =>
    (value: InjectState[K]) => dispatch({ key, value });

  const [openSections, setOpenSections] = useState<Set<string>>(
    new Set(['basic', 'identity', 'ssh', 'network'])
  );
  const toggle = (id: string) =>
    setOpenSections((prev) => {
      const next = new Set(prev);
      next.has(id) ? next.delete(id) : next.add(id);
      return next;
    });
  const is = (id: string) => openSections.has(id);

  const [result, setResult] = useState<InjectResult | null>(null);

  const run = async () => {
    if (!inj.source.trim()) { setStatus('Source ISO is required'); return; }
    if (!inj.outName.trim()) { setStatus('Output name is required'); return; }
    setBusy(true); setStatus('Injecting autoinstall configuration...');
    try {
      const r = await invoke<InjectResult>('inject_iso', {
        request: {
          source: inj.source,
          outputDir: inj.outputDir,
          outName: inj.outName,
          outputLabel: optStr(inj.outputLabel),
          autoinstallYaml: optStr(inj.autoinstallYaml),
          hostname: optStr(inj.hostname),
          username: optStr(inj.username),
          password: optStr(inj.password),
          realname: optStr(inj.realname),
          sshKeys: lines(inj.sshKeys),
          sshPasswordAuth: inj.sshPasswordAuth,
          sshInstallServer: inj.sshInstallServer,
          dnsServers: lines(inj.dnsServers),
          ntpServers: lines(inj.ntpServers),
          staticIp: optStr(inj.staticIp),
          gateway: optStr(inj.gateway),
          httpProxy: optStr(inj.httpProxy),
          httpsProxy: optStr(inj.httpsProxy),
          noProxy: lines(inj.noProxy),
          timezone: optStr(inj.timezone),
          locale: optStr(inj.locale),
          keyboardLayout: optStr(inj.keyboardLayout),
          storageLayout: optStr(inj.storageLayout),
          aptMirror: optStr(inj.aptMirror),
          groups: lines(inj.groups),
          shell: optStr(inj.shell),
          sudoNopasswd: inj.sudoNopasswd,
          sudoCommands: lines(inj.sudoCommands),
          firewallEnabled: inj.firewallEnabled,
          firewallPolicy: optStr(inj.firewallPolicy),
          allowPorts: lines(inj.allowPorts),
          denyPorts: lines(inj.denyPorts),
          enableServices: lines(inj.enableServices),
          disableServices: lines(inj.disableServices),
          sysctl: lines(inj.sysctl),
          swapSizeMb: optNum(inj.swapSizeMb),
          swapFile: optStr(inj.swapFile),
          swappiness: optNum(inj.swappiness) !== null ? Math.min(100, Math.max(0, optNum(inj.swappiness)!)) : null,
          docker: inj.docker,
          podman: inj.podman,
          dockerUsers: lines(inj.dockerUsers),
          grubTimeout: optNum(inj.grubTimeout),
          grubCmdline: lines(inj.grubCmdline),
          grubDefault: optStr(inj.grubDefault),
          encrypt: inj.encrypt,
          encryptPassphrase: optStr(inj.encryptPassphrase),
          mounts: lines(inj.mounts),
          packages: lines(inj.packages),
          aptRepos: lines(inj.aptRepos),
          runCommands: lines(inj.runCommands),
          extraLateCommands: lines(inj.extraLateCommands),
          noUserInteraction: inj.noUserInteraction,
        },
      });
      setResult(r);
      setStatus(`Inject complete: ${r.artifacts[0] ?? r.output_dir}`);
    } catch (e) { setStatus(`Inject failed: ${e}`); }
    finally { setBusy(false); }
  };

  return (
    <div className="tab-content">
      <div className="panel inject-panel">
        <div className="inject-header">
          <h2>Autoinstall Injection</h2>
          <p className="muted">Embed a cloud-init autoinstall configuration into an Ubuntu ISO for fully unattended installation.</p>
        </div>

        <Section id="basic" title="Basic" open={is('basic')} onToggle={toggle}>
          <div className="field-grid">
            <Field label="Source ISO path or URL *">
              <TextInput value={inj.source} onChange={set('source')} placeholder="/path/to/ubuntu.iso or https://…/ubuntu.iso" />
            </Field>
            <Field label="Output directory *">
              <TextInput value={inj.outputDir} onChange={set('outputDir')} placeholder="./artifacts" />
            </Field>
            <Field label="Output ISO name *">
              <TextInput value={inj.outName} onChange={set('outName')} placeholder="forgeiso-autoinstall" />
            </Field>
            <Field label="Volume label">
              <TextInput value={inj.outputLabel} onChange={set('outputLabel')} placeholder="Optional (≤32 chars)" />
            </Field>
            <Field label="Existing autoinstall YAML (merge mode)">
              <TextInput value={inj.autoinstallYaml} onChange={set('autoinstallYaml')} placeholder="Optional path to existing user-data" />
            </Field>
          </div>
        </Section>

        <Section id="identity" title="Identity" open={is('identity')} onToggle={toggle}>
          <div className="field-grid">
            <Field label="Hostname">
              <TextInput value={inj.hostname} onChange={set('hostname')} placeholder="my-server" />
            </Field>
            <Field label="Username">
              <TextInput value={inj.username} onChange={set('username')} placeholder="ubuntu" />
            </Field>
            <Field label="Password (plaintext — hashed automatically)">
              <input type="password" value={inj.password} onChange={(e) => set('password')(e.target.value)} placeholder="••••••••" />
            </Field>
            <Field label="Real name">
              <TextInput value={inj.realname} onChange={set('realname')} placeholder="Ubuntu User" />
            </Field>
          </div>
        </Section>

        <Section id="ssh" title="SSH" open={is('ssh')} onToggle={toggle}>
          <div className="field-grid">
            <Field label="Authorized keys (one per line)">
              <TextArea value={inj.sshKeys} onChange={set('sshKeys')} placeholder="ssh-ed25519 AAAA… user@host" rows={4} />
            </Field>
            <div className="check-stack">
              <Checkbox label="Allow password authentication" checked={inj.sshPasswordAuth} onChange={set('sshPasswordAuth')} />
              <Checkbox label="Install OpenSSH server" checked={inj.sshInstallServer} onChange={set('sshInstallServer')} />
            </div>
          </div>
        </Section>

        <Section id="network" title="Network" open={is('network')} onToggle={toggle}>
          <div className="field-grid">
            <Field label="DNS servers (one per line)">
              <TextArea value={inj.dnsServers} onChange={set('dnsServers')} placeholder="1.1.1.1&#10;8.8.8.8" rows={3} />
            </Field>
            <Field label="NTP servers (one per line)">
              <TextArea value={inj.ntpServers} onChange={set('ntpServers')} placeholder="pool.ntp.org" rows={3} />
            </Field>
            <Field label="Static IP (CIDR — leave blank for DHCP)">
              <TextInput value={inj.staticIp} onChange={set('staticIp')} placeholder="10.0.0.5/24" />
            </Field>
            <Field label="Gateway">
              <TextInput value={inj.gateway} onChange={set('gateway')} placeholder="10.0.0.1" />
            </Field>
            <Field label="HTTP proxy">
              <TextInput value={inj.httpProxy} onChange={set('httpProxy')} placeholder="http://proxy.corp:3128" />
            </Field>
            <Field label="HTTPS proxy">
              <TextInput value={inj.httpsProxy} onChange={set('httpsProxy')} placeholder="http://proxy.corp:3128" />
            </Field>
            <Field label="No-proxy hosts (one per line)">
              <TextArea value={inj.noProxy} onChange={set('noProxy')} placeholder="localhost&#10;10.0.0.0/8" rows={3} />
            </Field>
          </div>
        </Section>

        <Section id="system" title="System" open={is('system')} onToggle={toggle}>
          <div className="field-grid">
            <Field label="Timezone">
              <TextInput value={inj.timezone} onChange={set('timezone')} placeholder="America/New_York" />
            </Field>
            <Field label="Locale">
              <TextInput value={inj.locale} onChange={set('locale')} placeholder="en_US.UTF-8" />
            </Field>
            <Field label="Keyboard layout">
              <TextInput value={inj.keyboardLayout} onChange={set('keyboardLayout')} placeholder="us" />
            </Field>
          </div>
        </Section>

        <Section id="storage" title="Storage & APT" open={is('storage')} onToggle={toggle}>
          <div className="field-grid">
            <Field label="Storage layout">
              <select value={inj.storageLayout} onChange={(e) => set('storageLayout')(e.target.value)}>
                <option value="">Default (auto)</option>
                <option value="lvm">LVM</option>
                <option value="direct">Direct</option>
                <option value="zfs">ZFS</option>
              </select>
            </Field>
            <Field label="APT mirror URL">
              <TextInput value={inj.aptMirror} onChange={set('aptMirror')} placeholder="http://archive.ubuntu.com/ubuntu" />
            </Field>
          </div>
        </Section>

        <Section id="user" title="User & Access" open={is('user')} onToggle={toggle}>
          <div className="field-grid">
            <Field label="Groups (one per line)">
              <TextArea value={inj.groups} onChange={set('groups')} placeholder="sudo&#10;docker&#10;video" rows={3} />
            </Field>
            <div className="check-stack">
              <Field label="Login shell">
                <TextInput value={inj.shell} onChange={set('shell')} placeholder="/bin/bash" />
              </Field>
              <Checkbox label="Passwordless sudo (NOPASSWD:ALL)" checked={inj.sudoNopasswd} onChange={set('sudoNopasswd')} />
            </div>
            <Field label="Restricted sudo commands (one per line)">
              <TextArea value={inj.sudoCommands} onChange={set('sudoCommands')} placeholder="/usr/bin/apt&#10;/usr/sbin/service" rows={3} />
            </Field>
          </div>
        </Section>

        <Section id="firewall" title="Firewall (UFW)" open={is('firewall')} onToggle={toggle}>
          <div className="field-grid">
            <div className="check-stack">
              <Checkbox label="Enable UFW firewall" checked={inj.firewallEnabled} onChange={set('firewallEnabled')} />
            </div>
            <Field label="Default incoming policy">
              <select value={inj.firewallPolicy} onChange={(e) => set('firewallPolicy')(e.target.value)} disabled={!inj.firewallEnabled}>
                <option value="deny">Deny</option>
                <option value="allow">Allow</option>
                <option value="reject">Reject</option>
              </select>
            </Field>
            <Field label="Allow ports (one per line, e.g. 22/tcp)">
              <TextArea value={inj.allowPorts} onChange={set('allowPorts')} placeholder="22&#10;80&#10;443/tcp" rows={3} />
            </Field>
            <Field label="Deny ports (one per line)">
              <TextArea value={inj.denyPorts} onChange={set('denyPorts')} placeholder="3306&#10;5432" rows={3} />
            </Field>
          </div>
        </Section>

        <Section id="services" title="Services & Kernel" open={is('services')} onToggle={toggle}>
          <div className="field-grid">
            <Field label="Enable services (one per line)">
              <TextArea value={inj.enableServices} onChange={set('enableServices')} placeholder="nginx&#10;docker" rows={3} />
            </Field>
            <Field label="Disable services (one per line)">
              <TextArea value={inj.disableServices} onChange={set('disableServices')} placeholder="bluetooth&#10;cups" rows={3} />
            </Field>
            <Field label="sysctl parameters (key=value, one per line)">
              <TextArea value={inj.sysctl} onChange={set('sysctl')} placeholder="vm.swappiness=10&#10;net.ipv4.ip_forward=1" rows={4} />
            </Field>
          </div>
        </Section>

        <Section id="swap" title="Swap" open={is('swap')} onToggle={toggle}>
          <div className="field-grid col3">
            <Field label="Swap size (MB)">
              <input type="number" min={0} value={inj.swapSizeMb} onChange={(e) => set('swapSizeMb')(e.target.value)} placeholder="4096" />
            </Field>
            <Field label="Swap file path">
              <TextInput value={inj.swapFile} onChange={set('swapFile')} placeholder="/swapfile" />
            </Field>
            <Field label="Swappiness (0–100)">
              <input type="number" min={0} max={100} value={inj.swappiness} onChange={(e) => set('swappiness')(e.target.value)} placeholder="10" />
            </Field>
          </div>
        </Section>

        <Section id="containers" title="Containers" open={is('containers')} onToggle={toggle}>
          <div className="field-grid">
            <div className="check-stack">
              <Checkbox label="Install Docker (CE + Compose)" checked={inj.docker} onChange={set('docker')} />
              <Checkbox label="Install Podman" checked={inj.podman} onChange={set('podman')} />
            </div>
            <Field label="Add users to docker group (one per line)">
              <TextArea value={inj.dockerUsers} onChange={set('dockerUsers')} placeholder="ubuntu&#10;admin" rows={3} />
            </Field>
          </div>
        </Section>

        <Section id="grub" title="Boot (GRUB)" open={is('grub')} onToggle={toggle}>
          <div className="field-grid col3">
            <Field label="Timeout (seconds)">
              <input type="number" min={0} value={inj.grubTimeout} onChange={(e) => set('grubTimeout')(e.target.value)} placeholder="5" />
            </Field>
            <Field label="Default entry">
              <TextInput value={inj.grubDefault} onChange={set('grubDefault')} placeholder="0 or saved" />
            </Field>
          </div>
          <Field label="Extra kernel cmdline parameters (one per line)">
            <TextArea value={inj.grubCmdline} onChange={set('grubCmdline')} placeholder="quiet&#10;iommu=on&#10;nosplash" rows={3} />
          </Field>
        </Section>

        <Section id="packages" title="Packages & APT Repos" open={is('packages')} onToggle={toggle}>
          <div className="field-grid">
            <Field label="Extra packages (one per line)">
              <TextArea value={inj.packages} onChange={set('packages')} placeholder="curl&#10;git&#10;vim&#10;htop" rows={4} />
            </Field>
            <Field label="APT repositories (PPA or deb line, one per line)">
              <TextArea value={inj.aptRepos} onChange={set('aptRepos')} placeholder="ppa:deadsnakes/ppa&#10;deb https://dl.google.com/linux/chrome/deb/ stable main" rows={4} />
            </Field>
          </div>
        </Section>

        <Section id="encryption" title="Disk Encryption (LUKS)" open={is('encryption')} onToggle={toggle}>
          <div className="field-grid">
            <div className="check-stack">
              <Checkbox label="Enable LUKS full-disk encryption" checked={inj.encrypt} onChange={set('encrypt')} />
            </div>
            <Field label="Encryption passphrase">
              <input type="password" value={inj.encryptPassphrase} onChange={(e) => set('encryptPassphrase')(e.target.value)} placeholder="••••••••" disabled={!inj.encrypt} />
            </Field>
          </div>
        </Section>

        <Section id="mounts" title="Custom Mounts" open={is('mounts')} onToggle={toggle}>
          <Field label="fstab entries (one per line — DEVICE PATH FSTYPE OPTIONS DUMP PASS)">
            <TextArea value={inj.mounts} onChange={set('mounts')} placeholder="/dev/sdb1 /data ext4 defaults,noatime 0 2" rows={4} />
          </Field>
        </Section>

        <Section id="commands" title="Commands" open={is('commands')} onToggle={toggle}>
          <div className="field-grid">
            <Field label="Post-install run commands (one per line)">
              <TextArea value={inj.runCommands} onChange={set('runCommands')} placeholder="systemctl restart nginx&#10;curl -sL https://example.com/setup.sh | bash" rows={4} />
            </Field>
            <Field label="Extra late-commands (raw, one per line)">
              <TextArea value={inj.extraLateCommands} onChange={set('extraLateCommands')} placeholder="chroot /target apt-get clean" rows={4} />
            </Field>
            <div className="check-stack span-2">
              <Checkbox label="No user interaction (fully automated installation)" checked={inj.noUserInteraction} onChange={set('noUserInteraction')} />
            </div>
          </div>
        </Section>

        <div className="inject-actions">
          <button className="primary large" onClick={run} disabled={busy || !inj.source.trim()}>
            Inject Autoinstall ISO
          </button>
          <p className="status-line">{status}</p>
        </div>

        {result && (
          <div className="result-card">
            <h3>Inject Result</h3>
            <dl className="inspection-grid">
              {result.artifacts.map((a) => (
                <div key={a} className="span-2"><dt>Artifact</dt><dd className="mono">{a}</dd></div>
              ))}
              <div><dt>Report JSON</dt><dd className="mono">{result.report_json}</dd></div>
              <div><dt>Report HTML</dt><dd className="mono">{result.report_html}</dd></div>
            </dl>
          </div>
        )}
      </div>
    </div>
  );
}

// ── Verify Tab ────────────────────────────────────────────────────────────────

function VerifyTab({ busy, setBusy, setStatus, status }: {
  busy: boolean; status: string;
  setBusy: (v: boolean) => void; setStatus: (v: string) => void;
}) {
  const [source, setSource] = useState('');
  const [sumsUrl, setSumsUrl] = useState('');
  const [result, setResult] = useState<VerifyResult | null>(null);

  const run = async () => {
    if (!source.trim()) { setStatus('Source is required'); return; }
    setBusy(true); setStatus('Verifying SHA-256 checksum...');
    try {
      const r = await invoke<VerifyResult>('verify_iso', {
        source, sumsUrl: sumsUrl.trim() || null,
      });
      setResult(r);
      setStatus(r.matched ? `✓ Checksum matched: ${r.filename}` : `✗ Checksum MISMATCH: ${r.filename}`);
    } catch (e) { setStatus(`Verify failed: ${e}`); }
    finally { setBusy(false); }
  };

  return (
    <div className="tab-content">
      <div className="panel">
        <h2>SHA-256 Verification</h2>
        <p className="muted">Verify an ISO against its official Ubuntu SHA256SUMS file. The checksum URL is auto-detected from ISO metadata if not provided.</p>
        <div className="field-grid" style={{ marginTop: 16 }}>
          <Field label="ISO path or URL">
            <TextInput value={source} onChange={setSource} placeholder="/path/to/ubuntu.iso or https://…/ubuntu.iso" />
          </Field>
          <Field label="SHA256SUMS URL (optional — auto-detected for Ubuntu)">
            <TextInput value={sumsUrl} onChange={setSumsUrl} placeholder="https://releases.ubuntu.com/24.04/SHA256SUMS" />
          </Field>
        </div>
        <div className="actions">
          <button className="primary" onClick={run} disabled={busy || !source.trim()}>Verify Checksum</button>
        </div>
        <p className="status-line">{status}</p>

        {result && (
          <div className={`result-card ${result.matched ? 'success' : 'failure'}`}>
            <h3>{result.matched ? '✓ Checksum Matched' : '✗ Checksum Mismatch'}</h3>
            <dl className="inspection-grid">
              <div><dt>File</dt><dd>{result.filename}</dd></div>
              <div><dt>Match</dt><dd>{result.matched ? 'Yes' : 'No'}</dd></div>
              <div className="span-2"><dt>Expected</dt><dd className="mono">{result.expected}</dd></div>
              <div className="span-2"><dt>Actual</dt><dd className="mono">{result.actual}</dd></div>
            </dl>
          </div>
        )}
      </div>
    </div>
  );
}

// ── Diff Tab ──────────────────────────────────────────────────────────────────

function DiffTab({ busy, setBusy, setStatus, status }: {
  busy: boolean; status: string;
  setBusy: (v: boolean) => void; setStatus: (v: string) => void;
}) {
  const [base, setBase] = useState('');
  const [target, setTarget] = useState('');
  const [diff, setDiff] = useState<IsoDiff | null>(null);
  const [filter, setFilter] = useState<'added' | 'removed' | 'modified' | 'unchanged' | 'all'>('all');

  const run = async () => {
    if (!base.trim() || !target.trim()) { setStatus('Both ISO paths are required'); return; }
    setBusy(true); setStatus('Comparing ISOs...');
    try {
      setDiff(await invoke<IsoDiff>('diff_isos', { base, target }));
      setStatus('Diff complete');
    } catch (e) { setStatus(`Diff failed: ${e}`); }
    finally { setBusy(false); }
  };

  const counts = diff ? {
    added: diff.added.length,
    removed: diff.removed.length,
    modified: diff.modified.length,
    unchanged: diff.unchanged.length,
  } : null;

  const entries: { label: string; entries: DiffEntry[]; cls: string }[] = diff ? [
    { label: 'Added', entries: diff.added, cls: 'added' },
    { label: 'Removed', entries: diff.removed, cls: 'removed' },
    { label: 'Modified', entries: diff.modified, cls: 'modified' },
    { label: 'Unchanged', entries: diff.unchanged, cls: 'unchanged' },
  ] : [];

  const visible = filter === 'all' ? entries : entries.filter((g) => g.cls === filter);

  return (
    <div className="tab-content">
      <div className="panel">
        <h2>ISO Diff</h2>
        <p className="muted">Compare two ISOs to see added, removed, and modified files.</p>
        <div className="field-grid" style={{ marginTop: 16 }}>
          <Field label="Base ISO path">
            <TextInput value={base} onChange={setBase} placeholder="/path/to/base.iso" />
          </Field>
          <Field label="Target ISO path">
            <TextInput value={target} onChange={setTarget} placeholder="/path/to/modified.iso" />
          </Field>
        </div>
        <div className="actions">
          <button className="primary" onClick={run} disabled={busy || !base.trim() || !target.trim()}>Compare ISOs</button>
        </div>
        <p className="status-line">{status}</p>

        {diff && counts && (
          <>
            <div className="diff-summary">
              {(['added', 'removed', 'modified', 'unchanged', 'all'] as const).map((f) => (
                <button
                  key={f}
                  className={`diff-filter ${f} ${filter === f ? 'active' : ''}`}
                  onClick={() => setFilter(f)}
                >
                  {f === 'all' ? `All (${counts.added + counts.removed + counts.modified + counts.unchanged})`
                    : `${f.charAt(0).toUpperCase() + f.slice(1)} (${counts[f as keyof typeof counts]})`}
                </button>
              ))}
            </div>

            <div className="diff-list">
              {visible.map((group) =>
                group.entries.map((entry) => (
                  <div key={`${group.cls}-${entry.path}`} className={`diff-entry ${group.cls}`}>
                    <span className="diff-tag">{group.label[0]}</span>
                    <span className="mono diff-path">{entry.path}</span>
                    {entry.size_bytes !== undefined && (
                      <span className="diff-size">{(entry.size_bytes / 1024).toFixed(1)} KB</span>
                    )}
                  </div>
                ))
              )}
            </div>
          </>
        )}
      </div>
    </div>
  );
}

// ── Root App ──────────────────────────────────────────────────────────────────

export function App() {
  const [activeTab, setActiveTab] = useState<Tab>('build');
  const [doctor, setDoctor] = useState<any>(null);
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const [busy, setBusy] = useState(false);
  const [status, setStatus] = useState('Ready');
  const [lastIso, setLastIso] = useState('');
  const [lastBuildDir, setLastBuildDir] = useState('');

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    const start = async () => {
      unlisten = await listen<LogEntry>('forgeiso-log', (event) => {
        setLogs((prev) => [...prev.slice(-299), event.payload]);
      });
      await invoke('start_event_stream');
      setDoctor(await invoke('doctor'));
    };
    start().catch((e) => setStatus(`Startup error: ${e}`));
    return () => { unlisten?.(); };
  }, []);

  const tabs: { id: Tab; label: string }[] = [
    { id: 'build', label: 'Build' },
    { id: 'inject', label: 'Inject' },
    { id: 'verify', label: 'Verify' },
    { id: 'diff', label: 'Diff' },
  ];

  return (
    <div className="app-shell">
      <header className="hero">
        <div>
          <h1>ForgeISO</h1>
          <p>Build, inject, verify, and compare Linux ISOs locally on bare metal.</p>
        </div>
        <div className="doctor-card">
          <h2>System</h2>
          <p>{doctor ? `${doctor.host_os} / ${doctor.host_arch}` : 'Loading…'}</p>
          <p className={doctor?.linux_supported ? 'ok' : 'warn'}>
            {doctor?.linux_supported ? '✓ Linux build support enabled' : '✗ Linux host required'}
          </p>
          {doctor?.tooling && (
            <div className="tool-pills">
              {Object.entries(doctor.tooling as Record<string, string>).map(([tool, state]) => (
                <span key={tool} className={`tool-pill ${state === 'Passed' ? 'ok' : 'warn'}`}>{tool}</span>
              ))}
            </div>
          )}
        </div>
      </header>

      <div className="tabs">
        {tabs.map((t) => (
          <button
            key={t.id}
            className={`tab-btn ${activeTab === t.id ? 'active' : ''}`}
            onClick={() => setActiveTab(t.id)}
          >
            {t.label}
          </button>
        ))}
      </div>

      {activeTab === 'build' && (
        <BuildTab
          busy={busy} status={status} logs={logs}
          lastIso={lastIso} lastBuildDir={lastBuildDir}
          setBusy={setBusy} setStatus={setStatus} setLogs={setLogs}
          setLastIso={setLastIso} setLastBuildDir={setLastBuildDir}
        />
      )}
      {activeTab === 'inject' && (
        <InjectTab busy={busy} setBusy={setBusy} setStatus={setStatus} status={status} />
      )}
      {activeTab === 'verify' && (
        <VerifyTab busy={busy} setBusy={setBusy} setStatus={setStatus} status={status} />
      )}
      {activeTab === 'diff' && (
        <DiffTab busy={busy} setBusy={setBusy} setStatus={setStatus} status={status} />
      )}

      <section className="panel log-panel">
        <h2>Operation Log</h2>
        <div className="log-console">
          {logs.length === 0 && <p className="muted">Waiting for engine events…</p>}
          {logs.map((entry, i) => (
            <p key={`${entry.ts}-${i}`}>
              <span className="mono log-ts">[{entry.ts.slice(11, 19)}]</span>{' '}
              <span className={`badge phase-${entry.phase.toLowerCase()}`}>{entry.phase}</span>{' '}
              {entry.message}
            </p>
          ))}
        </div>
      </section>
    </div>
  );
}

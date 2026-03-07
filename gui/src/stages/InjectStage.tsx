import { useReducer, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { Dispatch } from 'react';
import type { InjectResult, InjectState } from '../types';
import type { AppAction } from '../store';
import { defaultInjectState, INJECT_PRESETS } from '../types';
import { Field, TextInput, TextArea, Toggle, Accordion, useAccordion } from '../components/forms';

// ── Local form reducer ────────────────────────────────────────────────────────

type InjectFormAction = { key: keyof InjectState; value: InjectState[keyof InjectState] };

function injectFormReducer(state: InjectState, action: InjectFormAction): InjectState {
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

// ── Component ─────────────────────────────────────────────────────────────────

export function InjectStage({
  dispatch,
  isRunning,
  lastSourceIso,
  lastOutputDir,
  injectResult,
}: {
  dispatch: Dispatch<AppAction>;
  isRunning: boolean;
  lastSourceIso: string;
  lastOutputDir: string;
  injectResult: InjectResult | null;
}) {
  const [inj, formDispatch] = useReducer(injectFormReducer, {
    ...defaultInjectState,
    source: lastSourceIso || defaultInjectState.source,
    outputDir: lastOutputDir || defaultInjectState.outputDir,
  });

  const set = <K extends keyof InjectState>(key: K) =>
    (value: InjectState[K]) => formDispatch({ key, value });

  const { toggle, is } = useAccordion(['basic', 'identity', 'ssh', 'network']);

  const [statusMsg, setStatusMsg] = useState('');
  const [statusKind, setStatusKind] = useState<'ok' | 'err' | ''>('');

  const setStatus = (msg: string, kind: 'ok' | 'err' | '' = '') => {
    setStatusMsg(msg);
    setStatusKind(kind);
  };

  const applyPreset = (presetId: string) => {
    const preset = INJECT_PRESETS.find((p) => p.id === presetId);
    if (!preset) return;
    for (const [k, v] of Object.entries(preset.overrides)) {
      formDispatch({ key: k as keyof InjectState, value: v as InjectState[keyof InjectState] });
    }
  };

  const run = async () => {
    if (!inj.source.trim()) { setStatus('Source ISO is required', 'err'); return; }
    dispatch({ type: 'JOB_START', stage: 'inject', operation: 'Injecting autoinstall configuration…' });
    try {
      const result = await invoke<InjectResult>('inject_iso', {
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
          swappiness: optNum(inj.swappiness) !== null
            ? Math.min(100, Math.max(0, optNum(inj.swappiness)!))
            : null,
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
          distro: inj.distro === 'ubuntu' ? null : inj.distro,
        },
      });
      const iso = result.artifacts[0] ?? result.output_dir;
      dispatch({ type: 'SET_INJECT_RESULT', result, injectedIso: iso });
      dispatch({ type: 'JOB_SUCCESS', stage: 'inject' });
      setStatus(`Inject complete: ${iso}`, 'ok');
    } catch (e) {
      dispatch({ type: 'JOB_ERROR', stage: 'inject', error: String(e) });
      setStatus(`Inject failed: ${e}`, 'err');
    }
  };

  return (
    <div className="main-content">
      {/* Presets */}
      <div className="card" style={{ marginBottom: 'var(--sp-4)' }}>
        <div className="card-header">
          <div>
            <h2>Autoinstall Injection</h2>
            <p>Embed an unattended installation configuration into a Linux ISO. Supports Ubuntu (cloud-init), Fedora (Kickstart), and Arch Linux (archinstall).</p>
          </div>
        </div>
        <p className="sidebar-section-title" style={{ marginBottom: 'var(--sp-2)' }}>Quick Presets</p>
        <div className="preset-grid">
          {INJECT_PRESETS.map((preset) => (
            <button
              key={preset.id}
              className="preset-btn"
              type="button"
              onClick={() => applyPreset(preset.id)}
              disabled={isRunning}
            >
              <div className="preset-btn-label">{preset.label}</div>
              <div className="preset-btn-desc">{preset.description}</div>
            </button>
          ))}
        </div>
      </div>

      {/* Accordion sections */}
      <Accordion id="basic" icon="📁" title="Basic" summary="Source, output paths" open={is('basic')} onToggle={toggle}>
        <div className="field-grid">
          <Field label="Source ISO path or URL *" className="span-2">
            <TextInput value={inj.source} onChange={set('source')} placeholder="/path/to/ubuntu.iso" disabled={isRunning} />
          </Field>
          <Field label="Output directory *">
            <TextInput value={inj.outputDir} onChange={set('outputDir')} placeholder="./artifacts" disabled={isRunning} />
          </Field>
          <Field label="Output ISO name *">
            <TextInput value={inj.outName} onChange={set('outName')} placeholder="forgeiso-autoinstall" disabled={isRunning} />
          </Field>
          <Field label="Volume label">
            <TextInput value={inj.outputLabel} onChange={set('outputLabel')} placeholder="Optional (≤32 chars)" disabled={isRunning} />
          </Field>
          <Field label="Target distro">
            <select
              value={inj.distro}
              onChange={(e) => set('distro')(e.target.value)}
              disabled={isRunning}
              className="text-input"
            >
              <option value="ubuntu">Ubuntu (cloud-init autoinstall)</option>
              <option value="fedora">Fedora / RHEL (Kickstart ks.cfg) — Beta</option>
              <option value="arch">Arch Linux (archinstall JSON) — Beta</option>
            </select>
          </Field>
          <Field label="Existing autoinstall YAML (merge mode, Ubuntu only)">
            <TextInput value={inj.autoinstallYaml} onChange={set('autoinstallYaml')} placeholder="Optional path to existing user-data" disabled={isRunning} />
          </Field>
        </div>
      </Accordion>

      <Accordion id="identity" icon="👤" title="Identity" summary="Hostname, username, password" open={is('identity')} onToggle={toggle}>
        <div className="field-grid">
          <Field label="Hostname">
            <TextInput value={inj.hostname} onChange={set('hostname')} placeholder="my-server" disabled={isRunning} />
          </Field>
          <Field label="Username">
            <TextInput value={inj.username} onChange={set('username')} placeholder="ubuntu" disabled={isRunning} />
          </Field>
          <Field label="Password (hashed automatically)">
            <TextInput type="password" value={inj.password} onChange={set('password')} placeholder="••••••••" disabled={isRunning} />
          </Field>
          <Field label="Real name">
            <TextInput value={inj.realname} onChange={set('realname')} placeholder="Ubuntu User" disabled={isRunning} />
          </Field>
        </div>
      </Accordion>

      <Accordion id="ssh" icon="🔑" title="SSH" summary="Keys, password auth" open={is('ssh')} onToggle={toggle}>
        <div className="field-grid">
          <Field label="Authorized keys (one per line)" className="span-2">
            <TextArea value={inj.sshKeys} onChange={set('sshKeys')} placeholder="ssh-ed25519 AAAA… user@host" rows={4} disabled={isRunning} />
          </Field>
          <div className="span-2">
            <Toggle label="Allow password authentication" checked={inj.sshPasswordAuth} onChange={set('sshPasswordAuth')} disabled={isRunning} />
            <Toggle label="Install OpenSSH server" checked={inj.sshInstallServer} onChange={set('sshInstallServer')} disabled={isRunning} />
          </div>
        </div>
      </Accordion>

      <Accordion id="network" icon="🌐" title="Network" summary="DNS, NTP, static IP, proxy" open={is('network')} onToggle={toggle}>
        <div className="field-grid">
          <Field label="DNS servers (one per line)">
            <TextArea value={inj.dnsServers} onChange={set('dnsServers')} placeholder={'1.1.1.1\n8.8.8.8'} rows={3} disabled={isRunning} />
          </Field>
          <Field label="NTP servers (one per line)">
            <TextArea value={inj.ntpServers} onChange={set('ntpServers')} placeholder="pool.ntp.org" rows={3} disabled={isRunning} />
          </Field>
          <Field label="Static IP (CIDR — leave blank for DHCP)">
            <TextInput value={inj.staticIp} onChange={set('staticIp')} placeholder="10.0.0.5/24" disabled={isRunning} />
          </Field>
          <Field label="Gateway">
            <TextInput value={inj.gateway} onChange={set('gateway')} placeholder="10.0.0.1" disabled={isRunning} />
          </Field>
          <Field label="HTTP proxy">
            <TextInput value={inj.httpProxy} onChange={set('httpProxy')} placeholder="http://proxy.corp:3128" disabled={isRunning} />
          </Field>
          <Field label="HTTPS proxy">
            <TextInput value={inj.httpsProxy} onChange={set('httpsProxy')} placeholder="http://proxy.corp:3128" disabled={isRunning} />
          </Field>
          <Field label="No-proxy hosts (one per line)" className="span-2">
            <TextArea value={inj.noProxy} onChange={set('noProxy')} placeholder={'localhost\n10.0.0.0/8'} rows={3} disabled={isRunning} />
          </Field>
        </div>
      </Accordion>

      <Accordion id="system" icon="⚙️" title="System" summary="Timezone, locale, keyboard" open={is('system')} onToggle={toggle}>
        <div className="field-grid cols-3">
          <Field label="Timezone">
            <TextInput value={inj.timezone} onChange={set('timezone')} placeholder="America/Chicago" disabled={isRunning} />
          </Field>
          <Field label="Locale">
            <TextInput value={inj.locale} onChange={set('locale')} placeholder="en_US.UTF-8" disabled={isRunning} />
          </Field>
          <Field label="Keyboard layout">
            <TextInput value={inj.keyboardLayout} onChange={set('keyboardLayout')} placeholder="us" disabled={isRunning} />
          </Field>
        </div>
      </Accordion>

      <Accordion id="storage" icon="💾" title="Storage & APT" summary="Layout, mirror, encryption" open={is('storage')} onToggle={toggle}>
        <div className="field-grid">
          <Field label="Storage layout">
            <select value={inj.storageLayout} onChange={(e) => set('storageLayout')(e.target.value)} disabled={isRunning}>
              <option value="">Default (auto)</option>
              <option value="lvm">LVM</option>
              <option value="direct">Direct</option>
              <option value="zfs">ZFS</option>
            </select>
          </Field>
          <Field label="APT mirror URL">
            <TextInput value={inj.aptMirror} onChange={set('aptMirror')} placeholder="http://archive.ubuntu.com/ubuntu" disabled={isRunning} />
          </Field>
          <div className="span-2">
            <Toggle label="Enable LUKS full-disk encryption" checked={inj.encrypt} onChange={set('encrypt')} disabled={isRunning} />
          </div>
          {inj.encrypt && (
            <Field label="Encryption passphrase" className="span-2">
              <TextInput type="password" value={inj.encryptPassphrase} onChange={set('encryptPassphrase')} placeholder="••••••••" disabled={isRunning} />
            </Field>
          )}
        </div>
      </Accordion>

      <Accordion id="user" icon="👥" title="User & Access" summary="Groups, shell, sudo" open={is('user')} onToggle={toggle}>
        <div className="field-grid">
          <Field label="Groups (one per line)">
            <TextArea value={inj.groups} onChange={set('groups')} placeholder={'sudo\ndocker\nvideo'} rows={3} disabled={isRunning} />
          </Field>
          <div>
            <Field label="Login shell">
              <TextInput value={inj.shell} onChange={set('shell')} placeholder="/bin/bash" disabled={isRunning} />
            </Field>
            <Toggle label="Passwordless sudo (NOPASSWD:ALL)" checked={inj.sudoNopasswd} onChange={set('sudoNopasswd')} disabled={isRunning} />
          </div>
          <Field label="Restricted sudo commands (one per line)" className="span-2">
            <TextArea value={inj.sudoCommands} onChange={set('sudoCommands')} placeholder={'/usr/bin/apt\n/usr/sbin/service'} rows={3} disabled={isRunning} />
          </Field>
        </div>
      </Accordion>

      <Accordion id="firewall" icon="🛡️" title="Firewall (UFW)" summary="UFW rules, ports" open={is('firewall')} onToggle={toggle}>
        <div className="field-grid">
          <div className="span-2">
            <Toggle label="Enable UFW firewall" checked={inj.firewallEnabled} onChange={set('firewallEnabled')} disabled={isRunning} />
          </div>
          <Field label="Default incoming policy">
            <select value={inj.firewallPolicy} onChange={(e) => set('firewallPolicy')(e.target.value)} disabled={isRunning || !inj.firewallEnabled}>
              <option value="deny">Deny</option>
              <option value="allow">Allow</option>
              <option value="reject">Reject</option>
            </select>
          </Field>
          <div />
          <Field label="Allow ports (one per line, e.g. 22/tcp)">
            <TextArea value={inj.allowPorts} onChange={set('allowPorts')} placeholder={'22\n80\n443/tcp'} rows={3} disabled={isRunning} />
          </Field>
          <Field label="Deny ports (one per line)">
            <TextArea value={inj.denyPorts} onChange={set('denyPorts')} placeholder={'3306\n5432'} rows={3} disabled={isRunning} />
          </Field>
        </div>
      </Accordion>

      <Accordion id="services" icon="⚡" title="Services & Kernel" summary="systemctl, sysctl" open={is('services')} onToggle={toggle}>
        <div className="field-grid">
          <Field label="Enable services (one per line)">
            <TextArea value={inj.enableServices} onChange={set('enableServices')} placeholder={'nginx\ndocker'} rows={3} disabled={isRunning} />
          </Field>
          <Field label="Disable services (one per line)">
            <TextArea value={inj.disableServices} onChange={set('disableServices')} placeholder={'bluetooth\ncups'} rows={3} disabled={isRunning} />
          </Field>
          <Field label="sysctl parameters (key=value, one per line)" className="span-2">
            <TextArea value={inj.sysctl} onChange={set('sysctl')} placeholder={'vm.swappiness=10\nnet.ipv4.ip_forward=1'} rows={4} disabled={isRunning} />
          </Field>
        </div>
      </Accordion>

      <Accordion id="swap" icon="🔄" title="Swap" summary="Swap file size and settings" open={is('swap')} onToggle={toggle}>
        <div className="field-grid cols-3">
          <Field label="Swap size (MB)">
            <input type="number" min={0} value={inj.swapSizeMb} onChange={(e) => set('swapSizeMb')(e.target.value)} placeholder="4096" disabled={isRunning} />
          </Field>
          <Field label="Swap file path">
            <TextInput value={inj.swapFile} onChange={set('swapFile')} placeholder="/swapfile" disabled={isRunning} />
          </Field>
          <Field label="Swappiness (0–100)">
            <input type="number" min={0} max={100} value={inj.swappiness} onChange={(e) => set('swappiness')(e.target.value)} placeholder="10" disabled={isRunning} />
          </Field>
        </div>
      </Accordion>

      <Accordion id="containers" icon="🐳" title="Containers" summary="Docker, Podman" open={is('containers')} onToggle={toggle}>
        <div className="field-grid">
          <div className="span-2">
            <Toggle label="Install Docker CE + Compose" checked={inj.docker} onChange={set('docker')} disabled={isRunning} />
            <Toggle label="Install Podman" checked={inj.podman} onChange={set('podman')} disabled={isRunning} />
          </div>
          <Field label="Add users to docker group (one per line)" className="span-2">
            <TextArea value={inj.dockerUsers} onChange={set('dockerUsers')} placeholder={'ubuntu\nadmin'} rows={3} disabled={isRunning} />
          </Field>
        </div>
      </Accordion>

      <Accordion id="grub" icon="🖥️" title="Boot (GRUB)" summary="Timeout, kernel cmdline" open={is('grub')} onToggle={toggle}>
        <div className="field-grid cols-3">
          <Field label="Timeout (seconds)">
            <input type="number" min={0} value={inj.grubTimeout} onChange={(e) => set('grubTimeout')(e.target.value)} placeholder="5" disabled={isRunning} />
          </Field>
          <Field label="Default entry">
            <TextInput value={inj.grubDefault} onChange={set('grubDefault')} placeholder="0 or saved" disabled={isRunning} />
          </Field>
          <div />
        </div>
        <div style={{ marginTop: 'var(--sp-3)' }}>
        <Field label="Extra kernel cmdline parameters (one per line)">
          <TextArea value={inj.grubCmdline} onChange={set('grubCmdline')} placeholder={'quiet\niommu=on\nnosplash'} rows={3} disabled={isRunning} />
        </Field>
        </div>
      </Accordion>

      <Accordion id="packages" icon="📦" title="Packages & APT Repos" summary="Extra packages, PPAs" open={is('packages')} onToggle={toggle}>
        <div className="field-grid">
          <Field label="Extra packages (one per line)">
            <TextArea value={inj.packages} onChange={set('packages')} placeholder={'curl\ngit\nvim\nhtop'} rows={4} disabled={isRunning} />
          </Field>
          <Field label="APT repos (PPA or deb line, one per line)">
            <TextArea value={inj.aptRepos} onChange={set('aptRepos')} placeholder={'ppa:deadsnakes/ppa\ndeb https://dl.google.com/linux/chrome/deb/ stable main'} rows={4} disabled={isRunning} />
          </Field>
        </div>
      </Accordion>

      <Accordion id="mounts" icon="💿" title="Custom Mounts" summary="fstab entries" open={is('mounts')} onToggle={toggle}>
        <Field label="fstab entries (one per line — DEVICE PATH FSTYPE OPTIONS DUMP PASS)">
          <TextArea value={inj.mounts} onChange={set('mounts')} placeholder="/dev/sdb1 /data ext4 defaults,noatime 0 2" rows={4} disabled={isRunning} />
        </Field>
      </Accordion>

      <Accordion id="commands" icon="⌨️" title="Commands" summary="Post-install run commands" open={is('commands')} onToggle={toggle}>
        <div className="field-grid">
          <Field label="Post-install run commands (one per line)">
            <TextArea value={inj.runCommands} onChange={set('runCommands')} placeholder={'systemctl restart nginx\ncurl -sL https://example.com/setup.sh | bash'} rows={4} disabled={isRunning} />
          </Field>
          <Field label="Extra late-commands (raw, one per line)">
            <TextArea value={inj.extraLateCommands} onChange={set('extraLateCommands')} placeholder="chroot /target apt-get clean" rows={4} disabled={isRunning} />
          </Field>
          <div className="span-2">
            <Toggle label="No user interaction (fully automated installation)" checked={inj.noUserInteraction} onChange={set('noUserInteraction')} disabled={isRunning} />
          </div>
        </div>
      </Accordion>

      {/* Run button */}
      <div className="card" style={{ marginTop: 'var(--sp-4)' }}>
        <div className="btn-group">
          <button
            className="btn btn-primary btn-xl"
            type="button"
            onClick={run}
            disabled={isRunning || !inj.source.trim()}
          >
            {isRunning
              ? <><span className="spinner" /> Injecting…</>
              : 'Inject Autoinstall ISO'}
          </button>
        </div>
        {statusMsg && (
          <p className={`status-line${statusKind === 'ok' ? ' ok' : statusKind === 'err' ? ' err' : ''}`} style={{ marginTop: 'var(--sp-3)' }}>
            {statusMsg}
          </p>
        )}
      </div>

      {/* Result */}
      {injectResult && (
        <div className="card card-green" style={{ marginTop: 'var(--sp-4)' }}>
          <div className="card-header">
            <h2>Inject Complete</h2>
          </div>
          <div className="artifact-list">
            {injectResult.artifacts.map((a) => (
              <div key={a} className="artifact-item">
                <span className="artifact-icon">💿</span>
                <span className="artifact-path">{a}</span>
              </div>
            ))}
          </div>
          <div style={{ marginTop: 'var(--sp-4)' }} className="btn-group btn-group-right">
            <button
              className="btn btn-primary"
              type="button"
              onClick={() => dispatch({ type: 'ADVANCE_STAGE', from: 'inject' })}
            >
              Continue to Verify →
            </button>
          </div>
        </div>
      )}
    </div>
  );
}

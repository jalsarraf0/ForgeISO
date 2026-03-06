import { useEffect, useMemo, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

const steps = [
  'Distro',
  'Release',
  'Profile',
  'Customize',
  'Security',
  'Review',
  'Build',
  'Test',
  'Export',
] as const;

type Distro = 'ubuntu' | 'mint' | 'fedora' | 'arch';

type LogEntry = {
  ts: string;
  phase: string;
  level: string;
  message: string;
};

export function App() {
  const [currentStep, setCurrentStep] = useState(0);
  const [theme, setTheme] = useState<'dark' | 'light'>('dark');
  const [distro, setDistro] = useState<Distro>('ubuntu');
  const [release, setRelease] = useState('24.04');
  const [profile, setProfile] = useState('hardened_server');
  const [packageSearch, setPackageSearch] = useState('');
  const [agentEndpoint, setAgentEndpoint] = useState('');
  const [agentConnected, setAgentConnected] = useState(false);
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const [busy, setBusy] = useState(false);
  const [status, setStatus] = useState('Ready');

  useEffect(() => {
    document.documentElement.dataset.theme = theme;
  }, [theme]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;

    const run = async () => {
      unlisten = await listen<LogEntry>('forgeiso-log', (event) => {
        setLogs((prev) => [...prev.slice(-299), event.payload]);
      });
      await invoke('start_event_stream');
    };

    run().catch((error) => {
      setStatus(`Failed to start log stream: ${error}`);
    });

    return () => {
      if (unlisten) {
        unlisten();
      }
    };
  }, []);

  const stepTitle = useMemo(() => steps[currentStep], [currentStep]);

  const nextStep = () => setCurrentStep((s) => Math.min(s + 1, steps.length - 1));
  const prevStep = () => setCurrentStep((s) => Math.max(s - 1, 0));

  const runDoctor = async () => {
    setBusy(true);
    setStatus('Running doctor...');
    try {
      const result = await invoke('doctor');
      setStatus(`Doctor complete: ${JSON.stringify(result).slice(0, 140)}...`);
    } catch (error) {
      setStatus(`Doctor failed: ${error}`);
    } finally {
      setBusy(false);
    }
  };

  const build = async () => {
    setBusy(true);
    setStatus('Building ISO...');
    try {
      await invoke('build_from_inline', {
        request: {
          name: 'gui-build',
          distro,
          release,
          profile,
        },
      });
      setStatus('Build completed');
      setCurrentStep(6);
    } catch (error) {
      setStatus(`Build failed: ${error}`);
    } finally {
      setBusy(false);
    }
  };

  const toggleAgent = async () => {
    try {
      if (agentConnected) {
        await invoke('disconnect_agent');
        setAgentConnected(false);
        setStatus('Remote agent disconnected');
      } else {
        await invoke('connect_agent', { endpoint: agentEndpoint });
        setAgentConnected(true);
        setStatus('Remote agent connected');
      }
    } catch (error) {
      setStatus(`Agent error: ${error}`);
    }
  };

  return (
    <div className="app-shell">
      <header className="topbar">
        <div>
          <h1>ForgeISO</h1>
          <p className="subtitle">Enterprise ISO customization studio</p>
        </div>
        <div className="topbar-actions">
          <button className="ghost" onClick={runDoctor} disabled={busy}>Doctor</button>
          <button className="ghost" onClick={() => setTheme(theme === 'dark' ? 'light' : 'dark')}>
            Theme: {theme}
          </button>
        </div>
      </header>

      <section className="wizard-track">
        {steps.map((label, index) => (
          <button
            key={label}
            className={`step-pill ${index === currentStep ? 'active' : ''} ${index < currentStep ? 'done' : ''}`}
            onClick={() => setCurrentStep(index)}
          >
            <span>{index + 1}</span>
            {label}
          </button>
        ))}
      </section>

      <main className="content-grid">
        <section className="panel form-panel">
          <h2>{stepTitle}</h2>
          <div className="field-grid">
            <label>
              Distro
              <select value={distro} onChange={(e) => setDistro(e.target.value as Distro)}>
                <option value="ubuntu">Ubuntu (LTS only)</option>
                <option value="mint">Linux Mint (LTS only)</option>
                <option value="fedora">Fedora (latest stable, non-LTS)</option>
                <option value="arch">Arch (rolling snapshot)</option>
              </select>
            </label>
            <label>
              Release
              <input value={release} onChange={(e) => setRelease(e.target.value)} />
            </label>
            <label>
              Profile
              <select value={profile} onChange={(e) => setProfile(e.target.value)}>
                <option value="hardened_server">Hardened Server</option>
                <option value="developer_workstation">Developer Workstation</option>
                <option value="minimal">Minimal</option>
                <option value="kiosk">Kiosk</option>
                <option value="gaming">Gaming</option>
              </select>
            </label>
            <label>
              Package Search
              <input
                placeholder="Search mapped packages and bundles"
                value={packageSearch}
                onChange={(e) => setPackageSearch(e.target.value)}
              />
            </label>
            <label>
              Remote Agent Endpoint
              <input
                placeholder="https://linux-agent.internal:7443"
                value={agentEndpoint}
                onChange={(e) => setAgentEndpoint(e.target.value)}
              />
            </label>
            <div className="inline-actions">
              <button className="ghost" onClick={toggleAgent}>
                {agentConnected ? 'Disconnect Agent' : 'Connect Agent'}
              </button>
            </div>
          </div>

          <div className="step-actions">
            <button className="ghost" disabled={currentStep === 0} onClick={prevStep}>Back</button>
            <button className="ghost" disabled={currentStep === steps.length - 1} onClick={nextStep}>Next</button>
            <button className="primary" disabled={busy} onClick={build}>Build</button>
          </div>
        </section>

        <section className="panel logs-panel">
          <h2>Live Logs</h2>
          <div className="log-console">
            {logs.length === 0 ? <p className="muted">Waiting for engine events...</p> : null}
            {logs.map((entry, idx) => (
              <p key={`${entry.ts}-${idx}`}>
                <span className="muted">[{entry.ts}]</span> <span className="badge">{entry.phase}</span>{' '}
                <span className={`level-${entry.level.toLowerCase()}`}>{entry.level}</span> {entry.message}
              </p>
            ))}
          </div>
          <p className="status">{status}</p>
        </section>
      </main>
    </div>
  );
}

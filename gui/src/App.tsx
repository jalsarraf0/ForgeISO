import { useEffect, useMemo, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

type LogEntry = {
  ts: string;
  phase: string;
  level: string;
  message: string;
};

type Inspection = {
  source_path: string;
  source_kind: string;
  source_value: string;
  size_bytes: number;
  sha256: string;
  volume_id?: string | null;
  distro?: string | null;
  release?: string | null;
  edition?: string | null;
  architecture?: string | null;
  warnings: string[];
};

type BuildResult = {
  output_dir: string;
  report_json: string;
  report_html: string;
  artifacts: string[];
};

const profiles = [
  { value: 'minimal', label: 'Minimal' },
  { value: 'desktop', label: 'Desktop' },
] as const;

export function App() {
  const [source, setSource] = useState('');
  const [outputDir, setOutputDir] = useState('./artifacts');
  const [buildName, setBuildName] = useState('forgeiso-local');
  const [overlayDir, setOverlayDir] = useState('');
  const [outputLabel, setOutputLabel] = useState('');
  const [profile, setProfile] = useState('minimal');
  const [doctor, setDoctor] = useState<any>(null);
  const [inspection, setInspection] = useState<Inspection | null>(null);
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const [busy, setBusy] = useState(false);
  const [status, setStatus] = useState('Ready');
  const [lastIso, setLastIso] = useState('');
  const [lastBuildDir, setLastBuildDir] = useState('');

  useEffect(() => {
    let unlisten: (() => void) | undefined;

    const start = async () => {
      unlisten = await listen<LogEntry>('forgeiso-log', (event) => {
        setLogs((prev) => [...prev.slice(-199), event.payload]);
      });
      await invoke('start_event_stream');
      const report = await invoke('doctor');
      setDoctor(report);
    };

    start().catch((error) => setStatus(`Startup failed: ${error}`));

    return () => {
      if (unlisten) {
        unlisten();
      }
    };
  }, []);

  const canBuild = useMemo(() => source.trim().length > 0 && outputDir.trim().length > 0, [source, outputDir]);

  const inspect = async () => {
    if (!source.trim()) {
      setStatus('Source is required');
      return;
    }
    setBusy(true);
    setStatus('Inspecting source ISO...');
    try {
      const value = await invoke<Inspection>('inspect_source', { source });
      setInspection(value);
      setStatus('Inspection completed');
    } catch (error) {
      setStatus(`Inspection failed: ${error}`);
    } finally {
      setBusy(false);
    }
  };

  const build = async () => {
    setBusy(true);
    setStatus('Building local ISO...');
    try {
      const result = await invoke<BuildResult>('build_local', {
        request: {
          source,
          outputDir,
          name: buildName,
          overlayDir,
          outputLabel,
          profile,
        },
      });
      setLastIso(result.artifacts[0] ?? '');
      setLastBuildDir(result.output_dir);
      setStatus(`Build completed: ${result.artifacts[0] ?? result.output_dir}`);
    } catch (error) {
      setStatus(`Build failed: ${error}`);
    } finally {
      setBusy(false);
    }
  };

  const scan = async () => {
    if (!lastIso) {
      setStatus('Build an ISO before scanning');
      return;
    }
    setBusy(true);
    setStatus('Running local scans...');
    try {
      const result = await invoke<any>('scan_artifact', { artifact: lastIso });
      setStatus(`Scan completed: ${result.report_json}`);
    } catch (error) {
      setStatus(`Scan failed: ${error}`);
    } finally {
      setBusy(false);
    }
  };

  const testIso = async () => {
    if (!lastIso) {
      setStatus('Build an ISO before testing');
      return;
    }
    setBusy(true);
    setStatus('Running local BIOS/UEFI smoke tests...');
    try {
      const result = await invoke<any>('test_iso', { iso: lastIso, bios: true, uefi: true });
      setStatus(`Test completed: passed=${result.passed}`);
    } catch (error) {
      setStatus(`Test failed: ${error}`);
    } finally {
      setBusy(false);
    }
  };

  const renderReport = async (format: 'html' | 'json') => {
    if (!lastBuildDir) {
      setStatus('Build an ISO before rendering a report');
      return;
    }
    setBusy(true);
    setStatus(`Rendering ${format} report...`);
    try {
      const path = await invoke<string>('render_report', { buildDir: lastBuildDir, format });
      setStatus(`Report written to ${path}`);
    } catch (error) {
      setStatus(`Report failed: ${error}`);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="app-shell">
      <header className="hero">
        <div>
          <h1>ForgeISO</h1>
          <p>Create Linux ISOs locally on bare metal. No agents, no endpoints, no Docker runtime.</p>
        </div>
        <div className="doctor-card">
          <h2>Doctor</h2>
          <p>{doctor ? `${doctor.host_os} / ${doctor.host_arch}` : 'Loading...'}</p>
          <p>{doctor?.linux_supported ? 'Linux build support enabled' : 'Linux host required for builds'}</p>
        </div>
      </header>

      <main className="layout">
        <section className="panel">
          <h2>Local Workflow</h2>
          <div className="field-grid">
            <label>
              Source ISO path or URL
              <input value={source} onChange={(e) => setSource(e.target.value)} placeholder="/path/to/base.iso or https://example/distro.iso" />
            </label>
            <label>
              Output directory
              <input value={outputDir} onChange={(e) => setOutputDir(e.target.value)} placeholder="./artifacts" />
            </label>
            <label>
              Build name
              <input value={buildName} onChange={(e) => setBuildName(e.target.value)} />
            </label>
            <label>
              Overlay directory
              <input value={overlayDir} onChange={(e) => setOverlayDir(e.target.value)} placeholder="Optional local file overlay" />
            </label>
            <label>
              Output label
              <input value={outputLabel} onChange={(e) => setOutputLabel(e.target.value)} placeholder="Optional ISO volume label" />
            </label>
            <label>
              Profile
              <select value={profile} onChange={(e) => setProfile(e.target.value)}>
                {profiles.map((item) => (
                  <option key={item.value} value={item.value}>{item.label}</option>
                ))}
              </select>
            </label>
          </div>

          <div className="actions">
            <button className="ghost" onClick={inspect} disabled={busy || !source.trim()}>Inspect</button>
            <button className="primary" onClick={build} disabled={busy || !canBuild}>Build ISO</button>
            <button className="ghost" onClick={scan} disabled={busy || !lastIso}>Scan</button>
            <button className="ghost" onClick={testIso} disabled={busy || !lastIso}>Test</button>
            <button className="ghost" onClick={() => renderReport('html')} disabled={busy || !lastBuildDir}>HTML Report</button>
            <button className="ghost" onClick={() => renderReport('json')} disabled={busy || !lastBuildDir}>JSON Report</button>
          </div>

          <p className="status">{status}</p>
        </section>

        <section className="panel">
          <h2>Detected ISO</h2>
          {inspection ? (
            <dl className="inspection-grid">
              <div><dt>Cached path</dt><dd>{inspection.source_path}</dd></div>
              <div><dt>Distro</dt><dd>{inspection.distro ?? 'unknown'}</dd></div>
              <div><dt>Release</dt><dd>{inspection.release ?? 'unknown'}</dd></div>
              <div><dt>Architecture</dt><dd>{inspection.architecture ?? 'unknown'}</dd></div>
              <div><dt>Volume ID</dt><dd>{inspection.volume_id ?? 'unknown'}</dd></div>
              <div><dt>SHA-256</dt><dd className="mono">{inspection.sha256}</dd></div>
            </dl>
          ) : (
            <p className="muted">Inspect a local ISO path or download URL to populate detected fields.</p>
          )}
          {inspection?.warnings?.length ? (
            <div className="warnings">
              {inspection.warnings.map((warning) => <p key={warning}>{warning}</p>)}
            </div>
          ) : null}
        </section>

        <section className="panel span-2">
          <h2>Operation Log</h2>
          <div className="log-console">
            {logs.length === 0 ? <p className="muted">Waiting for local engine events...</p> : null}
            {logs.map((entry, index) => (
              <p key={`${entry.ts}-${index}`}>
                <span className="mono">[{entry.ts}]</span> <span className="badge">{entry.phase}</span> {entry.message}
              </p>
            ))}
          </div>
          {lastIso ? <p className="status">Last ISO: {lastIso}</p> : null}
        </section>
      </main>
    </div>
  );
}

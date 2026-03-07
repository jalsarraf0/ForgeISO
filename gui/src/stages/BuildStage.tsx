import { useMemo, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { Dispatch } from 'react';
import type { BuildResult, Inspection, JobProgress } from '../types';
import type { AppAction } from '../store';
import { DISTRO_FAMILIES, capabilityClass, capabilityLabel } from '../distro';
import { Field, TextInput } from '../components/forms';
import { JobProgressCard } from '../components/JobProgress';

function MetaRow({ label, value }: { label: string; value: string }) {
  return (
    <>
      <span className="meta-key">{label}</span>
      <span className="meta-val">{value}</span>
    </>
  );
}

function fmtBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  if (n < 1024 * 1024 * 1024) return `${(n / (1024 * 1024)).toFixed(1)} MB`;
  return `${(n / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}

export function BuildStage({
  dispatch,
  isRunning,
  progress,
  lastSourceIso,
  lastOutputDir,
  lastDistro,
  buildResult,
}: {
  dispatch: Dispatch<AppAction>;
  isRunning: boolean;
  progress: JobProgress | null;
  lastSourceIso: string;
  lastOutputDir: string;
  lastDistro: string;
  buildResult: BuildResult | null;
}) {
  const [source, setSource] = useState(lastSourceIso);
  const [outputDir, setOutputDir] = useState(lastOutputDir || './artifacts');
  const [buildName, setBuildName] = useState('forgeiso-local');
  const [overlayDir, setOverlayDir] = useState('');
  const [outputLabel, setOutputLabel] = useState('');
  const [profile, setProfile] = useState('minimal');
  const [selectedDistro, setSelectedDistro] = useState(lastDistro || 'ubuntu');
  const [inspection, setInspection] = useState<Inspection | null>(null);
  const [statusMsg, setStatusMsg] = useState('');
  const [statusKind, setStatusKind] = useState<'ok' | 'err' | ''>('');

  const canBuild = useMemo(() => source.trim().length > 0 && outputDir.trim().length > 0, [source, outputDir]);

  const setStatus = (msg: string, kind: 'ok' | 'err' | '' = '') => {
    setStatusMsg(msg);
    setStatusKind(kind);
  };

  const inspect = async () => {
    if (!source.trim()) { setStatus('Source is required', 'err'); return; }
    dispatch({ type: 'JOB_START', stage: 'build', operation: 'Inspecting ISO…' });
    try {
      const result = await invoke<Inspection>('inspect_source', { source });
      setInspection(result);
      dispatch({ type: 'JOB_SUCCESS', stage: 'build' });
      setStatus('Inspection complete', 'ok');
    } catch (e) {
      dispatch({ type: 'JOB_ERROR', stage: 'build', error: String(e) });
      setStatus(`Inspect failed: ${e}`, 'err');
    }
  };

  const build = async () => {
    if (!canBuild) return;
    dispatch({ type: 'JOB_START', stage: 'build', operation: 'Building ISO…' });
    try {
      const result = await invoke<BuildResult>('build_local', {
        request: {
          source,
          outputDir,
          name: buildName,
          overlayDir: overlayDir.trim() || null,
          outputLabel: outputLabel.trim() || null,
          profile,
        },
      });
      dispatch({ type: 'SET_BUILD_RESULT', result, sourceIso: result.artifacts[0] ?? source, outputDir });
      dispatch({ type: 'JOB_SUCCESS', stage: 'build' });
      setStatus(`Build complete: ${result.artifacts[0] ?? result.output_dir}`, 'ok');
    } catch (e) {
      dispatch({ type: 'JOB_ERROR', stage: 'build', error: String(e) });
      setStatus(`Build failed: ${e}`, 'err');
    }
  };

  const scan = async () => {
    const iso = buildResult?.artifacts[0] ?? '';
    if (!iso) { setStatus('Build an ISO first', 'err'); return; }
    dispatch({ type: 'JOB_START', stage: 'build', operation: 'Scanning artifacts…' });
    try {
      await invoke('scan_artifact', { artifact: iso });
      dispatch({ type: 'JOB_SUCCESS', stage: 'build' });
      setStatus('Scan complete', 'ok');
    } catch (e) {
      dispatch({ type: 'JOB_ERROR', stage: 'build', error: String(e) });
      setStatus(`Scan failed: ${e}`, 'err');
    }
  };

  const testIso = async () => {
    const iso = buildResult?.artifacts[0] ?? '';
    if (!iso) { setStatus('Build an ISO first', 'err'); return; }
    dispatch({ type: 'JOB_START', stage: 'build', operation: 'Running BIOS/UEFI boot test…' });
    try {
      await invoke('test_iso', { iso, bios: true, uefi: true });
      dispatch({ type: 'JOB_SUCCESS', stage: 'build' });
      setStatus('Test complete', 'ok');
    } catch (e) {
      dispatch({ type: 'JOB_ERROR', stage: 'build', error: String(e) });
      setStatus(`Test failed: ${e}`, 'err');
    }
  };

  const report = async (format: 'html' | 'json') => {
    if (!buildResult) { setStatus('Build an ISO first', 'err'); return; }
    dispatch({ type: 'JOB_START', stage: 'build', operation: `Rendering ${format} report…` });
    try {
      await invoke<string>('render_report', { buildDir: buildResult.output_dir, format });
      dispatch({ type: 'JOB_SUCCESS', stage: 'build' });
      setStatus(`${format.toUpperCase()} report written`, 'ok');
    } catch (e) {
      dispatch({ type: 'JOB_ERROR', stage: 'build', error: String(e) });
      setStatus(`Report failed: ${e}`, 'err');
    }
  };

  const distro = DISTRO_FAMILIES.find((d) => d.id === selectedDistro) ?? DISTRO_FAMILIES[0];
  const hasArtifact = (buildResult?.artifacts.length ?? 0) > 0;

  return (
    <div className="main-content">
      {/* Inline progress — shown when a job is running */}
      {isRunning && progress && (
        <JobProgressCard progress={progress} />
      )}

      {/* Stage guidance */}
      <div className="stage-guidance">
        <span className="stage-guidance-step">Step 1</span>
        Select a Linux distribution and source ISO. Optionally inspect the image before building,
        then click <strong>Build ISO</strong> to package it.
      </div>

      {/* Distro selector */}
      <div className="card">
        <div className="card-header">
          <div>
            <h2>Target Distribution</h2>
            <p>Select the Linux distribution you are building for.</p>
          </div>
        </div>
        <div className="distro-grid">
          {DISTRO_FAMILIES.map((d) => {
            const isSupported = d.capabilities.build === 'supported' || d.capabilities.build === 'beta';
            return (
              <div
                key={d.id}
                className={[
                  'distro-card',
                  d.id === selectedDistro ? 'selected' : '',
                  !isSupported ? 'disabled' : '',
                ].filter(Boolean).join(' ')}
                onClick={() => { if (isSupported) { setSelectedDistro(d.id); dispatch({ type: 'SET_DISTRO', distro: d.id }); } }}
              >
                <div className="distro-icon">{d.iconChar}</div>
                <div className="distro-name">{d.label}</div>
                <div className="distro-desc">{d.description}</div>
              </div>
            );
          })}
        </div>

        {/* Capability matrix for selected distro */}
        <table className="capability-matrix">
          <thead>
            <tr>
              <th>Operation</th>
              <th>Status</th>
              <th>Method</th>
            </tr>
          </thead>
          <tbody>
            {(Object.entries(distro.capabilities) as [string, import('../distro').SupportLevel][]).map(([op, level]) => (
              <tr key={op}>
                <td style={{ color: 'var(--text-secondary)' }}>{op.charAt(0).toUpperCase() + op.slice(1)}</td>
                <td>
                  <span className={`badge ${capabilityClass(level)}`}>
                    {capabilityLabel(level)}
                  </span>
                </td>
                <td style={{ color: 'var(--text-muted)', fontSize: 'var(--text-xs)' }}>
                  {op === 'inject' ? distro.injectMethod : '—'}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      {/* Build config + Detected ISO — 2-column layout */}
      <div className="build-two-col">
        {/* Left: Build configuration form */}
        <div className="card">
          <div className="card-header">
            <div>
              <h2>Build Configuration</h2>
              <p>Fetch and package a base ISO from path or URL.</p>
            </div>
          </div>

          <div className="field-grid" style={{ marginBottom: 'var(--sp-4)' }}>
            <Field label="Source ISO path or URL *" className="span-2">
              <TextInput
                value={source}
                onChange={setSource}
                placeholder="/path/to/ubuntu.iso or https://releases.ubuntu.com/…"
                disabled={isRunning}
              />
            </Field>
            <Field label="Output directory *">
              <TextInput value={outputDir} onChange={setOutputDir} placeholder="./artifacts" disabled={isRunning} />
            </Field>
            <Field label="Build name">
              <TextInput value={buildName} onChange={setBuildName} disabled={isRunning} />
            </Field>
            <Field label="Overlay directory" hint="Optional filesystem overlay merged into ISO">
              <TextInput value={overlayDir} onChange={setOverlayDir} placeholder="/path/to/overlay" disabled={isRunning} />
            </Field>
            <Field label="Volume label" hint="Optional ISO label (≤32 chars)">
              <TextInput value={outputLabel} onChange={setOutputLabel} placeholder="FORGEISO" disabled={isRunning} />
            </Field>
            <Field label="Profile" className="span-2">
              <select value={profile} onChange={(e) => setProfile(e.target.value)} disabled={isRunning}>
                <option value="minimal">Minimal</option>
                <option value="desktop">Desktop</option>
              </select>
            </Field>
          </div>

          {/* Primary action */}
          <div className="btn-group" style={{ marginBottom: 'var(--sp-3)' }}>
            <button className="btn btn-primary btn-lg" type="button" onClick={build} disabled={isRunning || !canBuild}>
              {isRunning ? <><span className="spinner" /> Building…</> : 'Build ISO'}
            </button>
            <button className="btn" type="button" onClick={inspect} disabled={isRunning || !source.trim()}>
              Inspect
            </button>
          </div>

          {/* Secondary actions */}
          <div className="btn-group">
            <button className="btn btn-ghost btn-sm" type="button" onClick={scan} disabled={isRunning || !hasArtifact}>
              Scan
            </button>
            <button className="btn btn-ghost btn-sm" type="button" onClick={testIso} disabled={isRunning || !hasArtifact}>
              Test Boot
            </button>
            <button className="btn btn-ghost btn-sm" type="button" onClick={() => report('html')} disabled={isRunning || !buildResult}>
              HTML Report
            </button>
            <button className="btn btn-ghost btn-sm" type="button" onClick={() => report('json')} disabled={isRunning || !buildResult}>
              JSON Report
            </button>
          </div>

          {statusMsg && (
            <p className={`status-line${statusKind === 'ok' ? ' ok' : statusKind === 'err' ? ' err' : ''}`} style={{ marginTop: 'var(--sp-3)' }}>
              {statusMsg}
            </p>
          )}
        </div>

        {/* Right: Detected ISO metadata card */}
        <div className="card" style={{ display: 'flex', flexDirection: 'column' }}>
          <div className="card-header">
            <h2>Detected ISO</h2>
          </div>

          {!inspection && !buildResult?.iso ? (
            <div className="empty-state">
              <div className="empty-state-icon">🔍</div>
              <div className="empty-state-title">No ISO inspected yet</div>
              <div className="empty-state-body">
                Enter a source path or URL and click <strong>Inspect</strong> to see
                image metadata including distro, release, architecture, and checksum.
              </div>
            </div>
          ) : (
            <>
              <div className="meta-grid">
                {(() => {
                  const meta = inspection ?? buildResult?.iso;
                  if (!meta) return null;
                  return (
                    <>
                      <MetaRow label="Distro"   value={(meta as Inspection).distro ?? (buildResult?.iso?.distro ? String(buildResult.iso.distro) : 'Unknown')} />
                      <MetaRow label="Release"  value={(meta as Inspection).release ?? 'Unknown'} />
                      <MetaRow label="Arch"     value={(meta as Inspection).architecture ?? 'Unknown'} />
                      <MetaRow label="Volume ID" value={(meta as Inspection).volume_id ?? '—'} />
                      <MetaRow label="SHA-256"  value={(meta as Inspection).sha256} />
                      {(meta as Inspection).size_bytes !== undefined && (
                        <MetaRow label="Size" value={fmtBytes((meta as Inspection).size_bytes!)} />
                      )}
                      {(meta as Inspection).boot && (
                        <MetaRow
                          label="Boot"
                          value={[
                            (meta as Inspection).boot?.bios ? 'BIOS' : '',
                            (meta as Inspection).boot?.uefi ? 'UEFI' : '',
                          ].filter(Boolean).join(' / ') || 'Unknown'}
                        />
                      )}
                    </>
                  );
                })()}
              </div>
              {inspection?.warnings && inspection.warnings.length > 0 && (
                <div style={{ marginTop: 'var(--sp-3)' }}>
                  {inspection.warnings.map((w) => (
                    <div key={w} className="alert alert-amber" style={{ marginBottom: 'var(--sp-2)' }}>
                      <span className="alert-icon">⚠</span>
                      <span>{w}</span>
                    </div>
                  ))}
                </div>
              )}
            </>
          )}
        </div>
      </div>

      {/* Build result */}
      {buildResult && (
        <div className="card card-green">
          <div className="card-header">
            <h2>Build Complete</h2>
          </div>
          <div className="artifact-list">
            {buildResult.artifacts.map((a) => (
              <div key={String(a)} className="artifact-item">
                <span className="artifact-icon">💿</span>
                <span className="artifact-path">{String(a)}</span>
              </div>
            ))}
          </div>
          <div style={{ marginTop: 'var(--sp-4)' }} className="btn-group btn-group-right">
            <button
              className="btn btn-primary"
              type="button"
              onClick={() => dispatch({ type: 'ADVANCE_STAGE', from: 'build' })}
            >
              Continue to Inject →
            </button>
          </div>
        </div>
      )}
    </div>
  );
}

import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { Dispatch } from 'react';
import type { Iso9660Compliance, JobProgress, VerifyResult } from '../types';
import type { AppAction } from '../store';
import { Field, FileInput } from '../components/forms';
import { JobProgressCard } from '../components/JobProgress';
import { useStageAutoAdvance } from '../hooks';

function fmtBytes(n: number): string {
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  if (n < 1024 * 1024 * 1024) return `${(n / (1024 * 1024)).toFixed(1)} MB`;
  return `${(n / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}

function ComplianceRow({ ok, label, detail }: { ok: boolean; label: string; detail?: string }) {
  return (
    <div style={{ display: 'flex', alignItems: 'center', gap: 'var(--sp-3)', padding: '6px 0', borderBottom: '1px solid var(--border-subtle)' }}>
      <span style={{ fontSize: 14, flexShrink: 0 }}>{ok ? '✅' : '❌'}</span>
      <span style={{ color: 'var(--text-secondary)', fontSize: 'var(--text-sm)', flex: 1 }}>{label}</span>
      {detail && (
        <span className="mono" style={{ color: ok ? 'var(--green-light)' : 'var(--text-muted)', fontSize: 'var(--text-xs)' }}>{detail}</span>
      )}
    </div>
  );
}

export function VerifyStage({
  dispatch,
  isRunning,
  progress,
  lastInjectedIso,
  verifyResult,
  iso9660Result,
}: {
  dispatch: Dispatch<AppAction>;
  isRunning: boolean;
  progress: JobProgress | null;
  lastInjectedIso: string;
  verifyResult: VerifyResult | null;
  iso9660Result: Iso9660Compliance | null;
}) {
  const [source, setSource] = useState(lastInjectedIso);
  const [sumsUrl, setSumsUrl] = useState('');
  const [statusMsg, setStatusMsg] = useState('');
  const [statusKind, setStatusKind] = useState<'ok' | 'err' | ''>('');

  const setStatus = (msg: string, kind: 'ok' | 'err' | '' = '') => {
    setStatusMsg(msg);
    setStatusKind(kind);
  };

  const runVerify = async () => {
    if (!source.trim()) { setStatus('Source ISO is required', 'err'); return; }
    dispatch({ type: 'JOB_START', stage: 'verify', operation: 'Verifying SHA-256 checksum…' });
    try {
      const result = await invoke<VerifyResult>('verify_iso', {
        source,
        sumsUrl: sumsUrl.trim() || null,
      });
      dispatch({ type: 'SET_VERIFY_RESULT', result });
      dispatch({ type: 'JOB_SUCCESS', stage: 'verify' });
      setStatus(
        result.matched
          ? `Checksum matched: ${result.filename}`
          : `Checksum MISMATCH: ${result.filename}`,
        result.matched ? 'ok' : 'err',
      );
    } catch (e) {
      dispatch({ type: 'JOB_ERROR', stage: 'verify', error: String(e) });
      setStatus(`Verify failed: ${e}`, 'err');
    }
  };

  const runIso9660 = async () => {
    if (!source.trim()) { setStatus('Source ISO path is required', 'err'); return; }
    dispatch({ type: 'JOB_START', stage: 'verify', operation: 'Validating ISO-9660 structure…' });
    try {
      const result = await invoke<Iso9660Compliance>('validate_iso9660', { path: source });
      dispatch({ type: 'SET_ISO9660_RESULT', result });
      dispatch({ type: 'JOB_SUCCESS', stage: 'verify' });
      setStatus(
        result.compliant
          ? `ISO-9660 compliant — ${result.check_method}`
          : `ISO-9660 validation failed: ${result.error ?? 'unknown error'}`,
        result.compliant ? 'ok' : 'err',
      );
    } catch (e) {
      dispatch({ type: 'JOB_ERROR', stage: 'verify', error: String(e) });
      setStatus(`ISO-9660 check failed: ${e}`, 'err');
    }
  };

  const canProceed = verifyResult?.matched && iso9660Result?.compliant;
  const bothDone = verifyResult !== null && iso9660Result !== null;
  const { remaining: verifyRemaining, ref: verifyResultRef, skip: verifySkip } = useStageAutoAdvance(
    bothDone,
    () => dispatch({ type: 'ADVANCE_STAGE', from: 'verify' }),
  );

  return (
    <div className="main-content">
      {/* Inline progress */}
      {isRunning && progress && <JobProgressCard progress={progress} />}

      {/* Stage guidance */}
      <div className="stage-guidance">
        <span className="stage-guidance-step">Step 2</span>
        Verify the ISO checksum against official sources and confirm the image is a
        valid ISO-9660 filesystem. Both checks must pass before the image is safe to deploy.
      </div>

      {/* Checksum verification */}
      <div className="card" style={{ marginBottom: 'var(--sp-4)' }}>
        <div className="card-header">
          <div>
            <h2>SHA-256 Checksum</h2>
            <p>
              Verify an ISO against its official SHA256SUMS file.
              Auto-detected from ISO metadata for Ubuntu releases if not provided.
            </p>
          </div>
        </div>

        <div className="field-grid" style={{ marginBottom: 'var(--sp-4)' }}>
          <Field label="ISO path *" className="span-2">
            <FileInput
              value={source}
              onChange={setSource}
              placeholder="/path/to/ubuntu.iso or https://…/ubuntu.iso"
              disabled={isRunning}
              mode="iso"
            />
          </Field>
          <Field label="SHA256SUMS URL (optional — auto-detected for Ubuntu)" className="span-2">
            <input
              type="text"
              value={sumsUrl}
              onChange={(e) => setSumsUrl(e.target.value)}
              placeholder="https://releases.ubuntu.com/24.04/SHA256SUMS"
              disabled={isRunning}
            />
          </Field>
        </div>

        <div className="btn-group" style={{ marginBottom: 'var(--sp-3)' }}>
          <button
            className="btn btn-primary btn-lg"
            type="button"
            onClick={runVerify}
            disabled={isRunning || !source.trim()}
          >
            {isRunning ? <><span className="spinner" /> Verifying…</> : 'Verify Checksum'}
          </button>
        </div>

        {statusMsg && (
          <p className={`status-line${statusKind === 'ok' ? ' ok' : statusKind === 'err' ? ' err' : ''}`}>
            {statusMsg}
          </p>
        )}
      </div>

      {/* Checksum result banner */}
      {verifyResult && (
        <div className={`verify-match-banner ${verifyResult.matched ? 'matched' : 'mismatch'}`} style={{ marginBottom: 'var(--sp-4)' }}>
          <div className="verify-match-icon">{verifyResult.matched ? '✅' : '❌'}</div>
          <div style={{ flex: 1 }}>
            <div className="verify-match-title">
              {verifyResult.matched ? 'Checksum Matched' : 'Checksum Mismatch'}
            </div>
            <div className="verify-match-sub">{verifyResult.filename}</div>
          </div>
          <div style={{ display: 'flex', flexDirection: 'column', gap: 4, alignItems: 'flex-end' }}>
            <div style={{ fontSize: 'var(--text-xs)', color: 'var(--text-muted)' }}>Expected</div>
            <code className="verify-hash" style={{ fontSize: 10 }}>{verifyResult.expected.slice(0, 32)}…</code>
            <div style={{ fontSize: 'var(--text-xs)', color: 'var(--text-muted)', marginTop: 4 }}>Actual</div>
            <code className="verify-hash" style={{ fontSize: 10, color: verifyResult.matched ? 'var(--green-light)' : 'var(--red-light)' }}>
              {verifyResult.actual.slice(0, 32)}…
            </code>
          </div>
        </div>
      )}

      {/* ISO-9660 compliance section */}
      <div className="card" style={{ marginBottom: 'var(--sp-4)' }}>
        <div className="card-header">
          <div>
            <h2>ISO-9660 Compliance</h2>
            <p>
              Confirms the image has a valid ISO-9660 primary volume descriptor (CD001 at sector 16)
              and optionally checks El Torito boot catalog via xorriso.
            </p>
          </div>
          {iso9660Result && (
            <span className={`badge ${iso9660Result.compliant ? 'badge-green' : 'badge-red'}`}>
              {iso9660Result.compliant ? 'Compliant' : 'Non-Compliant'}
            </span>
          )}
        </div>

        <div className="btn-group" style={{ marginBottom: 'var(--sp-3)' }}>
          <button
            className="btn btn-primary"
            type="button"
            onClick={runIso9660}
            disabled={isRunning || !source.trim()}
          >
            {isRunning ? <><span className="spinner" /> Checking…</> : 'Validate ISO-9660'}
          </button>
        </div>

        {iso9660Result && (
          <div style={{ marginTop: 'var(--sp-3)' }}>
            {iso9660Result.error && (
              <div className="alert alert-red" style={{ marginBottom: 'var(--sp-3)' }}>
                <span className="alert-icon">✗</span>
                <span>{iso9660Result.error}</span>
              </div>
            )}
            <ComplianceRow
              ok={iso9660Result.compliant}
              label="ISO-9660 primary volume descriptor (CD001)"
              detail={iso9660Result.compliant ? 'Confirmed at sector 16' : 'Not found'}
            />
            {iso9660Result.volume_id && (
              <ComplianceRow
                ok
                label="Volume ID"
                detail={iso9660Result.volume_id}
              />
            )}
            <ComplianceRow
              ok={iso9660Result.size_bytes > 0}
              label="Image size"
              detail={iso9660Result.size_bytes > 0 ? fmtBytes(iso9660Result.size_bytes) : 'Unknown'}
            />
            <ComplianceRow
              ok={iso9660Result.el_torito_present}
              label="El Torito boot catalog"
              detail={iso9660Result.el_torito_present ? 'Present' : iso9660Result.check_method === 'iso9660_header' ? 'xorriso not available' : 'Not found'}
            />
            <ComplianceRow
              ok={iso9660Result.boot_bios}
              label="BIOS boot entry"
              detail={iso9660Result.check_method === 'iso9660_header' ? 'Requires xorriso' : iso9660Result.boot_bios ? 'Detected' : 'Not detected'}
            />
            <ComplianceRow
              ok={iso9660Result.boot_uefi}
              label="UEFI boot entry"
              detail={iso9660Result.check_method === 'iso9660_header' ? 'Requires xorriso' : iso9660Result.boot_uefi ? 'Detected' : 'Not detected'}
            />
            <div style={{ marginTop: 'var(--sp-3)', fontSize: 'var(--text-xs)', color: 'var(--text-muted)' }}>
              Check method: <span className="mono">{iso9660Result.check_method}</span>
            </div>
          </div>
        )}
      </div>

      {/* Continue */}
      {(verifyResult || iso9660Result) && (
        <div className="card card-green" ref={verifyResultRef}>
          <div className="card-header">
            <h2>{canProceed ? '✓ Verification Passed' : '⚠ Verification Complete'}</h2>
          </div>
          {!canProceed && (
            <p className="status-line warn" style={{ marginBottom: 'var(--sp-3)' }}>
              {verifyResult && !verifyResult.matched && 'Checksum has not passed. '}
              {iso9660Result && !iso9660Result.compliant && 'ISO-9660 check failed. '}
              You can still continue but deployment risk is elevated.
            </p>
          )}
          <div className="wizard-advance-row">
            {verifyRemaining !== null && (
              <span className="wizard-countdown">
                Continuing to Diff in {verifyRemaining}s…
              </span>
            )}
            <button className="btn btn-primary btn-lg" type="button" onClick={verifySkip}>
              Continue to Diff →
            </button>
          </div>
        </div>
      )}
    </div>
  );
}

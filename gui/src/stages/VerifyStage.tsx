import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { Dispatch } from 'react';
import type { VerifyResult } from '../types';
import type { AppAction } from '../store';
import { Field, TextInput } from '../components/forms';

export function VerifyStage({
  dispatch,
  isRunning,
  lastInjectedIso,
  verifyResult,
}: {
  dispatch: Dispatch<AppAction>;
  isRunning: boolean;
  lastInjectedIso: string;
  verifyResult: VerifyResult | null;
}) {
  const [source, setSource] = useState(lastInjectedIso);
  const [sumsUrl, setSumsUrl] = useState('');
  const [statusMsg, setStatusMsg] = useState('');
  const [statusKind, setStatusKind] = useState<'ok' | 'err' | ''>('');

  const setStatus = (msg: string, kind: 'ok' | 'err' | '' = '') => {
    setStatusMsg(msg);
    setStatusKind(kind);
  };

  const run = async () => {
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

  return (
    <div className="main-content">
      <div className="card" style={{ marginBottom: 'var(--sp-4)' }}>
        <div className="card-header">
          <div>
            <h2>SHA-256 Verification</h2>
            <p>
              Verify an ISO against its official SHA256SUMS file. The checksum URL is
              auto-detected from ISO metadata for Ubuntu releases if not provided.
            </p>
          </div>
        </div>

        <div className="field-grid" style={{ marginBottom: 'var(--sp-4)' }}>
          <Field label="ISO path or URL *" className="span-2">
            <TextInput
              value={source}
              onChange={setSource}
              placeholder="/path/to/ubuntu.iso or https://…/ubuntu.iso"
              disabled={isRunning}
            />
          </Field>
          <Field label="SHA256SUMS URL (optional — auto-detected for Ubuntu)" className="span-2">
            <TextInput
              value={sumsUrl}
              onChange={setSumsUrl}
              placeholder="https://releases.ubuntu.com/24.04/SHA256SUMS"
              disabled={isRunning}
            />
          </Field>
        </div>

        <div className="btn-group" style={{ marginBottom: 'var(--sp-3)' }}>
          <button
            className="btn btn-primary btn-lg"
            type="button"
            onClick={run}
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

      {/* Result banner */}
      {verifyResult && (
        <>
          <div className={`verify-match-banner ${verifyResult.matched ? 'matched' : 'mismatch'}`}>
            <div className="verify-match-icon">{verifyResult.matched ? '✅' : '❌'}</div>
            <div>
              <div className="verify-match-title">
                {verifyResult.matched ? 'Checksum Matched' : 'Checksum Mismatch'}
              </div>
              <div className="verify-match-sub">{verifyResult.filename}</div>
            </div>
          </div>

          <div className="card" style={{ marginBottom: 'var(--sp-4)' }}>
            <div style={{ marginBottom: 'var(--sp-3)' }}>
              <p className="sidebar-section-title" style={{ marginBottom: 'var(--sp-2)' }}>Expected</p>
              <div className="verify-hash-row">
                <span className="verify-hash">{verifyResult.expected}</span>
              </div>
            </div>
            <div>
              <p className="sidebar-section-title" style={{ marginBottom: 'var(--sp-2)' }}>Actual</p>
              <div className="verify-hash-row">
                <span
                  className="verify-hash"
                  style={{ color: verifyResult.matched ? 'var(--green-light)' : 'var(--red-light)' }}
                >
                  {verifyResult.actual}
                </span>
              </div>
            </div>
          </div>

          {verifyResult.matched && (
            <div className="btn-group btn-group-right">
              <button
                className="btn btn-primary"
                type="button"
                onClick={() => dispatch({ type: 'ADVANCE_STAGE', from: 'verify' })}
              >
                Continue to Diff →
              </button>
            </div>
          )}
        </>
      )}
    </div>
  );
}

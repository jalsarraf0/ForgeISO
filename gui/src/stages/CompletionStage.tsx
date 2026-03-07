import type React from 'react';
import type { Dispatch } from 'react';
import type { BuildResult, InjectResult, Iso9660Compliance, IsoDiff, VerifyResult } from '../types';
import type { AppAction } from '../store';

function ArtifactItem({ icon, path }: { icon: string; path: string }) {
  return (
    <div className="artifact-item">
      <span className="artifact-icon">{icon}</span>
      <span className="artifact-path">{path}</span>
    </div>
  );
}

function SummaryCard({
  title,
  color,
  children,
}: {
  title: string;
  color: 'blue' | 'green' | 'amber' | 'violet';
  children: React.ReactNode;
}) {
  return (
    <div className={`card card-${color}`} style={{ marginBottom: 'var(--sp-4)' }}>
      <div className="card-header">
        <h2>{title}</h2>
      </div>
      {children}
    </div>
  );
}

export function CompletionStage({
  dispatch,
  buildResult,
  injectResult,
  verifyResult,
  diffResult,
  iso9660Result,
}: {
  dispatch: Dispatch<AppAction>;
  buildResult: BuildResult | null;
  injectResult: InjectResult | null;
  verifyResult: VerifyResult | null;
  diffResult: IsoDiff | null;
  iso9660Result: Iso9660Compliance | null;
}) {
  const hasAnything = buildResult || injectResult || verifyResult || diffResult || iso9660Result;

  const resetAll = () => dispatch({ type: 'RESET_ALL' });

  return (
    <div className="main-content">
      {/* Hero */}
      <div className="completion-hero">
        <div className="completion-icon">{hasAnything ? '🎉' : '📋'}</div>
        <h1 className="completion-title">
          {hasAnything ? 'Pipeline Complete' : 'Nothing run yet'}
        </h1>
        <p className="completion-sub">
          {hasAnything
            ? 'All artifacts are ready. Review the results below.'
            : 'Run one or more pipeline stages to see results here.'}
        </p>
      </div>

      {/* Build result */}
      {buildResult && (
        <SummaryCard title="Build Artifacts" color="blue">
          <div className="artifact-list">
            {buildResult.artifacts.map((a) => (
              <ArtifactItem key={a} icon="💿" path={a} />
            ))}
            {buildResult.report_html && (
              <ArtifactItem icon="📄" path={buildResult.report_html} />
            )}
            {buildResult.report_json && (
              <ArtifactItem icon="🗂️" path={buildResult.report_json} />
            )}
          </div>
          {buildResult.iso && (
            <div className="meta-grid" style={{ marginTop: 'var(--sp-4)' }}>
              <span className="meta-key">Distro</span>
              <span className="meta-val">{buildResult.iso.distro ?? 'Unknown'}</span>
              <span className="meta-key">Release</span>
              <span className="meta-val">{buildResult.iso.release ?? 'Unknown'}</span>
              <span className="meta-key">Arch</span>
              <span className="meta-val">{buildResult.iso.architecture ?? 'Unknown'}</span>
              <span className="meta-key">SHA-256</span>
              <span className="meta-val">{buildResult.iso.sha256}</span>
            </div>
          )}
        </SummaryCard>
      )}

      {/* Inject result */}
      {injectResult && (
        <SummaryCard title="Inject Artifacts" color="violet">
          <div className="artifact-list">
            {injectResult.artifacts.map((a) => (
              <ArtifactItem key={a} icon="💉" path={a} />
            ))}
            {injectResult.report_html && (
              <ArtifactItem icon="📄" path={injectResult.report_html} />
            )}
          </div>
        </SummaryCard>
      )}

      {/* Verify result */}
      {verifyResult && (
        <SummaryCard title="Verification" color={verifyResult.matched ? 'green' : 'amber'}>
          <div className={`verify-match-banner ${verifyResult.matched ? 'matched' : 'mismatch'}`}>
            <div className="verify-match-icon">{verifyResult.matched ? '✅' : '❌'}</div>
            <div>
              <div className="verify-match-title">
                {verifyResult.matched ? 'Integrity Confirmed' : 'Integrity Check Failed'}
              </div>
              <div className="verify-match-sub">{verifyResult.filename}</div>
            </div>
          </div>
        </SummaryCard>
      )}

      {/* ISO-9660 compliance result */}
      {iso9660Result && (
        <SummaryCard title="ISO-9660 Compliance" color={iso9660Result.compliant ? 'green' : 'amber'}>
          <div style={{ display: 'flex', alignItems: 'center', gap: 'var(--sp-3)', marginBottom: 'var(--sp-3)' }}>
            <span style={{ fontSize: 22 }}>{iso9660Result.compliant ? '✅' : '❌'}</span>
            <div>
              <div style={{ fontWeight: 600, color: 'var(--text-primary)' }}>
                {iso9660Result.compliant ? 'Compliant' : 'Non-Compliant'}
              </div>
              {iso9660Result.volume_id && (
                <div style={{ fontSize: 'var(--text-xs)', color: 'var(--text-muted)' }}>
                  Volume: <span className="mono">{iso9660Result.volume_id}</span>
                </div>
              )}
            </div>
            <div style={{ marginLeft: 'auto', display: 'flex', gap: 'var(--sp-2)' }}>
              {iso9660Result.boot_bios && <span className="badge badge-blue">BIOS</span>}
              {iso9660Result.boot_uefi && <span className="badge badge-blue">UEFI</span>}
            </div>
          </div>
          {iso9660Result.error && (
            <div className="alert alert-red" style={{ marginTop: 'var(--sp-2)' }}>
              <span className="alert-icon">✗</span>
              <span>{iso9660Result.error}</span>
            </div>
          )}
        </SummaryCard>
      )}

      {/* Diff summary */}
      {diffResult && (
        <SummaryCard title="ISO Diff Summary" color="amber">
          <div className="diff-summary-row">
            <div className="diff-stat-card added">
              <div className="diff-stat-num">{diffResult.added.length}</div>
              <div className="diff-stat-label">Added</div>
            </div>
            <div className="diff-stat-card removed">
              <div className="diff-stat-num">{diffResult.removed.length}</div>
              <div className="diff-stat-label">Removed</div>
            </div>
            <div className="diff-stat-card modified">
              <div className="diff-stat-num">{diffResult.modified.length}</div>
              <div className="diff-stat-label">Modified</div>
            </div>
            <div className="diff-stat-card unchanged">
              <div className="diff-stat-num">{diffResult.unchanged}</div>
              <div className="diff-stat-label">Unchanged</div>
            </div>
          </div>
        </SummaryCard>
      )}

      {/* Actions */}
      <div className="completion-actions">
        <button className="btn btn-ghost" type="button" onClick={() => dispatch({ type: 'SET_STAGE', stage: 'inject' })}>
          ← Back to Inject
        </button>
        <button className="btn btn-ghost" type="button" onClick={() => dispatch({ type: 'SET_STAGE', stage: 'verify' })}>
          ← Back to Verify
        </button>
        <button className="btn btn-ghost" type="button" onClick={() => dispatch({ type: 'SET_STAGE', stage: 'diff' })}>
          ← Back to Diff
        </button>
        <button className="btn btn-ghost" type="button" onClick={() => dispatch({ type: 'SET_STAGE', stage: 'build' })}>
          ← Back to Build
        </button>
        <button className="btn btn-danger" type="button" onClick={resetAll}>
          Start New Pipeline
        </button>
      </div>
    </div>
  );
}

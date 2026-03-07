import { useMemo, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { Dispatch } from 'react';
import type { IsoDiff, JobProgress } from '../types';
import type { AppAction } from '../store';
import { Field, FileInput } from '../components/forms';
import { JobProgressCard } from '../components/JobProgress';
import { useStageAutoAdvance } from '../hooks';

type Filter = 'all' | 'added' | 'removed' | 'modified';

function fmtSize(bytes?: number): string {
  if (bytes === undefined || bytes === null) return '';
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

export function DiffStage({
  dispatch,
  isRunning,
  progress,
  lastSourceIso,
  lastInjectedIso,
  diffResult,
}: {
  dispatch: Dispatch<AppAction>;
  isRunning: boolean;
  progress: JobProgress | null;
  lastSourceIso: string;
  lastInjectedIso: string;
  diffResult: IsoDiff | null;
}) {
  const [base, setBase] = useState(lastSourceIso);
  const [target, setTarget] = useState(lastInjectedIso);
  const [filter, setFilter] = useState<Filter>('all');
  const [search, setSearch] = useState('');
  const [statusMsg, setStatusMsg] = useState('');
  const [statusKind, setStatusKind] = useState<'ok' | 'err' | ''>('');

  const setStatus = (msg: string, kind: 'ok' | 'err' | '' = '') => {
    setStatusMsg(msg);
    setStatusKind(kind);
  };

  const { remaining: diffRemaining, ref: diffResultRef, skip: diffSkip } = useStageAutoAdvance(
    diffResult !== null,
    () => dispatch({ type: 'ADVANCE_STAGE', from: 'diff' }),
  );

  const run = async () => {
    if (!base.trim() || !target.trim()) {
      setStatus('Both ISO paths are required', 'err');
      return;
    }
    dispatch({ type: 'JOB_START', stage: 'diff', operation: 'Comparing ISO contents…' });
    try {
      const result = await invoke<IsoDiff>('diff_isos', { base, target });
      dispatch({ type: 'SET_DIFF_RESULT', result });
      dispatch({ type: 'JOB_SUCCESS', stage: 'diff' });
      const total = result.added.length + result.removed.length + result.modified.length;
      setStatus(`Diff complete — ${total} changed files`, 'ok');
    } catch (e) {
      dispatch({ type: 'JOB_ERROR', stage: 'diff', error: String(e) });
      setStatus(`Diff failed: ${e}`, 'err');
    }
  };

  const rows = useMemo(() => {
    if (!diffResult) return [];
    const all: { type: 'added' | 'removed' | 'modified'; path: string; size?: number }[] = [
      ...diffResult.added.map((p) => ({ type: 'added' as const, path: p })),
      ...diffResult.removed.map((p) => ({ type: 'removed' as const, path: p })),
      ...diffResult.modified.map((e) => ({ type: 'modified' as const, path: e.path, size: e.target_size })),
    ];

    return all.filter((r) => {
      if (filter !== 'all' && r.type !== filter) return false;
      if (search && !r.path.toLowerCase().includes(search.toLowerCase())) return false;
      return true;
    });
  }, [diffResult, filter, search]);

  const counts = useMemo(() => ({
    added:    diffResult?.added.length ?? 0,
    removed:  diffResult?.removed.length ?? 0,
    modified: diffResult?.modified.length ?? 0,
    unchanged: diffResult?.unchanged ?? 0,
  }), [diffResult]);

  const filterBtnClass = (f: Filter | 'all') =>
    `diff-filter-btn${filter === f ? ` active-${f}` : ''}`;

  return (
    <div className="main-content">
      {/* Inline progress */}
      {isRunning && progress && <JobProgressCard progress={progress} />}

      {/* Stage guidance */}
      <div className="stage-guidance">
        <span className="stage-guidance-step">Step 3</span>
        Compare the original and injected ISOs to see exactly what changed — added, removed, and modified files.
      </div>

      <div className="card" style={{ marginBottom: 'var(--sp-4)' }}>
        <div className="card-header">
          <div>
            <h2>ISO Diff</h2>
            <p>Compare two ISOs to see added, removed, and modified files.</p>
          </div>
        </div>

        <div className="field-grid" style={{ marginBottom: 'var(--sp-4)' }}>
          <Field label="Base ISO path">
            <FileInput value={base} onChange={setBase} placeholder="/path/to/original.iso" disabled={isRunning} mode="iso" />
          </Field>
          <Field label="Target ISO path">
            <FileInput value={target} onChange={setTarget} placeholder="/path/to/modified.iso" disabled={isRunning} mode="iso" />
          </Field>
        </div>

        <div className="btn-group" style={{ marginBottom: 'var(--sp-3)' }}>
          <button
            className="btn btn-primary btn-lg"
            type="button"
            onClick={run}
            disabled={isRunning || !base.trim() || !target.trim()}
          >
            {isRunning ? <><span className="spinner" /> Comparing…</> : 'Compare ISOs'}
          </button>
        </div>

        {statusMsg && (
          <p className={`status-line${statusKind === 'ok' ? ' ok' : statusKind === 'err' ? ' err' : ''}`}>
            {statusMsg}
          </p>
        )}
      </div>

      {diffResult && (
        <>
          {/* Summary stats */}
          <div className="diff-summary-row" style={{ marginBottom: 'var(--sp-4)' }}>
            <div className="diff-stat-card added">
              <div className="diff-stat-num">{counts.added}</div>
              <div className="diff-stat-label">Added</div>
            </div>
            <div className="diff-stat-card removed">
              <div className="diff-stat-num">{counts.removed}</div>
              <div className="diff-stat-label">Removed</div>
            </div>
            <div className="diff-stat-card modified">
              <div className="diff-stat-num">{counts.modified}</div>
              <div className="diff-stat-label">Modified</div>
            </div>
            <div className="diff-stat-card unchanged">
              <div className="diff-stat-num">{counts.unchanged}</div>
              <div className="diff-stat-label">Unchanged</div>
            </div>
          </div>

          {/* Filter + search */}
          <div className="card" style={{ marginBottom: 'var(--sp-4)' }}>
            <div className="diff-filter-row">
              {(['all', 'added', 'removed', 'modified'] as const).map((f) => (
                <button
                  key={f}
                  className={filterBtnClass(f)}
                  type="button"
                  onClick={() => setFilter(f)}
                >
                  {f === 'all'
                    ? `All (${counts.added + counts.removed + counts.modified})`
                    : `${f.charAt(0).toUpperCase() + f.slice(1)} (${counts[f]})`}
                </button>
              ))}
            </div>

            <div className="diff-search">
              <input
                value={search}
                onChange={(e) => setSearch(e.target.value)}
                placeholder="Filter by path…"
              />
            </div>

            <div className="diff-list">
              {rows.length === 0 ? (
                <div style={{ padding: 'var(--sp-4)', color: 'var(--text-muted)', fontSize: 'var(--text-sm)' }}>
                  No matching entries.
                </div>
              ) : (
                rows.map((row) => (
                  <div key={`${row.type}-${row.path}`} className={`diff-row ${row.type}`}>
                    <div className="diff-tag">
                      {row.type === 'added' ? 'A' : row.type === 'removed' ? 'R' : 'M'}
                    </div>
                    <span className="diff-path">{row.path}</span>
                    {row.size !== undefined && (
                      <span className="diff-size">{fmtSize(row.size)}</span>
                    )}
                  </div>
                ))
              )}
            </div>
          </div>

          <div className="card card-green" ref={diffResultRef}>
            <div className="card-header">
              <h2>✓ Diff Complete</h2>
            </div>
            <div className="wizard-advance-row">
              {diffRemaining !== null && (
                <span className="wizard-countdown">
                  Continuing to Build in {diffRemaining}s…
                </span>
              )}
              <button className="btn btn-primary btn-lg" type="button" onClick={diffSkip}>
                Continue to Build →
              </button>
            </div>
          </div>
        </>
      )}
    </div>
  );
}

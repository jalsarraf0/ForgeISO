import { useEffect, useState } from 'react';
import type { JobProgress } from '../types';

function fmtBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  if (n < 1024 * 1024 * 1024) return `${(n / (1024 * 1024)).toFixed(1)} MB`;
  return `${(n / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}

function fmtElapsed(startedAt: Date): string {
  const s = Math.floor((Date.now() - startedAt.getTime()) / 1000);
  if (s < 60) return `${s}s`;
  const m = Math.floor(s / 60);
  return `${m}m ${s % 60}s`;
}

export function JobProgressCard({ progress }: { progress: JobProgress }) {
  const [, tick] = useState(0);

  useEffect(() => {
    if (progress.status !== 'running') return;
    const id = setInterval(() => tick((n) => n + 1), 1000);
    return () => clearInterval(id);
  }, [progress.status]);

  const isRunning = progress.status === 'running';
  const hasPercent = progress.percent !== null && progress.percent !== undefined;
  const hasByteProg =
    progress.bytesDone !== null &&
    progress.bytesTotal !== null &&
    progress.bytesTotal !== undefined &&
    progress.bytesDone !== undefined;

  return (
    <div className="progress-card">
      <div className="progress-header">
        <div className="progress-title">
          {isRunning && <span className="spinner" />}
          {progress.currentOperation}
        </div>
        {isRunning && (
          <span className="progress-elapsed">{fmtElapsed(progress.startedAt)}</span>
        )}
      </div>

      {progress.substage && (
        <div className="progress-substage">
          <span className="spinner" style={{ width: 10, height: 10 }} />
          {progress.substage}
        </div>
      )}

      <div className="progress-bar-track">
        {hasPercent ? (
          <div
            className="progress-bar-fill"
            style={{ width: `${progress.percent}%` }}
          />
        ) : (
          <div className="progress-bar-fill progress-bar-indeterminate" />
        )}
      </div>

      <div className="progress-meta">
        <span>{hasPercent ? `${Math.round(progress.percent!)}%` : 'Working…'}</span>
        {hasByteProg && (
          <span className="progress-bytes">
            {fmtBytes(progress.bytesDone!)} / {fmtBytes(progress.bytesTotal!)}
          </span>
        )}
      </div>
    </div>
  );
}

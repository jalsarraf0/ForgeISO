import { useEffect, useState } from 'react';
import type { JobProgress } from '../types';

const BAR_SLOTS = 32;

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

// Renders a pipe-style progress bar:  |████████████░░░░░░░░|  55%
function PipeBar({ percent, bytesDone, bytesTotal }: {
  percent: number | null;
  bytesDone: number | null;
  bytesTotal: number | null;
}) {
  const [frame, setFrame] = useState(0);

  useEffect(() => {
    if (percent !== null) return;
    const id = setInterval(() => setFrame((f) => (f + 1) % BAR_SLOTS), 120);
    return () => clearInterval(id);
  }, [percent]);

  const hasPct = percent !== null && percent !== undefined;
  const hasBytes = bytesDone !== null && bytesTotal !== null &&
    bytesDone !== undefined && bytesTotal !== undefined && bytesTotal > 0;

  if (hasPct) {
    const filled = Math.round((percent! / 100) * BAR_SLOTS);
    const empty  = BAR_SLOTS - filled;
    const pctStr = `${Math.round(percent!)}%`.padStart(4, ' ');
    return (
      <div className="pipe-bar">
        <span className="pipe-bracket">|</span>
        <span className="pipe-filled">{'█'.repeat(filled)}</span>
        <span className="pipe-empty">{'░'.repeat(empty)}</span>
        <span className="pipe-bracket">|</span>
        <span className="pipe-pct">{pctStr}</span>
        {hasBytes && (
          <span className="pipe-bytes">
            {fmtBytes(bytesDone!)} / {fmtBytes(bytesTotal!)}
          </span>
        )}
      </div>
    );
  }

  // Indeterminate — sliding block animation using frame counter
  const inner = Array.from({ length: BAR_SLOTS }, (_, i) => {
    const dist = Math.min(
      Math.abs(i - frame),
      Math.abs(i - frame + BAR_SLOTS),
      Math.abs(i - frame - BAR_SLOTS),
    );
    return dist <= 3 ? '▓' : '░';
  }).join('');

  return (
    <div className="pipe-bar">
      <span className="pipe-bracket">|</span>
      <span className="pipe-indeterminate">{inner}</span>
      <span className="pipe-bracket">|</span>
      <span className="pipe-pct pipe-pct-working">···</span>
    </div>
  );
}

export function JobProgressCard({ progress }: { progress: JobProgress }) {
  const [, tick] = useState(0);

  useEffect(() => {
    if (progress.status !== 'running') return;
    const id = setInterval(() => tick((n) => n + 1), 1000);
    return () => clearInterval(id);
  }, [progress.status]);

  const isRunning = progress.status === 'running';

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

      <PipeBar
        percent={progress.percent ?? null}
        bytesDone={progress.bytesDone ?? null}
        bytesTotal={progress.bytesTotal ?? null}
      />
    </div>
  );
}

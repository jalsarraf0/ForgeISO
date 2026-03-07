import { useEffect, useRef, useState } from 'react';
import type { LogEntry } from '../types';

function phaseBadgeClass(phase: string): string {
  const p = phase.toLowerCase();
  const known = ['build', 'inject', 'verify', 'diff', 'download', 'doctor', 'complete', 'scan', 'inspect'];
  return known.includes(p) ? `badge-phase badge-phase-${p}` : 'badge-phase badge-phase-default';
}

function levelClass(level: string): string {
  const l = level.toLowerCase();
  if (l === 'error') return 'error';
  if (l === 'warn') return 'warn';
  if (l === 'debug') return 'debug';
  return 'info';
}

export function LogPanel({
  logs,
  onClear,
}: {
  logs: LogEntry[];
  onClear: () => void;
}) {
  const [expanded, setExpanded] = useState(true);
  const [filter, setFilter] = useState<'all' | 'info' | 'warn' | 'error'>('all');
  const bodyRef = useRef<HTMLDivElement>(null);
  const atBottomRef = useRef(true);

  useEffect(() => {
    const el = bodyRef.current;
    if (!el || !atBottomRef.current) return;
    el.scrollTop = el.scrollHeight;
  }, [logs]);

  const handleScroll = () => {
    const el = bodyRef.current;
    if (!el) return;
    atBottomRef.current = el.scrollHeight - el.scrollTop - el.clientHeight < 40;
  };

  const visible =
    filter === 'all'
      ? logs
      : logs.filter((e) => e.level.toLowerCase() === filter);

  return (
    <div className="log-bar" style={{ maxHeight: expanded ? 220 : 38 }}>
      <div className="log-bar-header" onClick={() => setExpanded((v) => !v)}>
        <h3>Engine Log</h3>
        <span className="log-bar-count">{logs.length}</span>
        <div className="log-bar-controls" onClick={(e) => e.stopPropagation()}>
          {(['all', 'info', 'warn', 'error'] as const).map((f) => (
            <button
              key={f}
              className={`btn btn-ghost btn-sm${filter === f ? ' log-filter-active' : ''}`}
              style={{
                color: filter === f
                  ? f === 'error' ? 'var(--red-light)'
                    : f === 'warn' ? 'var(--amber-light)'
                    : 'var(--blue-light)'
                  : undefined,
                borderColor: filter === f ? 'currentColor' : undefined,
              }}
              type="button"
              onClick={() => setFilter(f)}
            >
              {f}
            </button>
          ))}
          <button className="btn btn-ghost btn-sm" type="button" onClick={onClear}>
            clear
          </button>
        </div>
      </div>

      {expanded && (
        <div className="log-body" ref={bodyRef} onScroll={handleScroll}>
          {visible.length === 0 ? (
            <div className="log-empty">Waiting for engine events…</div>
          ) : (
            visible.map((entry, i) => (
              <div
                key={`${entry.ts}-${i}`}
                className={`log-line ${levelClass(entry.level)}`}
              >
                <span className="log-ts">{entry.ts.slice(11, 19)}</span>
                <span className={phaseBadgeClass(entry.phase)}>{entry.phase}</span>
                <span className="log-msg">{entry.message}</span>
              </div>
            ))
          )}
        </div>
      )}
    </div>
  );
}

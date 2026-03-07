import type { DoctorReport, JobProgress } from '../types';
import { JobProgressCard } from './JobProgress';

export function DoctorSidebar({
  doctor,
  progress,
}: {
  doctor: DoctorReport | null;
  progress: JobProgress | null;
}) {
  return (
    <aside className="sidebar">
      {/* Active job progress */}
      {progress && progress.status === 'running' && (
        <div className="sidebar-section">
          <p className="sidebar-section-title">Active Job</p>
          <JobProgressCard progress={progress} />
        </div>
      )}

      {/* System info */}
      <div className="sidebar-section">
        <p className="sidebar-section-title">System</p>
        {doctor ? (
          <div className="card card-sm" style={{ marginBottom: 'var(--sp-2)' }}>
            <div className="meta-grid">
              <span className="meta-key">OS</span>
              <span className="meta-val">{doctor.host_os}</span>
              <span className="meta-key">Arch</span>
              <span className="meta-val">{doctor.host_arch}</span>
              <span className="meta-key">Linux</span>
              <span
                className="meta-val"
                style={{ color: doctor.linux_supported ? 'var(--green-light)' : 'var(--red-light)' }}
              >
                {doctor.linux_supported ? 'Supported' : 'Not supported'}
              </span>
            </div>
          </div>
        ) : (
          <div className="card card-sm" style={{ color: 'var(--text-muted)', fontSize: 'var(--text-sm)' }}>
            Running doctor check…
          </div>
        )}
      </div>

      {/* Tool availability */}
      {doctor && (
        <div className="sidebar-section">
          <p className="sidebar-section-title">Tools</p>
          <div className="tool-grid">
            {Object.entries(doctor.tooling).map(([tool, ok]) => (
              <div key={tool} className={`tool-item ${ok ? 'ok' : 'warn'}`}>
                <span className="tool-dot" />
                {tool}
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Warnings */}
      {doctor && doctor.warnings.length > 0 && (
        <div className="sidebar-section">
          <p className="sidebar-section-title">Warnings</p>
          {doctor.warnings.map((w, i) => (
            <div key={i} className="alert alert-amber" style={{ marginBottom: 'var(--sp-2)' }}>
              <span className="alert-icon">⚠</span>
              <span>{w}</span>
            </div>
          ))}
        </div>
      )}

      {/* Empty state */}
      {!doctor && !progress && (
        <div style={{ color: 'var(--text-muted)', fontSize: 'var(--text-sm)', padding: 'var(--sp-2) 0' }}>
          System check running…
        </div>
      )}
    </aside>
  );
}

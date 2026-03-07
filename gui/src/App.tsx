import { useEffect, useReducer } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

import type { LogEntry } from './types';
import { appReducer, initialState } from './store';

import { Stepper } from './components/Stepper';
import { LogPanel } from './components/LogPanel';
import { DoctorSidebar } from './components/DoctorSidebar';

import { BuildStage } from './stages/BuildStage';
import { InjectStage } from './stages/InjectStage';
import { VerifyStage } from './stages/VerifyStage';
import { DiffStage } from './stages/DiffStage';
import { CompletionStage } from './stages/CompletionStage';

// ── Root App ──────────────────────────────────────────────────────────────────

export function App() {
  const [state, dispatch] = useReducer(appReducer, initialState);

  // Boot: start event stream and run doctor
  useEffect(() => {
    let unlisten: (() => void) | undefined;

    const boot = async () => {
      unlisten = await listen<LogEntry>('forgeiso-log', (event) => {
        const entry = event.payload as LogEntry;
        dispatch({ type: 'APPEND_LOG', entry });

        // Mirror progress events into store
        if (entry.percent !== undefined || entry.substage !== undefined) {
          dispatch({
            type: 'JOB_PROGRESS',
            substage: entry.substage ?? null,
            percent: entry.percent ?? null,
            bytesDone: entry.bytesDone ?? null,
            bytesTotal: entry.bytesTotal ?? null,
            operation: entry.message,
          });
        }
      });

      await invoke('start_event_stream');

      const doctorResult = await invoke('doctor');
      dispatch({ type: 'SET_DOCTOR', doctor: doctorResult as import('./types').DoctorReport });
    };

    boot().catch((e) => {
      dispatch({
        type: 'APPEND_LOG',
        entry: {
          ts: new Date().toISOString(),
          phase: 'Configure',
          level: 'Error',
          message: `Startup error: ${e}`,
        },
      });
    });

    return () => { unlisten?.(); };
  }, []);

  // Top-bar chip status
  const chipStatus = state.isRunning
    ? 'running'
    : state.progress?.status === 'success'
    ? 'success'
    : state.progress?.status === 'error'
    ? 'error'
    : 'idle';

  const chipLabel = state.isRunning
    ? state.progress?.currentOperation ?? 'Running…'
    : state.progress?.status === 'success'
    ? 'Complete'
    : state.progress?.status === 'error'
    ? 'Error'
    : 'Ready';

  return (
    <div className="app-shell">
      {/* Top bar */}
      <header className="topbar">
        <div className="topbar-brand">
          <div className="topbar-logo">F</div>
          <span className="topbar-name">ForgeISO</span>
          <span className="topbar-subtitle">ISO Build & Configuration Pipeline</span>
        </div>
        <div className="topbar-spacer" />
        <div className={`topbar-status-chip ${chipStatus}`}>
          <div className="topbar-status-dot" />
          {chipLabel}
        </div>
      </header>

      {/* Pipeline stepper */}
      <Stepper
        activeStage={state.activeStage}
        stageStatus={state.stageStatus}
        onStepClick={(stage) => dispatch({ type: 'SET_STAGE', stage })}
      />

      {/* Active stage */}
      {state.activeStage === 'build' && (
        <BuildStage
          dispatch={dispatch}
          isRunning={state.isRunning}
          lastSourceIso={state.lastSourceIso}
          lastOutputDir={state.lastOutputDir}
          buildResult={state.buildResult}
        />
      )}
      {state.activeStage === 'inject' && (
        <InjectStage
          dispatch={dispatch}
          isRunning={state.isRunning}
          lastSourceIso={state.lastSourceIso}
          lastOutputDir={state.lastOutputDir}
          injectResult={state.injectResult}
        />
      )}
      {state.activeStage === 'verify' && (
        <VerifyStage
          dispatch={dispatch}
          isRunning={state.isRunning}
          lastInjectedIso={state.lastInjectedIso}
          verifyResult={state.verifyResult}
        />
      )}
      {state.activeStage === 'diff' && (
        <DiffStage
          dispatch={dispatch}
          isRunning={state.isRunning}
          lastSourceIso={state.lastSourceIso}
          lastInjectedIso={state.lastInjectedIso}
          diffResult={state.diffResult}
        />
      )}
      {state.activeStage === 'completion' && (
        <CompletionStage
          dispatch={dispatch}
          buildResult={state.buildResult}
          injectResult={state.injectResult}
          verifyResult={state.verifyResult}
          diffResult={state.diffResult}
        />
      )}

      {/* Right sidebar */}
      <DoctorSidebar doctor={state.doctor} progress={state.progress} />

      {/* Log panel */}
      <LogPanel
        logs={state.logs}
        onClear={() => dispatch({ type: 'CLEAR_LOGS' })}
      />
    </div>
  );
}

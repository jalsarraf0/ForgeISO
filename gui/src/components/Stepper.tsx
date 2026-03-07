import React from 'react';
import type { AppStage, StageStatus } from '../types';

type StepDef = {
  id: AppStage;
  label: string;
  sublabel: string;
  num: number;
};

const STEPS: StepDef[] = [
  { id: 'inject',     label: 'Inject',   sublabel: 'Autoinstall config',   num: 1 },
  { id: 'verify',     label: 'Verify',   sublabel: 'SHA-256 integrity',    num: 2 },
  { id: 'diff',       label: 'Diff',     sublabel: 'Compare ISOs',         num: 3 },
  { id: 'build',      label: 'Build',    sublabel: 'Fetch & package',      num: 4 },
  { id: 'completion', label: 'Complete', sublabel: 'Artifacts ready',      num: 5 },
];

function stepClass(id: AppStage, activeStage: AppStage, status: StageStatus): string {
  const parts = ['stepper-step'];
  if (id === activeStage) parts.push('active');
  if (status === 'success') parts.push('success');
  if (status === 'error') parts.push('error');
  if (status === 'running') parts.push('running');
  return parts.join(' ');
}

function stepIcon(status: StageStatus, num: number): string {
  if (status === 'success') return '✓';
  if (status === 'error') return '✗';
  return String(num);
}

export function Stepper({
  activeStage,
  stageStatus,
  onStepClick,
}: {
  activeStage: AppStage;
  stageStatus: Record<AppStage, StageStatus>;
  onStepClick: (stage: AppStage) => void;
}) {
  return (
    <nav className="stepper-bar">
      {STEPS.map((step, idx) => {
        const status = stageStatus[step.id];
        return (
          <React.Fragment key={step.id}>
            <button
              className={stepClass(step.id, activeStage, status)}
              type="button"
              onClick={() => onStepClick(step.id)}
            >
              <span className="stepper-num">{stepIcon(status, step.num)}</span>
              <div>
                <div className="stepper-label">{step.label}</div>
                <div className="stepper-sublabel">{step.sublabel}</div>
              </div>
            </button>
            {idx < STEPS.length - 1 && <div className="stepper-divider" />}
          </React.Fragment>
        );
      })}
    </nav>
  );
}

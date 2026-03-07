// ── Centralized App State Reducer ─────────────────────────────────────────────
import type {
  AppState,
  AppStage,
  StageStatus,
  JobProgress,
  BuildResult,
  InjectResult,
  VerifyResult,
  IsoDiff,
  Iso9660Compliance,
  DoctorReport,
  LogEntry,
} from './types';

// ── Initial state ─────────────────────────────────────────────────────────────

export const initialState: AppState = {
  activeStage: 'build',
  stageStatus: {
    build:      'active',
    inject:     'pending',
    verify:     'pending',
    diff:       'pending',
    completion: 'pending',
  },
  isRunning: false,
  progress: null,
  doctor: null,
  logs: [],
  buildResult:   null,
  injectResult:  null,
  verifyResult:  null,
  diffResult:    null,
  iso9660Result: null,
  lastSourceIso:    '',
  lastOutputDir:    './artifacts',
  lastInjectedIso:  '',
  lastDistro:       'ubuntu',
};

// ── Action types ──────────────────────────────────────────────────────────────

export type AppAction =
  | { type: 'SET_STAGE'; stage: AppStage }
  | { type: 'SET_DISTRO'; distro: string }
  | { type: 'SET_DOCTOR'; doctor: DoctorReport }
  | { type: 'APPEND_LOG'; entry: LogEntry }
  | { type: 'CLEAR_LOGS' }
  | { type: 'JOB_START'; stage: AppStage; operation: string }
  | { type: 'JOB_PROGRESS'; substage: string | null; percent: number | null; bytesDone: number | null; bytesTotal: number | null; operation: string }
  | { type: 'JOB_SUCCESS'; stage: AppStage }
  | { type: 'JOB_ERROR'; stage: AppStage; error: string }
  | { type: 'SET_BUILD_RESULT'; result: BuildResult; sourceIso: string; outputDir: string }
  | { type: 'SET_INJECT_RESULT'; result: InjectResult; injectedIso: string }
  | { type: 'SET_VERIFY_RESULT'; result: VerifyResult }
  | { type: 'SET_DIFF_RESULT'; result: IsoDiff }
  | { type: 'SET_ISO9660_RESULT'; result: Iso9660Compliance }
  | { type: 'ADVANCE_STAGE'; from: AppStage }
  | { type: 'RESET_STAGE'; stage: AppStage }
  | { type: 'RESET_ALL' };

// ── Stage ordering ────────────────────────────────────────────────────────────

const STAGE_ORDER: AppStage[] = ['build', 'inject', 'verify', 'diff', 'completion'];

function nextStage(current: AppStage): AppStage | null {
  const idx = STAGE_ORDER.indexOf(current);
  return idx >= 0 && idx < STAGE_ORDER.length - 1 ? STAGE_ORDER[idx + 1] : null;
}

// ── Reducer ───────────────────────────────────────────────────────────────────

export function appReducer(state: AppState, action: AppAction): AppState {
  switch (action.type) {
    case 'SET_STAGE': {
      const newStatus = { ...state.stageStatus };
      // Mark previous active as pending if not yet done
      if (state.activeStage !== action.stage) {
        if (newStatus[state.activeStage] === 'active') {
          newStatus[state.activeStage] = 'pending';
        }
        if (newStatus[action.stage] === 'pending') {
          newStatus[action.stage] = 'active';
        }
      }
      return { ...state, activeStage: action.stage, stageStatus: newStatus };
    }

    case 'SET_DISTRO':
      return { ...state, lastDistro: action.distro };

    case 'SET_DOCTOR':
      return { ...state, doctor: action.doctor };

    case 'APPEND_LOG':
      return { ...state, logs: [...state.logs.slice(-999), action.entry] };

    case 'CLEAR_LOGS':
      return { ...state, logs: [] };

    case 'JOB_START': {
      const newStatus = { ...state.stageStatus, [action.stage]: 'running' as StageStatus };
      const progress: JobProgress = {
        jobId: `${action.stage}-${Date.now()}`,
        stage: action.stage,
        status: 'running',
        currentOperation: action.operation,
        substage: null,
        percent: null,
        bytesDone: null,
        bytesTotal: null,
        startedAt: new Date(),
        updatedAt: new Date(),
        endedAt: null,
        warnings: [],
      };
      return { ...state, isRunning: true, stageStatus: newStatus, progress };
    }

    case 'JOB_PROGRESS': {
      if (!state.progress) return state;
      return {
        ...state,
        progress: {
          ...state.progress,
          substage: action.substage,
          percent: action.percent,
          bytesDone: action.bytesDone,
          bytesTotal: action.bytesTotal,
          currentOperation: action.operation || state.progress.currentOperation,
          updatedAt: new Date(),
        },
      };
    }

    case 'JOB_SUCCESS': {
      const newStatus = { ...state.stageStatus, [action.stage]: 'success' as StageStatus };
      const progress = state.progress
        ? { ...state.progress, status: 'success' as const, percent: 100, endedAt: new Date() }
        : null;
      return { ...state, isRunning: false, stageStatus: newStatus, progress };
    }

    case 'JOB_ERROR': {
      const newStatus = { ...state.stageStatus, [action.stage]: 'error' as StageStatus };
      const progress = state.progress
        ? { ...state.progress, status: 'error' as const, endedAt: new Date() }
        : null;
      return { ...state, isRunning: false, stageStatus: newStatus, progress };
    }

    case 'SET_BUILD_RESULT':
      return {
        ...state,
        buildResult: action.result,
        lastSourceIso: action.sourceIso,
        lastOutputDir: action.outputDir,
      };

    case 'SET_INJECT_RESULT':
      return {
        ...state,
        injectResult: action.result,
        lastInjectedIso: action.injectedIso,
      };

    case 'SET_VERIFY_RESULT':
      return { ...state, verifyResult: action.result };

    case 'SET_DIFF_RESULT':
      return { ...state, diffResult: action.result };

    case 'SET_ISO9660_RESULT':
      return { ...state, iso9660Result: action.result };

    case 'ADVANCE_STAGE': {
      const next = nextStage(action.from);
      if (!next) return state;
      const newStatus = { ...state.stageStatus };
      if (newStatus[action.from] === 'running' || newStatus[action.from] === 'active') {
        newStatus[action.from] = 'success';
      }
      newStatus[next] = 'active';
      return { ...state, activeStage: next, stageStatus: newStatus };
    }

    case 'RESET_STAGE': {
      const newStatus = { ...state.stageStatus, [action.stage]: 'pending' as StageStatus };
      const resultClear: Partial<AppState> = {};
      if (action.stage === 'build')      resultClear.buildResult   = null;
      if (action.stage === 'inject')     resultClear.injectResult  = null;
      if (action.stage === 'verify')     { resultClear.verifyResult = null; resultClear.iso9660Result = null; }
      if (action.stage === 'diff')       resultClear.diffResult    = null;
      return { ...state, stageStatus: newStatus, progress: null, ...resultClear };
    }

    case 'RESET_ALL': {
      return {
        ...initialState,
        // preserve doctor and logs across pipeline resets
        doctor: state.doctor,
        logs: state.logs,
      };
    }

    default:
      return state;
  }
}

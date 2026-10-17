// Types de l'API v1. Les types du modèle sont GÉNÉRÉS depuis Rust
// (`just gen-types` → api-types.ts) ; seuls les messages WebSocket, construits
// à la volée côté serveur, sont décrits ici.
import type { State, StateChanged } from './api-types';

export type {
	Area,
	Context,
	Device,
	EntityEntry,
	EntityStatus,
	Floor,
	Label,
	StateChanged
} from './api-types';

/** État d'une entité (nommé `State` côté Rust). */
export type EntityState = State;

export type WsMessage =
	| { type: 'snapshot'; states: EntityState[] }
	| { type: 'state_changed'; data: StateChanged }
	| { type: 'resync_required'; missed: number };

// --- Automatismes (M3) : décrits à la main, comme les messages WebSocket ---

export interface AutomationIssue {
	path: string;
	message: string;
}

export interface AutomationInfo {
	id: string;
	alias: string;
	entity_id: string;
	enabled: boolean;
	valid: boolean;
	errors: AutomationIssue[];
	issues: AutomationIssue[];
	last_triggered: string | null;
	running: number;
}

export type StepResult = 'ok' | 'false' | 'error_continued' | 'error' | 'skipped';
export type RunOutcome = 'finished' | 'condition_failed' | 'stopped' | 'error' | 'cancelled';

export interface TraceStep {
	path: string;
	kind: string;
	at: string;
	result: StepResult;
	detail?: string;
}

export interface AutomationTrace {
	run_id: number;
	automation_id: string;
	trigger: Record<string, unknown>;
	started: string;
	finished: string | null;
	outcome: RunOutcome | null;
	error: string | null;
	steps: TraceStep[];
}

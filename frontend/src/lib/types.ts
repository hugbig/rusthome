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

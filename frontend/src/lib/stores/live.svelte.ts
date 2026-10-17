import { SvelteMap } from 'svelte/reactivity';
import type { EntityState, WsMessage } from '../types';

/**
 * Applique un message WebSocket à la table des états. Fonction pure (hors
 * mutation de `states`) : testée sans réseau.
 * Renvoie `true` si le client doit se resynchroniser.
 */
export function applyMessage(states: Map<string, EntityState>, msg: WsMessage): boolean {
	switch (msg.type) {
		case 'snapshot':
			states.clear();
			for (const s of msg.states) states.set(s.entity_id, s);
			return false;
		case 'state_changed':
			if (msg.data.new_state) states.set(msg.data.entity_id, msg.data.new_state);
			else states.delete(msg.data.entity_id);
			return false;
		case 'resync_required':
			return true;
	}
}

export type Status = 'connecting' | 'live' | 'offline';

/** États en direct, alimentés par /api/v1/ws, avec reconnexion automatique. */
export class LiveStates {
	states = new SvelteMap<string, EntityState>();
	status = $state<Status>('connecting');
	#ws: WebSocket | null = null;
	#retry = 1000;
	#stopped = false;

	connect(url = defaultWsUrl()) {
		this.#stopped = false;
		this.status = 'connecting';
		const ws = new WebSocket(url);
		this.#ws = ws;
		ws.onopen = () => {
			this.status = 'live';
			this.#retry = 1000;
		};
		ws.onmessage = (ev) => {
			const msg = JSON.parse(ev.data as string) as WsMessage;
			if (applyMessage(this.states, msg)) ws.close(); // resync = reconnexion + snapshot
		};
		ws.onclose = () => {
			this.status = 'offline';
			if (this.#stopped) return;
			setTimeout(() => this.connect(url), this.#retry);
			this.#retry = Math.min(this.#retry * 2, 30_000);
		};
	}

	disconnect() {
		this.#stopped = true;
		this.#ws?.close();
	}
}

function defaultWsUrl(): string {
	const proto = location.protocol === 'https:' ? 'wss' : 'ws';
	return `${proto}://${location.host}/api/v1/ws`;
}

/** États en direct partagés par toute l’application (connectés dans le layout). */
export const live = new LiveStates();

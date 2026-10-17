import { describe, expect, it } from 'vitest';
import { applyMessage } from './live.svelte';
import type { EntityState } from '../types';

const st = (entity_id: string, state: string): EntityState => ({
	entity_id,
	state,
	attributes: {},
	last_changed: '2026-10-17T10:00:00Z',
	last_updated: '2026-10-17T10:00:00Z',
	context: { id: 'c' }
});

describe('applyMessage', () => {
	it('remplace tout au snapshot', () => {
		const m = new Map([['light.old', st('light.old', 'on')]]);
		applyMessage(m, { type: 'snapshot', states: [st('light.salon', 'off')] });
		expect([...m.keys()]).toEqual(['light.salon']);
	});

	it('met à jour et supprime sur state_changed', () => {
		const m = new Map([['light.salon', st('light.salon', 'off')]]);
		applyMessage(m, {
			type: 'state_changed',
			data: { entity_id: 'light.salon', old_state: null, new_state: st('light.salon', 'on') }
		});
		expect(m.get('light.salon')?.state).toBe('on');
		applyMessage(m, {
			type: 'state_changed',
			data: { entity_id: 'light.salon', old_state: null, new_state: null }
		});
		expect(m.size).toBe(0);
	});

	it('demande une resynchronisation quand le serveur a perdu des événements', () => {
		expect(applyMessage(new Map(), { type: 'resync_required', missed: 3 })).toBe(true);
	});
});

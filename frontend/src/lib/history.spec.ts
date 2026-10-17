import { describe, expect, it } from 'vitest';
import {
	niceDomain,
	normalizeHistory,
	stateColorIndex,
	ticks,
	toSegments,
	toSeries
} from './history';
import type { EntityState } from './types';

const at = (iso: string, state: string): EntityState => ({
	entity_id: 'sensor.x',
	state,
	attributes: {},
	last_changed: iso,
	last_updated: iso,
	context: { id: 'c' }
});

describe('normalizeHistory', () => {
	it('trie et retire les lignes dues à un simple changement d’attribut', () => {
		const late = at('2026-10-17T12:00:00Z', '21');
		const attrOnly = { ...at('2026-10-17T10:00:00Z', '20'), last_updated: '2026-10-17T11:00:00Z' };
		const out = normalizeHistory([late, at('2026-10-17T10:00:00Z', '20'), attrOnly]);
		expect(out.map((s) => s.state)).toEqual(['20', '21']);
	});
});

describe('toSeries', () => {
	it('coupe la courbe sur les états non numériques', () => {
		const s = toSeries([
			at('2026-10-17T10:00:00Z', '20.5'),
			at('2026-10-17T11:00:00Z', 'unavailable')
		]);
		expect(s[0]).toEqual({ t: Date.parse('2026-10-17T10:00:00Z'), v: 20.5 });
		expect(s[1]).toBeNull();
	});
});

describe('toSegments', () => {
	const from = Date.parse('2026-10-17T00:00:00Z');
	const to = Date.parse('2026-10-17T12:00:00Z');

	it('fusionne les répétitions et prolonge le dernier segment', () => {
		const segs = toSegments(
			[
				at('2026-10-16T22:00:00Z', 'off'), // avant la fenêtre : borné à `from`
				at('2026-10-17T06:00:00Z', 'on'),
				at('2026-10-17T07:00:00Z', 'on'),
				at('2026-10-17T08:00:00Z', 'off')
			],
			from,
			to
		);
		expect(
			segs.map((s) => [s.state, new Date(s.from).getUTCHours(), new Date(s.to).getUTCHours()])
		).toEqual([
			['off', 0, 6],
			['on', 6, 8],
			['off', 8, 12]
		]);
	});
});

describe('axes', () => {
	it('arrondit le domaine et produit des graduations propres', () => {
		expect(niceDomain([19.2, 21.7])).toEqual([19, 22]);
		expect(ticks([0, 20])).toEqual([0, 5, 10, 15, 20]);
		expect(niceDomain([5, 5])).toEqual([4, 6]);
	});
});

describe('stateColorIndex', () => {
	it("attribue les couleurs dans l'ordre et garde les neutres", () => {
		const m = stateColorIndex(['disarmed', 'armed_away', 'triggered', 'unavailable', 'armed_away']);
		expect(m.get('disarmed')).toBe('neutral');
		expect(m.get('armed_away')).toBe(0);
		expect(m.get('triggered')).toBe(1);
		expect(m.get('unavailable')).toBe('muted');
	});
});

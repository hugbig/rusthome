import { describe, expect, it } from 'vitest';
import { area, device, entry } from './test-fixtures';
import { applyFilter, countBy, EMPTY_FILTER, filterFromParams, filterToParams } from './filters';
import { RegistryIndex } from './stores/registry.svelte';
import type { EntityState } from './types';

const st = (entity_id: string, state: string, name?: string): EntityState => ({
	entity_id,
	state,
	attributes: name ? { friendly_name: name } : {},
	last_changed: '2026-10-17T10:00:00Z',
	last_updated: '2026-10-17T10:00:00Z',
	context: { id: 'c' }
});

const idx = new RegistryIndex(
	[],
	[area({ id: 'salon', name: 'Salon' })],
	[device({ id: 'd1', name: 'Hue', area_id: 'salon' })],
	[
		entry({
			uid: '1',
			entity_id: 'light.salon',
			integration: 'ha:hue',
			device_id: 'd1',
			hidden: false,
			disabled: false
		}),
		entry({
			uid: '2',
			entity_id: 'sensor.cpu',
			integration: 'ha:portainer',
			hidden: false,
			disabled: false
		})
	]
);
const items = [
	st('light.salon', 'on', 'Salon'),
	st('sensor.cpu', 'unavailable', 'CPU frigate'),
	st('sun.sun', 'above_horizon', 'Soleil')
];

describe('applyFilter', () => {
	it('sans filtre : tout, trié par libellé', () => {
		expect(applyFilter(items, EMPTY_FILTER, idx).map((s) => s.entity_id)).toEqual([
			'sensor.cpu',
			'light.salon',
			'sun.sun'
		]);
	});

	it('combine type, pièce, intégration, état et recherche', () => {
		const ids = (f: Partial<typeof EMPTY_FILTER>) =>
			applyFilter(items, { ...EMPTY_FILTER, ...f }, idx).map((s) => s.entity_id);
		expect(ids({ domain: 'light' })).toEqual(['light.salon']);
		expect(ids({ area: 'salon' })).toEqual(['light.salon']);
		expect(ids({ area: '_none' })).toEqual(['sensor.cpu', 'sun.sun']);
		expect(ids({ integration: 'portainer' })).toEqual(['sensor.cpu']);
		expect(ids({ status: 'indisponible' })).toEqual(['sensor.cpu']);
		expect(ids({ q: 'soleil' })).toEqual(['sun.sun']);
	});
});

describe('URL', () => {
	it('aller-retour des paramètres', () => {
		const f = filterFromParams(new URLSearchParams('etat=indisponible&type=light&q=sa'));
		expect(f).toMatchObject({ status: 'indisponible', domain: 'light', q: 'sa' });
		expect(filterToParams(f)).toBe('?q=sa&type=light&etat=indisponible');
		expect(filterFromParams(new URLSearchParams('etat=nimporte')).status).toBe('');
	});
});

describe('countBy', () => {
	it('compte et trie les valeurs', () => {
		expect(countBy(['b', 'a', 'b', undefined], (x) => x)).toEqual([
			['a', 1],
			['b', 2]
		]);
	});
});

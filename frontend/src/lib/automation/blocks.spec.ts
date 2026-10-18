import { describe as group, expect, it } from 'vitest';
import { blockId, describe, moveItem, setField } from './blocks';

group('blocs d’automatisation', () => {
	it('reconnaît les actions par leur forme', () => {
		expect(blockId('action', { action: 'light.turn_on' })).toBe('service');
		expect(blockId('action', { service: 'light.turn_on' })).toBe('service');
		expect(blockId('action', { delay: '00:00:05' })).toBe('delay');
		expect(blockId('action', { if: [], then: [] })).toBe('if');
		expect(blockId('action', { condition: 'state' })).toBe('condition');
		expect(blockId('trigger', { platform: 'state' })).toBe('state');
	});

	it('résume comme HA, avec les noms des entités', () => {
		const name = (id: string) => (id === 'light.salon' ? 'Salon' : id);
		expect(
			describe(
				'trigger',
				{ trigger: 'state', entity_id: 'light.salon', to: 'on', for: '00:05:00' },
				name
			)
		).toBe('Quand Salon change à on pendant 00:05:00');
		expect(
			describe('trigger', { trigger: 'numeric_state', entity_id: ['a', 'b'], above: 20 })
		).toBe('Quand a et b au-dessus de 20');
		expect(describe('condition', { condition: 'time', after: '08:00:00' })).toBe(
			'Si l’heure est après 08:00:00'
		);
		expect(
			describe('action', { action: 'light.turn_on', target: { entity_id: ['light.salon'] } }, name)
		).toBe('light.turn_on sur Salon');
		expect(describe('action', { repeat: { count: 3, sequence: [] } })).toBe('Répéter 3 fois');
		expect(describe('action', { alias: 'Allumer', delay: 1 })).toBe('Allumer');
	});

	it('retire les champs vidés et réordonne', () => {
		expect(setField({ to: 'on', from: 'off' }, 'from', '')).toEqual({ to: 'on' });
		expect(setField({}, 'entity_id', [])).toEqual({ entity_id: [] });
		expect(moveItem(['a', 'b', 'c'], 2, -1)).toEqual(['a', 'c', 'b']);
		expect(moveItem(['a'], 0, 1)).toEqual(['a']);
	});
});

import { describe, expect, it } from 'vitest';
import {
	domainLabel,
	domainOf,
	entityLabel,
	isNumericEntity,
	isNumericState,
	plural,
	relativeTime
} from './format';
import type { EntityState } from './types';

const st = (state: string, attributes: Record<string, unknown> = {}): EntityState => ({
	entity_id: 'sensor.temp',
	state,
	attributes,
	last_changed: '2026-10-17T10:00:00Z',
	last_updated: '2026-10-17T10:00:00Z',
	context: { id: 'c' }
});

describe('format', () => {
	it('choisit le meilleur libellé', () => {
		expect(entityLabel(st('1', { friendly_name: 'Température' }))).toBe('Température');
		expect(entityLabel(st('1'), 'Nom du registre')).toBe('Nom du registre');
		expect(entityLabel(st('1'))).toBe('sensor.temp');
	});

	it('détecte les états numériques', () => {
		expect(isNumericState('20.6')).toBe(true);
		expect(isNumericState('-3')).toBe(true);
		expect(isNumericState('on')).toBe(false);
		expect(isNumericState('')).toBe(false);
		expect(isNumericEntity(st('20.6', { unit_of_measurement: '°C' }))).toBe(true);
		expect(isNumericEntity(st('20.6'))).toBe(false);
	});

	it('traduit les domaines connus', () => {
		expect(domainOf('light.salon')).toBe('light');
		expect(domainLabel('light')).toBe('Lumière');
		expect(domainLabel('zz_inconnu')).toBe('zz_inconnu');
	});

	it('accorde au pluriel', () => {
		expect(plural(0, 'entité')).toBe('0 entité');
		expect(plural(1, 'appareil')).toBe('1 appareil');
		expect(plural(1353, 'entité')).toBe('1\u202f353 entités');
	});

	it('formate le temps relatif en français', () => {
		const now = new Date('2026-10-17T12:00:00Z');
		expect(relativeTime('2026-10-17T11:59:50Z', now)).toBe("à l'instant");
		expect(relativeTime('2026-10-17T11:55:00Z', now)).toBe('il y a 5 minutes');
		expect(relativeTime('2026-10-16T12:00:00Z', now)).toBe('hier');
	});
});

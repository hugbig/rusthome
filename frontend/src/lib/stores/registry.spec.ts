import { describe, expect, it } from 'vitest';
import { area, device, entry, floor } from '../test-fixtures';
import { RegistryIndex } from './registry.svelte';

const idx = new RegistryIndex(
	[floor({ id: 'rdc', name: 'RDC' })],
	[
		area({ id: 'salon', name: 'Salon', floor_id: 'rdc' }),
		area({ id: 'bureau', name: 'Bureau', floor_id: 'rdc' })
	],
	[device({ id: 'd1', name: 'Prise', area_id: 'salon' })],
	[
		entry({
			uid: 'u1',
			entity_id: 'switch.prise',
			integration: 'ha:mqtt',
			device_id: 'd1',
			hidden: false,
			disabled: false
		}),
		entry({
			uid: 'u2',
			entity_id: 'sensor.p',
			integration: 'ha:mqtt',
			device_id: 'd1',
			area_id: 'bureau',
			hidden: false,
			disabled: false
		}),
		entry({
			uid: 'u3',
			entity_id: 'sun.sun',
			integration: 'ha:sun',
			hidden: false,
			disabled: false
		})
	]
);

describe('RegistryIndex', () => {
	it("hérite de la pièce de l'appareil, sauf surcharge", () => {
		expect(idx.areaNameOf('switch.prise')).toBe('Salon');
		expect(idx.areaNameOf('sensor.p')).toBe('Bureau');
		expect(idx.areaNameOf('sun.sun')).toBeUndefined();
	});

	it('retire le préfixe du miroir HA', () => {
		expect(idx.integrationOf('switch.prise')).toBe('mqtt');
	});

	it('liste les entités par appareil et par pièce', () => {
		expect(idx.entitiesOfDevice('d1').map((e) => e.uid)).toEqual(['u1', 'u2']);
		expect(idx.entitiesOfArea('salon').map((e) => e.uid)).toEqual(['u1']);
		expect(idx.devicesOfArea('salon')).toHaveLength(1);
	});
});

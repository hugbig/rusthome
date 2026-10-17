// Fabriques pour les tests : objets complets (types générés stricts) avec
// des valeurs par défaut, seuls les champs utiles sont précisés.
import type { Area, Device, EntityEntry, Floor } from './types';

export const floor = (f: Pick<Floor, 'id' | 'name'> & Partial<Floor>): Floor => ({
	icon: null,
	level: null,
	...f
});

export const area = (a: Pick<Area, 'id' | 'name'> & Partial<Area>): Area => ({
	aliases: [],
	floor_id: null,
	icon: null,
	...a
});

export const device = (d: Pick<Device, 'id' | 'name'> & Partial<Device>): Device => ({
	area_id: null,
	disabled: false,
	integration: 'ha:test',
	labels: [],
	manufacturer: null,
	model: null,
	sw_version: null,
	...d
});

export const entry = (
	e: Pick<EntityEntry, 'uid' | 'entity_id' | 'integration'> & Partial<EntityEntry>
): EntityEntry => ({
	area_id: null,
	device_id: null,
	disabled: false,
	hidden: false,
	icon: null,
	labels: [],
	name: null,
	status: 'active',
	unique_id: null,
	...e
});

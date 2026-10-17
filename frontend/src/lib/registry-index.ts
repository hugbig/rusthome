import type { Area, Device, EntityEntry, Floor } from './types';

/** Index des registres pour retrouver pièce, appareil et nom d'une entité. Pur, testé. */
export class RegistryIndex {
	readonly floors: Floor[];
	readonly areas: Area[];
	readonly devices: Device[];
	readonly entities: EntityEntry[];
	readonly areaById: Map<string, Area>;
	readonly deviceById: Map<string, Device>;
	readonly entryByEntityId: Map<string, EntityEntry>;

	constructor(
		floors: Floor[] = [],
		areas: Area[] = [],
		devices: Device[] = [],
		entities: EntityEntry[] = []
	) {
		this.floors = floors;
		this.areas = areas;
		this.devices = devices;
		this.entities = entities;
		this.areaById = new Map(areas.map((a) => [a.id, a]));
		this.deviceById = new Map(devices.map((d) => [d.id, d]));
		this.entryByEntityId = new Map(entities.map((e) => [e.entity_id, e]));
	}

	/** Pièce effective : celle de l'entité, sinon celle de son appareil. */
	areaIdOf(entityId: string): string | undefined {
		const e = this.entryByEntityId.get(entityId);
		if (!e) return undefined;
		return (
			e.area_id ??
			(e.device_id ? (this.deviceById.get(e.device_id)?.area_id ?? undefined) : undefined)
		);
	}

	areaNameOf(entityId: string): string | undefined {
		const id = this.areaIdOf(entityId);
		return id ? this.areaById.get(id)?.name : undefined;
	}

	/** Intégration sans le préfixe `ha:` du miroir. */
	integrationOf(entityId: string): string | undefined {
		return this.entryByEntityId.get(entityId)?.integration.replace(/^ha:/, '');
	}

	entitiesOfDevice(deviceId: string): EntityEntry[] {
		return this.entities.filter((e) => e.device_id === deviceId);
	}

	entitiesOfArea(areaId: string): EntityEntry[] {
		return this.entities.filter((e) => this.areaIdOf(e.entity_id) === areaId);
	}

	devicesOfArea(areaId: string): Device[] {
		return this.devices.filter((d) => d.area_id === areaId);
	}
}

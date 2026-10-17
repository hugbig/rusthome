import { RegistryIndex } from '../registry-index';
import type { Area, Device, EntityEntry, Floor } from '../types';

export { RegistryIndex };

async function getJson<T>(path: string): Promise<T> {
	const r = await fetch(`/api/v1/${path}`);
	if (!r.ok) throw new Error(`${path}: HTTP ${r.status}`);
	return (await r.json()) as T;
}

/** Registres chargés une fois pour toute l'application. */
class RegistryStore {
	index = $state(new RegistryIndex());
	loaded = $state(false);
	error = $state<string | null>(null);

	async load() {
		try {
			const [floors, areas, devices, entities] = await Promise.all([
				getJson<Floor[]>('registry/floors'),
				getJson<Area[]>('registry/areas'),
				getJson<Device[]>('registry/devices'),
				getJson<EntityEntry[]>('registry/entities')
			]);
			this.index = new RegistryIndex(floors, areas, devices, entities);
			this.loaded = true;
			this.error = null;
		} catch (e) {
			this.error = e instanceof Error ? e.message : String(e);
		}
	}
}

export const registry = new RegistryStore();

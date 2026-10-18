// Résumé des intégrations à partir des registres : qui fournit quoi, et si
// c'est RustHome qui pilote ou encore le miroir de Home Assistant.
import type { Device, EntityEntry } from './types';

export interface IntegrationSummary {
	/** Identifiant sans le préfixe `ha:` (`hue`, `esphome`…). */
	id: string;
	/** `native` : reprise par RustHome ; `mirror` : lue dans HA. */
	source: 'native' | 'mirror';
	devices: number;
	entities: number;
}

const LABELS: Record<string, string> = {
	z2m: 'Zigbee2MQTT',
	hue: 'Philips Hue',
	frigate: 'Frigate',
	portainer: 'Portainer',
	automation: 'Automatisations',
	esphome: 'ESPHome',
	mqtt: 'MQTT',
	alarmo: 'Alarmo',
	meteo_france: 'Météo-France',
	mobile_app: 'Application mobile',
	homekit_controller: 'HomeKit',
	alexa_media: 'Alexa Media',
	spotify: 'Spotify',
	emby: 'Emby',
	google: 'Google Agenda',
	sabnzbd: 'SABnzbd',
	synology_dsm: 'Synology',
	yamaha_musiccast: 'Yamaha MusicCast',
	backup: 'Sauvegardes',
	hassio: 'Superviseur',
	sun: 'Soleil',
	template: 'Modèles',
	group: 'Groupes',
	person: 'Personnes',
	zone: 'Zones'
};

export function integrationLabel(id: string): string {
	return LABELS[id] ?? id.replace(/_/g, ' ').replace(/^\w/, (c) => c.toUpperCase());
}

export function summarize(devices: Device[], entities: EntityEntry[]): IntegrationSummary[] {
	const by = new Map<string, IntegrationSummary>();
	const get = (raw: string) => {
		const source = raw.startsWith('ha:') ? 'mirror' : 'native';
		const id = raw.replace(/^ha:/, '');
		const key = `${source}:${id}`;
		let s = by.get(key);
		if (!s) {
			s = { id, source, devices: 0, entities: 0 };
			by.set(key, s);
		}
		return s;
	};
	for (const d of devices) get(d.integration).devices++;
	for (const e of entities) get(e.integration).entities++;
	return [...by.values()].sort(
		(a, b) =>
			Number(a.source === 'mirror') - Number(b.source === 'mirror') ||
			b.entities - a.entities ||
			a.id.localeCompare(b.id)
	);
}

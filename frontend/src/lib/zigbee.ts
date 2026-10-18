// Réseau Zigbee (Zigbee2MQTT) : lecture de l'état du pont pour la page de
// gestion. Pur, testé.

export interface ZDevice {
	ieee: string;
	name: string;
	type: string;
	model?: string;
	vendor?: string;
	description?: string;
	power?: string;
	interviewed: boolean;
	supported: boolean;
	lqi?: number;
	battery?: number;
	lastSeen?: string;
	/** `available`, `updating`, `idle`… (OTA). */
	ota?: string;
	otaProgress?: number;
}

export interface ZGroup {
	id: number;
	name: string;
	members: { ieee: string; endpoint?: number }[];
}

export interface ZEvent {
	type: string;
	name?: string;
	ieee?: string;
	status?: string;
	received: number;
}

type Raw = Record<string, unknown>;
const obj = (v: unknown): Raw => (v && typeof v === 'object' ? (v as Raw) : {});
const arr = (v: unknown): Raw[] => (Array.isArray(v) ? (v as Raw[]) : []);
const num = (v: unknown) => (typeof v === 'number' ? v : undefined);
const str = (v: unknown) => (typeof v === 'string' && v ? v : undefined);

/** Appareils (hors coordinateur) avec leur dernier état connu. */
export function devicesOf(snapshot: unknown): ZDevice[] {
	const snap = obj(snapshot);
	const payloads = obj(snap.payloads);
	return arr(snap.devices)
		.filter((d) => d.type !== 'Coordinator')
		.map((d) => {
			const name = String(d.friendly_name ?? d.ieee_address);
			const p = obj(payloads[name]);
			const def = obj(d.definition);
			const update = obj(p.update);
			return {
				ieee: String(d.ieee_address ?? ''),
				name,
				type: String(d.type ?? ''),
				model: str(def.model) ?? str(d.model_id),
				vendor: str(def.vendor) ?? str(d.manufacturer),
				description: str(def.description),
				power: str(d.power_source),
				interviewed: d.interview_completed !== false,
				supported: d.supported !== false,
				lqi: num(p.linkquality),
				battery: num(p.battery),
				lastSeen: str(p.last_seen),
				ota: str(update.state),
				otaProgress: num(update.progress)
			};
		})
		.sort((a, b) => a.name.localeCompare(b.name, 'fr'));
}

export function groupsOf(snapshot: unknown): ZGroup[] {
	return arr(obj(snapshot).groups).map((g) => ({
		id: Number(g.id),
		name: String(g.friendly_name ?? g.id),
		members: arr(g.members).map((m) => ({
			ieee: String(m.ieee_address ?? ''),
			endpoint: num(m.endpoint)
		}))
	}));
}

export function eventsOf(snapshot: unknown): ZEvent[] {
	return arr(obj(snapshot).events).map((e) => {
		const data = obj(e.data);
		return {
			type: String(e.type ?? ''),
			name: str(data.friendly_name),
			ieee: str(data.ieee_address),
			status: str(data.status),
			received: num(e.received) ?? 0
		};
	});
}

/** Libellé d'un événement du réseau. */
export function eventLabel(e: ZEvent): string {
	const who = e.name ?? e.ieee ?? 'Appareil';
	switch (e.type) {
		case 'device_joined':
			return `${who} a rejoint le réseau`;
		case 'device_announce':
			return `${who} s’est annoncé`;
		case 'device_leave':
			return `${who} a quitté le réseau`;
		case 'device_interview':
			return e.status === 'successful'
				? `${who} : identification réussie`
				: e.status === 'failed'
					? `${who} : identification échouée`
					: `${who} : identification en cours`;
		default:
			return `${who} : ${e.type}`;
	}
}

/**
 * Secondes d'appairage restantes : `permit_join_end` (horodatage en ms, Z2M
 * 2.x) ou `permit_join_timeout` (secondes, Z2M 1.x) ; 0 si fermé.
 */
export function permitJoinLeft(snapshot: unknown, now: number = Date.now()): number {
	const info = obj(obj(snapshot).info);
	if (info.permit_join !== true) return 0;
	const end = num(info.permit_join_end);
	if (end !== undefined) return Math.max(0, Math.round((end - now) / 1000));
	return num(info.permit_join_timeout) ?? 254;
}

/** Qualité de lien : bonne, moyenne, faible (avec libellé, jamais la couleur seule). */
export function lqiLevel(lqi: number | undefined): 'good' | 'warning' | 'critical' | undefined {
	if (lqi === undefined) return undefined;
	return lqi >= 100 ? 'good' : lqi >= 40 ? 'warning' : 'critical';
}

/** Clusters proposés pour les liaisons (télécommande → lampe…). */
export const BIND_CLUSTERS: [string, string][] = [
	['genOnOff', 'Marche / arrêt'],
	['genLevelCtrl', 'Variation'],
	['lightingColorCtrl', 'Couleur'],
	['closuresWindowCovering', 'Volets']
];

// Navigateur multimédia : libellés, types de média, adresses des fichiers.

/** Objets détectés par Frigate, en français. */
const LABELS: Record<string, string> = {
	person: 'Personne',
	car: 'Voiture',
	bicycle: 'Vélo',
	motorcycle: 'Moto',
	cat: 'Chat',
	dog: 'Chien',
	bird: 'Oiseau',
	package: 'Colis',
	face: 'Visage',
	license_plate: 'Plaque'
};

export function labelFr(label: string | undefined): string {
	if (!label) return 'Événement';
	return LABELS[label] ?? label.replace(/_/g, ' ');
}

export interface FrigateEvent {
	id: string;
	camera: string;
	label?: string;
	start_time: number;
	end_time?: number | null;
	has_clip?: boolean;
	has_snapshot?: boolean;
	top_score?: number | null;
	data?: { top_score?: number; score?: number };
	zones?: string[];
}

/** Score de détection en %, quelle que soit la version de Frigate. */
export function scoreOf(e: FrigateEvent): number | undefined {
	const s = e.top_score ?? e.data?.top_score ?? e.data?.score;
	return typeof s === 'number' ? Math.round(s * 100) : undefined;
}

/** Durée lisible d'un événement (« 42 s », « 3 min 05 s »). */
export function duration(e: FrigateEvent): string | undefined {
	if (!e.end_time) return undefined;
	const s = Math.max(0, Math.round(e.end_time - e.start_time));
	if (s < 60) return `${s} s`;
	return `${Math.floor(s / 60)} min ${String(s % 60).padStart(2, '0')} s`;
}

const enc = encodeURIComponent;

/** Adresses des fichiers Frigate, servies par RustHome. */
export const frigateFile = {
	thumbnail: (id: string) => `/api/v1/media/frigate/file/thumbnail?id=${enc(id)}`,
	snapshot: (id: string) => `/api/v1/media/frigate/file/snapshot?id=${enc(id)}`,
	clip: (id: string) => `/api/v1/media/frigate/file/clip?id=${enc(id)}`,
	latest: (camera: string, t = 0) =>
		`/api/v1/media/frigate/file/latest?camera=${enc(camera)}&t=${t}`,
	recording: (camera: string, start: number, end: number) =>
		`/api/v1/media/frigate/file/recording?camera=${enc(camera)}&start=${start}&end=${end}`,
	/**
	 * Flux HLS d'un clip : les MP4 de Frigate sont fragmentés et sans durée
	 * totale, un navigateur n'en lit que le premier fragment (≈ 4 s).
	 */
	clipHls: (id: string) => `/api/v1/media/frigate/vod/event/${enc(id)}/index.m3u8`,
	recordingHls: (camera: string, start: number, end: number) =>
		`/api/v1/media/frigate/vod/recording/${enc(camera)}/${start}/${end}/index.m3u8`
};

/** Début et fin (secondes Unix) d'une heure locale d'enregistrement. */
export function hourRange(day: string, hour: number): [number, number] {
	const [y, m, d] = day.split('-').map(Number) as [number, number, number];
	const start = new Date(y, m - 1, d, hour).getTime() / 1000;
	return [start, start + 3600];
}

export type Kind = 'folder' | 'video' | 'audio' | 'image' | 'other';

/** Nature d'un élément des sources de HA (classe et type MIME). */
export function kindOf(item: {
	can_expand?: boolean;
	media_class?: string;
	media_content_type?: string;
}): Kind {
	if (item.can_expand) return 'folder';
	const t = `${item.media_class ?? ''} ${item.media_content_type ?? ''}`;
	if (/video|movie|episode|tv_show|clip/.test(t)) return 'video';
	if (/audio|music|track|podcast|album/.test(t)) return 'audio';
	if (/image|photo/.test(t)) return 'image';
	return 'other';
}

/** Vignette d'un élément de HA : chemin relatif servi par RustHome, sinon rien. */
export function haThumb(thumbnail: string | null | undefined): string | undefined {
	if (!thumbnail || !thumbnail.startsWith('/')) return undefined;
	return `/api/v1/media/ha/file?path=${enc(thumbnail)}`;
}

export const haPlay = (id: string) => `/api/v1/media/ha/play?id=${enc(id)}`;

const fileDate = new Intl.DateTimeFormat('fr-FR', {
	weekday: 'short',
	day: 'numeric',
	month: 'short',
	hour: '2-digit',
	minute: '2-digit'
});

/**
 * Nom lisible d'un fichier daté (`20261001-103837-salon.mp4` →
 * « mer. 1 oct., 10:38 · salon ») ; les autres noms restent tels quels.
 */
export function prettyFileName(title: string): string {
	const m = /^(\d{4})(\d{2})(\d{2})[-_ ]?(\d{2})(\d{2})(\d{2})(?:[-_ ](.+?))?(?:\.\w+)?$/.exec(
		title
	);
	if (!m) return title;
	const [, y, mo, d, h, mi, s, rest] = m;
	const date = new Date(+y!, +mo! - 1, +d!, +h!, +mi!, +s!);
	if (Number.isNaN(date.getTime())) return title;
	return rest ? `${fileDate.format(date)} · ${rest.replace(/[-_]/g, ' ')}` : fileDate.format(date);
}

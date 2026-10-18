// Données des cartes historiques (graph, journal) : /api/v1/history.
import { request } from '../api';
import { normalizeHistory } from '../history';
import type { EntityState } from '../types';

export async function fetchHistory(entityId: string, since: Date): Promise<EntityState[]> {
	const r = await request<EntityState[]>(
		'GET',
		`history/${encodeURIComponent(entityId)}?since=${encodeURIComponent(since.toISOString())}`
	);
	return r.ok ? normalizeHistory(r.data) : [];
}

export interface LogEntry {
	entity_id: string;
	state: string;
	when: number;
}

/**
 * Journal : changements d'état de plusieurs entités, du plus récent au plus
 * ancien. Les répétitions d'un même état (mises à jour d'attributs) sont
 * retirées.
 */
export function logEntries(histories: EntityState[][], limit = 50): LogEntry[] {
	const out: LogEntry[] = [];
	for (const h of histories) {
		let prev: string | undefined;
		for (const s of h) {
			if (s.state !== prev)
				out.push({ entity_id: s.entity_id, state: s.state, when: Date.parse(s.last_changed) });
			prev = s.state;
		}
	}
	return out.sort((a, b) => b.when - a.when).slice(0, limit);
}

export interface Bar {
	/** Début du jour (heure locale), en ms. */
	day: number;
	value: number;
}

/** Agrège un historique numérique par jour local : max ou dernière valeur. */
export function dailyBars(history: EntityState[], agg: 'max' | 'last' = 'last'): Bar[] {
	const days = new Map<number, number>();
	for (const s of history) {
		const v = Number(s.state);
		if (s.state.trim() === '' || !Number.isFinite(v)) continue;
		const d = new Date(s.last_changed);
		const day = new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
		const cur = days.get(day);
		days.set(day, agg === 'max' && cur !== undefined ? Math.max(cur, v) : v);
	}
	return [...days.entries()].sort((a, b) => a[0] - b[0]).map(([day, value]) => ({ day, value }));
}

export interface MediaItem {
	title: string;
	lines: string[];
	/** Rang de l'élément dans le capteur (pour le proxy d'images). */
	index: number;
	hasImage: boolean;
}

/**
 * Éléments d'un capteur « upcoming media » : `data` est une liste (souvent
 * sérialisée en JSON) dont le premier élément donne les modèles de lignes
 * (`line1_default: "$release"`…). Les lignes vides ou non résolues sautent ;
 * la durée, mal renseignée par Emby, est ignorée.
 */
export function mediaItems(data: unknown): MediaItem[] {
	let list: unknown = data;
	if (typeof list === 'string') {
		try {
			list = JSON.parse(list);
		} catch {
			return [];
		}
	}
	if (!Array.isArray(list) || list.length === 0) return [];
	const [defaults, ...rest] = list as Record<string, unknown>[];
	const templates = [1, 2, 3, 4]
		.map((n) => defaults?.[`line${n}_default`])
		.filter((t): t is string => typeof t === 'string' && !t.includes('$runtime'));
	const fill = (t: string, it: Record<string, unknown>) =>
		t.replace(/\$(\w+)/g, (_, k: string) => (it[k] === undefined ? '' : String(it[k]))).trim();
	return rest.flatMap((it, index) => {
		if (typeof it.title !== 'string') return [];
		const img = it.poster ?? it.fanart;
		return [
			{
				title: it.title,
				lines: templates.map((t) => fill(t, it)).filter((l) => l && !l.includes('$')),
				index,
				hasImage: typeof img === 'string' && /^https?:\/\//.test(img)
			}
		];
	});
}

export interface CalEvent {
	summary: string;
	start: number;
	allDay: boolean;
	calendar: string;
}

/** Événement HA (`start: {dateTime}` ou `{date}` pour une journée entière). */
export function parseEvent(raw: Record<string, unknown>, calendar: string): CalEvent | null {
	const start = raw.start as { dateTime?: string; date?: string } | string | undefined;
	const s = typeof start === 'string' ? start : (start?.dateTime ?? start?.date);
	if (!s) return null;
	const allDay = /^\d{4}-\d{2}-\d{2}$/.test(s);
	// une date seule est un jour local, pas minuit UTC
	const t = allDay ? new Date(`${s}T00:00:00`).getTime() : Date.parse(s);
	if (!Number.isFinite(t)) return null;
	return { summary: String(raw.summary ?? '(sans titre)'), start: t, allDay, calendar };
}

/** Événements groupés par jour local, dans l'ordre. */
export function groupByDay(events: CalEvent[]): { day: number; events: CalEvent[] }[] {
	const by = new Map<number, CalEvent[]>();
	for (const e of [...events].sort((a, b) => a.start - b.start)) {
		const d = new Date(e.start);
		const day = new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
		by.set(day, [...(by.get(day) ?? []), e]);
	}
	return [...by.entries()].map(([day, evs]) => ({ day, events: evs }));
}

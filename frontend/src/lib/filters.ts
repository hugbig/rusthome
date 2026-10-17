import { byLabel, domainOf, entityLabel, isUnavailable } from './format';
import type { RegistryIndex } from './stores/registry.svelte';
import type { EntityState } from './types';

export interface EntityFilter {
	q: string;
	domain: string;
	area: string; // '' = toutes, '_none' = sans pièce
	integration: string;
	status: '' | 'indisponible' | 'disponible';
}

export const EMPTY_FILTER: EntityFilter = {
	q: '',
	domain: '',
	area: '',
	integration: '',
	status: ''
};

/** Lit le filtre depuis l'URL (liens partageables, ex. ?etat=indisponible). */
export function filterFromParams(p: Pick<URLSearchParams, 'get'>): EntityFilter {
	const status = p.get('etat');
	return {
		q: p.get('q') ?? '',
		domain: p.get('type') ?? '',
		area: p.get('piece') ?? '',
		integration: p.get('integration') ?? '',
		status: status === 'indisponible' || status === 'disponible' ? status : ''
	};
}

export function filterToParams(f: EntityFilter): string {
	const p = new URLSearchParams();
	if (f.q) p.set('q', f.q);
	if (f.domain) p.set('type', f.domain);
	if (f.area) p.set('piece', f.area);
	if (f.integration) p.set('integration', f.integration);
	if (f.status) p.set('etat', f.status);
	const s = p.toString();
	return s ? `?${s}` : '';
}

/** Filtre et trie par libellé. Pur : testé sans DOM. */
export function applyFilter(
	items: EntityState[],
	f: EntityFilter,
	idx: RegistryIndex
): EntityState[] {
	const q = f.q.trim().toLowerCase();
	return items
		.filter((s) => {
			if (f.domain && domainOf(s.entity_id) !== f.domain) return false;
			if (f.status === 'indisponible' && !isUnavailable(s)) return false;
			if (f.status === 'disponible' && isUnavailable(s)) return false;
			if (f.integration && idx.integrationOf(s.entity_id) !== f.integration) return false;
			if (f.area) {
				const a = idx.areaIdOf(s.entity_id);
				if (f.area === '_none' ? a !== undefined : a !== f.area) return false;
			}
			if (q) {
				const name = entityLabel(s, idx.entryByEntityId.get(s.entity_id)?.name).toLowerCase();
				if (!name.includes(q) && !s.entity_id.includes(q)) return false;
			}
			return true;
		})
		.sort((a, b) =>
			byLabel(
				entityLabel(a, idx.entryByEntityId.get(a.entity_id)?.name),
				entityLabel(b, idx.entryByEntityId.get(b.entity_id)?.name)
			)
		);
}

/** Valeurs distinctes avec effectifs, pour alimenter les listes déroulantes. */
export function countBy<T>(items: T[], key: (x: T) => string | undefined): [string, number][] {
	const m = new Map<string, number>();
	for (const it of items) {
		const k = key(it);
		if (k) m.set(k, (m.get(k) ?? 0) + 1);
	}
	return [...m.entries()].sort(([a], [b]) => byLabel(a, b));
}

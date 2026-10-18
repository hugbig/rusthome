// Historique partagé entre cartes : une seule requête par entité et par
// période, réutilisée 5 minutes (les mini-courbes des tuiles en dépendent).
import type { EntityState } from '../types';
import { fetchHistory } from './data';

const TTL = 5 * 60_000;
const cache = new Map<string, { at: number; data: Promise<EntityState[]> }>();

export function cachedHistory(entityId: string, hours: number): Promise<EntityState[]> {
	const key = `${entityId}|${hours}`;
	const hit = cache.get(key);
	if (hit && Date.now() - hit.at < TTL) return hit.data;
	const data = fetchHistory(entityId, new Date(Date.now() - hours * 3600_000));
	cache.set(key, { at: Date.now(), data });
	return data;
}

// Tableau Énergie : réglages (compteurs, appareils), périodes et cumuls.
// Données : statistiques à long terme de HA (lecture seule) jusqu'à ce que
// RustHome tienne les siennes.

export interface EnergyConfig {
	/** Statistiques de consommation réseau (Linky…). */
	grid: string[];
	/** Retour au réseau (production), s'il y en a. */
	gridReturn: string[];
	/** Appareils suivis individuellement. */
	devices: string[];
}

type Raw = Record<string, unknown>;
const arr = (v: unknown): Raw[] => (Array.isArray(v) ? (v as Raw[]) : []);
const str = (v: unknown): string | undefined => (typeof v === 'string' && v ? v : undefined);

/** `energy/get_prefs` de HA → compteurs utiles (ancien et nouveau format). */
export function energyConfig(prefs: unknown): EnergyConfig {
	const p = (prefs ?? {}) as Raw;
	const out: EnergyConfig = { grid: [], gridReturn: [], devices: [] };
	for (const src of arr(p.energy_sources)) {
		if (src.type !== 'grid') continue;
		const from = str(src.stat_energy_from);
		if (from) out.grid.push(from);
		const to = str(src.stat_energy_to);
		if (to) out.gridReturn.push(to);
		// ancien format : listes flow_from / flow_to
		for (const f of arr(src.flow_from)) {
			const id = str(f.stat_energy_from);
			if (id) out.grid.push(id);
		}
		for (const f of arr(src.flow_to)) {
			const id = str(f.stat_energy_to);
			if (id) out.gridReturn.push(id);
		}
	}
	for (const d of arr(p.device_consumption)) {
		const id = str(d.stat_consumption);
		if (id) out.devices.push(id);
	}
	return out;
}

export type Span = 'day' | 'week' | 'month' | 'year';

/** Début et fin (exclue) de la période contenant `ref`, décalée de `offset`. */
export function periodRange(span: Span, offset: number, ref: Date = new Date()): [Date, Date] {
	const d = new Date(ref.getFullYear(), ref.getMonth(), ref.getDate());
	if (span === 'day') {
		d.setDate(d.getDate() + offset);
		const end = new Date(d);
		end.setDate(end.getDate() + 1);
		return [d, end];
	}
	if (span === 'week') {
		// semaine du lundi au dimanche
		d.setDate(d.getDate() - ((d.getDay() + 6) % 7) + offset * 7);
		const end = new Date(d);
		end.setDate(end.getDate() + 7);
		return [d, end];
	}
	if (span === 'month') {
		const start = new Date(d.getFullYear(), d.getMonth() + offset, 1);
		return [start, new Date(start.getFullYear(), start.getMonth() + 1, 1)];
	}
	const start = new Date(d.getFullYear() + offset, 0, 1);
	return [start, new Date(start.getFullYear() + 1, 0, 1)];
}

/** Grain des statistiques demandées pour une période. */
export function bucketOf(span: Span): 'hour' | 'day' | 'month' {
	return span === 'day' ? 'hour' : span === 'year' ? 'month' : 'day';
}

export interface StatRow {
	start: number;
	change?: number | null;
}

/** Somme des variations (kWh) sur la période. */
export function total(rows: StatRow[] | undefined): number {
	return (rows ?? []).reduce((a, r) => a + (typeof r.change === 'number' ? r.change : 0), 0);
}

/** Additionne plusieurs compteurs créneau par créneau. */
export function mergeSeries(series: (StatRow[] | undefined)[]): { start: number; value: number }[] {
	const by = new Map<number, number>();
	for (const rows of series)
		for (const r of rows ?? [])
			by.set(r.start, (by.get(r.start) ?? 0) + (typeof r.change === 'number' ? r.change : 0));
	return [...by.entries()].sort((a, b) => a[0] - b[0]).map(([start, value]) => ({ start, value }));
}

/** Garde les créneaux qui commencent dans [start, end[ (HA renvoie aussi celui de la borne de fin). */
export function clip(rows: StatRow[] | undefined, start: Date, end: Date): StatRow[] {
	return (rows ?? []).filter((r) => r.start >= start.getTime() && r.start < end.getTime());
}

export type Unit = 'hour' | 'day' | 'month';

/** Début du créneau (heure locale) qui contient `t`. */
export function bucketStart(t: number, unit: Unit): number {
	const d = new Date(t);
	if (unit === 'hour')
		return new Date(d.getFullYear(), d.getMonth(), d.getDate(), d.getHours()).getTime();
	if (unit === 'day') return new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
	return new Date(d.getFullYear(), d.getMonth(), 1).getTime();
}

/**
 * Regroupe des variations (5 minutes, heures…) en créneaux plus larges.
 * Sert à afficher l'heure en cours : HA ne calcule ses statistiques horaires
 * qu'à la fin de chaque heure, mais tient des statistiques sur 5 minutes.
 */
export function aggregate(rows: StatRow[] | undefined, unit: Unit): StatRow[] {
	const by = new Map<number, number>();
	for (const r of rows ?? []) {
		const k = bucketStart(r.start, unit);
		by.set(k, (by.get(k) ?? 0) + (typeof r.change === 'number' ? r.change : 0));
	}
	return [...by.entries()]
		.sort((a, b) => a[0] - b[0])
		.map(([start, change]) => ({ start, change }));
}

/** Tous les créneaux de la période, même vides (axe régulier). */
export function slots(start: Date, end: Date, unit: Unit): number[] {
	const out: number[] = [];
	const d = new Date(start);
	while (d < end) {
		out.push(d.getTime());
		if (unit === 'hour') d.setHours(d.getHours() + 1);
		else if (unit === 'day') d.setDate(d.getDate() + 1);
		else d.setMonth(d.getMonth() + 1);
	}
	return out;
}

/** Granularité des statistiques demandées à HA pour une période. */
export function fetchPeriod(span: Span, offset: number): '5minute' | 'hour' | 'day' | 'month' {
	// HA garde ~10 jours de statistiques sur 5 minutes
	if (span === 'day') return offset >= -9 ? '5minute' : 'hour';
	return bucketOf(span);
}

export interface Pricing {
	/** Statistiques de coût calculées par HA (€). */
	costStats: string[];
	/** Prix fixe (€/kWh). */
	price?: number;
	/** Entité donnant le prix courant (€/kWh). */
	priceEntity?: string;
}

/** Tarif du réseau configuré dans HA, s'il y en a un. */
export function gridPricing(prefs: unknown): Pricing {
	const p = (prefs ?? {}) as Raw;
	const out: Pricing = { costStats: [] };
	for (const src of arr(p.energy_sources)) {
		if (src.type !== 'grid') continue;
		for (const f of [src, ...arr(src.flow_from)]) {
			const cost = str(f.stat_cost);
			if (cost) out.costStats.push(cost);
			if (typeof f.number_energy_price === 'number') out.price = f.number_energy_price;
			const e = str(f.entity_energy_price);
			if (e) out.priceEntity = e;
		}
	}
	return out;
}

/** Variation relative (en %) d'une période sur la précédente, si calculable. */
export function changePct(now: number, before: number): number | undefined {
	if (!(before > 0)) return undefined;
	return Math.round(((now - before) / before) * 100);
}

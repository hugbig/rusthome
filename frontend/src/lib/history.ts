import { isNumericState } from './format';
import type { EntityState } from './types';

/**
 * Historique chronologique sans doublons. Un changement d'attribut seul crée
 * une ligne avec le même `last_changed` : on ne garde que les vrais changements
 * d'état, sinon la courbe repartirait en arrière dans le temps.
 */
export function normalizeHistory(history: EntityState[]): EntityState[] {
	const sorted = [...history].sort(
		(a, b) =>
			Date.parse(a.last_changed) - Date.parse(b.last_changed) ||
			Date.parse(a.last_updated) - Date.parse(b.last_updated)
	);
	return sorted.filter((s, i) => {
		const prev = sorted[i - 1];
		return !prev || prev.state !== s.state || prev.last_changed !== s.last_changed;
	});
}

export interface Point {
	t: number; // ms epoch
	v: number;
}

/** Points numériques ; les états non numériques (unavailable…) coupent la courbe. */
export function toSeries(history: EntityState[]): (Point | null)[] {
	return history.map((s) =>
		isNumericState(s.state) ? { t: Date.parse(s.last_changed), v: Number(s.state) } : null
	);
}

export interface Segment {
	state: string;
	from: number;
	to: number;
}

/**
 * Segments d'une frise d'états entre `from` et `to`. Les changements
 * successifs vers le même état sont fusionnés ; le dernier segment court
 * jusqu'à `to` (état courant).
 */
export function toSegments(history: EntityState[], from: number, to: number): Segment[] {
	const out: Segment[] = [];
	for (const s of history) {
		const t = Math.max(Date.parse(s.last_changed), from);
		if (t > to) break;
		const last = out.at(-1);
		if (last && last.state === s.state) continue;
		if (last) last.to = t;
		out.push({ state: s.state, from: t, to });
	}
	return out.filter((seg) => seg.to > seg.from);
}

/** Bornes « propres » d'un axe numérique, avec marge. */
export function niceDomain(values: number[]): [number, number] {
	if (values.length === 0) return [0, 1];
	let lo = Math.min(...values);
	let hi = Math.max(...values);
	if (lo === hi) {
		lo -= 1;
		hi += 1;
	}
	const step = niceStep((hi - lo) / 4);
	return [Math.floor(lo / step) * step, Math.ceil(hi / step) * step];
}

export function niceStep(raw: number): number {
	const pow = 10 ** Math.floor(Math.log10(raw));
	const n = raw / pow;
	return (n <= 1 ? 1 : n <= 2 ? 2 : n <= 5 ? 5 : 10) * pow;
}

export function ticks([lo, hi]: [number, number]): number[] {
	const step = niceStep((hi - lo) / 4);
	const out: number[] = [];
	for (let v = lo; v <= hi + step / 2; v += step) out.push(Number(v.toFixed(10)));
	return out;
}

/**
 * Couleurs de la frise : ordre catégoriel fixe (bleu RustHome d'abord), validé
 * avec scripts/validate_palette.js en clair (#fff) et en sombre (#161a20).
 * Les états « éteints » et indisponibles sont neutres, jamais colorés.
 */
export const CATEGORICAL_LIGHT = ['#1F5BC4', '#eb6834', '#1baf7a', '#eda100', '#e87ba4', '#008300'];
export const CATEGORICAL_DARK = ['#3987e5', '#d95926', '#199e70', '#c98500', '#d55181', '#008300'];
const NEUTRAL = new Set(['off', 'idle', 'disarmed', 'not_home', 'closed', 'standby', 'paused']);

/** Index de couleur par état, stable : dépend de l'ordre d'apparition, pas du rang. */
export function stateColorIndex(states: string[]): Map<string, number | 'neutral' | 'muted'> {
	const out = new Map<string, number | 'neutral' | 'muted'>();
	let next = 0;
	for (const s of states) {
		if (out.has(s)) continue;
		if (s === 'unavailable' || s === 'unknown') out.set(s, 'muted');
		else if (NEUTRAL.has(s)) out.set(s, 'neutral');
		else out.set(s, next < CATEGORICAL_LIGHT.length ? next++ : 'neutral');
	}
	return out;
}

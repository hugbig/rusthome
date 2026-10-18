// Documents de dashboard (M4) : forme Lovelace, cartes natives uniquement
// (cf. crates/rusthome-dashboard). Décrits à la main, comme les automatisations.
import { domainOf } from '../format';
import type { EntityState } from '../types';

export interface Action {
	action: 'more-info' | 'toggle' | 'navigate' | 'perform-action' | 'none' | 'url';
	navigation_path?: string;
	url_path?: string;
	perform_action?: string;
	target?: Record<string, unknown>;
	data?: Record<string, unknown>;
}

/** Une carte : `type` plus des champs propres au type, lus à la volée. */
export interface Card {
	type: string;
	entity?: string;
	name?: string;
	icon?: string;
	cards?: Card[];
	tap_action?: Action;
	[key: string]: unknown;
}

export interface Section {
	title?: string;
	column_span?: number;
	cards: Card[];
}

export interface View {
	type: 'sections' | 'masonry' | 'panel';
	title?: string;
	path?: string;
	icon?: string;
	max_columns?: number;
	sections?: Section[];
	cards?: Card[];
	badges?: { entity: string }[];
}

export interface DashboardDoc {
	url_path: string;
	title?: string;
	icon?: string;
	show_in_sidebar?: boolean;
	require_admin?: boolean;
	kiosk?: boolean;
	/** Onglets réduits à leur icône (le titre reste en info-bulle). */
	tabs_icon_only?: boolean;
	views: View[];
}

export interface DashboardSummary {
	url_path: string;
	title: string;
	icon: string | null;
	show_in_sidebar: boolean;
	require_admin: boolean;
	kiosk: boolean;
	views: number;
}

export interface DashboardIssue {
	path: string;
	message: string;
}

export interface DashboardResponse {
	summary: DashboardSummary;
	doc: DashboardDoc;
	yaml: string;
	issues: DashboardIssue[];
	editable: boolean;
}

/** Domaines qu'un clic bascule (service `<domaine>.toggle`). */
const TOGGLEABLE = new Set([
	'light',
	'switch',
	'fan',
	'input_boolean',
	'siren',
	'automation',
	'cover',
	'media_player'
]);

export function isToggleable(entityId: string): boolean {
	return TOGGLEABLE.has(domainOf(entityId));
}

/** États « actifs » (couleur primaire sur les tuiles). */
export function isActive(s: EntityState | undefined): boolean {
	if (!s) return false;
	return ['on', 'open', 'home', 'playing', 'heat', 'triggered', 'unlocked'].includes(s.state);
}

const STATE_LABELS: Record<string, string> = {
	on: 'Allumé',
	off: 'Éteint',
	unavailable: 'Indisponible',
	unknown: 'Inconnu',
	home: 'Maison',
	not_home: 'Absent',
	open: 'Ouvert',
	closed: 'Fermé',
	playing: 'Lecture',
	paused: 'En pause',
	idle: 'Inactif',
	standby: 'Veille',
	disarmed: 'Désarmée',
	armed_away: 'Armée (absence)',
	armed_home: 'Armée (présence)',
	armed_night: 'Armée (nuit)',
	armed_vacation: 'Armée (vacances)',
	arming: 'Armement…',
	pending: 'Délai d’entrée',
	triggered: 'Déclenchée',
	heat: 'Chauffe',
	auto: 'Auto',
	cool: 'Froid',
	sunny: 'Ensoleillé',
	'clear-night': 'Nuit claire',
	exceptional: 'Exceptionnel',
	hail: 'Grêle',
	'snowy-rainy': 'Neige et pluie',
	'windy-variant': 'Venteux et nuageux',
	cloudy: 'Nuageux',
	partlycloudy: 'Éclaircies',
	rainy: 'Pluie',
	pouring: 'Averses',
	snowy: 'Neige',
	fog: 'Brouillard',
	windy: 'Venteux',
	lightning: 'Orage',
	'lightning-rainy': 'Orage pluvieux'
};

const numberFmt = new Intl.NumberFormat('fr-FR', { maximumFractionDigits: 1 });

/** « 21,5 °C », « Allumé », « Indisponible »… */
export function formatState(s: EntityState | undefined, decimals?: number): string {
	if (!s) return 'Introuvable';
	const n = Number(s.state);
	const unit = s.attributes.unit_of_measurement;
	if (s.state.trim() !== '' && Number.isFinite(n)) {
		const fmt =
			decimals === undefined
				? numberFmt
				: new Intl.NumberFormat('fr-FR', {
						minimumFractionDigits: decimals,
						maximumFractionDigits: decimals
					});
		return typeof unit === 'string' && unit ? `${fmt.format(n)} ${unit}` : fmt.format(n);
	}
	return onOffLabel(s) ?? stateLabel(s.state);
}

/** `[on, off]` d'un capteur binaire selon sa classe (libellés de HA en français). */
const BINARY_CLASSES: Record<string, [string, string]> = {
	battery: ['Faible', 'Normale'],
	battery_charging: ['En charge', 'Pas en charge'],
	carbon_monoxide: ['Détecté', 'Dégagé'],
	cold: ['Froid', 'Normal'],
	connectivity: ['Connecté', 'Déconnecté'],
	door: ['Ouverte', 'Fermée'],
	garage_door: ['Ouverte', 'Fermée'],
	gas: ['Détecté', 'Dégagé'],
	heat: ['Chaud', 'Normal'],
	light: ['Lumière détectée', 'Pas de lumière'],
	lock: ['Déverrouillé', 'Verrouillé'],
	moisture: ['Humide', 'Sec'],
	motion: ['Détecté', 'Rien'],
	moving: ['En mouvement', 'Immobile'],
	occupancy: ['Détecté', 'Rien'],
	opening: ['Ouvert', 'Fermé'],
	plug: ['Branché', 'Débranché'],
	power: ['Alimenté', 'Non alimenté'],
	presence: ['Présent', 'Absent'],
	problem: ['Problème', 'OK'],
	running: ['En marche', 'À l’arrêt'],
	safety: ['Dangereux', 'Sûr'],
	smoke: ['Détectée', 'Dégagée'],
	sound: ['Détecté', 'Rien'],
	tamper: ['Sabotage', 'OK'],
	update: ['Mise à jour disponible', 'À jour'],
	vibration: ['Détectée', 'Rien'],
	window: ['Ouverte', 'Fermée']
};

/** Domaines où `on`/`off` veut dire allumé/éteint. */
const SWITCHED = new Set([
	'light',
	'switch',
	'fan',
	'input_boolean',
	'siren',
	'media_player',
	'remote',
	'humidifier',
	'water_heater'
]);

/**
 * Libellé de `on`/`off` selon le domaine : « Allumé » n'a de sens que pour ce
 * qui s'allume. Un capteur texte qui vaut `off` reste `off`.
 */
function onOffLabel(s: EntityState): string | undefined {
	if (s.state !== 'on' && s.state !== 'off') return undefined;
	const on = s.state === 'on';
	const d = domainOf(s.entity_id);
	if (d === 'update') return on ? 'Mise à jour disponible' : 'À jour';
	if (d === 'automation' || d === 'script') return on ? 'Activée' : 'Désactivée';
	if (d === 'binary_sensor') {
		const pair = BINARY_CLASSES[String(s.attributes.device_class ?? '')];
		return pair ? pair[on ? 0 : 1] : on ? 'Actif' : 'Inactif';
	}
	if (SWITCHED.has(d)) return on ? 'Allumé' : 'Éteint';
	return s.state;
}

export function stateLabel(state: string): string {
	return STATE_LABELS[state] ?? state;
}

/** Nom d'une carte : `name` explicite, sinon friendly_name, sinon l'entity_id. */
export function cardName(card: Card, s: EntityState | undefined): string {
	if (typeof card.name === 'string' && card.name) return card.name;
	const fn = s?.attributes.friendly_name;
	if (typeof fn === 'string' && fn) return fn;
	return card.entity ?? '';
}

export type Severity = 'good' | 'warning' | 'critical';

/**
 * Sévérité d'une jauge, façon HA : le plus grand seuil inférieur ou égal à la
 * valeur donne la couleur (`{green: 0, yellow: 75, red: 90}` : 80 → warning).
 */
export function severityOf(
	value: number,
	severity: Record<string, number> | undefined
): Severity | null {
	if (!severity) return null;
	const map: Record<string, Severity> = { green: 'good', yellow: 'warning', red: 'critical' };
	let best: { at: number; s: Severity } | null = null;
	for (const [color, at] of Object.entries(severity)) {
		const s = map[color];
		if (!s || typeof at !== 'number' || at > value) continue;
		if (!best || at > best.at) best = { at, s };
	}
	return best?.s ?? null;
}

/** Position de la valeur entre min et max, bornée à [0, 1]. */
export function fraction(value: number, min: number, max: number): number {
	if (max <= min) return 0;
	return Math.min(1, Math.max(0, (value - min) / (max - min)));
}

/** Action d'un clic : celle de la carte, sinon bascule ou fiche selon le domaine. */
export function tapAction(card: Card, fallback: 'toggle' | 'more-info' = 'more-info'): Action {
	if (card.tap_action) return card.tap_action;
	if (fallback === 'toggle' && card.entity && isToggleable(card.entity))
		return { action: 'toggle' };
	return { action: card.entity ? 'more-info' : 'none' };
}

/** `light.turn_on` → `['light', 'turn_on']`. */
export function splitService(name: string): [string, string] | null {
	// `domaine.service` strict : une valeur venue d'un tableau (éditable par un
	// résident) ne doit pas pouvoir viser une autre route de l'API (revue n° 5).
	const m = /^([a-z0-9_]+)\.([a-z0-9_]+)$/.exec(name.trim());
	return m ? [m[1]!, m[2]!] : null;
}

/** Adresse d'une action `url` : http(s) ou chemin du site, jamais `javascript:`. */
export function safeUrl(u: string): boolean {
	const t = u.trim();
	if (t.startsWith('/') && !t.startsWith('//')) return true;
	return /^https?:\/\//i.test(t);
}

/** Popups : tous les `#hash` d'un ensemble de cartes, récursivement. */
export function popupHashes(cards: Card[]): string[] {
	const out: string[] = [];
	for (const c of cards) {
		if (c.type === 'popup' && typeof c.hash === 'string') out.push(c.hash);
		if (Array.isArray(c.cards)) out.push(...popupHashes(c.cards));
	}
	return out;
}

/** Cartes d'une vue, quelle que soit sa disposition. */
export function viewCards(v: View): Card[] {
	return v.type === 'sections' ? (v.sections ?? []).flatMap((s) => s.cards) : (v.cards ?? []);
}

/** Largeur d'une carte dans une section : les petites cartes vont par deux. */
export function isHalfWidth(c: Card): boolean {
	return c.type === 'tile' || c.type === 'button';
}

/** Identifiant d'URL d'une vue : `path`, sinon son index. */
export function viewKey(v: View, i: number): string {
	return v.path ?? String(i);
}

/** sRGB approché d'une température de blanc (formule de Tanner Helland). */
export function kelvinToRgb(kelvin: number): [number, number, number] {
	const t = Math.min(40000, Math.max(1000, kelvin)) / 100;
	const clamp = (v: number) => Math.round(Math.min(255, Math.max(0, v)));
	const r = t <= 66 ? 255 : 329.698727446 * (t - 60) ** -0.1332047592;
	const g =
		t <= 66
			? 99.4708025861 * Math.log(t) - 161.1195681661
			: 288.1221695283 * (t - 60) ** -0.0755148492;
	const b = t >= 66 ? 255 : t <= 19 ? 0 : 138.5177312231 * Math.log(t - 10) - 305.0447927307;
	return [clamp(r), clamp(g), clamp(b)];
}

/** Jaune chaud des lumières allumées sans couleur connue (comme HA). */
export const LIGHT_ON = 'rgb(255 183 77)';

/**
 * Couleur d'accent d'une entité active : la couleur d'une lumière (ou de
 * son blanc), sinon `undefined` (couleur principale du thème).
 */
export function entityColor(s: EntityState | undefined): string | undefined {
	if (!s || s.state !== 'on' || domainOf(s.entity_id) !== 'light') return undefined;
	const rgb = s.attributes.rgb_color;
	if (Array.isArray(rgb) && rgb.length === 3) {
		const [r, g, b] = rgb.map(Number) as [number, number, number];
		// un blanc pur se lirait mal sur fond clair : on garde le jaune des lampes
		if (r > 240 && g > 240 && b > 240) return LIGHT_ON;
		return `rgb(${r} ${g} ${b})`;
	}
	const k = s.attributes.color_temp_kelvin;
	if (typeof k === 'number') {
		const [r, g, b] = kelvinToRgb(k);
		return k > 4500 ? LIGHT_ON : `rgb(${r} ${g} ${b})`;
	}
	return LIGHT_ON;
}

/** Capteur numérique avec unité : affiché en grand, avec sa courbe. */
export function isMeasurement(s: EntityState | undefined): boolean {
	if (!s) return false;
	const d = domainOf(s.entity_id);
	return (
		(d === 'sensor' || d === 'number' || d === 'input_number') &&
		typeof s.attributes.unit_of_measurement === 'string' &&
		s.state.trim() !== '' &&
		Number.isFinite(Number(s.state))
	);
}

/**
 * Zones de couleur d'une jauge, en fractions de l'arc : chaque seuil colore
 * l'arc jusqu'au seuil suivant (même règle que `severityOf`).
 */
export function severityZones(
	severity: Record<string, number> | undefined,
	min: number,
	max: number
): { from: number; to: number; level: Severity }[] {
	if (!severity) return [];
	const map: Record<string, Severity> = { green: 'good', yellow: 'warning', red: 'critical' };
	const steps = Object.entries(severity)
		.filter(([c, v]) => map[c] && typeof v === 'number')
		.map(([c, v]) => ({ at: v, level: map[c]! }))
		.sort((a, b) => a.at - b.at);
	return steps
		.map((st, i) => ({
			from: fraction(st.at, min, max),
			to: i + 1 < steps.length ? fraction(steps[i + 1]!.at, min, max) : 1,
			level: st.level
		}))
		.filter((z) => z.to > z.from);
}

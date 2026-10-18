// Disposition des vues et opérations d'édition, en fonctions pures (testées).
import type { Card, DashboardDoc, Section, View } from './model';

/** Hauteur estimée d'une carte, en lignes (même idée que getCardSize de HA). */
export function cardSize(c: Card): number {
	const kids = Array.isArray(c.cards) ? c.cards : [];
	switch (c.type) {
		case 'popup':
			return 0;
		case 'heading':
		case 'tile':
		case 'button':
		case 'chips':
		case 'unsupported':
			return 1;
		case 'entities':
			return 1 + (Array.isArray(c.entities) ? c.entities.length : 0);
		case 'gauge':
		case 'media-control':
			return 2;
		case 'camera':
		case 'graph':
		case 'weather':
			return 3;
		case 'todo-list':
			return 3;
		case 'logbook':
		case 'media-list':
		case 'calendar':
		case 'map':
		case 'thermostat':
		case 'alarm-panel':
			return 4;
		case 'vertical-stack':
			return kids.reduce((n, k) => n + cardSize(k), 0);
		case 'horizontal-stack':
			return Math.max(0, ...kids.map(cardSize));
		case 'grid': {
			const cols = typeof c.columns === 'number' && c.columns > 0 ? c.columns : 3;
			const rows = Math.ceil(kids.length / cols);
			return (c.title ? 1 : 0) + rows * Math.max(0, ...kids.map(cardSize));
		}
		default:
			return 1;
	}
}

/**
 * Répartit les cartes en colonnes (vue masonry) : chaque carte va dans la
 * colonne la moins haute, dans l'ordre. Renvoie les index par colonne.
 */
export function masonry(cards: Card[], columns: number): number[][] {
	const cols = Math.max(1, Math.min(columns, cards.length || 1));
	const out: number[][] = Array.from({ length: cols }, () => []);
	const heights = new Array<number>(cols).fill(0);
	cards.forEach((c, i) => {
		let best = 0;
		for (let k = 1; k < cols; k++) if (heights[k]! < heights[best]!) best = k;
		out[best]!.push(i);
		heights[best]! += Math.max(1, cardSize(c));
	});
	return out;
}

/** Nombre de colonnes pour une largeur donnée, borné par `max_columns`. */
export function columnsFor(width: number, minWidth: number, max: number | undefined): number {
	return Math.max(1, Math.min(max ?? 4, Math.floor(width / minWidth)));
}

// --- Édition : une carte de premier niveau est repérée par sa vue, sa
// section (vues « sections ») et son rang. ---

export interface CardLoc {
	view: number;
	section?: number;
	index: number;
}

function listOf(doc: DashboardDoc, loc: Omit<CardLoc, 'index'>): Card[] | undefined {
	const v: View | undefined = doc.views[loc.view];
	if (!v) return undefined;
	if (loc.section !== undefined) return v.sections?.[loc.section]?.cards;
	return (v.cards ??= []);
}

/** Copie profonde du document (les opérations ne modifient pas l'original). */
export function clone<T>(x: T): T {
	return JSON.parse(JSON.stringify(x)) as T;
}

export function moveCard(doc: DashboardDoc, loc: CardLoc, delta: -1 | 1): DashboardDoc {
	const out = clone(doc);
	const list = listOf(out, loc);
	const j = loc.index + delta;
	if (!list || j < 0 || j >= list.length) return doc;
	[list[loc.index], list[j]] = [list[j]!, list[loc.index]!];
	return out;
}

export function removeCard(doc: DashboardDoc, loc: CardLoc): DashboardDoc {
	const out = clone(doc);
	listOf(out, loc)?.splice(loc.index, 1);
	return out;
}

export function replaceCard(doc: DashboardDoc, loc: CardLoc, card: Card): DashboardDoc {
	const out = clone(doc);
	const list = listOf(out, loc);
	if (list && loc.index < list.length) list[loc.index] = card;
	return out;
}

/** Ajoute en fin de liste (section ou vue). */
export function appendCard(
	doc: DashboardDoc,
	where: Omit<CardLoc, 'index'>,
	card: Card
): DashboardDoc {
	const out = clone(doc);
	listOf(out, where)?.push(card);
	return out;
}

export function appendSection(doc: DashboardDoc, view: number): DashboardDoc {
	const out = clone(doc);
	const v = out.views[view];
	if (v?.type === 'sections') (v.sections ??= []).push({ cards: [] });
	return out;
}

/** Déplace une carte vers une autre liste (glisser-déposer entre sections). */
export function moveCardTo(
	doc: DashboardDoc,
	from: CardLoc,
	to: Omit<CardLoc, 'index'> & { index?: number }
): DashboardDoc {
	const out = clone(doc);
	const src = listOf(out, from);
	const card = src?.[from.index];
	if (!src || !card) return doc;
	src.splice(from.index, 1);
	const dst = listOf(out, to);
	if (!dst) return doc;
	let at = to.index ?? dst.length;
	// même liste : l'index cible a glissé d'un cran après le retrait
	if (src === dst && to.index !== undefined && to.index > from.index) at -= 1;
	dst.splice(Math.max(0, Math.min(at, dst.length)), 0, card);
	return out;
}

export function duplicateCard(doc: DashboardDoc, loc: CardLoc): DashboardDoc {
	const out = clone(doc);
	const list = listOf(out, loc);
	const card = list?.[loc.index];
	if (list && card) list.splice(loc.index + 1, 0, clone(card));
	return out;
}

export function removeSection(doc: DashboardDoc, view: number, section: number): DashboardDoc {
	const out = clone(doc);
	out.views[view]?.sections?.splice(section, 1);
	return out;
}

export function moveSection(
	doc: DashboardDoc,
	view: number,
	section: number,
	delta: -1 | 1
): DashboardDoc {
	const out = clone(doc);
	const list = out.views[view]?.sections;
	const j = section + delta;
	if (!list || j < 0 || j >= list.length) return doc;
	[list[section], list[j]] = [list[j]!, list[section]!];
	return out;
}

export function updateSection(
	doc: DashboardDoc,
	view: number,
	section: number,
	patch: Partial<Section>
): DashboardDoc {
	const out = clone(doc);
	const s = out.views[view]?.sections?.[section];
	if (s) Object.assign(s, patch);
	return out;
}

// --- Vues ---

/** Une nouvelle vue « sections », comme celles que crée HA par défaut. */
export function addView(doc: DashboardDoc, title: string): DashboardDoc {
	const out = clone(doc);
	out.views.push({ type: 'sections', title, max_columns: 4, sections: [{ cards: [] }] });
	return out;
}

/**
 * Remplace les réglages d'une vue. Changer de disposition convertit le
 * contenu : les sections se mettent à plat, ou les cartes forment une section.
 */
export function updateView(doc: DashboardDoc, index: number, patch: Partial<View>): DashboardDoc {
	const out = clone(doc);
	const v = out.views[index];
	if (!v) return doc;
	const before = v.type;
	Object.assign(v, patch);
	for (const k of Object.keys(v) as (keyof View)[])
		if (v[k] === '' || v[k] === undefined) delete v[k];
	if (before !== 'sections' && v.type === 'sections') {
		v.sections = [{ cards: v.cards ?? [] }];
		delete v.cards;
	} else if (before === 'sections' && v.type !== 'sections') {
		v.cards = (v.sections ?? []).flatMap((s) => s.cards);
		delete v.sections;
	}
	return out;
}

export function removeView(doc: DashboardDoc, index: number): DashboardDoc {
	const out = clone(doc);
	out.views.splice(index, 1);
	return out;
}

export function moveView(doc: DashboardDoc, index: number, delta: -1 | 1): DashboardDoc {
	const out = clone(doc);
	const j = index + delta;
	if (j < 0 || j >= out.views.length) return doc;
	[out.views[index], out.views[j]] = [out.views[j]!, out.views[index]!];
	return out;
}

// --- Taille dans une grille de section (12 colonnes, comme HA) ---

/** Colonnes par défaut : les petites cartes vont par deux, les autres en pleine largeur. */
const HALF = new Set(['tile', 'button', 'gauge']);

export function gridColumns(c: Card): number {
	const raw = (c.grid_options as { columns?: unknown } | undefined)?.columns;
	if (raw === 'full') return 12;
	if (typeof raw === 'number' && raw > 0) return Math.min(12, Math.round(raw));
	return HALF.has(c.type) ? 6 : 12;
}

/** Hauteur imposée en lignes (56 px chacune), si la carte en fixe une. */
export function gridRows(c: Card): number | undefined {
	const raw = (c.grid_options as { rows?: unknown } | undefined)?.rows;
	return typeof raw === 'number' && raw > 0 ? raw : undefined;
}

/** Fixe la taille ; `undefined` revient au défaut de la carte. */
export function withGrid(c: Card, columns?: number | 'full', rows?: number): Card {
	const out = clone(c);
	const g: Record<string, unknown> = {};
	if (columns !== undefined) g.columns = columns;
	if (rows !== undefined) g.rows = rows;
	if (Object.keys(g).length) out.grid_options = g;
	else delete out.grid_options;
	return out;
}

/** Modèles proposés à l'ajout d'une carte (catalogue v1). */
export const CARD_TEMPLATES: Record<string, Card> = {
	tile: { type: 'tile', entity: '' },
	heading: { type: 'heading', heading: 'Titre' },
	entities: { type: 'entities', entities: [] },
	button: {
		type: 'button',
		name: 'Bouton',
		tap_action: { action: 'navigate', navigation_path: '#popup' }
	},
	chips: { type: 'chips', chips: [] },
	gauge: { type: 'gauge', entity: '', min: 0, max: 100 },
	'alarm-panel': { type: 'alarm-panel', entity: 'alarm_control_panel.alarmo' },
	camera: { type: 'camera', entity: '' },
	logbook: { type: 'logbook', entities: [], hours_to_show: 24 },
	thermostat: { type: 'thermostat', entity: '' },
	'media-control': { type: 'media-control', entity: '' },
	graph: { type: 'graph', entities: [], hours_to_show: 24 },
	weather: { type: 'weather', entity: '' },
	grid: { type: 'grid', columns: 2, cards: [] },
	'vertical-stack': { type: 'vertical-stack', cards: [] },
	'horizontal-stack': { type: 'horizontal-stack', cards: [] },
	popup: { type: 'popup', hash: '#popup', title: 'Pop-up', cards: [] },
	'media-list': { type: 'media-list', entity: '', title: 'Films', max: 3 },
	calendar: { type: 'calendar', entities: [], days: 7 },
	'todo-list': { type: 'todo-list', entity: '' },
	map: { type: 'map', entities: [] }
};

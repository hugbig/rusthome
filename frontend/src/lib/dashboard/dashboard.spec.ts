import { describe, expect, it } from 'vitest';
import type { EntityState } from '../types';
import {
	addView,
	appendCard,
	appendSection,
	duplicateCard,
	gridColumns,
	gridRows,
	moveCardTo,
	moveSection,
	removeView,
	updateView,
	withGrid,
	cardSize,
	columnsFor,
	masonry,
	moveCard,
	removeCard,
	replaceCard
} from './layout';
import type { DashboardDoc } from './model';
import {
	cardName,
	formatState,
	fraction,
	popupHashes,
	severityOf,
	splitService,
	tapAction
} from './model';

const st = (entity_id: string, state: string, attributes: Record<string, unknown> = {}) =>
	({
		entity_id,
		state,
		attributes,
		last_changed: '2026-10-17T10:00:00Z',
		last_updated: '2026-10-17T10:00:00Z',
		context: { id: 'c' }
	}) as unknown as EntityState;

describe('états', () => {
	it('formate nombres, unités et états connus', () => {
		expect(formatState(st('sensor.t', '21.46', { unit_of_measurement: '°C' }))).toBe('21,5 °C');
		expect(formatState(st('sensor.p', '1013.2'), 0)).toBe('1\u202f013');
		expect(formatState(st('light.a', 'on'))).toBe('Allumé');
		expect(formatState(st('alarm_control_panel.a', 'armed_away'))).toBe('Armée (absence)');
		expect(formatState(st('binary_sensor.m', 'on', { device_class: 'motion' }))).toBe('Détecté');
		expect(formatState(st('update.ha', 'off'))).toBe('À jour');
		// capteurs binaires : selon la classe, jamais « Éteint »
		expect(formatState(st('binary_sensor.rpi', 'off', { device_class: 'problem' }))).toBe('OK');
		expect(formatState(st('binary_sensor.v', 'off', { device_class: 'update' }))).toBe('À jour');
		expect(formatState(st('binary_sensor.x', 'on'))).toBe('Actif');
		// un capteur texte qui vaut « off » reste tel quel
		expect(formatState(st('sensor.rasp_fan_state', 'off'))).toBe('off');
		expect(formatState(st('automation.a', 'on'))).toBe('Activée');
		expect(formatState(undefined)).toBe('Introuvable');
	});

	it('nomme une carte : nom, friendly_name, entity_id', () => {
		const s = st('light.a', 'on', { friendly_name: 'Lampe' });
		expect(cardName({ type: 'tile', entity: 'light.a', name: 'Bureau' }, s)).toBe('Bureau');
		expect(cardName({ type: 'tile', entity: 'light.a' }, s)).toBe('Lampe');
		expect(cardName({ type: 'tile', entity: 'light.b' }, undefined)).toBe('light.b');
	});
});

describe('jauge', () => {
	it('prend le plus grand seuil atteint, comme HA', () => {
		const sev = { green: 0, yellow: 75, red: 90 };
		expect(severityOf(10, sev)).toBe('good');
		expect(severityOf(80, sev)).toBe('warning');
		expect(severityOf(95, sev)).toBe('critical');
		// seuils inversés (tireuse : vert au-dessus de 11)
		expect(severityOf(8, { green: 11, yellow: 5, red: 0 })).toBe('warning');
		expect(severityOf(5, undefined)).toBeNull();
	});

	it('borne la fraction', () => {
		expect(fraction(50, 0, 100)).toBe(0.5);
		expect(fraction(-5, 0, 100)).toBe(0);
		expect(fraction(1100, 950, 1050)).toBe(1);
		expect(fraction(1, 5, 5)).toBe(0);
	});
});

describe('actions', () => {
	it('bascule par défaut les domaines pilotables, fiche sinon', () => {
		expect(tapAction({ type: 'tile', entity: 'light.a' }, 'toggle')).toEqual({ action: 'toggle' });
		expect(tapAction({ type: 'tile', entity: 'sensor.a' }, 'toggle')).toEqual({
			action: 'more-info'
		});
		expect(tapAction({ type: 'button' })).toEqual({ action: 'none' });
		const nav = { action: 'navigate' as const, navigation_path: '#media' };
		expect(tapAction({ type: 'button', tap_action: nav })).toBe(nav);
	});

	it('découpe un nom de service', () => {
		expect(splitService('light.turn_on')).toEqual(['light', 'turn_on']);
		expect(splitService('invalide')).toBeNull();
	});

	it('trouve les pop-ups imbriqués', () => {
		expect(
			popupHashes([
				{ type: 'grid', cards: [{ type: 'popup', hash: '#media', cards: [] }] },
				{ type: 'popup', hash: '#nas' }
			])
		).toEqual(['#media', '#nas']);
	});
});

describe('disposition', () => {
	it('estime la hauteur des conteneurs', () => {
		expect(cardSize({ type: 'entities', entities: [{}, {}] })).toBe(3);
		expect(
			cardSize({
				type: 'grid',
				columns: 2,
				cards: [{ type: 'gauge' }, { type: 'gauge' }, { type: 'tile' }]
			})
		).toBe(4);
		expect(cardSize({ type: 'popup', cards: [{ type: 'camera' }] })).toBe(0);
	});

	it('remplit la colonne la moins haute', () => {
		const cards = [
			{ type: 'logbook' },
			{ type: 'tile' },
			{ type: 'tile' },
			{ type: 'tile' },
			{ type: 'gauge' }
		];
		expect(masonry(cards, 2)).toEqual([[0], [1, 2, 3, 4]]);
		expect(masonry([{ type: 'tile' }], 3)).toEqual([[0]]);
	});

	it('borne le nombre de colonnes', () => {
		expect(columnsFor(1200, 330, 3)).toBe(3);
		expect(columnsFor(700, 330, 3)).toBe(2);
		expect(columnsFor(200, 330, 3)).toBe(1);
	});
});

describe('édition', () => {
	const doc = (): DashboardDoc => ({
		url_path: 'x',
		views: [
			{
				type: 'sections',
				sections: [{ cards: [{ type: 'heading' }, { type: 'tile', entity: 'light.a' }] }]
			},
			{ type: 'masonry', cards: [] }
		]
	});

	it('déplace, remplace et supprime sans modifier l’original', () => {
		const d = doc();
		const moved = moveCard(d, { view: 0, section: 0, index: 1 }, -1);
		expect(moved.views[0]!.sections![0]!.cards.map((c) => c.type)).toEqual(['tile', 'heading']);
		expect(d.views[0]!.sections![0]!.cards[0]!.type).toBe('heading');
		expect(moveCard(d, { view: 0, section: 0, index: 0 }, -1)).toBe(d);
		const replaced = replaceCard(d, { view: 0, section: 0, index: 0 }, { type: 'gauge' });
		expect(replaced.views[0]!.sections![0]!.cards[0]!.type).toBe('gauge');
		expect(
			removeCard(d, { view: 0, section: 0, index: 0 }).views[0]!.sections![0]!.cards
		).toHaveLength(1);
	});

	it('ajoute cartes et sections', () => {
		const d = appendCard(doc(), { view: 1 }, { type: 'tile' });
		expect(d.views[1]!.cards).toHaveLength(1);
		expect(appendSection(d, 0).views[0]!.sections).toHaveLength(2);
		expect(appendSection(d, 1).views[1]!.sections).toBeUndefined();
	});
});

describe('édition avancée', () => {
	const doc = (): DashboardDoc => ({
		url_path: 'x',
		views: [
			{
				type: 'sections',
				sections: [
					{ cards: [{ type: 'heading' }, { type: 'tile', entity: 'light.a' }] },
					{ cards: [{ type: 'gauge' }] }
				]
			}
		]
	});

	it('déplace une carte d’une section à l’autre ou dans la même', () => {
		const d = moveCardTo(
			doc(),
			{ view: 0, section: 0, index: 1 },
			{ view: 0, section: 1, index: 0 }
		);
		expect(d.views[0]!.sections![0]!.cards.map((c) => c.type)).toEqual(['heading']);
		expect(d.views[0]!.sections![1]!.cards.map((c) => c.type)).toEqual(['tile', 'gauge']);
		const same = moveCardTo(doc(), { view: 0, section: 0, index: 0 }, { view: 0, section: 0 });
		expect(same.views[0]!.sections![0]!.cards.map((c) => c.type)).toEqual(['tile', 'heading']);
	});

	it('duplique une carte et réordonne les sections', () => {
		const d = duplicateCard(doc(), { view: 0, section: 0, index: 1 });
		expect(d.views[0]!.sections![0]!.cards).toHaveLength(3);
		expect(moveSection(doc(), 0, 1, -1).views[0]!.sections![0]!.cards[0]!.type).toBe('gauge');
	});

	it('change la disposition d’une vue sans perdre de carte', () => {
		const flat = updateView(doc(), 0, { type: 'masonry', title: 'Salon' });
		expect(flat.views[0]!.cards!.map((c) => c.type)).toEqual(['heading', 'tile', 'gauge']);
		expect(flat.views[0]!.sections).toBeUndefined();
		const back = updateView(flat, 0, { type: 'sections' });
		expect(back.views[0]!.sections![0]!.cards).toHaveLength(3);
		const two = addView(doc(), 'Cuisine');
		expect(two.views[1]).toMatchObject({ type: 'sections', title: 'Cuisine' });
		expect(removeView(two, 0).views[0]!.title).toBe('Cuisine');
	});

	it('taille dans la grille de section (12 colonnes)', () => {
		expect(gridColumns({ type: 'tile' })).toBe(6);
		expect(gridColumns({ type: 'entities' })).toBe(12);
		expect(gridColumns({ type: 'tile', grid_options: { columns: 'full' } })).toBe(12);
		const c = withGrid({ type: 'tile' }, 4, 2);
		expect([gridColumns(c), gridRows(c)]).toEqual([4, 2]);
		expect(withGrid(c, undefined, undefined).grid_options).toBeUndefined();
	});
});

describe('icônes', () => {
	it('retombe sur l’icône du domaine tant que le jeu complet n’est pas chargé', async () => {
		const { iconPath } = await import('./icons');
		const { mdiLightbulb } = await import('@mdi/js');
		expect(iconPath('mdi:sofa-outline', 'light.salon')).toBe(mdiLightbulb);
	});
});

describe('couleur des entités', () => {
	it('prend la couleur de la lumière, ou de son blanc', async () => {
		const { entityColor, isMeasurement, LIGHT_ON } = await import('./model');
		expect(entityColor(st('light.a', 'on', { rgb_color: [255, 0, 0] }))).toBe('rgb(255 0 0)');
		expect(entityColor(st('light.a', 'on', { color_temp_kelvin: 2200 }))).toMatch(/^rgb\(255 /);
		expect(entityColor(st('light.a', 'on', { rgb_color: [255, 255, 255] }))).toBe(LIGHT_ON);
		expect(entityColor(st('light.a', 'off', { rgb_color: [255, 0, 0] }))).toBeUndefined();
		expect(entityColor(st('switch.a', 'on'))).toBeUndefined();
		expect(isMeasurement(st('sensor.t', '21.4', { unit_of_measurement: '°C' }))).toBe(true);
		expect(isMeasurement(st('sensor.t', 'abc', { unit_of_measurement: '°C' }))).toBe(false);
	});
});

describe('zones de jauge', () => {
	it('colore chaque zone jusqu’au seuil suivant', async () => {
		const { severityZones } = await import('./model');
		expect(severityZones({ green: 0, yellow: 75, red: 90 }, 0, 100)).toEqual([
			{ from: 0, to: 0.75, level: 'good' },
			{ from: 0.75, to: 0.9, level: 'warning' },
			{ from: 0.9, to: 1, level: 'critical' }
		]);
		// seuils inversés (tireuse) : rouge en bas, vert en haut
		expect(severityZones({ green: 11, yellow: 5, red: 0 }, 0, 24).map((z) => z.level)).toEqual([
			'critical',
			'warning',
			'good'
		]);
		expect(severityZones(undefined, 0, 1)).toEqual([]);
	});
});

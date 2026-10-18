// Catalogue des cartes pour l'éditeur visuel : libellé, catégorie, modèle
// d'ajout et champs du formulaire. Le formulaire est généré depuis `fields`.
import type { Card } from '../model';

export type Field = { key: string; label: string; help?: string } & (
	| { kind: 'entity'; domains?: string[] }
	| { kind: 'entities'; domains?: string[] }
	| { kind: 'text' }
	| { kind: 'icon' }
	| { kind: 'number'; min?: number; max?: number; step?: number }
	| { kind: 'boolean' }
	| { kind: 'select'; options: [string, string][] }
	| { kind: 'action' }
	| { kind: 'cards' }
	| { kind: 'rows' }
	| { kind: 'chips' }
	| { kind: 'severity' }
	| { kind: 'features' }
	| { kind: 'alarm-states' }
);

export type Category = 'entity' | 'layout' | 'info' | 'media' | 'other';

export const CATEGORIES: [Category, string][] = [
	['entity', 'Par entité'],
	['info', 'Informations et graphiques'],
	['media', 'Médias et caméras'],
	['layout', 'Disposition'],
	['other', 'Autres']
];

export interface CardInfo {
	type: string;
	label: string;
	description: string;
	icon: string;
	category: Category;
	template: Card;
	fields: Field[];
}

const name: Field = {
	key: 'name',
	label: 'Nom',
	kind: 'text',
	help: 'Par défaut : le nom de l’entité'
};
const icon: Field = { key: 'icon', label: 'Icône', kind: 'icon' };
const tap: Field = { key: 'tap_action', label: 'Action au toucher', kind: 'action' };
const title: Field = { key: 'title', label: 'Titre', kind: 'text' };

export const CATALOG: CardInfo[] = [
	{
		type: 'tile',
		label: 'Tuile',
		description: 'Une entité, son état et une commande rapide.',
		icon: 'mdi:gesture-tap-button',
		category: 'entity',
		template: { type: 'tile', entity: '' },
		fields: [
			{ key: 'entity', label: 'Entité', kind: 'entity' },
			name,
			icon,
			{ key: 'vertical', label: 'Disposition verticale', kind: 'boolean' },
			tap,
			{ key: 'features', label: 'Commandes', kind: 'features' }
		]
	},
	{
		type: 'button',
		label: 'Bouton',
		description: 'Un grand bouton : bascule, navigation, ouverture d’un pop-up…',
		icon: 'mdi:gesture-tap-button',
		category: 'entity',
		template: { type: 'button', name: 'Bouton', icon: 'mdi:home' },
		fields: [{ key: 'entity', label: 'Entité (facultatif)', kind: 'entity' }, name, icon, tap]
	},
	{
		type: 'entities',
		label: 'Entités',
		description: 'Une liste d’entités, avec interrupteurs et curseurs.',
		icon: 'mdi:view-dashboard',
		category: 'entity',
		template: { type: 'entities', entities: [] },
		fields: [title, { key: 'entities', label: 'Lignes', kind: 'rows' }]
	},
	{
		type: 'chips',
		label: 'Puces',
		description: 'Une rangée de petites puces d’état.',
		icon: 'mdi:view-dashboard',
		category: 'entity',
		template: { type: 'chips', chips: [] },
		fields: [{ key: 'chips', label: 'Puces', kind: 'chips' }]
	},
	{
		type: 'gauge',
		label: 'Jauge',
		description: 'Une valeur entre un minimum et un maximum, avec des seuils.',
		icon: 'mdi:home-analytics',
		category: 'info',
		template: { type: 'gauge', entity: '', min: 0, max: 100 },
		fields: [
			{
				key: 'entity',
				label: 'Entité',
				kind: 'entity',
				domains: ['sensor', 'number', 'counter', 'input_number']
			},
			name,
			{ key: 'unit', label: 'Unité', kind: 'text' },
			{ key: 'min', label: 'Minimum', kind: 'number' },
			{ key: 'max', label: 'Maximum', kind: 'number' },
			{ key: 'decimals', label: 'Décimales', kind: 'number', min: 0, max: 4 },
			{ key: 'needle', label: 'Aiguille', kind: 'boolean' },
			{ key: 'severity', label: 'Seuils', kind: 'severity' }
		]
	},
	{
		type: 'alarm-panel',
		label: 'Alarme',
		description: 'État de l’alarme, armement et désarmement.',
		icon: 'mdi:shield-home',
		category: 'entity',
		template: { type: 'alarm-panel', entity: 'alarm_control_panel.alarmo' },
		fields: [
			{ key: 'entity', label: 'Centrale', kind: 'entity', domains: ['alarm_control_panel'] },
			name,
			{ key: 'states', label: 'Modes d’armement proposés', kind: 'alarm-states' }
		]
	},
	{
		type: 'thermostat',
		label: 'Thermostat',
		description: 'Consigne et température mesurée.',
		icon: 'mdi:thermometer',
		category: 'entity',
		template: { type: 'thermostat', entity: '' },
		fields: [{ key: 'entity', label: 'Entité', kind: 'entity', domains: ['climate'] }, name]
	},
	{
		type: 'graph',
		label: 'Graphique',
		description: 'Historique en courbe, ou en barres par jour.',
		icon: 'mdi:home-analytics',
		category: 'info',
		template: { type: 'graph', entities: [], hours_to_show: 24 },
		fields: [
			{ key: 'name', label: 'Titre', kind: 'text' },
			{ key: 'entities', label: 'Entités', kind: 'entities' },
			{ key: 'hours_to_show', label: 'Heures affichées', kind: 'number', min: 1 },
			{
				key: 'chart',
				label: 'Type',
				kind: 'select',
				options: [
					['', 'Courbe'],
					['bar', 'Barres par jour']
				]
			},
			{
				key: 'aggregate',
				label: 'Agrégat (barres)',
				kind: 'select',
				options: [
					['', 'Dernière valeur'],
					['max', 'Maximum']
				]
			}
		]
	},
	{
		type: 'logbook',
		label: 'Journal',
		description: 'Les derniers changements d’état de quelques entités.',
		icon: 'mdi:history',
		category: 'info',
		template: { type: 'logbook', entities: [], hours_to_show: 24 },
		fields: [
			title,
			{ key: 'entities', label: 'Entités', kind: 'entities' },
			{ key: 'hours_to_show', label: 'Heures affichées', kind: 'number', min: 1 }
		]
	},
	{
		type: 'weather',
		label: 'Météo',
		description: 'Conditions actuelles et informations Météo-France.',
		icon: 'mdi:weather-cloudy',
		category: 'info',
		template: { type: 'weather', entity: '' },
		fields: [{ key: 'entity', label: 'Entité', kind: 'entity', domains: ['weather'] }, name]
	},
	{
		type: 'calendar',
		label: 'Agenda',
		description: 'Les événements des prochains jours.',
		icon: 'mdi:calendar',
		category: 'info',
		template: { type: 'calendar', entities: [], days: 7 },
		fields: [
			title,
			{ key: 'entities', label: 'Agendas', kind: 'entities', domains: ['calendar'] },
			{ key: 'days', label: 'Jours affichés', kind: 'number', min: 1, max: 60 }
		]
	},
	{
		type: 'todo-list',
		label: 'Liste de tâches',
		description: 'Une liste de tâches ou de courses.',
		icon: 'mdi:view-dashboard',
		category: 'info',
		template: { type: 'todo-list', entity: '' },
		fields: [title, { key: 'entity', label: 'Liste', kind: 'entity', domains: ['todo'] }]
	},
	{
		type: 'map',
		label: 'Carte',
		description: 'La position de personnes et de traceurs.',
		icon: 'mdi:home',
		category: 'info',
		template: { type: 'map', entities: [] },
		fields: [
			title,
			{ key: 'entities', label: 'Entités', kind: 'entities', domains: ['person', 'device_tracker'] }
		]
	},
	{
		type: 'heading',
		label: 'Titre',
		description: 'Un titre de section, avec une icône.',
		icon: 'mdi:view-dashboard',
		category: 'layout',
		template: { type: 'heading', heading: 'Titre' },
		fields: [
			{ key: 'heading', label: 'Texte', kind: 'text' },
			icon,
			{
				key: 'heading_style',
				label: 'Style',
				kind: 'select',
				options: [
					['', 'Titre'],
					['subtitle', 'Sous-titre']
				]
			}
		]
	},
	{
		type: 'grid',
		label: 'Grille',
		description: 'Plusieurs cartes sur des colonnes.',
		icon: 'mdi:view-dashboard',
		category: 'layout',
		template: { type: 'grid', columns: 2, cards: [] },
		fields: [
			title,
			{ key: 'columns', label: 'Colonnes', kind: 'number', min: 1, max: 6 },
			{ key: 'square', label: 'Cases carrées', kind: 'boolean' },
			{ key: 'cards', label: 'Cartes', kind: 'cards' }
		]
	},
	{
		type: 'vertical-stack',
		label: 'Pile verticale',
		description: 'Des cartes les unes sous les autres.',
		icon: 'mdi:view-dashboard',
		category: 'layout',
		template: { type: 'vertical-stack', cards: [] },
		fields: [title, { key: 'cards', label: 'Cartes', kind: 'cards' }]
	},
	{
		type: 'horizontal-stack',
		label: 'Pile horizontale',
		description: 'Des cartes côte à côte.',
		icon: 'mdi:view-dashboard',
		category: 'layout',
		template: { type: 'horizontal-stack', cards: [] },
		fields: [title, { key: 'cards', label: 'Cartes', kind: 'cards' }]
	},
	{
		type: 'popup',
		label: 'Pop-up',
		description: 'Une fenêtre ouverte par un bouton (action « naviguer » vers #ancre).',
		icon: 'mdi:view-dashboard',
		category: 'layout',
		template: { type: 'popup', hash: '#popup', title: 'Pop-up', cards: [] },
		fields: [
			{ key: 'hash', label: 'Ancre (#media…)', kind: 'text' },
			title,
			{ key: 'cards', label: 'Contenu', kind: 'cards' }
		]
	},
	{
		type: 'camera',
		label: 'Caméra',
		description: 'L’image d’une caméra, rafraîchie toutes les 10 s.',
		icon: 'mdi:cctv',
		category: 'media',
		template: { type: 'camera', entity: '' },
		fields: [{ key: 'entity', label: 'Caméra', kind: 'entity', domains: ['camera'] }, name]
	},
	{
		type: 'media-control',
		label: 'Lecteur média',
		description: 'Lecture, pause, morceau suivant, volume.',
		icon: 'mdi:speaker',
		category: 'media',
		template: { type: 'media-control', entity: '' },
		fields: [
			{ key: 'entity', label: 'Lecteur', kind: 'entity', domains: ['media_player'] },
			name,
			{ key: 'volume', label: 'Curseur de volume', kind: 'boolean' }
		]
	},
	{
		type: 'media-list',
		label: 'Derniers ajouts',
		description: 'Les derniers films ou séries (Emby…).',
		icon: 'mdi:multimedia',
		category: 'media',
		template: { type: 'media-list', entity: '', title: 'Films', max: 3 },
		fields: [
			title,
			{ key: 'entity', label: 'Capteur', kind: 'entity', domains: ['sensor'] },
			{ key: 'max', label: 'Nombre affiché', kind: 'number', min: 1, max: 20 }
		]
	}
];

export const CARD_INFO: Record<string, CardInfo> = Object.fromEntries(
	CATALOG.map((c) => [c.type, c])
);

export const ACTIONS: [string, string][] = [
	['more-info', 'Afficher la fiche'],
	['toggle', 'Basculer'],
	['navigate', 'Naviguer (page ou #pop-up)'],
	['perform-action', 'Appeler un service'],
	['url', 'Ouvrir une adresse'],
	['none', 'Rien']
];

export const ALARM_MODES: [string, string][] = [
	['arm_home', 'Présence'],
	['arm_away', 'Absence'],
	['arm_night', 'Nuit'],
	['arm_vacation', 'Vacances']
];

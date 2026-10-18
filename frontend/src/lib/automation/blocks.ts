// Blocs de l'éditeur visuel d'automatisations : types de déclencheurs,
// conditions et actions que le moteur sait compiler (crates/rusthome-automation
// compile.rs), leur modèle d'ajout, leurs champs et un résumé lisible.
// Le document reste au format HA : l'éditeur YAML et l'éditeur visuel sont
// deux vues du même objet.

export type Item = Record<string, unknown>;
export type Kind = 'trigger' | 'condition' | 'action';

export type FieldKind =
	| 'entity'
	| 'entities'
	| 'text'
	| 'number'
	| 'duration'
	| 'time'
	| 'template'
	| 'yaml'
	| 'select'
	| 'service'
	| 'target'
	| 'boolean';

export interface BlockField {
	key: string;
	label: string;
	kind: FieldKind;
	options?: [string, string][];
	placeholder?: string;
}

export interface BlockType {
	/** Identifiant : `state`, `numeric_state`… ; pour les actions, la forme (`service`, `delay`…). */
	id: string;
	label: string;
	template: Item;
	fields: BlockField[];
}

const forField: BlockField = {
	key: 'for',
	label: 'Pendant (durée)',
	kind: 'duration',
	placeholder: '00:05:00'
};

export const TRIGGERS: BlockType[] = [
	{
		id: 'state',
		label: 'État d’une entité',
		template: { trigger: 'state', entity_id: [] },
		fields: [
			{ key: 'entity_id', label: 'Entités', kind: 'entities' },
			{ key: 'attribute', label: 'Attribut (facultatif)', kind: 'text' },
			{ key: 'from', label: 'De', kind: 'text', placeholder: 'tout état' },
			{ key: 'to', label: 'À', kind: 'text', placeholder: 'tout état' },
			forField
		]
	},
	{
		id: 'numeric_state',
		label: 'Valeur numérique',
		template: { trigger: 'numeric_state', entity_id: [] },
		fields: [
			{ key: 'entity_id', label: 'Entités', kind: 'entities' },
			{ key: 'attribute', label: 'Attribut (facultatif)', kind: 'text' },
			{ key: 'above', label: 'Au-dessus de', kind: 'number' },
			{ key: 'below', label: 'En dessous de', kind: 'number' },
			forField
		]
	},
	{
		id: 'time',
		label: 'Heure',
		template: { trigger: 'time', at: '07:00:00' },
		fields: [{ key: 'at', label: 'À', kind: 'time' }]
	},
	{
		id: 'time_pattern',
		label: 'Périodique',
		template: { trigger: 'time_pattern', minutes: '/5' },
		fields: [
			{ key: 'hours', label: 'Heures', kind: 'text', placeholder: '/2 ou 7' },
			{ key: 'minutes', label: 'Minutes', kind: 'text', placeholder: '/5' },
			{ key: 'seconds', label: 'Secondes', kind: 'text' }
		]
	},
	{
		id: 'event',
		label: 'Événement',
		template: { trigger: 'event', event_type: '' },
		fields: [
			{ key: 'event_type', label: 'Type d’événement', kind: 'text' },
			{ key: 'event_data', label: 'Données attendues (YAML)', kind: 'yaml' }
		]
	},
	{
		id: 'tag',
		label: 'Badge NFC',
		template: { trigger: 'tag', tag_id: '' },
		fields: [{ key: 'tag_id', label: 'Identifiant du badge', kind: 'text' }]
	},
	{
		id: 'mqtt',
		label: 'Message MQTT',
		template: { trigger: 'mqtt', topic: '' },
		fields: [
			{ key: 'topic', label: 'Sujet', kind: 'text' },
			{ key: 'payload', label: 'Contenu attendu (facultatif)', kind: 'text' }
		]
	},
	{
		id: 'homeassistant',
		label: 'Démarrage',
		template: { trigger: 'homeassistant', event: 'start' },
		fields: [
			{
				key: 'event',
				label: 'Quand',
				kind: 'select',
				options: [
					['start', 'Au démarrage'],
					['shutdown', 'À l’arrêt']
				]
			}
		]
	},
	{
		id: 'template',
		label: 'Modèle (Jinja)',
		template: { trigger: 'template', value_template: '' },
		fields: [{ key: 'value_template', label: 'Modèle', kind: 'template' }, forField]
	}
];

export const CONDITIONS: BlockType[] = [
	{
		id: 'state',
		label: 'État d’une entité',
		template: { condition: 'state', entity_id: [], state: '' },
		fields: [
			{ key: 'entity_id', label: 'Entités', kind: 'entities' },
			{ key: 'attribute', label: 'Attribut (facultatif)', kind: 'text' },
			{ key: 'state', label: 'État', kind: 'text' },
			forField
		]
	},
	{
		id: 'numeric_state',
		label: 'Valeur numérique',
		template: { condition: 'numeric_state', entity_id: [] },
		fields: [
			{ key: 'entity_id', label: 'Entités', kind: 'entities' },
			{ key: 'above', label: 'Au-dessus de', kind: 'number' },
			{ key: 'below', label: 'En dessous de', kind: 'number' }
		]
	},
	{
		id: 'time',
		label: 'Plage horaire',
		template: { condition: 'time', after: '08:00:00', before: '22:00:00' },
		fields: [
			{ key: 'after', label: 'Après', kind: 'time' },
			{ key: 'before', label: 'Avant', kind: 'time' }
		]
	},
	{
		id: 'trigger',
		label: 'Déclencheur précis',
		template: { condition: 'trigger', id: '' },
		fields: [{ key: 'id', label: 'Identifiant du déclencheur', kind: 'text' }]
	},
	{
		id: 'template',
		label: 'Modèle (Jinja)',
		template: { condition: 'template', value_template: '' },
		fields: [{ key: 'value_template', label: 'Modèle', kind: 'template' }]
	},
	{
		id: 'and',
		label: 'Toutes vraies (ET)',
		template: { condition: 'and', conditions: [] },
		fields: []
	},
	{ id: 'or', label: 'Une vraie (OU)', template: { condition: 'or', conditions: [] }, fields: [] },
	{
		id: 'not',
		label: 'Aucune vraie (NON)',
		template: { condition: 'not', conditions: [] },
		fields: []
	}
];

export const ACTIONS: BlockType[] = [
	{
		id: 'service',
		label: 'Appeler un service',
		template: { action: '', target: { entity_id: [] } },
		fields: [
			{ key: 'action', label: 'Service', kind: 'service' },
			{ key: 'target', label: 'Cibles', kind: 'target' },
			{ key: 'data', label: 'Données (YAML)', kind: 'yaml' }
		]
	},
	{
		id: 'delay',
		label: 'Attendre une durée',
		template: { delay: '00:00:30' },
		fields: [{ key: 'delay', label: 'Durée', kind: 'duration' }]
	},
	{
		id: 'wait_template',
		label: 'Attendre une condition',
		template: { wait_template: '', timeout: '00:05:00' },
		fields: [
			{ key: 'wait_template', label: 'Modèle', kind: 'template' },
			{ key: 'timeout', label: 'Délai maximal', kind: 'duration' },
			{ key: 'continue_on_timeout', label: 'Continuer après le délai', kind: 'boolean' }
		]
	},
	{
		id: 'condition',
		label: 'Condition (arrête si fausse)',
		template: { condition: 'state', entity_id: [], state: '' },
		fields: []
	},
	{
		id: 'choose',
		label: 'Choisir',
		template: { choose: [{ conditions: [], sequence: [] }] },
		fields: []
	},
	{ id: 'if', label: 'Si / alors / sinon', template: { if: [], then: [] }, fields: [] },
	{
		id: 'repeat',
		label: 'Répéter',
		template: { repeat: { count: 2, sequence: [] } },
		fields: []
	},
	{ id: 'parallel', label: 'En parallèle', template: { parallel: [] }, fields: [] },
	{ id: 'sequence', label: 'Groupe d’actions', template: { sequence: [] }, fields: [] },
	{
		id: 'event',
		label: 'Émettre un événement',
		template: { event: '' },
		fields: [
			{ key: 'event', label: 'Type d’événement', kind: 'text' },
			{ key: 'event_data', label: 'Données (YAML)', kind: 'yaml' }
		]
	},
	{
		id: 'variables',
		label: 'Définir des variables',
		template: { variables: {} },
		fields: [{ key: 'variables', label: 'Variables (YAML)', kind: 'yaml' }]
	},
	{
		id: 'stop',
		label: 'Arrêter',
		template: { stop: '' },
		fields: [
			{ key: 'stop', label: 'Raison', kind: 'text' },
			{ key: 'error', label: 'Comme une erreur', kind: 'boolean' }
		]
	}
];

export const BLOCKS: Record<Kind, BlockType[]> = {
	trigger: TRIGGERS,
	condition: CONDITIONS,
	action: ACTIONS
};

/** Type d'un bloc, selon sa forme (les actions n'ont pas de clé de type). */
export function blockId(kind: Kind, it: Item): string {
	if (kind === 'trigger') return String(it.trigger ?? it.platform ?? '?');
	if (kind === 'condition') return String(it.condition ?? '?');
	if (typeof it.action === 'string' || typeof it.service === 'string') return 'service';
	for (const k of [
		'delay',
		'wait_template',
		'choose',
		'if',
		'repeat',
		'parallel',
		'sequence',
		'event',
		'variables',
		'stop',
		'condition'
	])
		if (k in it) return k;
	return '?';
}

export function blockType(kind: Kind, it: Item): BlockType | undefined {
	const id = blockId(kind, it);
	return BLOCKS[kind].find((b) => b.id === id);
}

const list = (v: unknown): string[] =>
	Array.isArray(v) ? v.map(String) : typeof v === 'string' && v ? [v] : [];

/** Liste lisible : « A », « A et B », « A, B et C ». */
function join(xs: string[]): string {
	if (xs.length <= 1) return xs[0] ?? '?';
	return `${xs.slice(0, -1).join(', ')} et ${xs.at(-1)}`;
}

/**
 * Résumé d'un bloc, façon HA (« Quand l'état de X passe à on »). `name`
 * traduit un entity_id en nom affiché.
 */
export function describe(kind: Kind, it: Item, name: (id: string) => string = (x) => x): string {
	if (typeof it.alias === 'string' && it.alias) return it.alias;
	const id = blockId(kind, it);
	const ents = join(list(it.entity_id).map(name));
	const dur = typeof it.for === 'string' ? ` pendant ${it.for}` : '';
	if (kind === 'trigger')
		switch (id) {
			case 'state':
				return `Quand ${ents}${it.from !== undefined ? ` passe de ${String(it.from)}` : ' change'}${it.to !== undefined ? ` à ${String(it.to)}` : ''}${dur}`;
			case 'numeric_state':
				return `Quand ${ents} ${bounds(it)}${dur}`;
			case 'time':
				return `À ${list(it.at).join(', ')}`;
			case 'time_pattern':
				return `Périodiquement (${['hours', 'minutes', 'seconds']
					.filter((k) => it[k] !== undefined)
					.map((k) => `${k === 'hours' ? 'h' : k === 'minutes' ? 'min' : 's'} ${String(it[k])}`)
					.join(', ')})`;
			case 'event':
				return `À l’événement ${String(it.event_type ?? '?')}`;
			case 'tag':
				return `Au passage du badge ${String(it.tag_id ?? '?')}`;
			case 'mqtt':
				return `Message MQTT sur ${String(it.topic ?? '?')}`;
			case 'homeassistant':
				return it.event === 'shutdown' ? 'À l’arrêt' : 'Au démarrage';
			case 'template':
				return 'Quand le modèle devient vrai';
			default:
				return `Déclencheur ${id}`;
		}
	if (kind === 'condition')
		switch (id) {
			case 'state':
				return `Si ${ents} est ${list(it.state).join(' ou ') || '?'}${dur}`;
			case 'numeric_state':
				return `Si ${ents} est ${bounds(it)}`;
			case 'time':
				return `Si l’heure est${it.after ? ` après ${String(it.after)}` : ''}${it.before ? ` avant ${String(it.before)}` : ''}`;
			case 'trigger':
				return `Si déclenché par « ${list(it.id).join(', ')} »`;
			case 'template':
				return 'Si le modèle est vrai';
			case 'and':
				return 'Si toutes ces conditions sont vraies';
			case 'or':
				return 'Si l’une de ces conditions est vraie';
			case 'not':
				return 'Si aucune de ces conditions n’est vraie';
			default:
				return `Condition ${id}`;
		}
	switch (id) {
		case 'service': {
			const svc = String(it.action ?? it.service ?? '?');
			const target = (it.target as { entity_id?: unknown } | undefined)?.entity_id;
			const t = list(target).map(name);
			return `${svc}${t.length ? ` sur ${join(t)}` : ''}`;
		}
		case 'delay':
			return `Attendre ${typeof it.delay === 'string' ? it.delay : JSON.stringify(it.delay)}`;
		case 'wait_template':
			return 'Attendre que le modèle soit vrai';
		case 'condition':
			return describe('condition', it, name);
		case 'choose':
			return `Choisir parmi ${Array.isArray(it.choose) ? it.choose.length : 0} option(s)`;
		case 'if':
			return 'Si … alors … sinon …';
		case 'repeat': {
			const r = (it.repeat ?? {}) as Item;
			if (r.count !== undefined) return `Répéter ${String(r.count)} fois`;
			if (r.while !== undefined) return 'Répéter tant que…';
			return 'Répéter jusqu’à ce que…';
		}
		case 'parallel':
			return 'En parallèle';
		case 'sequence':
			return 'Groupe d’actions';
		case 'event':
			return `Émettre l’événement ${String(it.event)}`;
		case 'variables':
			return `Définir ${Object.keys((it.variables as object) ?? {}).join(', ') || 'des variables'}`;
		case 'stop':
			return `Arrêter${it.stop ? ` : ${String(it.stop)}` : ''}`;
		default:
			return 'Action';
	}
}

function bounds(it: Item): string {
	const a = it.above !== undefined ? `au-dessus de ${String(it.above)}` : '';
	const b = it.below !== undefined ? `en dessous de ${String(it.below)}` : '';
	return [a, b].filter(Boolean).join(' et ') || 'change';
}

/** Copie profonde (les éditeurs ne modifient jamais l'objet reçu). */
export function copy<T>(x: T): T {
	return JSON.parse(JSON.stringify(x)) as T;
}

/** Applique un changement de champ ; une valeur vide retire la clé. */
export function setField(it: Item, key: string, value: unknown): Item {
	const out = { ...it };
	const empty =
		value === undefined ||
		value === '' ||
		value === null ||
		(Array.isArray(value) && value.length === 0 && key !== 'entity_id');
	if (empty) delete out[key];
	else out[key] = value;
	return out;
}

export function moveItem<T>(xs: T[], i: number, d: -1 | 1): T[] {
	const j = i + d;
	if (j < 0 || j >= xs.length) return xs;
	const out = [...xs];
	[out[i], out[j]] = [out[j]!, out[i]!];
	return out;
}

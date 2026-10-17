import type { EntityState } from './types';

/** Nom affiché d'une entité : friendly_name, nom du registre, sinon l'identifiant. */
export function entityLabel(s: EntityState | undefined, registryName?: string | null): string {
	if (registryName) return registryName;
	const fn = s?.attributes.friendly_name;
	return typeof fn === 'string' && fn ? fn : (s?.entity_id ?? '');
}

export function domainOf(entityId: string): string {
	return entityId.split('.', 1)[0] ?? '';
}

/** Libellés français des domaines les plus courants ; les autres restent bruts. */
const DOMAINS: Record<string, string> = {
	alarm_control_panel: 'Alarme',
	automation: 'Automatisme',
	binary_sensor: 'Capteur binaire',
	button: 'Bouton',
	calendar: 'Calendrier',
	camera: 'Caméra',
	climate: 'Climat',
	device_tracker: 'Suivi',
	event: 'Événement',
	input_boolean: 'Interrupteur virtuel',
	input_text: 'Texte',
	light: 'Lumière',
	media_player: 'Lecteur média',
	number: 'Nombre',
	person: 'Personne',
	remote: 'Télécommande',
	scene: 'Scène',
	script: 'Script',
	select: 'Sélection',
	sensor: 'Capteur',
	siren: 'Sirène',
	switch: 'Prise / interrupteur',
	update: 'Mise à jour',
	weather: 'Météo'
};

export function domainLabel(domain: string): string {
	return DOMAINS[domain] ?? domain;
}

export const UNAVAILABLE = new Set(['unavailable', 'unknown']);

export function isUnavailable(s: EntityState): boolean {
	return UNAVAILABLE.has(s.state);
}

/** Un état est numérique s'il se lit comme un nombre fini (« 20.6 », « -3 »). */
export function isNumericState(state: string): boolean {
	return state.trim() !== '' && Number.isFinite(Number(state));
}

/** Une entité se trace en courbe si elle a une unité et un état numérique. */
export function isNumericEntity(s: EntityState): boolean {
	return typeof s.attributes.unit_of_measurement === 'string' && isNumericState(s.state);
}

const rtf = new Intl.RelativeTimeFormat('fr', { numeric: 'auto' });

/** « il y a 5 minutes », « hier »… par rapport à `now`. */
export function relativeTime(iso: string, now: Date = new Date()): string {
	const diff = (new Date(iso).getTime() - now.getTime()) / 1000;
	const abs = Math.abs(diff);
	if (abs < 45) return "à l'instant";
	if (abs < 3600) return rtf.format(Math.round(diff / 60), 'minute');
	if (abs < 86400) return rtf.format(Math.round(diff / 3600), 'hour');
	return rtf.format(Math.round(diff / 86400), 'day');
}

/** Compare deux chaînes à la française (accents, casse). */
export const byLabel = new Intl.Collator('fr', { sensitivity: 'base', numeric: true }).compare;

/** « 1 entité », « 3 entités », « 1 appareil »… (pluriel régulier en -s). */
export function plural(n: number, word: string): string {
	return `${new Intl.NumberFormat('fr-FR').format(n)} ${word}${n > 1 ? 's' : ''}`;
}

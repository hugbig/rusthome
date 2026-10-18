// Projets ESPHome (spec §5.3 bis) : types de l'API et mise en forme de la
// sortie du compilateur. Pur, testé.

export interface EspDevice {
	configuration: string;
	name: string | null;
	friendly_name: string | null;
	comment: string | null;
	platform: string;
	board: string;
	address: string;
	ip: string;
	state: 'online' | 'offline' | 'unknown';
	built_version: string | null;
	deployed_version: string | null;
	update_available: boolean;
	pending_changes: boolean;
	/** Nœud de l'alarme : lecture seule. */
	locked: boolean;
	/** Installation sans fil autorisée. */
	flashable: boolean;
	/** Seule adresse jointe en Wi-Fi (configurée côté serveur), sinon null. */
	target: string | null;
}

export type EspAction = 'validate' | 'compile' | 'install' | 'logs';

/** Message reçu pendant une opération (validation, compilation, logs…). */
export type RunMessage =
	| { type: 'step'; text: string }
	| { type: 'line'; text: string }
	| { type: 'done'; ok: boolean; error?: string };

export const ACTION_LABELS: Record<EspAction, string> = {
	validate: 'Validation',
	compile: 'Compilation',
	install: 'Installation sans fil',
	logs: 'Logs (Wi-Fi)'
};

/**
 * Adresse WebSocket d'une opération, sur le même hôte que la page. La
 * carte visée est fixée par le serveur (jamais choisie ici).
 */
export function runUrl(
	loc: { protocol: string; host: string },
	configuration: string,
	action: EspAction
): string {
	const scheme = loc.protocol === 'https:' ? 'wss' : 'ws';
	return `${scheme}://${loc.host}/api/v1/esphome/configs/${encodeURIComponent(configuration)}/run?action=${action}`;
}

/** Nom d'hôte ESPHome : minuscules, chiffres et tirets. */
export function validName(n: string): boolean {
	return /^[a-z0-9][a-z0-9-]{0,62}$/.test(n);
}

export interface Segment {
	text: string;
	/** Couleur ANSI de base (30-37, 90-97), `bold`. */
	color?: string;
	bold?: boolean;
}

const COLORS: Record<number, string> = {
	30: 'black',
	31: 'red',
	32: 'green',
	33: 'yellow',
	34: 'blue',
	35: 'magenta',
	36: 'cyan',
	37: 'white',
	90: 'gray',
	91: 'red',
	92: 'green',
	93: 'yellow',
	94: 'blue',
	95: 'magenta',
	96: 'cyan',
	97: 'white'
};

/**
 * Découpe une ligne colorée en segments. Le Builder envoie les codes ANSI
 * soit bruts (ESC), soit échappés en texte (`\033[32m`, option `--dashboard`).
 */
export function ansiSegments(line: string): Segment[] {
	// eslint-disable-next-line no-control-regex -- codes ANSI bruts
	const re = /(?:\x1b|\\033)\[([0-9;]*)m/g;
	const out: Segment[] = [];
	let color: string | undefined;
	let bold = false;
	let last = 0;
	const push = (text: string) => {
		if (!text) return;
		const seg: Segment = { text };
		if (color) seg.color = color;
		if (bold) seg.bold = true;
		out.push(seg);
	};
	for (const m of line.matchAll(re)) {
		push(line.slice(last, m.index));
		last = m.index + m[0].length;
		const codes = (m[1] || '0').split(';').map(Number);
		for (const c of codes) {
			if (c === 0) {
				color = undefined;
				bold = false;
			} else if (c === 1) bold = true;
			else if (c === 22) bold = false;
			else if (c === 39) color = undefined;
			else if (COLORS[c]) color = COLORS[c];
		}
	}
	push(line.slice(last));
	return out;
}

/** Libellé d'état d'un appareil (jamais la couleur seule). */
export function stateLabel(s: EspDevice['state']): string {
	return s === 'online' ? 'En ligne' : s === 'offline' ? 'Hors ligne' : 'État inconnu';
}

/** Tri : nom affiché puis fichier. */
export function sortDevices(ds: EspDevice[]): EspDevice[] {
	const label = (d: EspDevice) => (d.friendly_name || d.name || d.configuration).toLowerCase();
	return [...ds].sort((a, b) => label(a).localeCompare(label(b), 'fr'));
}

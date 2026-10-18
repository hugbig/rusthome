// Jeu complet d'icônes Material Design (static/icons/mdi.json, généré au build
// par scripts/mdi-json.mjs), chargé à la demande dès qu'une icône non
// embarquée est demandée, puis mis en cache par le navigateur.
class IconSet {
	paths = $state<Record<string, string> | null>(null);
	#loading = false;

	/** `mdi:sofa-outline` → chemin SVG ; `undefined` tant que rien n'est chargé. */
	get(name: string): string | undefined {
		if (!this.paths) {
			this.load();
			return undefined;
		}
		return this.paths[name.replace(/^mdi:/, '')];
	}

	load() {
		if (this.#loading || typeof window === 'undefined' || typeof fetch === 'undefined') return;
		this.#loading = true;
		fetch('/icons/mdi.json')
			.then((r) => (r.ok ? (r.json() as Promise<Record<string, string>>) : Promise.reject()))
			.then((p) => (this.paths = p))
			.catch(() => {
				this.paths = {}; // pas de jeu complet : icônes de domaine
			});
	}
}

export const iconSet = new IconSet();

// Écrit static/icons/mdi.json (nom mdi → chemin SVG) depuis @mdi/js : le jeu
// complet, chargé à la demande par l'interface pour les icônes non embarquées.
import { mkdirSync, writeFileSync } from 'node:fs';
import * as mdi from '@mdi/js';

const out = {};
for (const [key, path] of Object.entries(mdi)) {
	if (!key.startsWith('mdi') || typeof path !== 'string') continue;
	// mdiSofaOutline → sofa-outline ; mdiLightbulbOn90 → lightbulb-on-90
	const name = key
		.slice(3)
		.replace(/([a-z0-9])([A-Z])/g, '$1-$2')
		.replace(/([a-zA-Z])([0-9])/g, '$1-$2')
		.toLowerCase();
	out[name] = path;
}
mkdirSync(new URL('../static/icons/', import.meta.url), { recursive: true });
writeFileSync(new URL('../static/icons/mdi.json', import.meta.url), JSON.stringify(out));
console.log(`mdi.json : ${Object.keys(out).length} icônes`);

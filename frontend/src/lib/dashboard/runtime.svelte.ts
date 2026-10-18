// État d'affichage partagé par les cartes : fiche ouverte, message, kiosque.
import { goto } from '$app/navigation';
import { callService, explainError } from '../api';
import { domainOf } from '../format';
import type { Action } from './model';
import { safeUrl, splitService } from './model';

class DashboardUi {
	/** Entité dont la fiche est ouverte. */
	moreInfo = $state<string | null>(null);
	/** Résultat de la dernière commande. */
	toast = $state<{ kind: 'ok' | 'error'; text: string } | null>(null);
	/** Mode kiosque : en-tête et navigation masqués (tablette murale). */
	kiosk = $state(false);
	/** Ancre courante (`#media`) : ouvre le pop-up correspondant. */
	hash = $state('');
	#timer: ReturnType<typeof setTimeout> | undefined;

	notify(kind: 'ok' | 'error', text: string) {
		this.toast = { kind, text };
		clearTimeout(this.#timer);
		this.#timer = setTimeout(() => (this.toast = null), 6000);
	}
}

export const ui = new DashboardUi();

/** Suit l'ancre de l'URL ; renvoie la fonction de nettoyage. */
export function watchHash(): () => void {
	const sync = () => (ui.hash = location.hash);
	sync();
	window.addEventListener('hashchange', sync);
	return () => window.removeEventListener('hashchange', sync);
}

/** Ferme le pop-up ouvert : retire l'ancre sans recharger ni naviguer. */
export function closeHash(): void {
	history.replaceState(history.state, '', location.pathname + location.search);
	ui.hash = '';
}

/** Appelle un service ; un refus (lecture seule, entité HA…) s'affiche en message. */
export async function call(service: string, data: Record<string, unknown>): Promise<boolean> {
	const parts = splitService(service);
	if (!parts) {
		ui.notify('error', `Service invalide : ${service}`);
		return false;
	}
	const r = await callService(parts[0], parts[1], data);
	if (!r.ok) ui.notify('error', explainError(r));
	return r.ok;
}

/** Exécute l'action d'une carte. */
export async function run(action: Action, entityId?: string): Promise<void> {
	switch (action.action) {
		case 'none':
			return;
		case 'more-info':
			if (entityId) ui.moreInfo = entityId;
			return;
		case 'toggle':
			if (entityId) await call(`${domainOf(entityId)}.toggle`, { entity_id: entityId });
			return;
		case 'navigate': {
			const path = action.navigation_path ?? '';
			if (path.startsWith('#')) location.hash = path;
			// Chemin libre saisi dans le dashboard : pas de route typée à résoudre.
			// eslint-disable-next-line svelte/no-navigation-without-resolve
			else if (path) await goto(path);
			return;
		}
		case 'url':
			// http(s) ou chemin du site seulement : jamais `javascript:` (revue n° 40)
			if (action.url_path && safeUrl(action.url_path))
				window.open(action.url_path, '_blank', 'noopener');
			return;
		case 'perform-action':
			if (action.perform_action)
				await call(action.perform_action, {
					...(action.data ?? {}),
					...(action.target ?? {})
				});
			return;
	}
}

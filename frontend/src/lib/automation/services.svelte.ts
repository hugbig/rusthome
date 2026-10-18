import { request } from '../api';

/** Services connus de RustHome, pour la saisie assistée des actions. */
class Services {
	names = $state<string[]>([]);
	#loaded = false;

	load() {
		if (this.#loaded) return;
		this.#loaded = true;
		void request<{ domain: string; service: string }[]>('GET', 'services').then((r) => {
			if (r.ok) this.names = [...new Set(r.data.map((s) => `${s.domain}.${s.service}`))].sort();
		});
	}
}

export const services = new Services();

export interface Me {
	name: string;
	role: 'admin' | 'resident' | 'display';
	/** Tableau imposé (compte de la tablette murale), sinon absent. */
	home?: string | null;
}

/** Utilisateur courant (`/api/v1/me`). `anonymous` = connexion requise. */
class SessionStore {
	me = $state<Me | null>(null);
	status = $state<'loading' | 'ok' | 'anonymous' | 'error'>('loading');

	async load() {
		try {
			const r = await fetch('/api/v1/me');
			if (r.status === 401) {
				this.status = 'anonymous';
				return;
			}
			if (!r.ok) throw new Error(`HTTP ${r.status}`);
			this.me = (await r.json()) as Me;
			this.status = 'ok';
		} catch {
			this.status = 'error';
		}
	}
}

export const session = new SessionStore();

export const ROLE_LABELS: Record<Me['role'], string> = {
	admin: 'Administrateur',
	resident: 'Résident',
	display: 'Écran'
};

/** URL de connexion qui ramène sur la page courante après Authelia. */
export function loginUrl(path: string): string {
	return `/api/v1/auth/login?next=${encodeURIComponent(path)}`;
}

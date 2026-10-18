import { request } from '../api';
import { byLabel } from '../format';
import type { DashboardSummary } from '../dashboard/model';

/** Tableaux de bord visibles par le compte (menu latéral, liste). */
class DashboardsStore {
	list = $state<DashboardSummary[]>([]);
	error = $state<string | null>(null);

	async load() {
		const r = await request<DashboardSummary[]>('GET', 'dashboards');
		if (r.ok) {
			this.list = [...r.data].sort((a, b) => byLabel(a.title, b.title));
			this.error = null;
		} else this.error = r.error;
	}

	/** Ceux à afficher dans le menu latéral (`show_in_sidebar`). */
	get sidebar(): DashboardSummary[] {
		return this.list.filter((d) => d.show_in_sidebar);
	}
}

export const dashboards = new DashboardsStore();

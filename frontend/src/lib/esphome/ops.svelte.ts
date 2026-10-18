// Opérations en cours sur un projet ESPHome (console, USB, choix
// d'installation), partagées par la liste et l'éditeur. La carte visée en
// Wi-Fi est fixée par le serveur (`RUSTHOME_ESPHOME_ADDRESSES`) : elle n'est
// jamais saisie ici, pour qu'une faute de frappe ne puisse pas viser un
// nœud de l'alarme.
import type { EspAction } from '../esphome';

export interface Project {
	configuration: string;
	title: string;
	locked: boolean;
	flashable: boolean;
	/** Adresse configurée de la carte, sinon null (pas d'opération Wi-Fi). */
	target: string | null;
}

export class EspOps {
	run = $state<{ project: Project; action: EspAction } | null>(null);
	usb = $state<Project | null>(null);
	choose = $state<Project | null>(null);
	logsFor = $state<Project | null>(null);
	/** Appelé à la fin d'une opération (rafraîchir la liste). */
	ondone?: () => void;

	start(project: Project, action: EspAction) {
		this.choose = null;
		this.logsFor = null;
		this.usb = null;
		this.run = { project, action };
	}
}

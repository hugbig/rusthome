<script lang="ts">
	import { resolve } from '$app/paths';
	import {
		mdiAccountGroup,
		mdiDevices,
		mdiHomeFloor1,
		mdiInformationOutline,
		mdiPuzzle,
		mdiRobot,
		mdiServer,
		mdiViewDashboard
	} from '@mdi/js';
	import Icon from '#lib/dashboard/Icon.svelte';
	import { plural } from '#lib/format.ts';
	import { dashboards } from '#lib/stores/dashboards.svelte.ts';
	import { registry } from '#lib/stores/registry.svelte.ts';

	const idx = $derived(registry.index);
	const integrations = $derived(new Set(idx.entities.map((e) => e.integration)).size);

	const items = $derived([
		{
			href: resolve('/parametres/integrations'),
			icon: mdiPuzzle,
			title: 'Intégrations',
			text: `${plural(integrations, 'intégration')} · ${plural(idx.devices.length, 'appareil')} · ${plural(idx.entities.length, 'entité')}`
		},
		{
			href: resolve('/appareils'),
			icon: mdiDevices,
			title: 'Appareils et entités',
			text: 'Appareils, entités, états et historique'
		},
		{
			href: resolve('/automatisations'),
			icon: mdiRobot,
			title: 'Automatisations',
			text: 'Création en visuel ou en YAML, traces'
		},
		{
			href: resolve('/pieces'),
			icon: mdiHomeFloor1,
			title: 'Pièces et étages',
			text: `${plural(idx.areas.length, 'pièce')} · ${plural(idx.floors.length, 'étage')}`
		},
		{
			href: resolve('/tableaux'),
			icon: mdiViewDashboard,
			title: 'Tableaux de bord',
			text: `${dashboards.list.length} tableau${dashboards.list.length > 1 ? 'x' : ''} de bord`
		},
		{
			href: resolve('/parametres/personnes'),
			icon: mdiAccountGroup,
			title: 'Personnes et comptes',
			text: 'Personnes suivies, comptes et rôles'
		},
		{
			href: resolve('/parametres/systeme'),
			icon: mdiServer,
			title: 'Système',
			text: 'Sources, modes lecture seule, version'
		},
		{
			href: resolve('/parametres/systeme'),
			icon: mdiInformationOutline,
			title: 'À propos',
			text: 'RustHome, version et licences'
		}
	]);
</script>

<h1>Paramètres</h1>

<div class="list card">
	{#each items as it (it.title)}
		<a href={it.href}>
			<span class="ic"><Icon path={it.icon} /></span>
			<span class="txt">
				<strong>{it.title}</strong>
				<small>{it.text}</small>
			</span>
			<span class="chev" aria-hidden="true">›</span>
		</a>
	{/each}
</div>

<style>
	.list {
		padding: 4px 0;
		max-width: 760px;
	}
	a {
		display: flex;
		align-items: center;
		gap: 16px;
		padding: 12px 16px;
		color: var(--text);
		text-decoration: none;
	}
	a + a {
		border-top: 1px solid var(--border);
	}
	a:hover {
		background: var(--surface-2);
	}
	.ic {
		display: grid;
		place-items: center;
		width: 40px;
		height: 40px;
		border-radius: 10px;
		background: var(--primary);
		color: var(--on-primary);
		flex: none;
	}
	.txt {
		display: flex;
		flex-direction: column;
		flex: 1;
		min-width: 0;
	}
	small {
		color: var(--text-muted);
	}
	.chev {
		color: var(--text-muted);
		font-size: 1.4rem;
	}
</style>

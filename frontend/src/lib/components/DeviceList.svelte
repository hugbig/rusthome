<script lang="ts" module>
	import type { Device } from '../types';

	export interface Column {
		label: string;
		/** Valeur affichée pour un appareil (`undefined` → « — »). */
		value: (d: Device, entityIds: string[]) => string | undefined;
	}
</script>

<script lang="ts">
	// Liste d'appareils d'une intégration (ESPHome, Zigbee2MQTT…) avec des
	// colonnes calculées à partir de leurs entités.
	import { resolve } from '$app/paths';
	import { byLabel } from '../format';
	import { registry } from '../stores/registry.svelte';
	import { live } from '../stores/live.svelte';

	interface Props {
		integrations: string[];
		columns?: Column[];
	}
	let { integrations, columns = [] }: Props = $props();

	let q = $state('');
	const idx = $derived(registry.index);
	const devices = $derived(
		idx.devices
			.filter((d) => integrations.includes(d.integration))
			.sort((a, b) => byLabel(a.name, b.name))
	);
	const shown = $derived(
		devices.filter((d) => !q || d.name.toLowerCase().includes(q.trim().toLowerCase()))
	);
	const ids = (d: Device) => idx.entitiesOfDevice(d.id).map((e) => e.entity_id);
	/** En ligne si au moins une entité n'est pas indisponible. */
	const online = (d: Device) =>
		ids(d).some((id) => {
			const s = live.states.get(id);
			return s && s.state !== 'unavailable';
		});
</script>

<div class="card bar">
	<input
		type="search"
		placeholder="Rechercher un appareil…"
		bind:value={q}
		aria-label="Recherche"
	/>
	<span class="muted">{shown.length} / {devices.length}</span>
</div>

<section class="card">
	<div class="scroll">
		<table>
			<thead>
				<tr>
					<th>Appareil</th>
					<th>Pièce</th>
					<th>Modèle</th>
					{#each columns as c (c.label)}<th>{c.label}</th>{/each}
					<th>État</th>
				</tr>
			</thead>
			<tbody>
				{#each shown as d (d.id)}
					{@const list = ids(d)}
					<tr>
						<td><a href={resolve('/appareils/[id]', { id: d.id })}>{d.name}</a></td>
						<td class="muted">{d.area_id ? (idx.areaById.get(d.area_id)?.name ?? '—') : '—'}</td>
						<td class="muted">{[d.manufacturer, d.model].filter(Boolean).join(' ') || '—'}</td>
						{#each columns as c (c.label)}<td>{c.value(d, list) ?? '—'}</td>{/each}
						<td>
							<span class="dot" class:on={online(d)}></span>{online(d) ? 'En ligne' : 'Hors ligne'}
						</td>
					</tr>
				{:else}
					<tr><td colspan={4 + columns.length} class="muted">Aucun appareil.</td></tr>
				{/each}
			</tbody>
		</table>
	</div>
</section>

<style>
	.bar {
		display: flex;
		gap: 8px;
		align-items: center;
	}
	input {
		flex: 1;
		padding: 6px 10px;
		border: 1px solid var(--border);
		border-radius: 8px;
		background: var(--bg);
		color: var(--text);
		font: inherit;
	}
	.scroll {
		overflow-x: auto;
	}
	table {
		width: 100%;
		border-collapse: collapse;
		font-size: 0.9rem;
	}
	th {
		text-align: left;
		font-size: 0.8rem;
		color: var(--text-muted);
		font-weight: 600;
		padding: 6px;
		white-space: nowrap;
	}
	td {
		padding: 7px 6px;
		border-top: 1px solid var(--border);
		white-space: nowrap;
	}
	td a {
		color: var(--text);
		font-weight: 500;
	}
	.muted {
		color: var(--text-muted);
	}
	.dot {
		display: inline-block;
		width: 8px;
		height: 8px;
		border-radius: 50%;
		margin-right: 6px;
		background: #b3261e;
	}
	.dot.on {
		background: #1c7c4a;
	}
</style>

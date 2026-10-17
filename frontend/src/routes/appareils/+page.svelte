<script lang="ts">
	import { resolve } from '$app/paths';
	import { byLabel } from '#lib/format.ts';
	import { countBy } from '#lib/filters.ts';
	import { registry } from '#lib/stores/registry.svelte.ts';

	const idx = $derived(registry.index);
	let q = $state('');
	let integration = $state('');

	const integrationOf = (deviceId: string) => {
		const e = idx.entities.find((x) => x.device_id === deviceId);
		return e?.integration.replace(/^ha:/, '');
	};
	const rows = $derived(
		idx.devices
			.map((d) => ({
				...d,
				fromHa: d.integration.startsWith('ha:'),
				integration: integrationOf(d.id) ?? '—',
				area: d.area_id ? idx.areaById.get(d.area_id)?.name : undefined,
				count: idx.entitiesOfDevice(d.id).length
			}))
			// Appareils HA dont toutes les entités ont été reprises nativement (Zigbee2MQTT…).
			.filter((d) => d.count > 0 || !d.fromHa)
			.sort((a, b) => byLabel(a.name, b.name))
	);
	const integrations = $derived(countBy(rows, (r) => r.integration));
	const shown = $derived(
		rows.filter(
			(r) =>
				(!integration || r.integration === integration) &&
				(!q || r.name.toLowerCase().includes(q.trim().toLowerCase()))
		)
	);
</script>

<h1>Appareils <small>{shown.length} / {rows.length}</small></h1>

<div class="filters card">
	<input
		type="search"
		placeholder="Rechercher un appareil…"
		bind:value={q}
		aria-label="Recherche"
	/>
	<select bind:value={integration} aria-label="Intégration">
		<option value="">Toutes les intégrations</option>
		{#each integrations as [i, n] (i)}<option value={i}>{i} ({n})</option>{/each}
	</select>
</div>

<section class="card">
	<table>
		<thead
			><tr><th>Appareil</th><th>Intégration</th><th>Pièce</th><th class="right">Entités</th></tr
			></thead
		>
		<tbody>
			{#each shown as d (d.id)}
				<tr>
					<td><a href={resolve('/appareils/[id]', { id: d.id })}>{d.name}</a></td>
					<td>{d.integration}</td>
					<td>{d.area ?? '—'}</td>
					<td class="right">{d.count}</td>
				</tr>
			{:else}
				<tr><td colspan="4" class="muted">Aucun appareil.</td></tr>
			{/each}
		</tbody>
	</table>
</section>

<style>
	small {
		color: var(--text-muted);
		font-weight: normal;
		font-size: 0.9rem;
	}
	.filters {
		display: flex;
		flex-wrap: wrap;
		gap: 8px;
	}
	input,
	select {
		padding: 6px 10px;
		border: 1px solid var(--border);
		border-radius: 8px;
		background: var(--bg);
		color: var(--text);
		font: inherit;
		min-width: 0;
	}
	input {
		flex: 1 1 240px;
	}
	table {
		width: 100%;
		border-collapse: collapse;
	}
	th {
		text-align: left;
		font-size: 0.8rem;
		color: var(--text-muted);
		font-weight: 600;
		padding: 6px 4px;
	}
	td {
		padding: 8px 4px;
		border-top: 1px solid var(--border);
	}
	td a {
		color: var(--text);
		font-weight: 500;
	}
	.right {
		text-align: right;
	}
	.muted {
		color: var(--text-muted);
	}
</style>

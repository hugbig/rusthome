<script lang="ts">
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { byLabel } from '#lib/format.ts';
	import EntityTable from '#lib/components/EntityTable.svelte';
	import { live } from '#lib/stores/live.svelte.ts';
	import { registry } from '#lib/stores/registry.svelte.ts';
	import type { EntityState } from '#lib/types.ts';

	const id = $derived(page.params.id ?? '');
	const idx = $derived(registry.index);
	const area = $derived(idx.areaById.get(id));
	const floor = $derived(
		area?.floor_id ? idx.floors.find((f) => f.id === area.floor_id) : undefined
	);
	const devices = $derived([...idx.devicesOfArea(id)].sort((a, b) => byLabel(a.name, b.name)));
	const states = $derived(
		idx
			.entitiesOfArea(id)
			.map((e) => live.states.get(e.entity_id))
			.filter((s): s is EntityState => s !== undefined)
	);
</script>

<p class="crumbs">
	<a href={resolve('/pieces')}>Pièces</a>{#if floor}
		› {floor.name}{/if}
</p>
<h1>{area?.name ?? id}</h1>

{#if registry.loaded && !area}
	<p class="muted">Pièce inconnue.</p>
{:else}
	<section class="card">
		<h2>Appareils ({devices.length})</h2>
		<ul class="devices">
			{#each devices as d (d.id)}
				<li><a href={resolve('/appareils/[id]', { id: d.id })}>{d.name}</a></li>
			{:else}
				<li class="muted">Aucun appareil.</li>
			{/each}
		</ul>
	</section>

	<section class="card">
		<h2>Entités ({states.length})</h2>
		<EntityTable items={states} index={idx} showArea={false} />
	</section>
{/if}

<style>
	.crumbs {
		margin: 0 0 8px;
		color: var(--text-muted);
		font-size: 0.9rem;
	}
	.devices {
		display: flex;
		flex-wrap: wrap;
		gap: 8px;
		list-style: none;
		margin: 0;
		padding: 0;
	}
	.devices a {
		display: inline-block;
		padding: 4px 10px;
		border-radius: 999px;
		background: var(--surface-2);
		text-decoration: none;
		color: var(--text);
	}
	.muted {
		color: var(--text-muted);
	}
</style>

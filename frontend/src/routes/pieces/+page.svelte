<script lang="ts">
	import { resolve } from '$app/paths';
	import { byLabel, isUnavailable, plural } from '#lib/format.ts';
	import { live } from '#lib/stores/live.svelte.ts';
	import { registry } from '#lib/stores/registry.svelte.ts';
	import type { Area } from '#lib/types.ts';

	const idx = $derived(registry.index);

	/** Étages triés par niveau, puis « Sans étage » pour les pièces orphelines. */
	const floors = $derived.by(() => {
		const sorted = [...idx.floors].sort((a, b) => (a.level ?? 0) - (b.level ?? 0));
		const groups = sorted.map((f) => ({ id: f.id, name: f.name, areas: [] as Area[] }));
		const none = { id: '_none', name: 'Sans étage', areas: [] as Area[] };
		for (const a of idx.areas) (groups.find((g) => g.id === a.floor_id) ?? none).areas.push(a);
		return [...groups, none]
			.filter((g) => g.areas.length)
			.map((g) => ({
				...g,
				areas: g.areas.sort((a, b) => byLabel(a.name, b.name))
			}));
	});

	function stats(areaId: string) {
		const entities = idx.entitiesOfArea(areaId);
		const off = entities.filter((e) => {
			const s = live.states.get(e.entity_id);
			return s ? isUnavailable(s) : false;
		}).length;
		return { devices: idx.devicesOfArea(areaId).length, entities: entities.length, off };
	}
</script>

<h1>Pièces</h1>

{#each floors as f (f.id)}
	<h2>{f.name}</h2>
	<div class="areas">
		{#each f.areas as a (a.id)}
			{@const s = stats(a.id)}
			<a class="area card" href={resolve('/pieces/[id]', { id: a.id })}>
				<strong>{a.name}</strong>
				<span>{plural(s.devices, 'appareil')} · {plural(s.entities, 'entité')}</span>
				{#if s.off}<em>{plural(s.off, 'indisponible')}</em>{/if}
			</a>
		{/each}
	</div>
{:else}
	<p class="muted">Aucune pièce.</p>
{/each}

<style>
	.areas {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
		gap: 12px;
		margin-bottom: 16px;
	}
	.area {
		display: flex;
		flex-direction: column;
		gap: 4px;
		text-decoration: none;
		color: var(--text);
		margin: 0;
	}
	.area:hover {
		border-color: var(--primary);
	}
	span,
	em {
		font-size: 0.85rem;
		color: var(--text-muted);
	}
	.muted {
		color: var(--text-muted);
	}
</style>

<script lang="ts">
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import EntityTable from '#lib/components/EntityTable.svelte';
	import { live } from '#lib/stores/live.svelte.ts';
	import { registry } from '#lib/stores/registry.svelte.ts';
	import type { EntityState } from '#lib/types.ts';

	const id = $derived(page.params.id ?? '');
	const idx = $derived(registry.index);
	const device = $derived(idx.deviceById.get(id));
	const area = $derived(device?.area_id ? idx.areaById.get(device.area_id) : undefined);
	const entries = $derived(idx.entitiesOfDevice(id));
	const states = $derived(
		entries
			.map((e) => live.states.get(e.entity_id))
			.filter((s): s is EntityState => s !== undefined)
	);
	const integration = $derived(entries[0]?.integration.replace(/^ha:/, ''));
</script>

<p class="crumbs"><a href={resolve('/appareils')}>Appareils</a></p>
<h1>{device?.name ?? id}</h1>

{#if registry.loaded && !device}
	<p class="muted">Appareil inconnu.</p>
{:else if device}
	<section class="card">
		<dl>
			<dt>Pièce</dt>
			<dd>
				{#if area}<a href={resolve('/pieces/[id]', { id: area.id })}>{area.name}</a>{:else}—{/if}
			</dd>
			<dt>Intégration</dt>
			<dd>{integration ?? '—'}</dd>
			<dt>Fabricant</dt>
			<dd>{device.manufacturer ?? '—'}</dd>
			<dt>Modèle</dt>
			<dd>{device.model ?? '—'}</dd>
			<dt>Firmware</dt>
			<dd>{device.sw_version ?? '—'}</dd>
		</dl>
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
	dl {
		display: grid;
		grid-template-columns: max-content 1fr;
		gap: 6px 16px;
		margin: 0;
	}
	dt {
		color: var(--text-muted);
	}
	dd {
		margin: 0;
	}
	.muted {
		color: var(--text-muted);
	}
</style>

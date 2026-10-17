<script lang="ts">
	import { replaceState } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { domainLabel, domainOf } from '#lib/format.ts';
	import {
		applyFilter,
		countBy,
		filterFromParams,
		filterToParams,
		type EntityFilter
	} from '#lib/filters.ts';
	import EntityTable from '#lib/components/EntityTable.svelte';
	import { live } from '#lib/stores/live.svelte.ts';
	import { registry } from '#lib/stores/registry.svelte.ts';

	const idx = $derived(registry.index);
	const all = $derived([...live.states.values()]);

	// Filtre initialisé depuis l'URL, puis reflété dans l'URL (lien partageable).
	let filter = $state<EntityFilter>(filterFromParams(page.url.searchParams));
	$effect(() => {
		const qs = filterToParams(filter);
		// Chemin résolu ; la règle ne gère pas l'ajout d'une chaîne de requête.
		// eslint-disable-next-line svelte/no-navigation-without-resolve
		if (qs !== page.url.search) replaceState(resolve('/entites') + qs, {});
	});

	const results = $derived(applyFilter(all, filter, idx));
	const domains = $derived(countBy(all, (s) => domainOf(s.entity_id)));
	const integrations = $derived(countBy(all, (s) => idx.integrationOf(s.entity_id)));
	const nf = new Intl.NumberFormat('fr-FR');
</script>

<h1>Entités <small>{nf.format(results.length)} / {nf.format(all.length)}</small></h1>

<form class="filters card" onsubmit={(e) => e.preventDefault()}>
	<input
		type="search"
		placeholder="Rechercher un nom ou un identifiant…"
		bind:value={filter.q}
		aria-label="Recherche"
	/>
	<select bind:value={filter.domain} aria-label="Type">
		<option value="">Tous les types</option>
		{#each domains as [d, n] (d)}<option value={d}>{domainLabel(d)} ({n})</option>{/each}
	</select>
	<select bind:value={filter.area} aria-label="Pièce">
		<option value="">Toutes les pièces</option>
		<option value="_none">Sans pièce</option>
		{#each idx.areas as a (a.id)}<option value={a.id}>{a.name}</option>{/each}
	</select>
	<select bind:value={filter.integration} aria-label="Intégration">
		<option value="">Toutes les intégrations</option>
		{#each integrations as [i, n] (i)}<option value={i}>{i} ({n})</option>{/each}
	</select>
	<select bind:value={filter.status} aria-label="État">
		<option value="">Tous les états</option>
		<option value="disponible">Disponibles</option>
		<option value="indisponible">Indisponibles</option>
	</select>
</form>

<section class="card">
	<EntityTable items={results} index={idx} />
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
	select {
		flex: 0 1 200px;
	}
</style>

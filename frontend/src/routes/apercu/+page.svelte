<script lang="ts">
	import { resolve } from '$app/paths';
	import { byLabel, isUnavailable, plural } from '#lib/format.ts';
	import EntityTable from '#lib/components/EntityTable.svelte';
	import { live } from '#lib/stores/live.svelte.ts';
	import { registry } from '#lib/stores/registry.svelte.ts';
	import type { EntityState } from '#lib/types.ts';

	const idx = $derived(registry.index);
	const all = $derived([...live.states.values()]);
	const unavailable = $derived(all.filter(isUnavailable));
	const nf = new Intl.NumberFormat('fr-FR');

	const tiles = $derived([
		{ label: 'Entités', value: all.length, path: '/entites', query: '' },
		{
			label: 'Indisponibles',
			value: unavailable.length,
			path: '/entites',
			query: '?etat=indisponible'
		},
		{ label: 'Appareils', value: idx.devices.length, path: '/appareils', query: '' },
		{ label: 'Pièces', value: idx.areas.length, path: '/pieces', query: '' }
	] as const);

	// Derniers changements : ce qui bouge dans la maison en ce moment.
	const recent = $derived(
		all
			.filter((s) => !isUnavailable(s))
			.sort((a, b) => b.last_changed.localeCompare(a.last_changed))
			.slice(0, 12)
	);

	const byArea = $derived.by(() => {
		const groups: Record<string, EntityState[]> = {};
		for (const s of all) {
			const id = idx.areaIdOf(s.entity_id);
			if (id) (groups[id] ??= []).push(s);
		}
		return Object.entries(groups)
			.map(([id, list]) => ({
				id,
				name: idx.areaById.get(id)?.name ?? id,
				count: list.length,
				off: list.filter(isUnavailable).length
			}))
			.sort((a, b) => byLabel(a.name, b.name));
	});
</script>

<h1>Vue d'ensemble</h1>

<div class="tiles">
	{#each tiles as t (t.label)}
		<!-- La règle n'accepte pas resolve() + chaîne de requête (?etat=…) : chemin bien résolu. -->
		<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -->
		<a class="tile card" href={resolve(t.path) + t.query}>
			<span class="label">{t.label}</span>
			<span class="value">{nf.format(t.value)}</span>
		</a>
	{/each}
</div>

<div class="grid">
	<section class="card">
		<h2>Derniers changements</h2>
		<EntityTable items={recent} index={idx} showDomain={false} />
	</section>

	<section class="card">
		<h2>Par pièce</h2>
		<ul class="areas">
			{#each byArea as a (a.id)}
				<li>
					<a href={resolve('/pieces/[id]', { id: a.id })}>{a.name}</a>
					<span
						>{plural(a.count, 'entité')}{#if a.off}, <em>{plural(a.off, 'indisponible')}</em
							>{/if}</span
					>
				</li>
			{:else}
				<li class="muted">Aucune pièce renseignée.</li>
			{/each}
		</ul>
	</section>
</div>

<style>
	.tiles {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(160px, 1fr));
		gap: 12px;
		margin-bottom: 4px;
	}
	.tile {
		display: flex;
		flex-direction: column;
		gap: 4px;
		text-decoration: none;
		color: var(--text);
	}
	.tile:hover {
		border-color: var(--primary);
	}
	.label {
		color: var(--text-muted);
		font-size: 0.85rem;
	}
	.value {
		font-size: 1.8rem;
		font-weight: 600;
	}
	.grid {
		display: grid;
		grid-template-columns: minmax(0, 2fr) minmax(0, 1fr);
		gap: 16px;
	}
	@media (max-width: 900px) {
		.grid {
			grid-template-columns: minmax(0, 1fr);
		}
	}
	.areas {
		list-style: none;
		margin: 0;
		padding: 0;
	}
	.areas li {
		display: flex;
		justify-content: space-between;
		gap: 8px;
		padding: 8px 0;
		border-top: 1px solid var(--border);
	}
	.areas span {
		color: var(--text-muted);
		font-size: 0.9rem;
		text-align: right;
	}
	.muted {
		color: var(--text-muted);
		font-size: 0.85rem;
	}
</style>

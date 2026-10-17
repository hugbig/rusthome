<script lang="ts">
	import { resolve } from '$app/paths';
	import { domainLabel, domainOf, entityLabel, relativeTime } from '../format';
	import type { RegistryIndex } from '../stores/registry.svelte';
	import type { EntityState } from '../types';
	import StateBadge from './StateBadge.svelte';

	interface Props {
		items: EntityState[];
		index: RegistryIndex;
		showArea?: boolean;
		showDomain?: boolean;
		pageSize?: number;
	}
	let { items, index, showArea = true, showDomain = true, pageSize = 200 }: Props = $props();

	// Remis à pageSize si la prop change ; « Afficher plus » l'augmente localement.
	let limit = $derived(pageSize);
	const shown = $derived(items.slice(0, limit));
</script>

<div class="wrap">
	<table>
		<thead>
			<tr>
				<th>Entité</th>
				{#if showDomain}<th class="opt">Type</th>{/if}
				{#if showArea}<th class="opt">Pièce</th>{/if}
				<th>État</th>
				<th class="opt right">Modifié</th>
			</tr>
		</thead>
		<tbody>
			{#each shown as s (s.entity_id)}
				<tr>
					<td class="name">
						<a href={resolve('/entites/[entity_id]', { entity_id: s.entity_id })}>
							{entityLabel(s, index.entryByEntityId.get(s.entity_id)?.name)}
						</a>
						<code>{s.entity_id}</code>
					</td>
					{#if showDomain}<td class="opt">{domainLabel(domainOf(s.entity_id))}</td>{/if}
					{#if showArea}<td class="opt">{index.areaNameOf(s.entity_id) ?? '—'}</td>{/if}
					<td><StateBadge entity={s} /></td>
					<td class="opt right muted" title={s.last_changed}>{relativeTime(s.last_changed)}</td>
				</tr>
			{:else}
				<tr><td colspan="5" class="muted">Aucune entité.</td></tr>
			{/each}
		</tbody>
	</table>
	{#if items.length > limit}
		<button onclick={() => (limit += pageSize)}>
			Afficher plus ({items.length - limit} restantes)
		</button>
	{/if}
</div>

<style>
	.wrap {
		overflow-x: auto;
	}
	table {
		width: 100%;
		border-collapse: collapse;
	}
	th {
		text-align: left;
		font-weight: 600;
		font-size: 0.8rem;
		color: var(--text-muted);
		padding: 6px 4px;
	}
	td {
		padding: 8px 4px;
		border-top: 1px solid var(--border);
		vertical-align: middle;
	}
	.name {
		min-width: 160px;
	}
	.name a {
		color: var(--text);
		text-decoration: none;
		font-weight: 500;
	}
	.name a:hover {
		color: var(--primary-text);
		text-decoration: underline;
	}
	.name code {
		display: block;
		/* Identifiants longs : retour à la ligne plutôt que pousser les colonnes. */
		overflow-wrap: anywhere;
		font-size: 0.75rem;
		color: var(--text-muted);
	}
	.right {
		text-align: right;
		white-space: nowrap;
	}
	.muted {
		color: var(--text-muted);
	}
	button {
		margin: 12px auto 4px;
		display: block;
		padding: 6px 14px;
		border-radius: 8px;
		border: 1px solid var(--border);
		background: var(--surface-2);
		color: var(--text);
		cursor: pointer;
	}
	@media (max-width: 640px) {
		.opt {
			display: none;
		}
	}
</style>

<script lang="ts">
	// Liste de tâches, en lecture : la modifier écrirait dans HA (interdit tant
	// que la liste n'a pas migré).
	import { request } from '../../api';
	import { live } from '../../stores/live.svelte';
	import { cardName, type Card } from '../model';

	let { card }: { card: Card } = $props();

	interface Item {
		summary: string;
		status: 'needs_action' | 'completed';
	}
	const s = $derived(card.entity ? live.states.get(card.entity) : undefined);
	let items = $state<Item[]>([]);
	let error = $state<string | null>(null);

	// Recharge quand le nombre de tâches (l'état) change.
	$effect(() => {
		const id = card.entity;
		void s?.state;
		if (!id) return;
		void request<Item[]>('GET', `todo/${encodeURIComponent(id)}/items`).then((r) => {
			if (r.ok) {
				items = r.data;
				error = null;
			} else error = r.error;
		});
	});
	const open = $derived(items.filter((i) => i.status !== 'completed'));
	const done = $derived(items.filter((i) => i.status === 'completed'));
</script>

<div class="dcard">
	<h3>{typeof card.title === 'string' && card.title ? card.title : cardName(card, s)}</h3>
	{#if error}
		<p class="muted">Liste indisponible : {error}</p>
	{:else if items.length === 0}
		<p class="muted">Aucune tâche.</p>
	{:else}
		<ul>
			{#each open as it, i (i)}<li><span class="box"></span>{it.summary}</li>{/each}
			{#each done as it, i (i)}<li class="done"><span class="box">✓</span>{it.summary}</li>{/each}
		</ul>
	{/if}
</div>

<style>
	ul {
		list-style: none;
		margin: 0;
		padding: 0;
	}
	li {
		display: flex;
		align-items: center;
		gap: 10px;
		padding: 5px 0;
		font-size: 0.9rem;
	}
	.box {
		display: grid;
		place-items: center;
		width: 18px;
		height: 18px;
		border: 2px solid var(--border);
		border-radius: 4px;
		font-size: 12px;
		flex: none;
	}
	.done {
		color: var(--text-muted);
		text-decoration: line-through;
	}
</style>

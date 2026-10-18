<script lang="ts">
	import { relativeTime } from '../../format';
	import { live } from '../../stores/live.svelte';
	import { fetchHistory, logEntries, type LogEntry } from '../data';
	import { cardName, formatState, type Card } from '../model';

	let { card }: { card: Card } = $props();

	const ids = $derived(
		(Array.isArray(card.entities) ? card.entities : []).filter(
			(e): e is string => typeof e === 'string'
		)
	);
	const hours = $derived(typeof card.hours_to_show === 'number' ? card.hours_to_show : 24);
	let entries = $state<LogEntry[]>([]);
	let loaded = $state(false);

	// Recharge à chaque changement d'état d'une entité suivie.
	const key = $derived(ids.map((e) => live.states.get(e)?.last_changed ?? '').join('|'));
	$effect(() => {
		void key;
		const since = new Date(Date.now() - hours * 3600_000);
		void Promise.all(ids.map((e) => fetchHistory(e, since))).then((h) => {
			entries = logEntries(h, 40);
			loaded = true;
		});
	});
	const time = new Intl.DateTimeFormat('fr-FR', { hour: '2-digit', minute: '2-digit' });
</script>

<div class="dcard">
	{#if typeof card.title === 'string' && card.title}<h3>{card.title}</h3>{/if}
	{#if !loaded}
		<p class="muted">Chargement…</p>
	{:else if entries.length === 0}
		<p class="muted">Aucun changement sur les {hours} dernières heures.</p>
	{:else}
		<ol>
			{#each entries as e (e.entity_id + e.when)}
				{@const s = live.states.get(e.entity_id)}
				<li>
					<time
						datetime={new Date(e.when).toISOString()}
						title={relativeTime(new Date(e.when).toISOString())}>{time.format(e.when)}</time
					>
					<span class="who">{cardName({ type: 'row', entity: e.entity_id }, s)}</span>
					<span class="what">{formatState(s ? { ...s, state: e.state } : undefined)}</span>
				</li>
			{/each}
		</ol>
	{/if}
</div>

<style>
	ol {
		list-style: none;
		margin: 0;
		padding: 0;
		max-height: 320px;
		overflow-y: auto;
	}
	li {
		display: grid;
		grid-template-columns: auto minmax(0, 1fr) auto;
		gap: 8px;
		padding: 6px 0;
		border-top: 1px solid var(--border);
		font-size: 0.85rem;
		align-items: baseline;
	}
	time {
		color: var(--text-muted);
		font-variant-numeric: tabular-nums;
	}
	.who {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.what {
		color: var(--text-muted);
		white-space: nowrap;
	}
</style>

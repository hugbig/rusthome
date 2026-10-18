<script lang="ts">
	// Une courbe (ou des barres journalières) par entité : petits multiples
	// plutôt qu'un graphe à plusieurs échelles.
	import BarChart from '../../components/BarChart.svelte';
	import LineChart from '../../components/LineChart.svelte';
	import { toSeries } from '../../history';
	import { live } from '../../stores/live.svelte';
	import type { EntityState } from '../../types';
	import { dailyBars, fetchHistory } from '../data';
	import { cardName, type Card } from '../model';

	let { card }: { card: Card } = $props();

	const ids = $derived(
		(Array.isArray(card.entities) ? card.entities : []).filter(
			(e): e is string => typeof e === 'string'
		)
	);
	const hours = $derived(typeof card.hours_to_show === 'number' ? card.hours_to_show : 24);
	const bars = $derived(card.chart === 'bar');
	let histories = $state<Record<string, EntityState[]>>({});
	let range = $state({ from: 0, to: 0 });

	async function load() {
		const to = Date.now();
		const from = to - hours * 3600_000;
		const got = await Promise.all(ids.map((e) => fetchHistory(e, new Date(from))));
		histories = Object.fromEntries(ids.map((e, i) => [e, got[i] ?? []]));
		range = { from, to };
	}
	$effect(() => {
		void ids;
		void hours;
		void load();
		const t = setInterval(load, 5 * 60_000);
		return () => clearInterval(t);
	});
</script>

<div class="dcard">
	{#if typeof card.name === 'string' && card.name}<h3>{card.name}</h3>{/if}
	{#each ids as id (id)}
		{@const s = live.states.get(id)}
		{@const unit =
			typeof s?.attributes.unit_of_measurement === 'string' ? s.attributes.unit_of_measurement : ''}
		{#if ids.length > 1 || !card.name}<p class="muted">
				{cardName({ type: 'row', entity: id }, s)}
			</p>{/if}
		{#if bars}
			<BarChart
				bars={dailyBars(histories[id] ?? [], card.aggregate === 'max' ? 'max' : 'last')}
				{unit}
				height={160}
				label={cardName({ type: 'row', entity: id }, s)}
			/>
		{:else}
			<LineChart
				points={toSeries(histories[id] ?? [])}
				from={range.from}
				to={range.to}
				{unit}
				height={160}
				label={cardName({ type: 'row', entity: id }, s)}
			/>
		{/if}
	{/each}
</div>

<style>
	.muted {
		margin: 4px 0;
	}
</style>

<script lang="ts">
	import LineChart from '#lib/components/LineChart.svelte';
	import StateTimeline from '#lib/components/StateTimeline.svelte';
	import EntityPicker from '#lib/components/EntityPicker.svelte';
	import { fetchHistory } from '#lib/dashboard/data.ts';
	import { cardName } from '#lib/dashboard/model.ts';
	import { isNumericEntity } from '#lib/format.ts';
	import { normalizeHistory, toSegments, toSeries } from '#lib/history.ts';
	import { live } from '#lib/stores/live.svelte.ts';
	import type { EntityState } from '#lib/types.ts';

	// Mémorisé pour la prochaine visite (confort, perdu sans conséquence).
	const KEY = 'rusthome.historique';
	function stored(): string[] {
		try {
			const v: unknown = JSON.parse(localStorage.getItem(KEY) ?? '[]');
			return Array.isArray(v) ? v.filter((x): x is string => typeof x === 'string') : [];
		} catch {
			return [];
		}
	}
	let ids = $state<string[]>(stored());
	let hours = $state(24);
	let histories = $state<Record<string, EntityState[]>>({});
	let range = $state({ from: Date.now() - 86400_000, to: Date.now() });

	$effect(() => {
		try {
			localStorage.setItem(KEY, JSON.stringify(ids));
		} catch {
			/* stockage indisponible : sans conséquence */
		}
	});
	$effect(() => {
		const list = [...ids];
		const to = Date.now();
		const from = to - hours * 3600_000;
		void Promise.all(list.map((e) => fetchHistory(e, new Date(from)))).then((got) => {
			histories = Object.fromEntries(list.map((e, i) => [e, got[i] ?? []]));
			range = { from, to };
		});
	});
	const remove = (id: string) => (ids = ids.filter((x) => x !== id));
</script>

<h1>Historique</h1>

<div class="card bar">
	<EntityPicker
		placeholder="Ajouter une entité…"
		onpick={(id) => {
			if (!ids.includes(id)) ids = [...ids, id];
		}}
	/>
	<select bind:value={hours} aria-label="Période">
		<option value={6}>6 h</option>
		<option value={24}>24 h</option>
		<option value={72}>3 jours</option>
		<option value={168}>7 jours</option>
	</select>
</div>

{#if ids.length === 0}
	<p class="muted">Choisissez une ou plusieurs entités à tracer.</p>
{/if}

{#each ids as id (id)}
	{@const s = live.states.get(id)}
	{@const h = normalizeHistory(histories[id] ?? (s ? [s] : []))}
	{@const name = cardName({ type: 'row', entity: id }, s)}
	<section class="card">
		<div class="head">
			<h2>{name} <code>{id}</code></h2>
			<button onclick={() => remove(id)} aria-label="Retirer {name}">Retirer</button>
		</div>
		{#if s && isNumericEntity(s)}
			<LineChart
				points={toSeries(h)}
				from={range.from}
				to={range.to}
				unit={String(s.attributes.unit_of_measurement ?? '')}
				label="Historique de {name}"
			/>
		{:else}
			<StateTimeline
				segments={toSegments(h, range.from, range.to)}
				from={range.from}
				to={range.to}
				label="États de {name}"
			/>
		{/if}
	</section>
{/each}

<style>
	.bar {
		display: flex;
		flex-wrap: wrap;
		gap: 8px;
		align-items: center;
	}
	select,
	button {
		padding: 6px 10px;
		border: 1px solid var(--border);
		border-radius: 8px;
		background: var(--surface-2);
		color: var(--text);
		font: inherit;
		cursor: pointer;
	}
	.head {
		display: flex;
		justify-content: space-between;
		align-items: center;
		gap: 8px;
	}
	code {
		font-size: 0.75rem;
		color: var(--text-muted);
		font-weight: normal;
	}
	.muted {
		color: var(--text-muted);
	}
</style>

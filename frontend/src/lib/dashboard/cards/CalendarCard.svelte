<script lang="ts">
	import { request } from '../../api';
	import { live } from '../../stores/live.svelte';
	import { groupByDay, parseEvent, type CalEvent } from '../data';
	import { cardName, type Card } from '../model';

	let { card }: { card: Card } = $props();

	const ids = $derived(
		(Array.isArray(card.entities) ? card.entities : []).filter(
			(e): e is string => typeof e === 'string'
		)
	);
	const days = $derived(typeof card.days === 'number' ? card.days : 7);
	let events = $state<CalEvent[]>([]);
	let error = $state<string | null>(null);
	let loaded = $state(false);

	async function load() {
		const all = await Promise.all(
			ids.map(async (id) => {
				const r = await request<Record<string, unknown>[]>(
					'GET',
					`calendars/${encodeURIComponent(id)}/events?days=${days}`
				);
				if (!r.ok) error = r.error;
				return r.ok
					? r.data.map((e) => parseEvent(e, id)).filter((e): e is CalEvent => e !== null)
					: [];
			})
		);
		events = all.flat();
		loaded = true;
	}
	$effect(() => {
		void ids;
		void days;
		void load();
		const t = setInterval(load, 15 * 60_000);
		return () => clearInterval(t);
	});

	const groups = $derived(groupByDay(events));
	const dayFmt = new Intl.DateTimeFormat('fr-FR', {
		weekday: 'long',
		day: 'numeric',
		month: 'long'
	});
	const timeFmt = new Intl.DateTimeFormat('fr-FR', { hour: '2-digit', minute: '2-digit' });
	const calName = (id: string) => cardName({ type: 'row', entity: id }, live.states.get(id));
</script>

<div class="dcard">
	{#if typeof card.title === 'string' && card.title}<h3>{card.title}</h3>{/if}
	{#if !loaded}
		<p class="muted">Chargement…</p>
	{:else if groups.length === 0}
		<p class="muted">
			{error ? `Agenda indisponible : ${error}` : `Rien sur les ${days} prochains jours.`}
		</p>
	{:else}
		{#each groups as g (g.day)}
			<h4>{dayFmt.format(g.day)}</h4>
			<ul>
				{#each g.events as e, i (i)}
					<li>
						<span class="when">{e.allDay ? 'Journée' : timeFmt.format(e.start)}</span>
						<span class="what">
							{e.summary}
							{#if ids.length > 1}<small class="muted">{calName(e.calendar)}</small>{/if}
						</span>
					</li>
				{/each}
			</ul>
		{/each}
	{/if}
</div>

<style>
	h4 {
		margin: 10px 0 4px;
		font-size: 0.85rem;
		color: var(--text-muted);
		text-transform: capitalize;
	}
	ul {
		list-style: none;
		margin: 0;
		padding: 0;
	}
	li {
		display: grid;
		grid-template-columns: 64px minmax(0, 1fr);
		gap: 8px;
		padding: 4px 0;
		font-size: 0.9rem;
	}
	.when {
		color: var(--text-muted);
		font-variant-numeric: tabular-nums;
	}
	.what {
		display: flex;
		flex-direction: column;
	}
</style>

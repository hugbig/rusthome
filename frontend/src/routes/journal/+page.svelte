<script lang="ts">
	import { resolve } from '$app/paths';
	import { request } from '#lib/api.ts';
	import { cardName, formatState } from '#lib/dashboard/model.ts';
	import { domainLabel, domainOf } from '#lib/format.ts';
	import { live } from '#lib/stores/live.svelte.ts';

	interface Entry {
		entity_id: string;
		state: string;
		previous: string;
		when: string;
	}

	let hours = $state(24);
	let q = $state('');
	let entries = $state<Entry[]>([]);
	let error = $state<string | null>(null);
	let loading = $state(true);

	async function load() {
		loading = true;
		const r = await request<Entry[]>('GET', `logbook?hours=${hours}&limit=2000`);
		loading = false;
		if (r.ok) {
			entries = r.data;
			error = null;
		} else error = r.error;
	}
	$effect(() => {
		void hours;
		void load();
	});

	const nameOf = (id: string) => cardName({ type: 'row', entity: id }, live.states.get(id));
	/** Libellé d'un état passé, avec les attributs actuels (unité, classe). */
	const label = (id: string, state: string) => {
		const s = live.states.get(id);
		return formatState(s ? { ...s, state } : undefined) === 'Introuvable'
			? state
			: formatState(s ? { ...s, state } : undefined);
	};
	const shown = $derived(
		entries.filter(
			(e) =>
				!q ||
				nameOf(e.entity_id).toLowerCase().includes(q.trim().toLowerCase()) ||
				e.entity_id.includes(q.trim().toLowerCase())
		)
	);
	const dayFmt = new Intl.DateTimeFormat('fr-FR', {
		weekday: 'long',
		day: 'numeric',
		month: 'long'
	});
	const timeFmt = new Intl.DateTimeFormat('fr-FR', { hour: '2-digit', minute: '2-digit' });
	const groups = $derived.by(() => {
		const out: { day: string; items: Entry[] }[] = [];
		for (const e of shown) {
			const day = dayFmt.format(new Date(e.when));
			if (out.at(-1)?.day !== day) out.push({ day, items: [] });
			out.at(-1)!.items.push(e);
		}
		return out;
	});
</script>

<h1>Journal</h1>

<div class="filters card">
	<input type="search" placeholder="Filtrer par entité…" bind:value={q} aria-label="Filtre" />
	<select bind:value={hours} aria-label="Période">
		<option value={6}>6 dernières heures</option>
		<option value={24}>24 dernières heures</option>
		<option value={72}>3 derniers jours</option>
		<option value={168}>7 derniers jours</option>
	</select>
</div>

{#if error}
	<p class="card">Journal indisponible : {error}</p>
{:else if loading && entries.length === 0}
	<p class="muted">Chargement…</p>
{:else if groups.length === 0}
	<p class="muted">Aucun changement sur la période.</p>
{:else}
	{#each groups as g (g.day)}
		<section class="card">
			<h2>{g.day}</h2>
			<ol>
				{#each g.items as e, i (i)}
					<li>
						<time>{timeFmt.format(new Date(e.when))}</time>
						<span>
							<a href={resolve('/entites/[entity_id]', { entity_id: e.entity_id })}
								>{nameOf(e.entity_id)}</a
							>
							<small>{domainLabel(domainOf(e.entity_id))}</small>
						</span>
						<span class="change"
							>{label(e.entity_id, e.previous)} →
							<strong>{label(e.entity_id, e.state)}</strong></span
						>
					</li>
				{/each}
			</ol>
		</section>
	{/each}
{/if}

<style>
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
	}
	input {
		flex: 1 1 220px;
	}
	h2 {
		text-transform: capitalize;
	}
	ol {
		list-style: none;
		margin: 0;
		padding: 0;
	}
	li {
		display: grid;
		grid-template-columns: 52px minmax(0, 1fr) auto;
		gap: 10px;
		padding: 7px 0;
		border-top: 1px solid var(--border);
		font-size: 0.9rem;
		align-items: baseline;
	}
	time {
		color: var(--text-muted);
		font-variant-numeric: tabular-nums;
	}
	li a {
		color: var(--text);
		text-decoration: none;
		font-weight: 500;
	}
	small {
		display: block;
		color: var(--text-muted);
		font-size: 0.75rem;
	}
	.change {
		color: var(--text-muted);
		text-align: right;
	}
	.change strong {
		color: var(--text);
	}
	.muted {
		color: var(--text-muted);
	}
	@media (max-width: 600px) {
		li {
			grid-template-columns: 44px minmax(0, 1fr);
		}
		.change {
			grid-column: 2;
			text-align: left;
		}
	}
</style>

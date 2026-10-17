<script lang="ts">
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import {
		domainLabel,
		domainOf,
		entityLabel,
		isNumericEntity,
		relativeTime
	} from '#lib/format.ts';
	import { normalizeHistory, toSegments, toSeries } from '#lib/history.ts';
	import LineChart from '#lib/components/LineChart.svelte';
	import EntityControls from '#lib/components/EntityControls.svelte';
	import StateBadge from '#lib/components/StateBadge.svelte';
	import StateTimeline from '#lib/components/StateTimeline.svelte';
	import { live } from '#lib/stores/live.svelte.ts';
	import { registry } from '#lib/stores/registry.svelte.ts';
	import type { EntityState } from '#lib/types.ts';

	const entityId = $derived(page.params.entity_id ?? '');
	const current = $derived(live.states.get(entityId));
	const idx = $derived(registry.index);
	const entry = $derived(idx.entryByEntityId.get(entityId));
	const device = $derived(entry?.device_id ? idx.deviceById.get(entry.device_id) : undefined);
	const areaId = $derived(idx.areaIdOf(entityId));
	const area = $derived(areaId ? idx.areaById.get(areaId) : undefined);
	const name = $derived(entityLabel(current, entry?.name) || entityId);

	const PERIODS = [
		{ id: '24h', label: '24 h', ms: 86400_000 },
		{ id: '7j', label: '7 jours', ms: 7 * 86400_000 }
	] as const;
	let period = $state<(typeof PERIODS)[number]>(PERIODS[0]);

	let history = $state<EntityState[]>([]);
	let loading = $state(false);
	let error = $state<string | null>(null);
	let now = $state(Date.now());
	const from = $derived(now - period.ms);

	$effect(() => {
		const id = entityId;
		const since = new Date(Date.now() - period.ms).toISOString();
		loading = true;
		error = null;
		fetch(`/api/v1/history/${encodeURIComponent(id)}?since=${encodeURIComponent(since)}`)
			.then((r) =>
				r.ok ? (r.json() as Promise<EntityState[]>) : Promise.reject(new Error(`HTTP ${r.status}`))
			)
			.then((h) => {
				history = h;
				now = Date.now();
			})
			.catch((e: unknown) => (error = e instanceof Error ? e.message : String(e)))
			.finally(() => (loading = false));
	});

	// Les changements reçus en direct prolongent l'historique sans recharger.
	$effect(() => {
		const s = current;
		if (!s) return;
		now = Date.now();
		const last = history.at(-1);
		if (!last || last.last_changed !== s.last_changed || last.state !== s.state)
			history = [...history, s];
	});

	// Sans historique local, l'état courant suffit à tracer quelque chose.
	const effective = $derived(normalizeHistory(history.length ? history : current ? [current] : []));
	const numeric = $derived(current ? isNumericEntity(current) : false);
	const unit = $derived(current?.attributes.unit_of_measurement as string | undefined);
	const changes = $derived([...effective].reverse().slice(0, 50));
	const attributes = $derived(
		Object.entries(current?.attributes ?? {}).sort(([a], [b]) => a.localeCompare(b))
	);
	const fmt = new Intl.DateTimeFormat('fr-FR', { dateStyle: 'short', timeStyle: 'medium' });

	function show(v: unknown): string {
		return typeof v === 'string' ? v : JSON.stringify(v);
	}
</script>

<p class="crumbs">
	<a href={resolve('/entites')}>Entités</a> › {domainLabel(domainOf(entityId))}
</p>

<header class="head card">
	<div>
		<h1>{name}</h1>
		<code>{entityId}</code>
		<p class="meta">
			{#if area}Pièce : <a href={resolve('/pieces/[id]', { id: area.id })}>{area.name}</a> ·
			{/if}
			{#if device}Appareil : <a href={resolve('/appareils/[id]', { id: device.id })}
					>{device.name}</a
				> ·
			{/if}
			{#if entry}Intégration : {entry.integration.replace(/^ha:/, '')}{/if}
		</p>
	</div>
	{#if current}
		<div class="now">
			<StateBadge entity={current} />
			<small title={current.last_changed}>depuis {relativeTime(current.last_changed)}</small>
			<EntityControls entity={current} />
		</div>
	{:else}
		<p class="muted">Entité absente des états en direct.</p>
	{/if}
</header>

<section class="card">
	<div class="row">
		<h2>Historique</h2>
		<div class="periods" role="group" aria-label="Période">
			{#each PERIODS as p (p.id)}
				<button aria-pressed={period.id === p.id} onclick={() => (period = p)}>{p.label}</button>
			{/each}
		</div>
	</div>
	{#if error}
		<p class="muted">Historique indisponible : {error}</p>
	{:else if loading && history.length === 0}
		<p class="muted">Chargement…</p>
	{:else if numeric}
		<LineChart points={toSeries(effective)} {from} to={now} {unit} label="Historique de {name}" />
	{:else}
		<StateTimeline
			segments={toSegments(effective, from, now)}
			{from}
			to={now}
			label="États de {name}"
		/>
	{/if}
	<p class="note">
		Historique enregistré par RustHome depuis son démarrage (l'historique de Home Assistant n'est
		pas encore importé).
	</p>
</section>

<div class="grid">
	<section class="card">
		<h2>Derniers changements</h2>
		<table>
			<tbody>
				{#each changes as c, i (i)}
					<tr><td>{fmt.format(new Date(c.last_changed))}</td><td><StateBadge entity={c} /></td></tr>
				{:else}
					<tr><td class="muted">Aucun.</td></tr>
				{/each}
			</tbody>
		</table>
	</section>

	<section class="card">
		<h2>Attributs</h2>
		<table>
			<tbody>
				{#each attributes as [k, v] (k)}
					<tr><th>{k}</th><td>{show(v)}</td></tr>
				{:else}
					<tr><td class="muted">Aucun.</td></tr>
				{/each}
			</tbody>
		</table>
	</section>
</div>

<style>
	.crumbs {
		margin: 0 0 8px;
		color: var(--text-muted);
		font-size: 0.9rem;
	}
	.head {
		display: flex;
		flex-wrap: wrap;
		justify-content: space-between;
		gap: 12px;
		align-items: flex-start;
	}
	h1 {
		margin: 0;
	}
	code {
		color: var(--text-muted);
		font-size: 0.8rem;
	}
	.meta {
		margin: 8px 0 0;
		font-size: 0.9rem;
		color: var(--text-muted);
	}
	.now {
		display: flex;
		flex-direction: column;
		align-items: flex-end;
		gap: 4px;
		font-size: 1.3rem;
	}
	.now small {
		font-size: 0.8rem;
		color: var(--text-muted);
	}
	.row {
		display: flex;
		justify-content: space-between;
		align-items: center;
		gap: 8px;
	}
	.periods {
		display: flex;
		gap: 4px;
	}
	.periods button {
		padding: 4px 10px;
		border-radius: 8px;
		border: 1px solid var(--border);
		background: var(--surface-2);
		color: var(--text);
		cursor: pointer;
	}
	.periods button[aria-pressed='true'] {
		background: var(--primary);
		color: var(--on-primary);
		border-color: var(--primary);
	}
	.note {
		margin: 8px 0 0;
		font-size: 0.8rem;
		color: var(--text-muted);
	}
	.grid {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(320px, 1fr));
		gap: 16px;
	}
	table {
		width: 100%;
		border-collapse: collapse;
		font-size: 0.9rem;
	}
	th,
	td {
		text-align: left;
		padding: 6px 4px;
		border-top: 1px solid var(--border);
		vertical-align: top;
		overflow-wrap: anywhere;
	}
	th {
		font-weight: 500;
		color: var(--text-muted);
		width: 40%;
	}
	.muted {
		color: var(--text-muted);
	}
</style>

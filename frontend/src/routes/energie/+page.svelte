<script lang="ts">
	// Tableau Énergie, comme celui de HA : indicateurs, répartition, utilisation
	// dans le temps, sources (avec coût si un tarif est configuré), appareils au
	// total et dans le temps. Données : statistiques de HA (lecture seule) ;
	// l'heure en cours vient des statistiques sur 5 minutes.
	import { onMount } from 'svelte';
	import {
		mdiChevronLeft,
		mdiChevronRight,
		mdiCurrencyEur,
		mdiPowerPlug,
		mdiTransmissionTower
	} from '@mdi/js';
	import { page } from '$app/state';
	import { request } from '#lib/api.ts';
	import EnergyFlow from '#lib/components/EnergyFlow.svelte';
	import StackedBars, { type Series } from '#lib/components/StackedBars.svelte';
	import Icon from '#lib/dashboard/Icon.svelte';
	import { cardName } from '#lib/dashboard/model.ts';
	import {
		aggregate,
		bucketOf,
		changePct,
		clip,
		energyConfig,
		fetchPeriod,
		gridPricing,
		periodRange,
		slots as slotsOf,
		total,
		type EnergyConfig,
		type Pricing,
		type Span,
		type StatRow
	} from '#lib/energy.ts';
	import { live } from '#lib/stores/live.svelte.ts';

	let config = $state<EnergyConfig | null>(null);
	let pricing = $state<Pricing>({ costStats: [] });
	let error = $state<string | null>(null);
	// Période initiale depuis l'URL : ?periode=semaine&decalage=-1
	const PERIODS: Record<string, Span> = {
		jour: 'day',
		semaine: 'week',
		mois: 'month',
		annee: 'year'
	};
	let span = $state<Span>(PERIODS[page.url.searchParams.get('periode') ?? ''] ?? 'day');
	let offset = $state(Math.min(0, Number(page.url.searchParams.get('decalage')) || 0));
	let compare = $state(false);
	let stats = $state<Record<string, StatRow[]>>({});
	let previous = $state<Record<string, StatRow[]>>({});
	let loading = $state(false);

	onMount(async () => {
		const r = await request<unknown>('GET', 'energy/prefs');
		if (r.ok) {
			config = energyConfig(r.data);
			pricing = gridPricing(r.data);
		} else error = r.error;
	});

	const range = $derived(periodRange(span, offset));
	const unit = $derived(bucketOf(span));

	async function fetchStats(
		ids: string[],
		s: Span,
		off: number
	): Promise<Record<string, StatRow[]> | null> {
		const [start, end] = periodRange(s, off);
		const q = new URLSearchParams({
			ids: ids.join(','),
			start: start.toISOString(),
			end: end.toISOString(),
			period: fetchPeriod(s, off)
		});
		const r = await request<Record<string, StatRow[]>>('GET', `energy/statistics?${q}`);
		if (!r.ok) {
			error = r.error;
			return null;
		}
		error = null;
		const u = bucketOf(s);
		return Object.fromEntries(
			Object.entries(r.data).map(([k, rows]) => [k, aggregate(clip(rows, start, end), u)])
		);
	}

	$effect(() => {
		const c = config;
		if (!c) return;
		const ids = [...c.grid, ...c.gridReturn, ...c.devices, ...pricing.costStats];
		if (!ids.length) return;
		const s = span;
		const off = offset;
		const cmp = compare;
		loading = true;
		void Promise.all([fetchStats(ids, s, off), cmp ? fetchStats(ids, s, off - 1) : null]).then(
			([now, before]) => {
				loading = false;
				if (now) stats = now;
				previous = before ?? {};
			}
		);
	});

	const sum = (src: Record<string, StatRow[]>, ids: string[]) =>
		ids.reduce((a, id) => a + total(src[id]), 0);
	const gridTotal = $derived(config ? sum(stats, config.grid) : 0);
	const returnTotal = $derived(config ? sum(stats, config.gridReturn) : 0);
	const prevGrid = $derived(config ? sum(previous, config.grid) : 0);
	const delta = $derived(compare ? changePct(gridTotal, prevGrid) : undefined);

	const name = (id: string) => cardName({ type: 'row', entity: id }, live.states.get(id));
	/** Couleur fixe par appareil : son rang dans la configuration, pas dans le classement. */
	const deviceColor = (i: number) => `var(--series-${(i % 6) + 1})`;
	const devices = $derived(
		(config?.devices ?? []).map((id, i) => ({
			id,
			name: name(id),
			color: deviceColor(i),
			value: total(stats[id])
		}))
	);
	const ranked = $derived([...devices].sort((a, b) => b.value - a.value));
	const tracked = $derived(devices.reduce((a, d) => a + d.value, 0));
	const untracked = $derived(Math.max(0, gridTotal - tracked));
	const maxDevice = $derived(Math.max(untracked, ...devices.map((d) => d.value), 0.001));

	// Coût : statistiques de coût de HA, sinon prix fixe, sinon prix courant.
	const cost = $derived.by(() => {
		if (pricing.costStats.length) return sum(stats, pricing.costStats);
		if (pricing.price !== undefined) return gridTotal * pricing.price;
		const p = pricing.priceEntity ? Number(live.states.get(pricing.priceEntity)?.state) : NaN;
		return Number.isFinite(p) ? gridTotal * p : undefined;
	});

	// Séries alignées sur tous les créneaux de la période (axe régulier).
	const slots = $derived(slotsOf(range[0], range[1], unit));
	const valuesOf = (rows: StatRow[] | undefined) => {
		const m = new Map((rows ?? []).map((r) => [r.start, r.change ?? 0]));
		return slots.map((t) => m.get(t) ?? 0);
	};
	const gridValues = $derived(
		slots.map((_, i) => (config?.grid ?? []).reduce((a, id) => a + valuesOf(stats[id])[i]!, 0))
	);
	const prevValues = $derived.by(() => {
		if (!compare || !config) return [];
		const [ps, pe] = periodRange(span, offset - 1);
		const pslots = slotsOf(ps, pe, unit);
		return pslots.map((t) =>
			config!.grid.reduce(
				(a, id) => a + ((previous[id] ?? []).find((r) => r.start === t)?.change ?? 0),
				0
			)
		);
	});
	const usageSeries = $derived<Series[]>([
		{ name: 'Réseau', color: 'var(--series-1)', values: gridValues }
	]);
	const ghost = $derived<Series | undefined>(
		compare
			? { name: 'Période précédente', color: 'var(--neutral-fill)', values: prevValues }
			: undefined
	);
	const deviceSeries = $derived.by<Series[]>(() => {
		const per = devices.map((d) => ({
			name: d.name,
			color: d.color,
			values: valuesOf(stats[d.id])
		}));
		const rest = gridValues.map((g, i) =>
			Math.max(0, g - per.reduce((a, s) => a + s.values[i]!, 0))
		);
		return [...per, { name: 'Non suivi', color: 'var(--neutral-fill)', values: rest }];
	});

	const kwh = new Intl.NumberFormat('fr-FR', { maximumFractionDigits: 2 });
	const eur = new Intl.NumberFormat('fr-FR', { style: 'currency', currency: 'EUR' });
	const title = $derived.by(() => {
		const [s, e] = range;
		if (span === 'day')
			return new Intl.DateTimeFormat('fr-FR', {
				weekday: 'long',
				day: 'numeric',
				month: 'long'
			}).format(s);
		if (span === 'week') {
			const last = new Date(e.getTime() - 86400_000);
			const f = new Intl.DateTimeFormat('fr-FR', { day: 'numeric', month: 'short' });
			return `${f.format(s)} – ${f.format(last)}`;
		}
		if (span === 'month')
			return new Intl.DateTimeFormat('fr-FR', { month: 'long', year: 'numeric' }).format(s);
		return String(s.getFullYear());
	});
	const tick = $derived(
		span === 'day'
			? (t: number) => `${new Date(t).getHours()} h`
			: span === 'year'
				? (t: number) => new Intl.DateTimeFormat('fr-FR', { month: 'short' }).format(t)
				: span === 'week'
					? (t: number) => new Intl.DateTimeFormat('fr-FR', { weekday: 'short' }).format(t)
					: (t: number) => String(new Date(t).getDate())
	);
	const tip = $derived(
		span === 'day'
			? (t: number) => {
					const h = new Date(t).getHours();
					return `${h} h – ${h + 1} h`;
				}
			: span === 'year'
				? (t: number) =>
						new Intl.DateTimeFormat('fr-FR', { month: 'long', year: 'numeric' }).format(t)
				: (t: number) => new Intl.DateTimeFormat('fr-FR', { dateStyle: 'full' }).format(t)
	);
	const per = $derived(span === 'day' ? 'par heure' : span === 'year' ? 'par mois' : 'par jour');
	const SPANS: [Span, string][] = [
		['day', 'Jour'],
		['week', 'Semaine'],
		['month', 'Mois'],
		['year', 'Année']
	];
</script>

<div class="head">
	<h1>Énergie</h1>
	<div class="seg" role="radiogroup" aria-label="Période">
		{#each SPANS as [v, l] (v)}
			<button
				role="radio"
				aria-checked={span === v}
				onclick={() => {
					span = v;
					offset = 0;
				}}>{l}</button
			>
		{/each}
	</div>
</div>

<div class="nav">
	<button class="round" aria-label="Période précédente" onclick={() => offset--}
		><Icon path={mdiChevronLeft} /></button
	>
	<strong class="when">{title}</strong>
	<button
		class="round"
		aria-label="Période suivante"
		disabled={offset >= 0}
		onclick={() => offset++}><Icon path={mdiChevronRight} /></button
	>
	{#if offset !== 0}<button class="chip" onclick={() => (offset = 0)}>Aujourd’hui</button>{/if}
	<label class="cmp"
		><input type="checkbox" bind:checked={compare} /> Comparer à la période précédente</label
	>
	{#if loading}<span class="muted">Chargement…</span>{/if}
</div>

{#if error}
	<p class="card">Énergie indisponible : {error}</p>
{:else if config && !config.grid.length && !config.devices.length}
	<p class="card">Aucun compteur d’énergie n’est configuré dans Home Assistant.</p>
{:else if config}
	<div class="kpis">
		<div class="card kpi">
			<span class="kic" style:--c="var(--series-1)"><Icon path={mdiTransmissionTower} /></span>
			<div>
				<span class="kl">Consommation du réseau</span>
				<span class="kv">{kwh.format(gridTotal)} <small>kWh</small></span>
				{#if delta !== undefined}
					<span class="delta" class:up={delta > 0}
						>{delta > 0 ? '+' : ''}{delta} % vs période précédente</span
					>
				{/if}
			</div>
		</div>
		<div class="card kpi">
			<span class="kic" style:--c="var(--series-4)"><Icon path={mdiPowerPlug} /></span>
			<div>
				<span class="kl">Appareils suivis</span>
				<span class="kv"
					>{kwh.format(tracked)}
					<small>kWh{gridTotal > 0 ? ` · ${Math.round((tracked / gridTotal) * 100)} %` : ''}</small
					></span
				>
			</div>
		</div>
		<div class="card kpi">
			<span class="kic" style:--c="var(--series-3)"><Icon path={mdiCurrencyEur} /></span>
			<div>
				<span class="kl">Coût estimé</span>
				<span class="kv">{cost !== undefined ? eur.format(cost) : '—'}</span>
				{#if cost === undefined}<span class="delta">Aucun tarif configuré dans HA</span>{/if}
			</div>
		</div>
	</div>

	<div class="cols">
		<section class="card">
			<h2>Répartition de l’énergie</h2>
			<EnergyFlow fromGrid={gridTotal} toGrid={returnTotal} {tracked} />
		</section>

		<section class="card">
			<h2>Sources</h2>
			<table>
				<thead><tr><th>Source</th><th class="r">Énergie</th><th class="r">Coût</th></tr></thead>
				<tbody>
					{#each config.grid as id (id)}
						<tr>
							<td><span class="sw" style:background="var(--series-1)"></span>{name(id)}</td>
							<td class="r">{kwh.format(total(stats[id]))} kWh</td>
							<td class="r">
								{pricing.price !== undefined
									? eur.format(total(stats[id]) * pricing.price)
									: config.grid.length === 1 && cost !== undefined
										? eur.format(cost)
										: '—'}
							</td>
						</tr>
					{/each}
					{#each config.gridReturn as id (id)}
						<tr>
							<td><span class="sw" style:background="var(--series-3)"></span>{name(id)} (retour)</td
							>
							<td class="r">−{kwh.format(total(stats[id]))} kWh</td>
							<td class="r">—</td>
						</tr>
					{/each}
				</tbody>
				<tfoot>
					<tr>
						<td>Total</td>
						<td class="r">{kwh.format(gridTotal - returnTotal)} kWh</td>
						<td class="r">{cost !== undefined ? eur.format(cost) : '—'}</td>
					</tr>
				</tfoot>
			</table>
		</section>
	</div>

	<section class="card">
		<h2>Utilisation de l’énergie {per}</h2>
		<StackedBars
			{slots}
			series={usageSeries}
			{ghost}
			unit="kWh"
			label="Consommation du réseau {per}"
			{tick}
			{tip}
		/>
	</section>

	<div class="cols">
		<section class="card">
			<h2>Consommation des appareils</h2>
			{#if devices.length === 0}
				<p class="muted">Aucun appareil suivi dans HA.</p>
			{:else}
				<ul class="devs">
					{#each [...ranked, { id: '', name: 'Non suivi', color: 'var(--neutral-fill)', value: untracked }] as d (d.id || 'rest')}
						<li class:rest={!d.id}>
							<span class="dn"><span class="sw" style:background={d.color}></span>{d.name}</span>
							<span class="track"
								><span
									class="fill"
									style:width="{(d.value / maxDevice) * 100}%"
									style:background={d.color}
								></span></span
							>
							<span class="dv">{kwh.format(d.value)} kWh</span>
						</li>
					{/each}
				</ul>
			{/if}
		</section>

		<section class="card">
			<h2>Utilisation détaillée des appareils</h2>
			<StackedBars
				{slots}
				series={deviceSeries}
				unit="kWh"
				label="Consommation par appareil {per}"
				{tick}
				{tip}
			/>
		</section>
	</div>
	<p class="note">
		Statistiques lues dans Home Assistant (lecture seule) ; l’heure en cours vient des statistiques
		sur 5 minutes.
	</p>
{/if}

<style>
	.head {
		display: flex;
		flex-wrap: wrap;
		justify-content: space-between;
		align-items: center;
		gap: 8px;
	}
	.seg {
		display: flex;
		gap: 2px;
		padding: 3px;
		border-radius: 999px;
		background: var(--surface-2);
	}
	.seg button {
		padding: 6px 14px;
		border: none;
		border-radius: 999px;
		background: none;
		color: var(--text-muted);
		font: inherit;
		cursor: pointer;
	}
	.seg button[aria-checked='true'] {
		background: var(--surface);
		color: var(--text);
		box-shadow: var(--shadow);
	}
	.nav {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 8px 10px;
		margin-bottom: 16px;
	}
	.round {
		display: grid;
		place-items: center;
		width: 36px;
		height: 36px;
		border: none;
		border-radius: 50%;
		background: var(--surface-2);
		color: var(--text);
		cursor: pointer;
	}
	.round:disabled {
		opacity: 0.35;
		cursor: default;
	}
	.when {
		font-size: 1.05rem;
		text-transform: capitalize;
		min-width: 12ch;
		text-align: center;
	}
	.chip {
		padding: 6px 12px;
		border: 1px solid var(--border);
		border-radius: 999px;
		background: none;
		color: var(--primary-text);
		font: inherit;
		cursor: pointer;
	}
	.cmp {
		display: flex;
		align-items: center;
		gap: 6px;
		margin-left: auto;
		font-size: 0.9rem;
		color: var(--text-muted);
	}
	.kpis {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
		gap: 12px;
	}
	.kpi {
		display: flex;
		align-items: center;
		gap: 14px;
		margin: 0;
	}
	.kpi > div {
		display: flex;
		flex-direction: column;
	}
	.kic {
		display: grid;
		place-items: center;
		width: 44px;
		height: 44px;
		border-radius: 12px;
		flex: none;
		background: color-mix(in srgb, var(--c) 18%, transparent);
		color: var(--c);
	}
	.kl {
		color: var(--text-muted);
		font-size: 0.85rem;
	}
	.kv {
		font-size: 1.6rem;
		font-weight: 650;
		letter-spacing: -0.02em;
		font-variant-numeric: tabular-nums;
	}
	.kv small {
		font-size: 0.85rem;
		font-weight: 500;
		color: var(--text-muted);
	}
	.delta {
		font-size: 0.8rem;
		color: var(--text-muted);
	}
	.cols {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(360px, 1fr));
		gap: 16px;
		margin-top: 16px;
	}
	.cols > .card {
		margin: 0;
	}
	section.card:not(.cols > .card) {
		margin-top: 16px;
	}
	table {
		width: 100%;
		border-collapse: collapse;
		font-size: 0.9rem;
	}
	th {
		text-align: left;
		font-size: 0.8rem;
		color: var(--text-muted);
		font-weight: 600;
		padding: 6px 4px;
	}
	td {
		padding: 8px 4px;
		border-top: 1px solid var(--border);
	}
	tfoot td {
		font-weight: 650;
	}
	.r {
		text-align: right;
		font-variant-numeric: tabular-nums;
	}
	.sw {
		display: inline-block;
		width: 10px;
		height: 10px;
		border-radius: 3px;
		margin-right: 8px;
		vertical-align: middle;
	}
	.devs {
		list-style: none;
		margin: 0;
		padding: 0;
		display: flex;
		flex-direction: column;
		gap: 10px;
	}
	.devs li {
		display: grid;
		grid-template-columns: minmax(120px, 200px) minmax(0, 1fr) auto;
		align-items: center;
		gap: 12px;
		font-size: 0.9rem;
	}
	.dn {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.track {
		height: 10px;
		border-radius: 999px;
		background: var(--surface-2);
		overflow: hidden;
	}
	.fill {
		display: block;
		height: 100%;
		border-radius: 999px;
	}
	.rest .dn {
		color: var(--text-muted);
	}
	.dv {
		font-variant-numeric: tabular-nums;
		color: var(--text-muted);
		min-width: 9ch;
		text-align: right;
	}
	.muted,
	.note {
		color: var(--text-muted);
		font-size: 0.85rem;
	}
	.note {
		margin-top: 12px;
	}
</style>

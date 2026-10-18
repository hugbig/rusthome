<script lang="ts" module>
	export interface Series {
		name: string;
		/** Couleur (jeton CSS) ; neutre pour le reste. */
		color: string;
		values: number[];
	}
</script>

<script lang="ts">
	// Barres empilées par créneau (consommation des appareils dans le temps).
	// Couleurs dans l'ordre fixe de la palette validée, attachées à la série et
	// non à son rang ; « non suivi » en gris neutre. Légende toujours présente,
	// 2px de fond entre segments, haut arrondi, infobulle détaillée.
	import { niceDomain, ticks } from '../history';

	interface Props {
		slots: number[];
		series: Series[];
		/** Série de comparaison, dessinée en barre fantôme derrière (non empilée). */
		ghost?: Series;
		unit?: string;
		height?: number;
		label: string;
		tick: (t: number) => string;
		tip: (t: number) => string;
	}
	let { slots, series, ghost, unit = '', height = 240, label, tick, tip }: Props = $props();

	let width = $state(600);
	const pad = { top: 12, right: 8, bottom: 24, left: 44 };
	const totals = $derived(
		slots.map((_, i) => series.reduce((a, s) => a + Math.max(0, s.values[i] ?? 0), 0))
	);
	const domain = $derived(niceDomain([0, ...totals, ...(ghost?.values ?? [])]));
	const yTicks = $derived(ticks(domain));
	const innerW = $derived(Math.max(10, width - pad.left - pad.right));
	const innerH = $derived(height - pad.top - pad.bottom);
	const slot = $derived(innerW / Math.max(1, slots.length));
	const barW = $derived(Math.max(2, Math.min(slot - 4, 40)));
	const y = (v: number) => pad.top + (1 - (v - domain[0]) / (domain[1] - domain[0])) * innerH;
	const every = $derived(Math.max(1, Math.ceil(slots.length / Math.max(1, innerW / 48))));
	const numFmt = new Intl.NumberFormat('fr-FR', { maximumFractionDigits: 2 });
	let hover = $state<number | null>(null);

	/** Segments d'un créneau, de bas en haut, avec 2px d'écart. */
	function stack(i: number) {
		let base = 0;
		const out: { color: string; y0: number; y1: number; top: boolean }[] = [];
		const visible = series.filter((s) => (s.values[i] ?? 0) > 0);
		visible.forEach((s, k) => {
			const v = s.values[i] ?? 0;
			const y0 = y(base);
			const y1 = y(base + v);
			base += v;
			out.push({
				color: s.color,
				y0,
				y1: Math.min(y0 - 1, y1 + (k < visible.length - 1 ? 1 : 0)),
				top: k === visible.length - 1
			});
		});
		return out;
	}
	function rect(x: number, w: number, y0: number, y1: number, top: boolean): string {
		const h = y0 - y1;
		const r = top ? Math.min(4, w / 2, h) : 0;
		return `M${x},${y0}V${y1 + r}Q${x},${y1} ${x + r},${y1}H${x + w - r}Q${x + w},${y1} ${x + w},${y1 + r}V${y0}Z`;
	}
</script>

<div class="chart" bind:clientWidth={width}>
	<ul class="legend" aria-label="Légende">
		{#each series as s (s.name)}
			<li><span class="sw" style:background={s.color}></span>{s.name}</li>
		{/each}
		{#if ghost}<li>
				<span class="sw ghostsw" style:background={ghost.color}></span>{ghost.name}
			</li>{/if}
	</ul>
	{#if totals.every((t) => t === 0) && !(ghost?.values ?? []).some((v) => v > 0)}
		<p class="empty">Aucune consommation sur la période.</p>
	{:else}
		<svg {width} {height} role="img" aria-label={label} onpointerleave={() => (hover = null)}>
			{#each yTicks as v (v)}
				<line class="grid" x1={pad.left} x2={pad.left + innerW} y1={y(v)} y2={y(v)} />
				<text class="axis" x={pad.left - 8} y={y(v)} text-anchor="end" dominant-baseline="middle"
					>{numFmt.format(v)}</text
				>
			{/each}
			{#each slots as t, i (t)}
				{@const x = pad.left + i * slot + (slot - barW) / 2}
				{#if ghost && (ghost.values[i] ?? 0) > 0}
					<path
						class="ghost"
						d={rect(x - 3, barW + 6, y(0), y(ghost.values[i]!), true)}
						fill={ghost.color}
					/>
				{/if}
				<g class:dim={hover !== null && hover !== i}>
					{#each stack(i) as seg, k (k)}
						<path d={rect(x, barW, seg.y0, seg.y1, seg.top)} fill={seg.color} />
					{/each}
				</g>
				{#if i % every === 0}
					<text class="axis" x={pad.left + i * slot + slot / 2} y={height - 6} text-anchor="middle"
						>{tick(t)}</text
					>
				{/if}
				<rect
					class="hit"
					x={pad.left + i * slot}
					y={pad.top}
					width={slot}
					height={innerH}
					role="presentation"
					onpointerenter={() => (hover = i)}
				/>
			{/each}
		</svg>
		{#if hover !== null}
			{@const i = hover}
			<div
				class="tooltip"
				style:left="{Math.min(pad.left + i * slot + slot, width - 210)}px"
				style:top="{pad.top + 24}px"
			>
				<strong>{tip(slots[i]!)}</strong>
				{#each series as s (s.name)}
					{#if (s.values[i] ?? 0) > 0}
						<span class="row"
							><span class="sw" style:background={s.color}></span>{s.name}
							<b>{numFmt.format(s.values[i]!)} {unit}</b></span
						>
					{/if}
				{/each}
				<span class="row total">Total <b>{numFmt.format(totals[i]!)} {unit}</b></span>
				{#if ghost}
					<span class="row"
						><span class="sw" style:background={ghost.color}></span>{ghost.name}
						<b>{numFmt.format(ghost.values[i] ?? 0)} {unit}</b></span
					>
				{/if}
			</div>
		{/if}
	{/if}
</div>

<style>
	.chart {
		position: relative;
		width: 100%;
	}
	svg {
		display: block;
	}
	.grid {
		stroke: var(--border);
	}
	.axis {
		fill: var(--text-muted);
		font-size: 11px;
		font-variant-numeric: tabular-nums;
	}
	g {
		transition: opacity 0.1s;
	}
	g.dim {
		opacity: 0.45;
	}
	.hit {
		fill: transparent;
	}
	.ghost {
		opacity: 0.45;
	}
	.ghostsw {
		opacity: 0.6;
	}
	.legend {
		display: flex;
		flex-wrap: wrap;
		gap: 6px 16px;
		margin: 0 0 10px;
		padding: 0;
		list-style: none;
		font-size: 0.85rem;
		color: var(--text-muted);
	}
	.legend li {
		display: flex;
		align-items: center;
		gap: 6px;
	}
	.sw {
		display: inline-block;
		width: 10px;
		height: 10px;
		border-radius: 3px;
		flex: none;
	}
	.tooltip {
		position: absolute;
		pointer-events: none;
		z-index: 2;
		display: flex;
		flex-direction: column;
		gap: 3px;
		min-width: 180px;
		padding: 8px 12px;
		border-radius: 10px;
		background: var(--surface);
		border: 1px solid var(--border);
		box-shadow: var(--shadow-pop);
		font-size: 0.82rem;
	}
	.row {
		display: flex;
		align-items: center;
		gap: 6px;
		color: var(--text-muted);
	}
	.row b {
		margin-left: auto;
		color: var(--text);
		font-weight: 600;
		font-variant-numeric: tabular-nums;
	}
	.total {
		border-top: 1px solid var(--border);
		padding-top: 3px;
	}
	.empty {
		color: var(--text-muted);
	}
</style>

<script lang="ts">
	// Barres journalières d'une série unique (pas de légende : le titre la nomme).
	// Specs dataviz : haut arrondi 4px ancré à la ligne de base, 2px d'écart
	// entre barres, grille hairline, infobulle au survol de chaque barre.
	import type { Bar } from '../dashboard/data';
	import { niceDomain, ticks } from '../history';

	interface Props {
		bars: Bar[];
		/** Libellé de l'axe (défaut : « 17 oct. »). */
		tick?: (t: number) => string;
		/** Libellé de l'infobulle (défaut : date complète). */
		tip?: (t: number) => string;
		unit?: string;
		height?: number;
		label: string;
	}
	let { bars, unit = '', height = 200, label, tick, tip }: Props = $props();

	let width = $state(600);
	const pad = { top: 12, right: 8, bottom: 24, left: 44 };
	const domain = $derived(niceDomain([0, ...bars.map((b) => b.value)]));
	const yTicks = $derived(ticks(domain));
	const innerW = $derived(Math.max(10, width - pad.left - pad.right));
	const innerH = $derived(height - pad.top - pad.bottom);
	const slot = $derived(innerW / Math.max(1, bars.length));
	const barW = $derived(Math.max(1, slot - 2));
	const y = (v: number) => pad.top + (1 - (v - domain[0]) / (domain[1] - domain[0])) * innerH;
	const base = $derived(y(Math.max(0, domain[0])));

	/** Barre au sommet arrondi (4px), pied droit sur la ligne de base. */
	function barPath(i: number, v: number): string {
		const x0 = pad.left + i * slot + 1;
		const top = Math.min(y(v), base);
		const h = base - top;
		const r = Math.min(4, barW / 2, h);
		return `M${x0},${base}V${top + r}Q${x0},${top} ${x0 + r},${top}H${x0 + barW - r}Q${x0 + barW},${top} ${x0 + barW},${top + r}V${base}Z`;
	}

	const numFmt = new Intl.NumberFormat('fr-FR', { maximumFractionDigits: 2 });
	const dayFmt = new Intl.DateTimeFormat('fr-FR', { day: 'numeric', month: 'short' });
	const fullFmt = new Intl.DateTimeFormat('fr-FR', { dateStyle: 'full' });
	const every = $derived(Math.max(1, Math.ceil(bars.length / Math.max(1, innerW / 56))));
	let hover = $state<number | null>(null);
</script>

<div class="chart" bind:clientWidth={width}>
	{#if bars.length === 0}
		<p class="empty">Pas encore de valeur sur la période.</p>
	{:else}
		<svg {width} {height} role="img" aria-label={label} onpointerleave={() => (hover = null)}>
			{#each yTicks as v (v)}
				<line class="grid" x1={pad.left} x2={pad.left + innerW} y1={y(v)} y2={y(v)} />
				<text class="axis" x={pad.left - 8} y={y(v)} text-anchor="end" dominant-baseline="middle"
					>{numFmt.format(v)}</text
				>
			{/each}
			{#each bars as b, i (b.day)}
				<path class="bar" class:dim={hover !== null && hover !== i} d={barPath(i, b.value)} />
				{#if i % every === 0}
					<text class="axis" x={pad.left + i * slot + slot / 2} y={height - 6} text-anchor="middle"
						>{tick ? tick(b.day) : dayFmt.format(b.day)}</text
					>
				{/if}
				<!-- zone de survol plus large que la barre -->
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
		{#if hover !== null && bars[hover]}
			{@const b = bars[hover]}
			<div
				class="tooltip"
				style:left="{Math.min(pad.left + hover * slot + slot, width - 170)}px"
				style:top="{pad.top}px"
			>
				<strong>{numFmt.format(b.value)}{unit ? ` ${unit}` : ''}</strong>
				<span>{tip ? tip(b.day) : fullFmt.format(b.day)}</span>
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
		stroke-width: 1;
	}
	.axis {
		fill: var(--text-muted);
		font-size: 11px;
		font-variant-numeric: tabular-nums;
	}
	.bar {
		fill: var(--series-1);
		transition: opacity 0.1s;
	}
	.bar.dim {
		opacity: 0.45;
	}
	.hit {
		fill: transparent;
	}
	.tooltip {
		position: absolute;
		pointer-events: none;
		display: flex;
		flex-direction: column;
		gap: 2px;
		padding: 6px 10px;
		border-radius: 8px;
		background: var(--surface);
		border: 1px solid var(--border);
		box-shadow: 0 4px 12px rgb(0 0 0 / 0.15);
		font-size: 0.85rem;
		white-space: nowrap;
	}
	.tooltip span {
		color: var(--text-muted);
	}
	.empty {
		color: var(--text-muted);
	}
</style>

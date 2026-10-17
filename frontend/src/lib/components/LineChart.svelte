<script lang="ts">
	// Courbe d'une série unique (pas de légende : le titre de la section la nomme).
	// Specs dataviz : trait 2px, lavis 10 %, point final r=4 + anneau surface 2px,
	// grille hairline, réticule + infobulle qui s'aimante au point le plus proche.
	import { niceDomain, ticks, type Point } from '../history';

	interface Props {
		points: (Point | null)[];
		from: number;
		to: number;
		unit?: string;
		height?: number;
		label: string;
	}
	let { points, from, to, unit = '', height = 220, label }: Props = $props();

	let width = $state(600);
	const pad = { top: 12, right: 16, bottom: 28, left: 48 };

	const valid = $derived(points.filter((p): p is Point => p !== null));
	const domain = $derived(niceDomain(valid.map((p) => p.v)));
	const yTicks = $derived(ticks(domain));
	const innerW = $derived(Math.max(10, width - pad.left - pad.right));
	const innerH = $derived(height - pad.top - pad.bottom);

	const x = (t: number) => pad.left + ((t - from) / Math.max(1, to - from)) * innerW;
	const y = (v: number) => pad.top + (1 - (v - domain[0]) / (domain[1] - domain[0])) * innerH;

	/** Segments continus ; le dernier point est prolongé jusqu'à maintenant. */
	const runs = $derived.by(() => {
		const out: Point[][] = [];
		let cur: Point[] = [];
		for (const p of points) {
			if (p === null) {
				if (cur.length) out.push(cur);
				cur = [];
			} else cur.push({ t: Math.max(p.t, from), v: p.v });
		}
		if (cur.length) {
			const last = cur.at(-1)!;
			cur.push({ t: to, v: last.v });
			out.push(cur);
		}
		return out;
	});

	const pathOf = (run: Point[]) =>
		run.map((p, i) => `${i ? 'L' : 'M'}${x(p.t)},${y(p.v)}`).join('');
	const areaOf = (run: Point[]) =>
		`${pathOf(run)}L${x(run.at(-1)!.t)},${pad.top + innerH}L${x(run[0]!.t)},${pad.top + innerH}Z`;

	const span = $derived(to - from);
	const timeFmt = $derived(
		new Intl.DateTimeFormat(
			'fr-FR',
			span > 2 * 86400_000
				? { weekday: 'short', day: 'numeric' }
				: { hour: '2-digit', minute: '2-digit' }
		)
	);
	const xTicks = $derived(Array.from({ length: 5 }, (_, i) => from + (i * span) / 4));
	const fullFmt = new Intl.DateTimeFormat('fr-FR', { dateStyle: 'short', timeStyle: 'short' });
	const numFmt = new Intl.NumberFormat('fr-FR', { maximumFractionDigits: 2 });

	let hover = $state<Point | null>(null);

	function onMove(ev: PointerEvent) {
		const rect = (ev.currentTarget as SVGElement).getBoundingClientRect();
		const t = from + ((ev.clientX - rect.left - pad.left) / innerW) * span;
		// Valeur en vigueur à l'instant t = dernier changement avant t.
		let best: Point | null = null;
		for (const p of valid) if (p.t <= t) best = p;
		hover = best ? { t: Math.min(Math.max(t, from), to), v: best.v } : null;
	}
</script>

<div class="chart" bind:clientWidth={width}>
	{#if valid.length === 0}
		<p class="empty">Pas encore de valeur numérique sur la période.</p>
	{:else}
		<svg
			{width}
			{height}
			role="img"
			aria-label={label}
			onpointermove={onMove}
			onpointerleave={() => (hover = null)}
		>
			{#each yTicks as v (v)}
				<line class="grid" x1={pad.left} x2={pad.left + innerW} y1={y(v)} y2={y(v)} />
				<text class="axis" x={pad.left - 8} y={y(v)} text-anchor="end" dominant-baseline="middle"
					>{numFmt.format(v)}</text
				>
			{/each}
			{#each xTicks as t, i (i)}
				<text
					class="axis"
					x={x(t)}
					y={height - 8}
					text-anchor={i === 0 ? 'start' : i === 4 ? 'end' : 'middle'}>{timeFmt.format(t)}</text
				>
			{/each}
			{#each runs as run, i (i)}
				<path class="area" d={areaOf(run)} />
				<path class="line" d={pathOf(run)} />
			{/each}
			{#if runs.length}
				{@const last = runs.at(-1)!.at(-1)!}
				<circle class="end" cx={x(last.t)} cy={y(last.v)} r="4" />
			{/if}
			{#if hover}
				<line
					class="crosshair"
					x1={x(hover.t)}
					x2={x(hover.t)}
					y1={pad.top}
					y2={pad.top + innerH}
				/>
				<circle class="end" cx={x(hover.t)} cy={y(hover.v)} r="4" />
			{/if}
		</svg>
		{#if hover}
			<div
				class="tooltip"
				style:left="{Math.min(x(hover.t) + 12, width - 150)}px"
				style:top="{pad.top}px"
			>
				<strong>{numFmt.format(hover.v)}{unit ? ` ${unit}` : ''}</strong>
				<span>{fullFmt.format(hover.t)}</span>
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
		touch-action: none;
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
	.line {
		fill: none;
		stroke: var(--series-1);
		stroke-width: 2;
		stroke-linejoin: round;
		stroke-linecap: round;
	}
	.area {
		fill: var(--series-1);
		opacity: 0.1;
	}
	.end {
		fill: var(--series-1);
		stroke: var(--surface);
		stroke-width: 2;
	}
	.crosshair {
		stroke: var(--text-muted);
		stroke-width: 1;
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

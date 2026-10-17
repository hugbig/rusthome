<script lang="ts">
	// Frise des états discrets. Identité jamais portée par la couleur seule :
	// légende étiquetée (état + durée) + infobulle par segment ; « indisponible »
	// en hachures neutres. Écart de 2px couleur surface entre segments.
	import { stateColorIndex, type Segment } from '../history';

	interface Props {
		segments: Segment[];
		from: number;
		to: number;
		label: string;
	}
	let { segments, from, to, label }: Props = $props();

	let width = $state(600);
	const height = 32;
	const colors = $derived(stateColorIndex(segments.map((s) => s.state)));
	const x = (t: number) => ((t - from) / Math.max(1, to - from)) * width;

	function fill(state: string): string {
		const c = colors.get(state);
		if (c === 'muted') return 'url(#hatch)';
		if (c === 'neutral' || c === undefined) return 'var(--neutral-fill)';
		return `var(--series-${c + 1})`;
	}

	const totals = $derived.by(() => {
		const m: Record<string, number> = {};
		for (const s of segments) m[s.state] = (m[s.state] ?? 0) + (s.to - s.from);
		return Object.entries(m);
	});

	function duration(ms: number): string {
		const min = Math.round(ms / 60000);
		if (min < 60) return `${min} min`;
		const h = Math.floor(min / 60);
		return h < 48 ? `${h} h ${String(min % 60).padStart(2, '0')}` : `${Math.round(h / 24)} j`;
	}

	const fmt = new Intl.DateTimeFormat('fr-FR', { dateStyle: 'short', timeStyle: 'short' });
	let hover = $state<{ seg: Segment; left: number } | null>(null);
</script>

<div class="timeline" bind:clientWidth={width}>
	{#if segments.length === 0}
		<p class="empty">Aucun changement enregistré sur la période.</p>
	{:else}
		<svg {width} {height} role="img" aria-label={label} onpointerleave={() => (hover = null)}>
			<defs>
				<pattern
					id="hatch"
					width="6"
					height="6"
					patternUnits="userSpaceOnUse"
					patternTransform="rotate(45)"
				>
					<rect width="6" height="6" fill="var(--surface-2)" />
					<line x1="0" y1="0" x2="0" y2="6" stroke="var(--text-muted)" stroke-width="1.5" />
				</pattern>
			</defs>
			{#each segments as seg (seg.from)}
				{@const x0 = x(seg.from)}
				{@const w = Math.max(1, x(seg.to) - x0 - 2)}
				<rect
					role="presentation"
					x={x0}
					y="0"
					width={w}
					{height}
					rx="4"
					fill={fill(seg.state)}
					class:hovered={hover?.seg === seg}
					onpointerenter={() => (hover = { seg, left: Math.min(x0 + w / 2, width - 160) })}
				/>
			{/each}
		</svg>
		{#if hover}
			<div class="tooltip" style:left="{Math.max(0, hover.left)}px">
				<strong>{hover.seg.state}</strong>
				<span>{fmt.format(hover.seg.from)} → {fmt.format(hover.seg.to)}</span>
			</div>
		{/if}
		<ul class="legend">
			{#each totals as [state, ms] (state)}
				<li>
					<svg width="12" height="12" aria-hidden="true"
						><rect width="12" height="12" rx="3" fill={fill(state)} /></svg
					>
					<span>{state}</span>
					<small>{duration(ms)}</small>
				</li>
			{/each}
		</ul>
	{/if}
</div>

<style>
	.timeline {
		position: relative;
		width: 100%;
	}
	svg {
		display: block;
	}
	rect.hovered {
		opacity: 0.8;
	}
	.legend {
		display: flex;
		flex-wrap: wrap;
		gap: 4px 16px;
		margin: 8px 0 0;
		padding: 0;
		list-style: none;
		font-size: 0.85rem;
	}
	.legend li {
		display: flex;
		align-items: center;
		gap: 6px;
	}
	.legend small {
		color: var(--text-muted);
	}
	.tooltip {
		position: absolute;
		top: 38px;
		pointer-events: none;
		display: flex;
		flex-direction: column;
		padding: 6px 10px;
		border-radius: 8px;
		background: var(--surface);
		border: 1px solid var(--border);
		box-shadow: 0 4px 12px rgb(0 0 0 / 0.15);
		font-size: 0.85rem;
		white-space: nowrap;
		z-index: 1;
	}
	.tooltip span {
		color: var(--text-muted);
	}
	.empty {
		color: var(--text-muted);
	}
</style>

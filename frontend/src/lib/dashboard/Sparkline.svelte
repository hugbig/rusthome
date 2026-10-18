<script lang="ts">
	// Mini-courbe d'une mesure sur 24 h (tuiles de capteurs) : trait 2px et
	// lavis dégradé, sans axes ; le titre de la tuile porte la valeur.
	import { cachedHistory } from './history-cache';

	interface Props {
		entityId: string;
		hours?: number;
		color?: string;
		height?: number;
		/** Valeur courante, ajoutée en bout de courbe. */
		current?: number;
	}
	let { entityId, hours = 24, color = 'var(--series-1)', height = 40, current }: Props = $props();

	let points = $state<{ t: number; v: number }[]>([]);
	const gid = `sg-${Math.random().toString(36).slice(2, 8)}`;

	$effect(() => {
		void cachedHistory(entityId, hours).then((h) => {
			points = h
				.map((s) => ({ t: Date.parse(s.last_changed), v: Number(s.state) }))
				.filter((p) => Number.isFinite(p.v));
		});
	});

	const W = 200;
	const path = $derived.by(() => {
		const now = Date.now();
		const from = now - hours * 3600_000;
		const pts = [...points.filter((p) => p.t >= from)];
		if (current !== undefined && Number.isFinite(current)) pts.push({ t: now, v: current });
		if (pts.length < 2) return null;
		const vs = pts.map((p) => p.v);
		let lo = Math.min(...vs);
		let hi = Math.max(...vs);
		if (hi - lo < 1e-9) {
			lo -= 1;
			hi += 1;
		}
		const pad = (hi - lo) * 0.15;
		lo -= pad;
		hi += pad;
		const x = (t: number) => ((Math.max(t, from) - from) / (now - from)) * W;
		const y = (v: number) => height - ((v - lo) / (hi - lo)) * height;
		// marches : une mesure garde sa valeur jusqu'au changement suivant
		let d = `M${x(pts[0]!.t).toFixed(1)},${y(pts[0]!.v).toFixed(1)}`;
		for (let i = 1; i < pts.length; i++) {
			d += `L${x(pts[i]!.t).toFixed(1)},${y(pts[i - 1]!.v).toFixed(1)}`;
			d += `L${x(pts[i]!.t).toFixed(1)},${y(pts[i]!.v).toFixed(1)}`;
		}
		return { line: d, area: `${d}L${W},${height}L${x(pts[0]!.t).toFixed(1)},${height}Z` };
	});
</script>

{#if path}
	<svg
		class="spark"
		viewBox="0 0 {W} {height}"
		preserveAspectRatio="none"
		style:height="{height}px"
		aria-hidden="true"
	>
		<defs>
			<linearGradient id={gid} x1="0" y1="0" x2="0" y2="1">
				<stop offset="0%" stop-color={color} stop-opacity="0.28" />
				<stop offset="100%" stop-color={color} stop-opacity="0" />
			</linearGradient>
		</defs>
		<path d={path.area} fill="url(#{gid})" />
		<path
			d={path.line}
			fill="none"
			stroke={color}
			stroke-width="2"
			vector-effect="non-scaling-stroke"
			stroke-linejoin="round"
		/>
	</svg>
{/if}

<style>
	.spark {
		display: block;
		width: 100%;
	}
</style>

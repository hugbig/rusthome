<script lang="ts">
	// Répartition de l'énergie (carte « energy distribution » de HA) : réseau →
	// maison, retour au réseau s'il y en a, et la part des appareils suivis.
	import { mdiHome, mdiPowerPlug, mdiTransmissionTower } from '@mdi/js';
	import Icon from '../dashboard/Icon.svelte';

	interface Props {
		fromGrid: number;
		toGrid?: number;
		tracked: number;
	}
	let { fromGrid, toGrid = 0, tracked }: Props = $props();
	const fmt = new Intl.NumberFormat('fr-FR', { maximumFractionDigits: 1 });
	const home = $derived(Math.max(0, fromGrid - toGrid));
	const share = $derived(home > 0 ? Math.min(100, Math.round((tracked / home) * 100)) : 0);
</script>

<div
	class="flow"
	role="img"
	aria-label="Réseau {fmt.format(fromGrid)} kWh vers la maison {fmt.format(home)} kWh"
>
	<div class="node grid">
		<span class="ring"><Icon path={mdiTransmissionTower} size={26} /></span>
		<span class="v">{fmt.format(fromGrid)} kWh</span>
		{#if toGrid > 0}<span class="sub">← {fmt.format(toGrid)} kWh</span>{/if}
		<span class="n">Réseau</span>
	</div>

	<svg class="link" viewBox="0 0 100 10" preserveAspectRatio="none" aria-hidden="true">
		<line x1="0" y1="5" x2="100" y2="5" class="line" />
		{#if fromGrid > 0}
			<circle r="2.2" cy="5" class="dot">
				<animate attributeName="cx" from="0" to="100" dur="2.4s" repeatCount="indefinite" />
			</circle>
			<circle r="2.2" cy="5" class="dot">
				<animate
					attributeName="cx"
					from="0"
					to="100"
					dur="2.4s"
					begin="1.2s"
					repeatCount="indefinite"
				/>
			</circle>
		{/if}
	</svg>

	<div class="node home">
		<span class="ring"><Icon path={mdiHome} size={30} /></span>
		<span class="v">{fmt.format(home)} kWh</span>
		<span class="n">Maison</span>
	</div>

	<svg class="link" viewBox="0 0 100 10" preserveAspectRatio="none" aria-hidden="true">
		<line x1="0" y1="5" x2="100" y2="5" class="line dev" />
	</svg>

	<div class="node dev">
		<span class="ring"><Icon path={mdiPowerPlug} size={26} /></span>
		<span class="v">{fmt.format(tracked)} kWh</span>
		<span class="sub">{share} % suivis</span>
		<span class="n">Appareils</span>
	</div>
</div>

<style>
	.flow {
		display: flex;
		align-items: center;
		justify-content: center;
		gap: 0;
		padding: 12px 0 4px;
	}
	.node {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 2px;
		min-width: 96px;
		text-align: center;
	}
	.ring {
		display: grid;
		place-items: center;
		width: 72px;
		height: 72px;
		border-radius: 50%;
		border: 3px solid var(--c);
		color: var(--c);
		background: color-mix(in srgb, var(--c) 10%, transparent);
	}
	.home .ring {
		width: 88px;
		height: 88px;
	}
	.grid {
		--c: var(--series-1);
	}
	.home {
		--c: var(--text-muted);
	}
	.dev {
		--c: var(--series-4);
	}
	.v {
		margin-top: 6px;
		font-weight: 650;
		font-variant-numeric: tabular-nums;
	}
	.sub {
		font-size: 0.78rem;
		color: var(--text-muted);
	}
	.n {
		font-size: 0.82rem;
		color: var(--text-muted);
	}
	.link {
		flex: 1 1 80px;
		max-width: 160px;
		height: 10px;
		margin-bottom: 52px;
	}
	.line {
		stroke: var(--series-1);
		stroke-width: 2;
		vector-effect: non-scaling-stroke;
		opacity: 0.6;
	}
	.line.dev {
		stroke: var(--series-4);
		stroke-dasharray: 3 3;
	}
	.dot {
		fill: var(--series-1);
	}
	@media (prefers-reduced-motion: reduce) {
		.dot {
			display: none;
		}
	}
</style>

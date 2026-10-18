<script lang="ts">
	// Jauge demi-cercle : piste neutre, arc de valeur (couleur de statut si des
	// seuils sont définis, avec le libellé du niveau : jamais la couleur seule).
	import { live } from '../../stores/live.svelte';
	import {
		cardName,
		formatState,
		fraction,
		severityOf,
		severityZones,
		tapAction,
		type Card
	} from '../model';
	import { run } from '../runtime.svelte';

	let { card }: { card: Card } = $props();

	const s = $derived(card.entity ? live.states.get(card.entity) : undefined);
	const min = $derived(typeof card.min === 'number' ? card.min : 0);
	const max = $derived(typeof card.max === 'number' ? card.max : 100);
	const value = $derived(s ? Number(s.state) : NaN);
	const ok = $derived(Number.isFinite(value));
	const f = $derived(ok ? fraction(value, min, max) : 0);
	const sev = $derived(
		ok ? severityOf(value, card.severity as Record<string, number> | undefined) : null
	);
	const SEV_LABEL = { good: 'normal', warning: 'attention', critical: 'alerte' };

	const zones = $derived(
		severityZones(card.severity as Record<string, number> | undefined, min, max)
	);
	const R = 46;
	const CX = 60;
	const CY = 60;
	/** Point de l'arc à la fraction t (0 = gauche, 1 = droite). */
	const at = (t: number, r = R) => {
		const a = Math.PI * (1 - t);
		return [CX + r * Math.cos(a), CY - r * Math.sin(a)] as const;
	};
	/** Arc de la fraction a à la fraction b (rayon r). */
	const arc = (a: number, b: number, r = R) => {
		const [x0, y0] = at(a, r);
		const [x1, y1] = at(b, r);
		return `M${x0.toFixed(2)},${y0.toFixed(2)} A${r},${r} 0 0 1 ${x1.toFixed(2)},${y1.toFixed(2)}`;
	};
	/** Rayon de l'anneau intérieur qui montre les zones de seuils. */
	const ZR = R - 11;
	const needle = $derived.by(() => {
		const [tx, ty] = at(f, R + 4);
		const a = Math.PI * (1 - f) + Math.PI / 2;
		const w = 3.2;
		const [bx, by] = [CX + w * Math.cos(a), CY - w * Math.sin(a)];
		const [cx2, cy2] = [CX - w * Math.cos(a), CY + w * Math.sin(a)];
		return `M${bx.toFixed(2)},${by.toFixed(2)} L${tx.toFixed(2)},${ty.toFixed(2)} L${cx2.toFixed(2)},${cy2.toFixed(2)} Z`;
	});
	const display = $derived.by(() => {
		if (!s) return 'Introuvable';
		const unit = typeof card.unit === 'string' ? card.unit : undefined;
		const decimals = typeof card.decimals === 'number' ? card.decimals : undefined;
		if (!unit) return formatState(s, decimals);
		return `${formatState({ ...s, attributes: { ...s.attributes, unit_of_measurement: undefined } }, decimals)} ${unit}`;
	});
</script>

<button class="dcard gauge" onclick={() => run(tapAction(card), card.entity)}>
	<svg viewBox="0 0 120 70" role="img" aria-label="{cardName(card, s)} : {display}">
		<path d={arc(0, 1)} class="track" />
		{#if card.needle === true}
			{#each zones as z, i (i)}
				<path d={arc(z.from, z.to)} class="zone {z.level}" stroke-linecap="butt" />
			{/each}
			{#if ok}
				<path d={needle} class="needle" />
				<circle cx={CX} cy={CY} r="5" class="hub" />
			{/if}
		{:else}
			{#each zones as z, i (i)}
				<path d={arc(z.from, z.to, ZR)} class="zone ring {z.level}" stroke-linecap="butt" />
			{/each}
			{#if ok && f > 0.001}
				<path d={arc(0, f)} class="value {sev ?? 'plain'}" />
			{/if}
		{/if}
	</svg>
	<span class="val" class:low={card.needle === true}>{display}</span>
	<span class="name">{cardName(card, s)}</span>
	{#if sev}<span class="sev {sev}">{SEV_LABEL[sev]}</span>{/if}
</button>

<style>
	.gauge {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 2px;
		width: 100%;
		padding: 14px 12px 12px;
		color: var(--text);
		font: inherit;
		cursor: pointer;
	}
	svg {
		width: 100%;
		max-width: 190px;
	}
	path {
		fill: none;
		stroke-width: 11;
		stroke-linecap: round;
	}
	.track {
		stroke: var(--surface-3);
	}
	.zone.good {
		stroke: var(--good);
	}
	.zone.warning {
		stroke: var(--warning);
	}
	.zone.critical {
		stroke: var(--critical);
	}
	.zone.ring {
		stroke-width: 3;
		opacity: 0.85;
	}
	.value.plain {
		stroke: var(--primary);
	}
	.value.good {
		stroke: var(--good);
	}
	.value.warning {
		stroke: var(--warning);
	}
	.value.critical {
		stroke: var(--critical);
	}
	.needle {
		fill: var(--text);
		stroke: none;
	}
	.hub {
		fill: var(--text);
		stroke: var(--surface);
		stroke-width: 2;
	}
	.val {
		margin-top: -18px;
		font-size: 1.35rem;
		font-weight: 650;
		letter-spacing: -0.02em;
		font-variant-numeric: tabular-nums;
	}
	.val.low {
		margin-top: 2px;
	}
	.name {
		font-size: 0.85rem;
		color: var(--text-muted);
		text-align: center;
		margin-top: 4px;
	}
	.sev {
		font-size: 0.72rem;
		color: var(--text-muted);
		display: inline-flex;
		align-items: center;
		gap: 4px;
	}
	.sev::before {
		content: '';
		width: 7px;
		height: 7px;
		border-radius: 50%;
		background: currentColor;
	}
	.sev.good::before {
		background: var(--good);
	}
	.sev.warning::before {
		background: var(--warning);
	}
	.sev.critical::before {
		background: var(--critical);
	}
</style>

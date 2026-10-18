<script lang="ts">
	// Tuile : une entité, son état et une commande rapide. Seule l'icône prend
	// la couleur des lumières (interrupteur et curseur restent neutres) ; les mesures (température, pression, puissance…) s'affichent
	// en grand avec leur courbe des dernières 24 h.
	import { domainOf } from '../../format';
	import { live } from '../../stores/live.svelte';
	import Icon from '../Icon.svelte';
	import { iconPath } from '../icons';
	import {
		cardName,
		entityColor,
		formatState,
		isActive,
		isMeasurement,
		isToggleable,
		tapAction,
		type Card
	} from '../model';
	import { call, run } from '../runtime.svelte';
	import ControlSlider from '../ControlSlider.svelte';
	import Sparkline from '../Sparkline.svelte';

	let { card }: { card: Card } = $props();

	const s = $derived(card.entity ? live.states.get(card.entity) : undefined);
	const active = $derived(isActive(s));
	const accent = $derived(entityColor(s));
	const measure = $derived(isMeasurement(s) && card.vertical !== true);
	const features = $derived(
		(Array.isArray(card.features) ? card.features : []) as { type?: string }[]
	);
	const options = $derived(
		Array.isArray(s?.attributes.options) ? (s.attributes.options as string[]) : []
	);
	const brightness = $derived(
		s?.state === 'on' && typeof s.attributes.brightness === 'number'
			? Math.round((s.attributes.brightness / 255) * 100)
			: undefined
	);
	const stateText = $derived(
		brightness !== undefined ? `${formatState(s)} · ${brightness} %` : formatState(s)
	);

	/** Couleur de courbe selon la grandeur (identité stable, cf. palette validée). */
	const CLASS_COLOR: Record<string, string> = {
		temperature: 'var(--series-2)',
		humidity: 'var(--series-1)',
		pressure: 'var(--series-3)',
		atmospheric_pressure: 'var(--series-3)',
		power: 'var(--series-4)',
		energy: 'var(--series-4)',
		apparent_power: 'var(--series-4)',
		illuminance: 'var(--series-5)'
	};
	const curveColor = $derived(
		CLASS_COLOR[String(s?.attributes.device_class ?? '')] ?? 'var(--series-1)'
	);
	const valueText = $derived.by(() => {
		if (!s) return { value: '—', unit: '' };
		const n = Number(s.state);
		const unit = String(s.attributes.unit_of_measurement ?? '');
		const digits = Math.abs(n) >= 100 ? 0 : 1;
		return {
			value: new Intl.NumberFormat('fr-FR', { maximumFractionDigits: digits }).format(n),
			unit
		};
	});

	function iconTap() {
		if (card.entity && isToggleable(card.entity)) void run({ action: 'toggle' }, card.entity);
		else void run(tapAction(card), card.entity);
	}
	function select(e: Event) {
		if (!card.entity) return;
		const option = (e.currentTarget as HTMLSelectElement).value;
		void call(`${domainOf(card.entity)}.select_option`, { entity_id: card.entity, option });
	}
	function setBrightness(v: number) {
		if (!card.entity) return;
		void call(v === 0 ? 'light.turn_off' : 'light.turn_on', {
			entity_id: card.entity,
			...(v === 0 ? {} : { brightness_pct: v })
		});
	}
</script>

<div
	class="dcard tile"
	class:active
	class:measure
	class:vertical={card.vertical === true}
	style:--accent={accent}
>
	{#if measure}
		<button class="mhead" onclick={() => run(tapAction(card), card.entity)}>
			<span class="mic" style:color={curveColor}
				><Icon
					path={iconPath(card.icon, card.entity, s?.attributes.device_class)}
					size={18}
				/></span
			>
			<span class="mname">{cardName(card, s)}</span>
		</button>
		<div class="mval">
			<span class="num">{valueText.value}</span>
			<span class="unit">{valueText.unit}</span>
		</div>
		{#if card.entity && card.graph !== false}
			<div class="spark">
				<Sparkline entityId={card.entity} color={curveColor} current={Number(s?.state)} />
			</div>
		{/if}
	{:else}
		<div class="main">
			<button
				class="dicon"
				class:on={active}
				onclick={iconTap}
				aria-label={card.entity && isToggleable(card.entity)
					? `Basculer ${cardName(card, s)}`
					: cardName(card, s)}
			>
				<Icon path={iconPath(card.icon, card.entity, s?.attributes.device_class)} size={20} />
			</button>
			<button class="body" onclick={() => run(tapAction(card), card.entity)}>
				<span class="name">{cardName(card, s)}</span>
				<span class="state">{stateText}</span>
			</button>
		</div>
	{/if}
	{#each features as f, i (i)}
		{#if f.type === 'toggle' && card.entity}
			<div class="feature">
				<button
					class="dswitch"
					role="switch"
					aria-checked={s?.state === 'on'}
					aria-label="Basculer {cardName(card, s)}"
					onclick={() => run({ action: 'toggle' }, card.entity)}
				></button>
			</div>
		{:else if f.type === 'light-brightness' && card.entity}
			<ControlSlider
				value={brightness ?? 0}
				label="Luminosité de {cardName(card, s)}"
				onchange={setBrightness}
			/>
		{:else if f.type === 'select-options' && options.length}
			<select class="feature" value={s?.state} onchange={select} aria-label={cardName(card, s)}>
				{#each options as o (o)}<option value={o}>{o}</option>{/each}
			</select>
		{/if}
	{/each}
</div>

<style>
	.tile {
		display: flex;
		flex-direction: column;
		gap: 8px;
		padding: 10px 12px;
		transition: border-color 0.2s;
	}
	.main {
		display: flex;
		align-items: center;
		gap: 12px;
		min-width: 0;
	}
	.vertical .main {
		flex-direction: column;
		text-align: center;
		gap: 8px;
		padding: 4px 0;
	}
	.body {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: 1px;
		min-width: 0;
		flex: 1;
		background: none;
		border: none;
		padding: 0;
		color: inherit;
		font: inherit;
		text-align: left;
		cursor: pointer;
	}
	.vertical .body {
		align-items: center;
		text-align: center;
	}
	.dicon {
		border: none;
		cursor: pointer;
	}
	.name {
		font-weight: 600;
		font-size: 0.9rem;
		max-width: 100%;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.state {
		color: var(--text-muted);
		font-size: 0.82rem;
	}
	/* Mesures */
	.measure {
		gap: 2px;
		padding: 12px 14px 0;
		overflow: hidden;
	}
	.mhead {
		display: flex;
		align-items: center;
		gap: 8px;
		min-width: 0;
		padding: 0;
		border: none;
		background: none;
		color: var(--text-muted);
		font: inherit;
		text-align: left;
		cursor: pointer;
	}
	.mic {
		display: grid;
	}
	.mname {
		font-size: 0.85rem;
		font-weight: 500;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.mval {
		display: flex;
		align-items: baseline;
		gap: 4px;
		padding-bottom: 2px;
	}
	.num {
		font-size: 1.6rem;
		font-weight: 650;
		letter-spacing: -0.02em;
		font-variant-numeric: tabular-nums;
	}
	.unit {
		color: var(--text-muted);
		font-size: 0.9rem;
	}
	.spark {
		margin: 2px -14px 0;
	}
	.feature {
		display: flex;
		justify-content: flex-end;
	}
	select.feature {
		padding: 6px 8px;
		border-radius: var(--radius-sm);
		border: 1px solid var(--border);
		background: var(--surface-2);
		color: var(--text);
		font: inherit;
	}
</style>

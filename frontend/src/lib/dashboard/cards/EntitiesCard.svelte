<script lang="ts">
	import { domainOf } from '../../format';
	import { live } from '../../stores/live.svelte';
	import Icon from '../Icon.svelte';
	import { iconPath } from '../icons';
	import {
		cardName,
		entityColor,
		formatState,
		isActive,
		isToggleable,
		tapAction,
		type Card
	} from '../model';
	import { call, run } from '../runtime.svelte';

	let { card }: { card: Card } = $props();

	const rows = $derived((Array.isArray(card.entities) ? card.entities : []) as Card[]);

	/** Valeur du curseur : luminosité en % pour une lumière, sinon l'état. */
	function sliderValue(row: Card): { value: number; min: number; max: number; step: number } {
		const s = row.entity ? live.states.get(row.entity) : undefined;
		if (!row.entity || !s) return { value: 0, min: 0, max: 100, step: 1 };
		if (domainOf(row.entity) === 'light') {
			const b = typeof s.attributes.brightness === 'number' ? s.attributes.brightness : 0;
			return {
				value: s.state === 'on' ? Math.round((b / 255) * 100) : 0,
				min: 0,
				max: 100,
				step: 1
			};
		}
		const num = (k: string, d: number) =>
			typeof s.attributes[k] === 'number' ? (s.attributes[k] as number) : d;
		return {
			value: Number(s.state) || 0,
			min: num('min', 0),
			max: num('max', 100),
			step: num('step', 1)
		};
	}

	function slide(row: Card, e: Event) {
		if (!row.entity) return;
		const v = Number((e.currentTarget as HTMLInputElement).value);
		const d = domainOf(row.entity);
		if (d === 'light')
			void call(v === 0 ? 'light.turn_off' : 'light.turn_on', {
				entity_id: row.entity,
				...(v === 0 ? {} : { brightness_pct: v })
			});
		else void call(`${d}.set_value`, { entity_id: row.entity, value: v });
	}

	function attribute(row: Card): string {
		const s = row.entity ? live.states.get(row.entity) : undefined;
		const v = typeof row.attribute === 'string' ? s?.attributes[row.attribute] : undefined;
		if (v === undefined || v === null) return '—';
		if (typeof v === 'boolean') return v ? 'Oui' : 'Non';
		return `${String(v)}${typeof row.suffix === 'string' ? ` ${row.suffix}` : ''}`;
	}
</script>

<div class="dcard">
	{#if typeof card.title === 'string' && card.title}<h3>{card.title}</h3>{/if}
	<ul>
		{#each rows as row, i (i)}
			{#if row.type === 'divider'}
				<li class="divider" role="separator"></li>
			{:else if row.type === 'unsupported'}
				<li class="muted">Ligne non supportée</li>
			{:else}
				{@const s = row.entity ? live.states.get(row.entity) : undefined}
				<li class:slider={row.slider === true}>
					<button class="who" onclick={() => run(tapAction(row), row.entity)}>
						<span class="dicon" class:on={isActive(s)} style:--accent={entityColor(s)}
							><Icon
								path={iconPath(row.icon, row.entity, s?.attributes.device_class)}
								size={20}
							/></span
						>
						<span class="name">{cardName(row, s)}</span>
					</button>
					{#if row.type === 'attribute'}
						<span class="value">{attribute(row)}</span>
					{:else if row.slider === true && s}
						{@const sv = sliderValue(row)}
						<input
							type="range"
							min={sv.min}
							max={sv.max}
							step={sv.step}
							value={sv.value}
							style:--val="{((sv.value - sv.min) / (sv.max - sv.min || 1)) * 100}%"
							onchange={(e) => slide(row, e)}
							aria-label="Niveau {cardName(row, s)}"
						/>
						{#if row.toggle === true && row.entity}
							<button
								class="dswitch"
								role="switch"
								aria-checked={s.state === 'on'}
								aria-label="Basculer {cardName(row, s)}"
								onclick={() => run({ action: 'toggle' }, row.entity)}
							></button>
						{/if}
					{:else if row.entity && isToggleable(row.entity) && s && ['on', 'off'].includes(s.state)}
						<button
							class="dswitch"
							role="switch"
							aria-checked={s.state === 'on'}
							aria-label="Basculer {cardName(row, s)}"
							onclick={() => run({ action: 'toggle' }, row.entity)}
						></button>
					{:else}
						<span class="value">{formatState(s)}</span>
					{/if}
				</li>
			{/if}
		{/each}
	</ul>
</div>

<style>
	ul {
		list-style: none;
		margin: 0;
		padding: 0;
		display: flex;
		flex-direction: column;
		gap: 4px;
	}
	li {
		display: flex;
		align-items: center;
		gap: 8px;
		min-height: 40px;
	}
	li.slider {
		flex-wrap: wrap;
		column-gap: 12px;
	}
	/* Le nom garde au moins un tiers de la ligne ; sinon le curseur passe dessous. */
	li.slider .who {
		flex: 1 0 34%;
	}
	.who {
		display: flex;
		align-items: center;
		gap: 10px;
		flex: 1;
		min-width: 0;
		background: none;
		border: none;
		padding: 0;
		color: inherit;
		font: inherit;
		text-align: left;
		cursor: pointer;
	}
	.name {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.value {
		color: var(--text-muted);
		font-size: 0.9rem;
		white-space: nowrap;
	}
	input[type='range'] {
		flex: 1 1 120px;
	}
	.divider {
		min-height: 0;
		border-top: 1px solid var(--border);
		margin: 4px 0;
	}
</style>

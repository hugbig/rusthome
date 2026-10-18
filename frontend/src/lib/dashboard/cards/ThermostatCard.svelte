<script lang="ts">
	import { live } from '../../stores/live.svelte';
	import Icon from '../Icon.svelte';
	import { UI } from '../icons';
	import { cardName, stateLabel, type Card } from '../model';
	import { call } from '../runtime.svelte';

	let { card }: { card: Card } = $props();

	const s = $derived(card.entity ? live.states.get(card.entity) : undefined);
	const num = (v: unknown) => (typeof v === 'number' ? v : undefined);
	const current = $derived(num(s?.attributes.current_temperature));
	const target = $derived(num(s?.attributes.temperature));
	const step = $derived(num(s?.attributes.target_temp_step) ?? 0.5);
	const fmt = new Intl.NumberFormat('fr-FR', {
		minimumFractionDigits: 1,
		maximumFractionDigits: 1
	});

	function set(delta: number) {
		if (!card.entity || target === undefined) return;
		void call('climate.set_temperature', { entity_id: card.entity, temperature: target + delta });
	}
</script>

<div class="dcard thermostat">
	<h3>{cardName(card, s)}</h3>
	<div class="temps">
		<div>
			<span class="label">Consigne</span>
			<span class="big">{target !== undefined ? `${fmt.format(target)} °C` : '—'}</span>
		</div>
		<div>
			<span class="label">Mesurée</span>
			<span class="mid">{current !== undefined ? `${fmt.format(current)} °C` : '—'}</span>
		</div>
	</div>
	<p class="muted">
		{s ? stateLabel(s.state) : 'Introuvable'}{#if typeof s?.attributes.hvac_action === 'string'}
			· {stateLabel(s.attributes.hvac_action)}{/if}
	</p>
	<div class="actions">
		<button
			class="dbtn"
			disabled={target === undefined}
			onclick={() => set(-step)}
			aria-label="Baisser la consigne"><Icon path={UI.minus} size={20} /></button
		>
		<button
			class="dbtn"
			disabled={target === undefined}
			onclick={() => set(step)}
			aria-label="Monter la consigne"><Icon path={UI.add} size={20} /></button
		>
	</div>
</div>

<style>
	.temps {
		display: flex;
		justify-content: space-around;
		text-align: center;
	}
	.temps div {
		display: flex;
		flex-direction: column;
	}
	.label {
		font-size: 0.8rem;
		color: var(--text-muted);
	}
	.big {
		font-size: 1.8rem;
		font-weight: 600;
		font-variant-numeric: tabular-nums;
	}
	.mid {
		font-size: 1.3rem;
		font-variant-numeric: tabular-nums;
	}
	.muted {
		text-align: center;
		margin: 8px 0;
	}
	.actions {
		display: flex;
		justify-content: center;
		gap: 16px;
	}
</style>

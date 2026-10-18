<script lang="ts">
	import { live } from '../../stores/live.svelte';
	import Icon from '../Icon.svelte';
	import { iconPath } from '../icons';
	import { cardName, isActive, tapAction, type Card } from '../model';
	import { run } from '../runtime.svelte';

	let { card }: { card: Card } = $props();

	const s = $derived(card.entity ? live.states.get(card.entity) : undefined);
	const name = $derived(cardName(card, s));
</script>

<button
	class="dcard button"
	class:active={isActive(s)}
	onclick={() => run(tapAction(card, 'toggle'), card.entity)}
	aria-label={name || card.icon || 'Bouton'}
>
	<Icon path={iconPath(card.icon, card.entity, s?.attributes.device_class)} size={32} />
	{#if name}<span>{name}</span>{/if}
</button>

<style>
	.button {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: 6px;
		width: 100%;
		min-height: 84px;
		color: var(--text);
		font: inherit;
		cursor: pointer;
	}
	.button.active {
		background: var(--active-bg);
		color: var(--primary-text);
	}
	span {
		font-size: 0.9rem;
		font-weight: 500;
	}
</style>

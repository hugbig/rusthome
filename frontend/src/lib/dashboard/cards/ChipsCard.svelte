<script lang="ts">
	import { live } from '../../stores/live.svelte';
	import Icon from '../Icon.svelte';
	import { iconPath } from '../icons';
	import { cardName, formatState, tapAction, type Card } from '../model';
	import { run } from '../runtime.svelte';

	let { card }: { card: Card } = $props();

	const chips = $derived((Array.isArray(card.chips) ? card.chips : []) as Card[]);
</script>

<div class="chips">
	{#each chips as chip, i (i)}
		{@const s = chip.entity ? live.states.get(chip.entity) : undefined}
		<button
			class="chip"
			title={cardName(chip, s)}
			onclick={() => run(tapAction(chip), chip.entity)}
		>
			<Icon path={iconPath(chip.icon, chip.entity, s?.attributes.device_class)} size={18} />
			<span>{formatState(s)}</span>
		</button>
	{/each}
</div>

<style>
	.chips {
		display: flex;
		flex-wrap: wrap;
		justify-content: center;
		gap: 6px;
	}
	.chip {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		padding: 6px 12px;
		border-radius: 999px;
		border: 1px solid var(--border);
		background: var(--surface);
		color: var(--text);
		font: inherit;
		font-size: 0.85rem;
		cursor: pointer;
		max-width: 100%;
	}
	span {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
</style>

<script lang="ts">
	import Icon from '../Icon.svelte';
	import { iconPath } from '../icons';
	import type { Card } from '../model';

	let { card }: { card: Card } = $props();
	const heading = $derived(typeof card.heading === 'string' ? card.heading : '');
</script>

<h2 class="heading" class:subtitle={card.heading_style === 'subtitle'}>
	{#if card.icon}<Icon path={iconPath(card.icon)} size={20} />{/if}
	<span>{heading}</span>
</h2>

<style>
	.heading {
		display: flex;
		align-items: center;
		gap: 8px;
		margin: 8px 0 0;
		font-size: 1.05rem;
		font-weight: 600;
		min-height: 24px;
	}
	.subtitle {
		font-size: 0.9rem;
		color: var(--text-muted);
	}
</style>

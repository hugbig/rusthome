<script lang="ts">
	// Image de la caméra via /api/v1/cameras/<id>/snapshot (Frigate, sinon HA en
	// lecture), rafraîchie toutes les 10 s tant que la page est visible.
	import { onDestroy, onMount } from 'svelte';
	import { live } from '../../stores/live.svelte';
	import { cardName, type Card } from '../model';
	import { ui } from '../runtime.svelte';

	let { card }: { card: Card } = $props();

	const s = $derived(card.entity ? live.states.get(card.entity) : undefined);
	let tick = $state(Date.now());
	let failed = $state(false);
	let timer: ReturnType<typeof setInterval> | undefined;

	onMount(() => {
		timer = setInterval(() => {
			if (document.visibilityState === 'visible') tick = Date.now();
		}, 10_000);
	});
	onDestroy(() => clearInterval(timer));

	const src = $derived(
		card.entity
			? `/api/v1/cameras/${encodeURIComponent(card.entity)}/snapshot?t=${tick}`
			: undefined
	);
</script>

<button class="dcard camera" onclick={() => card.entity && (ui.moreInfo = card.entity)}>
	{#if src && !failed}
		<img
			{src}
			alt="Caméra {cardName(card, s)}"
			onerror={() => (failed = true)}
			onload={() => (failed = false)}
		/>
	{:else}
		<div class="none">Image indisponible</div>
	{/if}
	<span class="name">{cardName(card, s)}</span>
</button>

<style>
	.camera {
		position: relative;
		padding: 0;
		overflow: hidden;
		width: 100%;
		color: var(--text);
		font: inherit;
		cursor: pointer;
		display: block;
	}
	img,
	.none {
		display: block;
		width: 100%;
		aspect-ratio: 16 / 9;
		object-fit: cover;
		background: var(--surface-2);
	}
	.none {
		display: grid;
		place-items: center;
		color: var(--text-muted);
		font-size: 0.85rem;
	}
	.name {
		position: absolute;
		left: 0;
		right: 0;
		bottom: 0;
		padding: 6px 10px;
		background: linear-gradient(transparent, rgb(0 0 0 / 0.65));
		color: #fff;
		font-size: 0.85rem;
		text-align: left;
	}
</style>

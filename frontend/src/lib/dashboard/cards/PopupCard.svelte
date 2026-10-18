<script lang="ts">
	// Pop-up ouvert par une ancre (`#media`), comme les pop-ups Bubble Card :
	// invisible dans la vue, affiché en fenêtre quand l'URL porte son ancre.
	import Card from '../Card.svelte';
	import Icon from '../Icon.svelte';
	import { UI } from '../icons';
	import type { Card as CardT } from '../model';
	import { closeHash, ui } from '../runtime.svelte';

	let { card }: { card: CardT } = $props();
	const open = $derived(typeof card.hash === 'string' && ui.hash === card.hash);
	const title = $derived(typeof card.title === 'string' ? card.title : '');
</script>

{#if open}
	<div class="backdrop" role="presentation" onclick={closeHash}></div>
	<div class="popup" role="dialog" aria-modal="true" aria-label={title || 'Pop-up'}>
		<div class="bar">
			<strong>{title}</strong>
			<button class="dbtn" onclick={closeHash} aria-label="Fermer"
				><Icon path={UI.close} size={20} /></button
			>
		</div>
		<div class="dstack">
			{#each card.cards ?? [] as c, i (i)}<Card card={c} />{/each}
		</div>
	</div>
{/if}

<style>
	.backdrop {
		position: fixed;
		inset: 0;
		background: rgb(0 0 0 / 0.5);
		z-index: 40;
	}
	.popup {
		position: fixed;
		z-index: 41;
		left: 50%;
		bottom: 0;
		transform: translateX(-50%);
		width: min(640px, 100vw);
		max-height: 85vh;
		overflow-y: auto;
		background: var(--bg);
		border-radius: 16px 16px 0 0;
		padding: 12px 16px 24px;
		box-sizing: border-box;
	}
	.bar {
		display: flex;
		justify-content: space-between;
		align-items: center;
		margin-bottom: 12px;
	}
</style>

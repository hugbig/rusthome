<script lang="ts">
	// Boîte de dialogue modale : titre, contenu défilant, pied d'actions.
	// Échap ou clic sur le fond ferme.
	import type { Snippet } from 'svelte';
	import Icon from '../dashboard/Icon.svelte';
	import { UI } from '../dashboard/icons';

	interface Props {
		title: string;
		onclose: () => void;
		/** Largeur maximale en pixels. */
		width?: number;
		children: Snippet;
		footer?: Snippet;
	}
	let { title, onclose, width = 560, children, footer }: Props = $props();
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && onclose()} />

<div class="backdrop" role="presentation" onclick={onclose}></div>
<div class="dialog" role="dialog" aria-modal="true" aria-label={title} style:--w="{width}px">
	<div class="head">
		<h2>{title}</h2>
		<button class="close" onclick={onclose} aria-label="Fermer"><Icon path={UI.close} /></button>
	</div>
	<div class="body">{@render children()}</div>
	{#if footer}<div class="foot">{@render footer()}</div>{/if}
</div>

<style>
	.backdrop {
		position: fixed;
		inset: 0;
		background: rgb(0 0 0 / 0.55);
		z-index: 100;
	}
	.dialog {
		position: fixed;
		z-index: 101;
		left: 50%;
		top: 50%;
		transform: translate(-50%, -50%);
		width: min(var(--w), calc(100vw - 24px));
		max-height: 88vh;
		display: flex;
		flex-direction: column;
		background: var(--surface);
		border: 1px solid var(--border);
		border-radius: 20px;
		box-shadow: var(--shadow-pop);
	}
	.head,
	.foot {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 12px 18px;
	}
	.head {
		justify-content: space-between;
		border-bottom: 1px solid var(--border);
	}
	.foot {
		justify-content: flex-end;
		flex-wrap: wrap;
		border-top: 1px solid var(--border);
	}
	h2 {
		margin: 0;
		font-size: 1.1rem;
	}
	.close {
		display: grid;
		place-items: center;
		width: 40px;
		height: 40px;
		border: none;
		border-radius: 50%;
		background: none;
		color: var(--text);
		cursor: pointer;
	}
	.body {
		display: flex;
		flex-direction: column;
		gap: 14px;
		padding: 16px 18px;
		overflow-y: auto;
		min-height: 0;
	}
</style>

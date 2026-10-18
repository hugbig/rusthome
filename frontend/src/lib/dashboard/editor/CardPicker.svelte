<script lang="ts">
	// Choix du type de carte à ajouter, par catégorie, avec recherche.
	import Icon from '../Icon.svelte';
	import { iconPath, UI } from '../icons';
	import { CATALOG, CATEGORIES, type CardInfo } from './catalog';

	let { onpick, onclose }: { onpick: (c: CardInfo) => void; onclose: () => void } = $props();
	let q = $state('');
	const shown = $derived(
		CATALOG.filter(
			(c) =>
				!q || `${c.label} ${c.description} ${c.type}`.toLowerCase().includes(q.trim().toLowerCase())
		)
	);
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && onclose()} />

<div class="backdrop" role="presentation" onclick={onclose}></div>
<div class="dialog" role="dialog" aria-modal="true" aria-label="Ajouter une carte">
	<div class="head">
		<h2>Ajouter une carte</h2>
		<button class="close" onclick={onclose} aria-label="Fermer"><Icon path={UI.close} /></button>
	</div>
	<!-- svelte-ignore a11y_autofocus -->
	<input type="search" placeholder="Rechercher une carte…" bind:value={q} autofocus />
	<div class="scroll">
		{#each CATEGORIES as [cat, label] (cat)}
			{@const list = shown.filter((c) => c.category === cat)}
			{#if list.length}
				<h3>{label}</h3>
				<div class="grid">
					{#each list as c (c.type)}
						<button class="item" onclick={() => onpick(c)}>
							<span class="ic"><Icon path={iconPath(c.icon)} /></span>
							<strong>{c.label}</strong>
							<small>{c.description}</small>
						</button>
					{/each}
				</div>
			{/if}
		{/each}
	</div>
</div>

<style>
	.backdrop {
		position: fixed;
		inset: 0;
		background: rgb(0 0 0 / 0.5);
		z-index: 100;
	}
	.dialog {
		position: fixed;
		z-index: 101;
		left: 50%;
		top: 50%;
		transform: translate(-50%, -50%);
		width: min(860px, calc(100vw - 24px));
		height: min(80vh, 760px);
		display: flex;
		flex-direction: column;
		gap: 12px;
		padding: 16px;
		box-sizing: border-box;
		background: var(--surface);
		border: 1px solid var(--border);
		border-radius: 16px;
	}
	.head {
		display: flex;
		justify-content: space-between;
		align-items: center;
	}
	h2 {
		margin: 0;
		font-size: 1.15rem;
	}
	h3 {
		margin: 12px 0 8px;
		font-size: 0.85rem;
		color: var(--text-muted);
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
	input {
		padding: 8px 12px;
		border: 1px solid var(--border);
		border-radius: 8px;
		background: var(--bg);
		color: var(--text);
		font: inherit;
	}
	.scroll {
		overflow-y: auto;
	}
	.grid {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
		gap: 8px;
	}
	.item {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: 4px;
		padding: 12px;
		border: 1px solid var(--border);
		border-radius: 12px;
		background: var(--bg);
		color: var(--text);
		font: inherit;
		text-align: left;
		cursor: pointer;
	}
	.item:hover {
		border-color: var(--primary);
	}
	.ic {
		color: var(--primary-text);
	}
	small {
		color: var(--text-muted);
		font-size: 0.8rem;
	}
</style>

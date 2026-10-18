<script lang="ts">
	// Sous-cartes d'une pile, d'une grille ou d'un pop-up : un onglet par carte,
	// comme l'éditeur de piles de HA. Chaque onglet a son formulaire (récursif).
	import Icon from '../Icon.svelte';
	import { UI } from '../icons';
	import type { Card } from '../model';
	import { CARD_INFO, CATALOG } from './catalog';
	import CardForm from './CardForm.svelte';
	import CardYaml from './CardYaml.svelte';

	let { cards, onchange }: { cards: Card[]; onchange: (cards: Card[]) => void } = $props();

	let selected = $state(0);
	let adding = $state(false);
	let yaml = $state(false);
	const current = $derived(cards[Math.min(selected, cards.length - 1)]);
	const at = $derived(Math.min(selected, cards.length - 1));

	function replace(c: Card) {
		onchange(cards.map((x, i) => (i === at ? c : x)));
	}
	function move(d: -1 | 1) {
		const j = at + d;
		if (j < 0 || j >= cards.length) return;
		const out = [...cards];
		[out[at], out[j]] = [out[j]!, out[at]!];
		onchange(out);
		selected = j;
	}
	function add(type: string) {
		const t = CARD_INFO[type]?.template ?? { type };
		onchange([...cards, structuredClone(t)]);
		selected = cards.length;
		adding = false;
	}
</script>

<div class="sub">
	<span class="lbl">Cartes</span>
	<div class="tabs" role="tablist">
		{#each cards as c, i (i)}
			<button
				role="tab"
				aria-selected={i === at && !adding}
				onclick={() => ((selected = i), (adding = false))}
			>
				{i + 1}. {CARD_INFO[c.type]?.label ?? c.type}
			</button>
		{/each}
		<button
			role="tab"
			aria-selected={adding}
			aria-label="Ajouter une carte"
			onclick={() => (adding = true)}><Icon path={UI.add} size={18} /></button
		>
	</div>

	{#if adding || cards.length === 0}
		<div class="picker">
			{#each CATALOG as c (c.type)}
				<button class="dbtn" onclick={() => add(c.type)}>{c.label}</button>
			{/each}
		</div>
	{:else if current}
		<div class="bar">
			<button class="dbtn" onclick={() => (yaml = !yaml)}>{yaml ? 'Visuel' : 'YAML'}</button>
			<span class="spacer"></span>
			<button aria-label="Déplacer avant" disabled={at === 0} onclick={() => move(-1)}
				><Icon path={UI.up} size={16} /></button
			>
			<button aria-label="Déplacer après" disabled={at === cards.length - 1} onclick={() => move(1)}
				><Icon path={UI.down} size={16} /></button
			>
			<button
				aria-label="Supprimer cette carte"
				onclick={() => {
					onchange(cards.filter((_, i) => i !== at));
					selected = Math.max(0, at - 1);
				}}><Icon path={UI.remove} size={16} /></button
			>
		</div>
		<div class="body">
			{#key at}
				{#if yaml}
					<CardYaml card={current} onchange={replace} />
				{:else}
					<CardForm card={current} onchange={replace} />
				{/if}
			{/key}
		</div>
	{/if}
</div>

<style>
	.sub {
		display: flex;
		flex-direction: column;
		gap: 6px;
	}
	.lbl {
		font-size: 0.8rem;
		color: var(--text-muted);
	}
	.tabs {
		display: flex;
		flex-wrap: wrap;
		gap: 2px;
		border-bottom: 1px solid var(--border);
	}
	.tabs button {
		display: flex;
		align-items: center;
		padding: 6px 10px;
		border: none;
		border-bottom: 2px solid transparent;
		background: none;
		color: var(--text-muted);
		font: inherit;
		font-size: 0.85rem;
		cursor: pointer;
	}
	.tabs button[aria-selected='true'] {
		color: var(--text);
		border-bottom-color: var(--primary);
	}
	.picker {
		display: flex;
		flex-wrap: wrap;
		gap: 6px;
		padding: 8px 0;
	}
	.bar {
		display: flex;
		align-items: center;
		gap: 2px;
	}
	.bar > button:not(.dbtn) {
		display: grid;
		place-items: center;
		width: 30px;
		height: 30px;
		border: none;
		border-radius: 6px;
		background: none;
		color: var(--text-muted);
		cursor: pointer;
	}
	.bar > button:disabled {
		opacity: 0.3;
	}
	.spacer {
		flex: 1;
	}
	.body {
		padding: 10px;
		border: 1px solid var(--border);
		border-radius: 8px;
	}
</style>

<script lang="ts">
	// Liste de blocs (déclencheurs, conditions ou actions) avec ajout par type.
	import Icon from '../dashboard/Icon.svelte';
	import { UI } from '../dashboard/icons';
	import BlockCard from './BlockCard.svelte';
	import { BLOCKS, copy, moveItem, type Item, type Kind } from './blocks';

	interface Props {
		kind: Kind;
		items: Item[];
		onchange: (items: Item[]) => void;
		addLabel?: string;
	}
	let { kind, items, onchange, addLabel }: Props = $props();
	let picking = $state(false);

	const LABEL: Record<Kind, string> = {
		trigger: 'Ajouter un déclencheur',
		condition: 'Ajouter une condition',
		action: 'Ajouter une action'
	};
</script>

<div class="list">
	{#each items as it, i (i)}
		<BlockCard
			{kind}
			item={it}
			index={i}
			count={items.length}
			onchange={(v) => onchange(items.map((x, k) => (k === i ? v : x)))}
			onremove={() => onchange(items.filter((_, k) => k !== i))}
			onmove={(d) => onchange(moveItem(items, i, d))}
			onduplicate={() => onchange([...items.slice(0, i + 1), copy(it), ...items.slice(i + 1)])}
		/>
	{/each}
	{#if picking}
		<div class="picker">
			{#each BLOCKS[kind] as b (b.id)}
				<button
					class="dbtn"
					onclick={() => {
						onchange([...items, copy(b.template)]);
						picking = false;
					}}>{b.label}</button
				>
			{/each}
			<button class="dbtn" onclick={() => (picking = false)}>Annuler</button>
		</div>
	{:else}
		<button class="add" onclick={() => (picking = true)}>
			<Icon path={UI.add} size={18} />
			{addLabel ?? LABEL[kind]}
		</button>
	{/if}
</div>

<style>
	.list {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}
	.add {
		align-self: flex-start;
		display: inline-flex;
		align-items: center;
		gap: 6px;
		padding: 7px 12px;
		border: 1px dashed var(--border);
		border-radius: 8px;
		background: none;
		color: var(--primary-text);
		font: inherit;
		cursor: pointer;
	}
	.add:hover {
		border-color: var(--primary);
	}
	.picker {
		display: flex;
		flex-wrap: wrap;
		gap: 6px;
		padding: 8px;
		border: 1px dashed var(--primary);
		border-radius: 10px;
	}
</style>

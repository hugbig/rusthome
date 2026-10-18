<script lang="ts" module>
	import type { CardLoc } from './layout';

	/** Ce que l'éditeur demande au tableau de bord (qui applique et enregistre). */
	export type EditOp =
		| { kind: 'edit'; loc: CardLoc }
		| { kind: 'add'; where: Omit<CardLoc, 'index'> }
		| { kind: 'remove'; loc: CardLoc }
		| { kind: 'move'; loc: CardLoc; delta: -1 | 1 }
		| { kind: 'moveTo'; from: CardLoc; to: Omit<CardLoc, 'index'> & { index?: number } }
		| { kind: 'duplicate'; loc: CardLoc }
		| { kind: 'addSection' }
		| { kind: 'section'; section: number; action: 'up' | 'down' | 'remove' | 'span' };
</script>

<script lang="ts">
	// Disposition d'une vue (sections, maçonnerie, panneau) et, en édition, les
	// commandes façon HA : bouton Modifier et menu ⋮ par carte, glisser-déposer,
	// menu ⋮ par section.
	import { mdiArrowExpandHorizontal, mdiContentCopy, mdiDragVariant } from '@mdi/js';
	import Menu from '../components/Menu.svelte';
	import Card from './Card.svelte';
	import ChipsCard from './cards/ChipsCard.svelte';
	import Icon from './Icon.svelte';
	import { UI } from './icons';
	import { columnsFor, gridColumns, gridRows, masonry } from './layout';
	import type { Card as CardT, View } from './model';

	interface Props {
		view: View;
		viewIndex: number;
		editing?: boolean;
		onop?: (op: EditOp) => void;
	}
	let { view, viewIndex, editing = false, onop }: Props = $props();

	let width = $state(1000);
	const cards = $derived(view.cards ?? []);
	const sectionCols = $derived(columnsFor(width, 320, view.max_columns));
	const masonryCols = $derived(masonry(cards, columnsFor(width, 340, view.max_columns)));
	const badges = $derived(view.badges?.map((b) => ({ type: 'chip', entity: b.entity })) ?? []);

	// Glisser-déposer : la carte saisie et l'endroit survolé.
	let dragging = $state<CardLoc | null>(null);
	let over = $state<string | null>(null);
	const key = (l: Omit<CardLoc, 'index'> & { index?: number }) =>
		`${l.section ?? 'v'}:${l.index ?? 'end'}`;

	function drop(to: Omit<CardLoc, 'index'> & { index?: number }) {
		if (dragging) onop?.({ kind: 'moveTo', from: dragging, to });
		dragging = null;
		over = null;
	}
	const rowsStyle = (c: CardT) => {
		const r = gridRows(c);
		return r ? `${r * 56 + (r - 1) * 8}px` : undefined;
	};
</script>

{#snippet cell(card: CardT, loc: CardLoc, count: number)}
	{#if editing}
		<div
			class="editable"
			class:drop={over === key(loc)}
			class:lifted={dragging && dragging.index === loc.index && dragging.section === loc.section}
			draggable="true"
			role="group"
			aria-label="Carte {loc.index + 1}"
			ondragstart={(e) => {
				dragging = loc;
				e.dataTransfer?.setData('text/plain', key(loc));
			}}
			ondragend={() => ((dragging = null), (over = null))}
			ondragover={(e) => {
				if (!dragging) return;
				e.preventDefault();
				over = key(loc);
			}}
			ondrop={(e) => {
				e.preventDefault();
				e.stopPropagation();
				drop(loc);
			}}
		>
			{#if card.type === 'popup'}
				<div class="dcard popup-ph">
					Pop-up <code>{card.hash}</code> · {card.cards?.length ?? 0} carte(s)
					<button class="linkish" onclick={() => (location.hash = String(card.hash))}>ouvrir</button
					>
				</div>
			{/if}
			<div class="inert"><Card {card} /></div>
			<div class="overlay">
				<span class="handle" aria-hidden="true"><Icon path={mdiDragVariant} size={18} /></span>
				<button class="edit" onclick={() => onop?.({ kind: 'edit', loc })}>Modifier</button>
				<Menu
					label="Options de la carte"
					items={[
						{
							label: 'Dupliquer',
							icon: mdiContentCopy,
							onclick: () => onop?.({ kind: 'duplicate', loc })
						},
						{
							label: 'Monter',
							icon: UI.up,
							disabled: loc.index === 0,
							onclick: () => onop?.({ kind: 'move', loc, delta: -1 })
						},
						{
							label: 'Descendre',
							icon: UI.down,
							disabled: loc.index === count - 1,
							onclick: () => onop?.({ kind: 'move', loc, delta: 1 })
						},
						{
							label: 'Supprimer',
							icon: UI.remove,
							danger: true,
							divider: true,
							onclick: () => onop?.({ kind: 'remove', loc })
						}
					]}
				/>
			</div>
		</div>
	{:else}
		<Card {card} />
	{/if}
{/snippet}

{#snippet addButton(where: Omit<CardLoc, 'index'>)}
	{#if editing}
		<button
			class="add"
			class:drop={over === key(where)}
			onclick={() => onop?.({ kind: 'add', where })}
			ondragover={(e) => {
				if (!dragging) return;
				e.preventDefault();
				over = key(where);
			}}
			ondrop={(e) => {
				e.preventDefault();
				drop(where);
			}}
		>
			<Icon path={UI.add} size={18} /> Ajouter une carte
		</button>
	{/if}
{/snippet}

<div class="view" class:editing bind:clientWidth={width}>
	{#if badges.length}
		<div class="badges"><ChipsCard card={{ type: 'chips', chips: badges }} /></div>
	{/if}

	{#if view.type === 'sections'}
		<div class="sections" style:--cols={sectionCols}>
			{#each view.sections ?? [] as section, si (si)}
				{@const span = Math.min(section.column_span ?? 1, sectionCols)}
				<section class="section" class:framed={editing} style:grid-column="span {span}">
					{#if editing}
						<div class="sbar">
							<span>Section {si + 1}</span>
							<Menu
								label="Options de la section"
								items={[
									{
										label: 'Déplacer avant',
										icon: UI.up,
										disabled: si === 0,
										onclick: () => onop?.({ kind: 'section', section: si, action: 'up' })
									},
									{
										label: 'Déplacer après',
										icon: UI.down,
										disabled: si === (view.sections?.length ?? 1) - 1,
										onclick: () => onop?.({ kind: 'section', section: si, action: 'down' })
									},
									{
										label: span > 1 ? 'Largeur : une colonne' : 'Largeur : deux colonnes',
										icon: mdiArrowExpandHorizontal,
										onclick: () => onop?.({ kind: 'section', section: si, action: 'span' })
									},
									{
										label: 'Supprimer la section',
										icon: UI.remove,
										danger: true,
										divider: true,
										onclick: () => onop?.({ kind: 'section', section: si, action: 'remove' })
									}
								]}
							/>
						</div>
					{/if}
					{#if section.title}<h2>{section.title}</h2>{/if}
					<div class="grid12">
						{#each section.cards as card, i (i)}
							<div
								class="slot"
								style:grid-column="span {gridColumns(card)}"
								style:min-height={rowsStyle(card)}
							>
								{@render cell(
									card,
									{ view: viewIndex, section: si, index: i },
									section.cards.length
								)}
							</div>
						{/each}
					</div>
					{@render addButton({ view: viewIndex, section: si })}
				</section>
			{/each}
			{#if editing}
				<button class="add section-add" onclick={() => onop?.({ kind: 'addSection' })}>
					<Icon path={UI.add} size={18} /> Créer une section
				</button>
			{/if}
		</div>
	{:else if view.type === 'panel'}
		{#if cards[0]}
			<div class="panel">{@render cell(cards[0], { view: viewIndex, index: 0 }, cards.length)}</div>
		{/if}
		{#if !cards.length}{@render addButton({ view: viewIndex })}{/if}
	{:else}
		<div class="masonry">
			{#each masonryCols as col, ci (ci)}
				<div class="col">
					{#each col as i (i)}
						{@render cell(cards[i]!, { view: viewIndex, index: i }, cards.length)}
					{/each}
				</div>
			{/each}
		</div>
		{@render addButton({ view: viewIndex })}
	{/if}
</div>

<style>
	.view {
		width: 100%;
	}
	.badges {
		margin-bottom: 12px;
	}
	.sections {
		display: grid;
		grid-template-columns: repeat(var(--cols), minmax(0, 1fr));
		gap: 16px;
		align-items: start;
	}
	.section {
		display: flex;
		flex-direction: column;
		gap: 8px;
		min-width: 0;
	}
	.section.framed {
		padding: 8px;
		border: 2px dashed var(--border);
		border-radius: 16px;
	}
	.sbar {
		display: flex;
		justify-content: space-between;
		align-items: center;
		font-size: 0.8rem;
		color: var(--text-muted);
		padding-left: 4px;
	}
	.section h2 {
		margin: 0;
	}
	.grid12 {
		display: grid;
		grid-template-columns: repeat(12, minmax(0, 1fr));
		gap: 8px;
	}
	.slot {
		min-width: 0;
		display: flex;
		flex-direction: column;
	}
	.slot > :global(*) {
		flex: 1;
	}
	.masonry {
		display: flex;
		gap: 12px;
		align-items: flex-start;
	}
	.col {
		flex: 1 1 0;
		min-width: 0;
		display: flex;
		flex-direction: column;
		gap: 12px;
	}
	.editable {
		position: relative;
		display: flex;
		flex-direction: column;
		border-radius: 12px;
		outline: 1px solid var(--border);
		outline-offset: 2px;
		cursor: grab;
	}
	.editable.lifted {
		opacity: 0.4;
	}
	.editable.drop {
		outline: 2px solid var(--primary);
	}
	/* En édition, la carte ne réagit pas : le clic sert à l'éditer. */
	.inert {
		pointer-events: none;
		flex: 1;
	}
	.overlay {
		display: flex;
		align-items: center;
		gap: 4px;
		padding: 4px 4px 4px 8px;
		margin-top: 4px;
		border-radius: 10px;
		background: var(--surface-2);
	}
	.handle {
		color: var(--text-muted);
		display: grid;
	}
	.edit {
		flex: 1;
		padding: 6px;
		border: none;
		border-radius: 8px;
		background: none;
		color: var(--primary-text);
		font: inherit;
		font-weight: 600;
		cursor: pointer;
	}
	.edit:hover {
		background: color-mix(in srgb, var(--primary) 14%, transparent);
	}
	.overlay :global(.trigger) {
		width: 34px;
		height: 34px;
	}
	.add {
		display: flex;
		align-items: center;
		justify-content: center;
		gap: 6px;
		width: 100%;
		margin-top: 4px;
		padding: 10px;
		border: 2px dashed var(--border);
		border-radius: 12px;
		background: none;
		color: var(--text-muted);
		font: inherit;
		cursor: pointer;
	}
	.add:hover,
	.add.drop {
		border-color: var(--primary);
		color: var(--primary-text);
	}
	.section-add {
		min-height: 120px;
		margin: 0;
	}
	.linkish {
		background: none;
		border: none;
		padding: 0;
		color: var(--primary-text);
		text-decoration: underline;
		cursor: pointer;
		font: inherit;
		pointer-events: auto;
	}
	.popup-ph {
		border-style: dashed;
		font-size: 0.85rem;
		color: var(--text-muted);
		margin-bottom: 4px;
	}
</style>

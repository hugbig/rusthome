<script lang="ts">
	// Éditeur d'une carte façon HA : formulaire (ou YAML) à gauche, aperçu en
	// direct à droite ; onglet Disposition pour la taille dans une section.
	import Card from '../Card.svelte';
	import Icon from '../Icon.svelte';
	import { UI } from '../icons';
	import { clone, gridColumns, gridRows, withGrid } from '../layout';
	import type { Card as CardT } from '../model';
	import { CARD_INFO } from './catalog';
	import CardForm from './CardForm.svelte';
	import CardYaml from './CardYaml.svelte';

	interface Props {
		card: CardT;
		/** Carte d'une section : l'onglet Disposition règle colonnes et lignes. */
		inSection?: boolean;
		isNew?: boolean;
		onsave: (c: CardT) => void;
		onclose: () => void;
	}
	let { card, inSection = false, isNew = false, onsave, onclose }: Props = $props();

	// svelte-ignore state_referenced_locally
	let draft = $state<CardT>(clone(card));
	let mode = $state<'visual' | 'yaml'>('visual');
	let tab = $state<'config' | 'layout'>('config');
	const info = $derived(CARD_INFO[draft.type]);
	const cols = $derived(gridColumns(draft));
	const rows = $derived(gridRows(draft));
	const explicitCols = $derived(
		(draft.grid_options as { columns?: unknown } | undefined)?.columns as
			number | 'full' | undefined
	);
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && onclose()} />

<div class="backdrop" role="presentation" onclick={onclose}></div>
<div class="dialog" role="dialog" aria-modal="true" aria-label="Éditeur de carte">
	<div class="head">
		<h2>{isNew ? 'Ajouter' : 'Modifier'} : {info?.label ?? draft.type}</h2>
		<button class="close" onclick={onclose} aria-label="Fermer"><Icon path={UI.close} /></button>
	</div>

	<div class="cols">
		<div class="edit">
			<div class="tabs" role="tablist">
				<button role="tab" aria-selected={tab === 'config'} onclick={() => (tab = 'config')}
					>Configuration</button
				>
				{#if inSection}
					<button role="tab" aria-selected={tab === 'layout'} onclick={() => (tab = 'layout')}
						>Disposition</button
					>
				{/if}
				<span class="spacer"></span>
				{#if tab === 'config'}
					<button class="dbtn" onclick={() => (mode = mode === 'visual' ? 'yaml' : 'visual')}>
						{mode === 'visual' ? 'Afficher en YAML' : 'Éditeur visuel'}
					</button>
				{/if}
			</div>
			<div class="scroll">
				{#if tab === 'layout'}
					<div class="layout">
						<p class="note">
							Une section fait 12 colonnes. Par défaut, tuiles, boutons et jauges en prennent 6, les
							autres cartes toute la largeur.
						</p>
						<label>
							<span>Largeur : {cols === 12 ? 'toute la section' : `${cols} / 12 colonnes`}</span>
							<input
								type="range"
								min="1"
								max="12"
								step="1"
								value={cols}
								oninput={(e) => (draft = withGrid(draft, Number(e.currentTarget.value), rows))}
							/>
						</label>
						<div class="presets">
							{#each [[3, '1/4'], [4, '1/3'], [6, '1/2'], [12, 'Pleine']] as [v, l] (v)}
								<button
									class="dbtn"
									aria-pressed={cols === v}
									onclick={() => (draft = withGrid(draft, v as number, rows))}>{l}</button
								>
							{/each}
							<button class="dbtn" onclick={() => (draft = withGrid(draft, undefined, undefined))}
								>Par défaut</button
							>
						</div>
						<label>
							<span>Hauteur : {rows ? `${rows} ligne(s)` : 'automatique'}</span>
							<input
								type="range"
								min="0"
								max="12"
								step="1"
								value={rows ?? 0}
								oninput={(e) => {
									const r = Number(e.currentTarget.value);
									draft = withGrid(draft, explicitCols, r === 0 ? undefined : r);
								}}
							/>
						</label>
					</div>
				{:else if mode === 'yaml'}
					<CardYaml card={draft} onchange={(c) => (draft = c)} />
				{:else}
					<CardForm card={draft} onchange={(c) => (draft = c)} />
				{/if}
			</div>
		</div>
		<div class="preview" aria-label="Aperçu">
			<span class="plabel">Aperçu</span>
			{#if draft.type === 'popup'}
				<p class="note">
					Un pop-up s’ouvre par son ancre {String(draft.hash)} : contenu ci-dessous.
				</p>
				<div class="dstack">
					{#each draft.cards ?? [] as c, i (i)}<Card card={c} />{/each}
				</div>
			{:else}
				<div style:max-width={inSection ? `${(cols / 12) * 100}%` : undefined}>
					<Card card={draft} />
				</div>
			{/if}
		</div>
	</div>

	<div class="foot">
		<button class="dbtn" onclick={onclose}>Annuler</button>
		<button class="dbtn primary" onclick={() => onsave(draft)}>Enregistrer</button>
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
		width: min(1100px, calc(100vw - 24px));
		height: min(88vh, 900px);
		display: flex;
		flex-direction: column;
		box-sizing: border-box;
		background: var(--surface);
		border: 1px solid var(--border);
		border-radius: 16px;
		overflow: hidden;
	}
	.head {
		display: flex;
		justify-content: space-between;
		align-items: center;
		padding: 12px 16px;
		border-bottom: 1px solid var(--border);
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
	.cols {
		flex: 1;
		min-height: 0;
		display: grid;
		grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
	}
	@media (max-width: 760px) {
		.cols {
			grid-template-columns: minmax(0, 1fr);
			overflow-y: auto;
		}
	}
	.edit {
		display: flex;
		flex-direction: column;
		min-height: 0;
		border-right: 1px solid var(--border);
	}
	.tabs {
		display: flex;
		align-items: center;
		gap: 4px;
		padding: 8px 12px 0;
		border-bottom: 1px solid var(--border);
	}
	.tabs [role='tab'] {
		padding: 8px 12px;
		border: none;
		border-bottom: 2px solid transparent;
		background: none;
		color: var(--text-muted);
		font: inherit;
		cursor: pointer;
	}
	.tabs [role='tab'][aria-selected='true'] {
		color: var(--text);
		border-bottom-color: var(--primary);
	}
	.tabs .dbtn {
		margin-bottom: 6px;
		min-height: 30px;
		font-size: 0.85rem;
	}
	.spacer {
		flex: 1;
	}
	.scroll {
		flex: 1;
		min-height: 0;
		overflow-y: auto;
		padding: 14px 16px;
	}
	.preview {
		padding: 14px 16px;
		background: var(--bg);
		overflow-y: auto;
	}
	.plabel {
		display: block;
		margin-bottom: 8px;
		font-size: 0.8rem;
		color: var(--text-muted);
	}
	.layout {
		display: flex;
		flex-direction: column;
		gap: 14px;
	}
	.layout label {
		display: flex;
		flex-direction: column;
		gap: 6px;
	}
	.layout input[type='range'] {
		accent-color: var(--primary);
	}
	.presets {
		display: flex;
		flex-wrap: wrap;
		gap: 6px;
	}
	.presets [aria-pressed='true'] {
		border-color: var(--primary);
		color: var(--primary-text);
	}
	.note {
		margin: 0;
		font-size: 0.85rem;
		color: var(--text-muted);
	}
	.foot {
		display: flex;
		justify-content: flex-end;
		gap: 8px;
		padding: 12px 16px;
		border-top: 1px solid var(--border);
	}
</style>

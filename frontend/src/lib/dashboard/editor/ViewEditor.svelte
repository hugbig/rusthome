<script lang="ts">
	// Réglages d'une vue : titre, icône, adresse, disposition, colonnes.
	import Icon from '../Icon.svelte';
	import { iconPath, UI } from '../icons';
	import type { View } from '../model';

	interface Props {
		view: View;
		isNew?: boolean;
		canDelete?: boolean;
		onsave: (patch: Partial<View>) => void;
		ondelete?: () => void;
		onclose: () => void;
	}
	let { view, isNew = false, canDelete = true, onsave, ondelete, onclose }: Props = $props();

	// svelte-ignore state_referenced_locally
	let title = $state(view.title ?? '');
	// svelte-ignore state_referenced_locally
	let icon = $state(view.icon ?? '');
	// svelte-ignore state_referenced_locally
	let path = $state(view.path ?? '');
	// svelte-ignore state_referenced_locally
	let type = $state<View['type']>(view.type);
	// svelte-ignore state_referenced_locally
	let maxColumns = $state(view.max_columns ?? 4);
	const validPath = $derived(path === '' || /^[a-z0-9-]+$/.test(path));

	function save() {
		onsave({
			title: title.trim() || undefined,
			icon: icon.trim() || undefined,
			path: path.trim() || undefined,
			type,
			max_columns: type === 'panel' ? undefined : maxColumns
		});
	}
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && onclose()} />

<div class="backdrop" role="presentation" onclick={onclose}></div>
<div class="dialog" role="dialog" aria-modal="true" aria-label="Réglages de la vue">
	<div class="head">
		<h2>{isNew ? 'Nouvelle vue' : 'Modifier la vue'}</h2>
		<button class="close" onclick={onclose} aria-label="Fermer"><Icon path={UI.close} /></button>
	</div>
	<div class="body">
		<label><span>Titre</span><input bind:value={title} placeholder="Accueil" /></label>
		<label>
			<span>Icône</span>
			<span class="row">
				<span class="preview"><Icon path={iconPath(icon || 'mdi:view-dashboard')} size={20} /></span
				>
				<input bind:value={icon} placeholder="mdi:home" />
			</span>
		</label>
		<label>
			<span>Adresse (dans l’URL)</span>
			<input bind:value={path} placeholder="salon" aria-invalid={!validPath} />
			{#if !validPath}<small class="err">Minuscules, chiffres et tirets seulement.</small>{/if}
		</label>
		<fieldset>
			<legend>Disposition</legend>
			{#each [['sections', 'Sections', 'Grille de sections, comme les vues récentes de HA'], ['masonry', 'Maçonnerie', 'Cartes réparties en colonnes'], ['panel', 'Panneau', 'Une seule carte en pleine page']] as [v, l, d] (v)}
				<label class="radio">
					<input type="radio" name="type" value={v} bind:group={type} />
					<span><strong>{l}</strong> <small>{d}</small></span>
				</label>
			{/each}
		</fieldset>
		{#if type !== 'panel'}
			<label>
				<span>Colonnes au maximum : {maxColumns}</span>
				<input type="range" min="1" max="6" bind:value={maxColumns} />
			</label>
		{/if}
		{#if !isNew && type !== view.type}
			<p class="note">
				{type === 'sections'
					? 'Les cartes seront regroupées dans une section.'
					: 'Les sections seront mises à plat.'}
			</p>
		{/if}
	</div>
	<div class="foot">
		{#if !isNew && ondelete}
			<button class="dbtn danger" disabled={!canDelete} onclick={ondelete}>Supprimer la vue</button>
		{/if}
		<span class="spacer"></span>
		<button class="dbtn" onclick={onclose}>Annuler</button>
		<button class="dbtn primary" disabled={!validPath} onclick={save}>Enregistrer</button>
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
		width: min(520px, calc(100vw - 24px));
		max-height: 90vh;
		display: flex;
		flex-direction: column;
		background: var(--surface);
		border: 1px solid var(--border);
		border-radius: 16px;
	}
	.head,
	.foot {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 12px 16px;
	}
	.head {
		justify-content: space-between;
		border-bottom: 1px solid var(--border);
	}
	.foot {
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
		padding: 16px;
		overflow-y: auto;
	}
	label {
		display: flex;
		flex-direction: column;
		gap: 4px;
	}
	label > span:first-child {
		font-size: 0.8rem;
		color: var(--text-muted);
	}
	input:not([type='radio']):not([type='range']) {
		padding: 7px 10px;
		border: 1px solid var(--border);
		border-radius: 8px;
		background: var(--bg);
		color: var(--text);
		font: inherit;
	}
	input[type='range'] {
		accent-color: var(--primary);
	}
	.row {
		display: flex;
		gap: 8px;
		align-items: center;
	}
	.row input {
		flex: 1;
	}
	.preview {
		display: grid;
		place-items: center;
		width: 36px;
		height: 36px;
		border-radius: 8px;
		background: var(--surface-2);
	}
	fieldset {
		border: 1px solid var(--border);
		border-radius: 8px;
		padding: 8px 12px;
		display: flex;
		flex-direction: column;
		gap: 6px;
	}
	legend {
		font-size: 0.8rem;
		color: var(--text-muted);
	}
	.radio {
		flex-direction: row;
		align-items: center;
		gap: 10px;
	}
	small {
		color: var(--text-muted);
	}
	.err {
		color: #e66767;
	}
	.note {
		margin: 0;
		font-size: 0.85rem;
		color: var(--text-muted);
	}
	.spacer {
		flex: 1;
	}
	.danger {
		color: #e66767;
	}
</style>

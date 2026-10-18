<script lang="ts">
	// Choix d'une icône par recherche dans le jeu Material Design complet, avec
	// aperçu, comme le sélecteur d'icône de HA. Vide = icône par défaut de
	// l'entité (affichée en aperçu).
	import Icon from '../Icon.svelte';
	import { iconSet } from '../icon-set.svelte';
	import { iconPath, searchIcons, UI } from '../icons';

	interface Props {
		value?: string;
		/** Entité dont l'icône par défaut sert d'aperçu quand rien n'est choisi. */
		entityId?: string;
		label?: string;
		onchange: (icon: string | undefined) => void;
	}
	let { value, entityId, label = 'Icône', onchange }: Props = $props();

	let q = $state('');
	let open = $state(false);
	let active = $state(0);
	const listId = `ip-${Math.random().toString(36).slice(2, 8)}`;

	const matches = $derived(iconSet.paths ? searchIcons(Object.keys(iconSet.paths), q) : []);

	function pick(icon: string | undefined) {
		onchange(icon);
		q = '';
		open = false;
	}
	function focus() {
		iconSet.load();
		open = true;
	}
	function key(e: KeyboardEvent) {
		if (e.key === 'ArrowDown') {
			active = Math.min(active + 1, matches.length - 1);
			open = true;
			e.preventDefault();
		} else if (e.key === 'ArrowUp') {
			active = Math.max(active - 1, 0);
			e.preventDefault();
		} else if (e.key === 'Enter') {
			e.preventDefault();
			if (open && matches[active]) pick(matches[active]);
			// nom saisi en entier (`mdi:…`) sans passer par la liste
			else if (q.trim().startsWith('mdi:')) pick(q.trim());
		} else if (e.key === 'Escape') open = false;
	}
</script>

<div class="picker">
	<span class="lbl">{label}</span>
	<div class="field">
		<span class="preview"><Icon path={iconPath(value, entityId)} size={20} /></span>
		<input
			type="search"
			role="combobox"
			aria-expanded={open}
			aria-controls={listId}
			aria-label={label}
			placeholder={value ?? 'Par défaut (rechercher : lampe, sofa…)'}
			bind:value={q}
			onfocus={focus}
			oninput={() => {
				open = true;
				active = 0;
			}}
			onkeydown={key}
			onblur={() => setTimeout(() => (open = false), 150)}
		/>
		{#if value}
			<button
				type="button"
				class="clear"
				aria-label="Revenir à l’icône par défaut"
				onclick={() => pick(undefined)}><Icon path={UI.close} size={16} /></button
			>
		{/if}
	</div>
	{#if open && q.trim()}
		<ul id={listId} role="listbox">
			{#if !iconSet.paths}
				<li class="muted">Chargement des icônes…</li>
			{:else if !matches.length}
				<li class="muted">Aucune icône.</li>
			{/if}
			{#each matches as m, i (m)}
				<li role="option" aria-selected={i === active}>
					<button type="button" onmousedown={(e) => e.preventDefault()} onclick={() => pick(m)}>
						<Icon path={iconPath(m)} size={20} />
						<span>{m}</span>
					</button>
				</li>
			{/each}
		</ul>
	{/if}
</div>

<style>
	.picker {
		position: relative;
		display: flex;
		flex-direction: column;
		gap: 4px;
		min-width: 0;
	}
	.lbl {
		font-size: 0.8rem;
		color: var(--text-muted);
	}
	.field {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 0 4px 0 8px;
		border: 1px solid var(--border);
		border-radius: 8px;
		background: var(--bg);
	}
	.preview {
		display: grid;
		color: var(--text-muted);
	}
	input {
		flex: 1;
		min-width: 0;
		padding: 7px 0;
		border: none;
		background: none;
		color: var(--text);
		font: inherit;
		outline: none;
	}
	input::placeholder {
		color: var(--text-muted);
	}
	.clear {
		display: grid;
		place-items: center;
		width: 28px;
		height: 28px;
		border: none;
		border-radius: 50%;
		background: none;
		color: var(--text-muted);
		cursor: pointer;
	}
	.clear:hover {
		background: var(--surface-2);
	}
	ul {
		position: absolute;
		top: 100%;
		left: 0;
		right: 0;
		z-index: 80;
		max-height: 280px;
		overflow-y: auto;
		margin: 4px 0 0;
		padding: 4px;
		list-style: none;
		background: var(--surface);
		border: 1px solid var(--border);
		border-radius: 10px;
		box-shadow: var(--shadow-pop);
	}
	li button {
		display: flex;
		align-items: center;
		gap: 10px;
		width: 100%;
		padding: 6px 8px;
		border: none;
		border-radius: 6px;
		background: none;
		color: var(--text);
		font: inherit;
		font-size: 0.9rem;
		text-align: left;
		cursor: pointer;
	}
	li[aria-selected='true'] button,
	li button:hover {
		background: var(--surface-2);
	}
	.muted {
		padding: 6px 8px;
		color: var(--text-muted);
		font-size: 0.9rem;
	}
</style>

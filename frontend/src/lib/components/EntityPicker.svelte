<script lang="ts">
	// Choix d'une entité par recherche (nom ou entity_id), comme le sélecteur
	// d'entité de HA. Utilisé par l'historique et les éditeurs visuels.
	import { cardName } from '../dashboard/model';
	import { byLabel } from '../format';
	import { live } from '../stores/live.svelte';

	interface Props {
		/** Valeur courante (mode champ) ; absent en mode « ajouter ». */
		value?: string;
		/** Domaines acceptés (`light`, `sensor`…) ; tous si vide. */
		domains?: string[];
		placeholder?: string;
		label?: string;
		onpick: (entityId: string) => void;
	}
	let { value, domains = [], placeholder = 'Entité', label, onpick }: Props = $props();

	let q = $state('');
	let open = $state(false);
	let active = $state(0);
	const listId = `ep-${Math.random().toString(36).slice(2, 8)}`;

	const all = $derived(
		[...live.states.values()]
			.filter((s) => !domains.length || domains.includes(s.entity_id.split('.')[0] ?? ''))
			.map((s) => ({ id: s.entity_id, name: cardName({ type: 'row', entity: s.entity_id }, s) }))
			.sort((a, b) => byLabel(a.name, b.name))
	);
	const matches = $derived.by(() => {
		const words = q.toLowerCase().split(/\s+/).filter(Boolean);
		return all
			.filter((e) => words.every((w) => e.name.toLowerCase().includes(w) || e.id.includes(w)))
			.slice(0, 30);
	});
	const current = $derived(value ? all.find((e) => e.id === value) : undefined);

	function pick(id: string) {
		onpick(id);
		q = '';
		open = false;
	}
	function key(e: KeyboardEvent) {
		if (e.key === 'ArrowDown') {
			active = Math.min(active + 1, matches.length - 1);
			open = true;
			e.preventDefault();
		} else if (e.key === 'ArrowUp') {
			active = Math.max(active - 1, 0);
			e.preventDefault();
		} else if (e.key === 'Enter' && open && matches[active]) {
			pick(matches[active].id);
			e.preventDefault();
		} else if (e.key === 'Escape') open = false;
	}
</script>

<div class="picker">
	{#if label}<span class="lbl">{label}</span>{/if}
	<input
		type="search"
		role="combobox"
		aria-expanded={open}
		aria-controls={listId}
		aria-label={label ?? placeholder}
		placeholder={current ? `${current.name} (${current.id})` : (value ?? placeholder)}
		bind:value={q}
		onfocus={() => (open = true)}
		oninput={() => {
			open = true;
			active = 0;
		}}
		onkeydown={key}
		onblur={() => setTimeout(() => (open = false), 150)}
	/>
	{#if open && matches.length}
		<ul id={listId} role="listbox">
			{#each matches as m, i (m.id)}
				<li role="option" aria-selected={i === active}>
					<button type="button" onmousedown={(e) => e.preventDefault()} onclick={() => pick(m.id)}>
						<span>{m.name}</span>
						<small>{m.id}</small>
					</button>
				</li>
			{/each}
		</ul>
	{/if}
</div>

<style>
	.picker {
		position: relative;
		/* largeur souple en ligne, sans imposer de hauteur dans une colonne */
		flex: 1 1 auto;
		min-width: min(240px, 100%);
		display: flex;
		flex-direction: column;
		gap: 4px;
	}
	.lbl {
		font-size: 0.8rem;
		color: var(--text-muted);
	}
	input {
		padding: 7px 10px;
		border: 1px solid var(--border);
		border-radius: 8px;
		background: var(--bg);
		color: var(--text);
		font: inherit;
	}
	input::placeholder {
		color: var(--text-muted);
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
		box-shadow: 0 8px 20px rgb(0 0 0 / 0.2);
	}
	li button {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		width: 100%;
		padding: 6px 8px;
		border: none;
		border-radius: 6px;
		background: none;
		color: var(--text);
		font: inherit;
		text-align: left;
		cursor: pointer;
	}
	li[aria-selected='true'] button,
	li button:hover {
		background: var(--surface-2);
	}
	small {
		color: var(--text-muted);
		font-size: 0.75rem;
	}
</style>

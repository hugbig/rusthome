<script lang="ts">
	// Nouveau projet ESPHome : à partir d'une carte du catalogue du Builder,
	// ou en important un fichier YAML existant (par exemple copié depuis
	// l'ESPHome de HA).
	import { request } from '../api';
	import Dialog from '../components/Dialog.svelte';
	import { validName } from '../esphome';

	interface Board {
		id: string;
		name: string;
		description?: string;
	}
	interface Props {
		onclose: () => void;
		oncreated: (configuration: string) => void;
	}
	let { onclose, oncreated }: Props = $props();

	let mode = $state<'board' | 'import'>('board');
	let name = $state('');
	let friendly = $state('');
	let query = $state('');
	let boards = $state<Board[]>([]);
	let board = $state<string>('');
	let content = $state('');
	let busy = $state(false);
	let error = $state<string | null>(null);

	// Recherche dans le catalogue (avec un léger délai).
	$effect(() => {
		const q = query.trim();
		const t = setTimeout(async () => {
			const r = await request<{ boards: Board[] }>(
				'GET',
				`esphome/boards${q ? `?q=${encodeURIComponent(q)}` : ''}`
			);
			if (r.ok) boards = r.data.boards;
		}, 250);
		return () => clearTimeout(t);
	});

	async function pick(e: Event) {
		const f = (e.currentTarget as HTMLInputElement).files?.[0];
		if (!f) return;
		content = await f.text();
		if (!name) {
			const m = /^\s*name:\s*["']?([a-z0-9-]+)/m.exec(content);
			name = m?.[1] ?? f.name.replace(/\.ya?ml$/, '').toLowerCase();
		}
	}

	const valid = $derived(
		validName(name) && (mode === 'board' ? board !== '' : content.trim() !== '')
	);

	async function create() {
		busy = true;
		error = null;
		const r = await request<{ configuration: string }>('POST', 'esphome/configs', {
			name,
			friendly_name: friendly || undefined,
			board_id: mode === 'board' ? board : undefined,
			content: mode === 'import' ? content : undefined
		});
		busy = false;
		if (r.ok) oncreated(r.data.configuration);
		else error = r.error;
	}
</script>

<Dialog title="Nouveau projet ESPHome" {onclose} width={620}>
	<div class="tabs" role="tablist">
		<button role="tab" aria-selected={mode === 'board'} onclick={() => (mode = 'board')}
			>Nouvelle carte</button
		>
		<button role="tab" aria-selected={mode === 'import'} onclick={() => (mode = 'import')}
			>Importer un YAML</button
		>
	</div>
	<label>
		<span>Nom d’hôte</span>
		<input
			bind:value={name}
			placeholder="salon-ir"
			aria-invalid={name !== '' && !validName(name)}
		/>
		<small>Minuscules, chiffres et tirets ; c’est aussi le nom du fichier.</small>
	</label>
	<label>
		<span>Nom affiché (facultatif)</span>
		<input bind:value={friendly} placeholder="Boîtier IR du salon" />
	</label>
	{#if mode === 'board'}
		<label>
			<span>Carte</span>
			<input
				type="search"
				bind:value={query}
				placeholder="Rechercher : lolin s2, esp32-s3, atom…"
			/>
		</label>
		<ul class="boards" role="listbox" aria-label="Cartes">
			{#each boards as b (b.id)}
				<li role="option" aria-selected={board === b.id}>
					<button onclick={() => (board = b.id)}>
						<strong>{b.name}</strong>
						<small>{b.id}</small>
					</button>
				</li>
			{:else}
				<li class="muted">Aucune carte.</li>
			{/each}
		</ul>
		<p class="note">
			Le Wi-Fi vient des secrets <code>wifi_ssid</code> et <code>wifi_password</code> du Builder.
		</p>
	{:else}
		<label>
			<span>Fichier YAML</span>
			<input type="file" accept=".yaml,.yml" onchange={pick} />
		</label>
		{#if content}<p class="note">{content.split('\n').length} lignes lues.</p>{/if}
	{/if}
	{#if error}<p class="err">{error}</p>{/if}
	{#snippet footer()}
		<button class="dbtn" onclick={onclose}>Annuler</button>
		<button class="dbtn primary" disabled={!valid || busy} onclick={create}
			>{busy ? 'Création…' : 'Créer'}</button
		>
	{/snippet}
</Dialog>

<style>
	.tabs {
		display: flex;
		gap: 4px;
		padding: 4px;
		border-radius: 12px;
		background: var(--surface-2);
	}
	.tabs button {
		flex: 1;
		padding: 8px;
		border: none;
		border-radius: 9px;
		background: none;
		color: var(--text-muted);
		font: inherit;
		cursor: pointer;
	}
	.tabs [aria-selected='true'] {
		background: var(--surface);
		color: var(--text);
		box-shadow: var(--shadow);
	}
	label {
		display: flex;
		flex-direction: column;
		gap: 4px;
	}
	label > span {
		font-size: 0.8rem;
		color: var(--text-muted);
	}
	small,
	.note,
	.muted {
		color: var(--text-muted);
		font-size: 0.8rem;
	}
	input:not([type='file']) {
		padding: 8px 10px;
		border: 1px solid var(--border);
		border-radius: 8px;
		background: var(--bg);
		color: var(--text);
		font: inherit;
	}
	.boards {
		max-height: 240px;
		overflow-y: auto;
		margin: 0;
		padding: 4px;
		list-style: none;
		border: 1px solid var(--border);
		border-radius: 10px;
	}
	.boards button {
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
	.boards [aria-selected='true'] button,
	.boards button:hover {
		background: var(--surface-2);
	}
	.boards [aria-selected='true'] button {
		box-shadow: inset 3px 0 0 var(--primary);
	}
	p {
		margin: 0;
	}
	.err {
		color: var(--critical);
	}
</style>

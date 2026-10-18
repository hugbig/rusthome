<script lang="ts">
	// Secrets du Builder (`!secret wifi_password`…) : seuls les noms sont
	// lus ; une valeur se remplace sans jamais être affichée.
	import { onMount } from 'svelte';
	import { request } from '../api';
	import Dialog from '../components/Dialog.svelte';

	let { onclose }: { onclose: () => void } = $props();

	let keys = $state<string[]>([]);
	let key = $state('');
	let value = $state('');
	let message = $state<string | null>(null);
	let error = $state<string | null>(null);
	const validKey = $derived(/^[A-Za-z0-9_]{1,64}$/.test(key));

	async function load() {
		const r = await request<string[]>('GET', 'esphome/secrets');
		if (r.ok) keys = r.data;
		else error = r.error;
	}
	onMount(load);

	async function save() {
		error = null;
		const r = await request<{ created: boolean }>(
			'PUT',
			`esphome/secrets/${encodeURIComponent(key)}`,
			{ value }
		);
		if (!r.ok) {
			error = r.error;
			return;
		}
		message = r.data.created ? `Secret « ${key} » ajouté.` : `Secret « ${key} » remplacé.`;
		value = '';
		await load();
	}
</script>

<Dialog title="Secrets ESPHome" {onclose}>
	<p class="note">
		Valeurs gardées par le Builder (fichier <code>secrets.yaml</code>), jamais affichées ni
		versionnées. Dans un YAML : <code>password: !secret wifi_password</code>.
	</p>
	<ul>
		{#each keys as k (k)}
			<li><button class="link" onclick={() => (key = k)}><code>{k}</code></button></li>
		{:else}
			<li class="note">Aucun secret.</li>
		{/each}
	</ul>
	<div class="form">
		<label><span>Nom</span><input bind:value={key} placeholder="wifi_password" /></label>
		<label
			><span>Nouvelle valeur</span><input
				type="password"
				autocomplete="new-password"
				bind:value
			/></label
		>
		<button class="dbtn primary" disabled={!validKey || !value} onclick={save}>Enregistrer</button>
	</div>
	{#if message}<p role="status">{message}</p>{/if}
	{#if error}<p class="err">{error}</p>{/if}
	{#snippet footer()}
		<button class="dbtn" onclick={onclose}>Fermer</button>
	{/snippet}
</Dialog>

<style>
	p {
		margin: 0;
	}
	.note {
		color: var(--text-muted);
		font-size: 0.85rem;
	}
	ul {
		display: flex;
		flex-wrap: wrap;
		gap: 6px;
		margin: 0;
		padding: 0;
		list-style: none;
	}
	.link {
		padding: 4px 8px;
		border: 1px solid var(--border);
		border-radius: 8px;
		background: var(--surface-2);
		color: var(--text);
		cursor: pointer;
	}
	.form {
		display: flex;
		gap: 8px;
		align-items: flex-end;
		flex-wrap: wrap;
	}
	label {
		display: flex;
		flex-direction: column;
		gap: 4px;
		flex: 1 1 180px;
	}
	label > span {
		font-size: 0.8rem;
		color: var(--text-muted);
	}
	input {
		padding: 8px 10px;
		border: 1px solid var(--border);
		border-radius: 8px;
		background: var(--bg);
		color: var(--text);
		font: inherit;
	}
	.err {
		color: var(--critical);
	}
</style>

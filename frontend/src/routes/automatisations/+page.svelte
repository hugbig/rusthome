<script lang="ts">
	import { onMount } from 'svelte';
	import { resolve } from '$app/paths';
	import { request } from '#lib/api.ts';
	import { byLabel, relativeTime } from '#lib/format.ts';
	import { session } from '#lib/stores/session.svelte.ts';
	import type { AutomationInfo } from '#lib/types.ts';

	let items = $state<AutomationInfo[]>([]);
	let error = $state<string | null>(null);
	let notice = $state<string | null>(null);
	let busy = $state(false);
	let q = $state('');
	let filter = $state<'all' | 'enabled' | 'disabled' | 'problems'>('all');
	const isAdmin = $derived(session.me?.role === 'admin');

	async function load() {
		const r = await request<AutomationInfo[]>('GET', 'automations');
		if (r.ok) {
			items = r.data;
			error = null;
		} else error = r.error;
	}
	onMount(load);

	const shown = $derived(
		items
			.filter((a) => {
				if (filter === 'enabled' && !a.enabled) return false;
				if (filter === 'disabled' && a.enabled) return false;
				if (filter === 'problems' && a.valid && a.issues.length === 0) return false;
				return !q || a.alias.toLowerCase().includes(q.trim().toLowerCase());
			})
			.sort((a, b) => byLabel(a.alias, b.alias))
	);
	const counts = $derived({
		enabled: items.filter((a) => a.enabled).length,
		problems: items.filter((a) => !a.valid || a.issues.length > 0).length
	});

	async function toggle(a: AutomationInfo) {
		const r = await request<AutomationInfo>(
			'POST',
			`automations/${encodeURIComponent(a.id)}/${a.enabled ? 'disable' : 'enable'}`
		);
		if (r.ok) items = items.map((x) => (x.id === a.id ? r.data : x));
		else notice = r.error;
	}

	async function importHa() {
		busy = true;
		notice = null;
		const r = await request<{ imported: number; invalid: number; skipped_existing: number }>(
			'POST',
			'automations/import-ha'
		);
		busy = false;
		if (r.ok) {
			notice = `${r.data.imported} importées (désactivées), ${r.data.skipped_existing} déjà présentes, ${r.data.invalid} à corriger.`;
			await load();
		} else notice = r.error;
	}
</script>

<div class="head">
	<h1>Automatisations <small>{counts.enabled} actives / {items.length}</small></h1>
	{#if isAdmin}
		<div class="actions">
			<a class="button" href={resolve('/automatisations/[id]', { id: 'nouveau' })}>Nouvelle</a>
			<button onclick={importHa} disabled={busy}
				>{busy ? 'Import…' : 'Importer depuis Home Assistant'}</button
			>
		</div>
	{/if}
</div>

{#if notice}<p class="notice card">{notice}</p>{/if}
{#if error}<p class="notice card">Automatisations indisponibles : {error}</p>{/if}

<div class="filters card">
	<input type="search" placeholder="Rechercher…" bind:value={q} aria-label="Recherche" />
	<div class="seg" role="group" aria-label="Filtre">
		{#each [['all', 'Toutes'], ['enabled', 'Actives'], ['disabled', 'Désactivées'], ['problems', `À corriger (${counts.problems})`]] as [k, label] (k)}
			<button aria-pressed={filter === k} onclick={() => (filter = k as typeof filter)}
				>{label}</button
			>
		{/each}
	</div>
</div>

<section class="card">
	<table>
		<thead>
			<tr><th>Automatisation</th><th>Dernier déclenchement</th><th class="right">Active</th></tr>
		</thead>
		<tbody>
			{#each shown as a (a.id)}
				<tr>
					<td>
						<a href={resolve('/automatisations/[id]', { id: a.id })}>{a.alias}</a>
						{#if !a.valid}<span class="tag bad">erreur</span>
						{:else if a.issues.length}<span class="tag warn">à vérifier</span>{/if}
						{#if a.running}<span class="tag run">en cours</span>{/if}
						<code>{a.entity_id}</code>
					</td>
					<td class="muted">{a.last_triggered ? relativeTime(a.last_triggered) : 'jamais'}</td>
					<td class="right">
						<button
							class="switch"
							role="switch"
							aria-checked={a.enabled}
							aria-label="Activer {a.alias}"
							disabled={!isAdmin || !a.valid}
							onclick={() => toggle(a)}><span></span></button
						>
					</td>
				</tr>
			{:else}
				<tr><td colspan="3" class="muted">Aucune automatisation.</td></tr>
			{/each}
		</tbody>
	</table>
</section>

<style>
	.head {
		display: flex;
		flex-wrap: wrap;
		justify-content: space-between;
		align-items: center;
		gap: 8px;
	}
	small {
		color: var(--text-muted);
		font-weight: normal;
		font-size: 0.9rem;
	}
	.actions {
		display: flex;
		gap: 8px;
	}
	.button,
	.actions button {
		padding: 6px 14px;
		border-radius: 8px;
		border: 1px solid var(--border);
		background: var(--surface-2);
		color: var(--text);
		text-decoration: none;
		cursor: pointer;
		font: inherit;
	}
	.button {
		background: var(--primary);
		color: var(--on-primary);
		border-color: var(--primary);
	}
	.notice {
		font-size: 0.9rem;
	}
	.filters {
		display: flex;
		flex-wrap: wrap;
		gap: 8px;
		align-items: center;
	}
	input {
		flex: 1 1 220px;
		min-width: 0;
		padding: 6px 10px;
		border: 1px solid var(--border);
		border-radius: 8px;
		background: var(--bg);
		color: var(--text);
		font: inherit;
	}
	.seg {
		display: flex;
		flex-wrap: wrap;
		gap: 4px;
	}
	.seg button {
		padding: 4px 10px;
		border-radius: 8px;
		border: 1px solid var(--border);
		background: var(--surface-2);
		color: var(--text);
		cursor: pointer;
	}
	.seg button[aria-pressed='true'] {
		background: var(--primary);
		color: var(--on-primary);
		border-color: var(--primary);
	}
	table {
		width: 100%;
		border-collapse: collapse;
	}
	th {
		text-align: left;
		font-size: 0.8rem;
		color: var(--text-muted);
		font-weight: 600;
		padding: 6px 4px;
	}
	td {
		padding: 8px 4px;
		border-top: 1px solid var(--border);
		vertical-align: middle;
	}
	td a {
		color: var(--text);
		font-weight: 500;
	}
	code {
		display: block;
		font-size: 0.75rem;
		color: var(--text-muted);
		overflow-wrap: anywhere;
	}
	.tag {
		margin-left: 6px;
		font-size: 0.7rem;
		padding: 1px 6px;
		border-radius: 999px;
		background: var(--surface-2);
	}
	.tag.bad {
		background: #b3261e;
		color: #fff;
	}
	.tag.warn {
		background: #8a5a00;
		color: #fff;
	}
	.tag.run {
		background: var(--primary);
		color: var(--on-primary);
	}
	.right {
		text-align: right;
	}
	.muted {
		color: var(--text-muted);
		font-size: 0.9rem;
	}
	.switch {
		width: 40px;
		height: 22px;
		border-radius: 999px;
		border: 1px solid var(--border);
		background: var(--surface-2);
		position: relative;
		cursor: pointer;
		padding: 0;
	}
	.switch span {
		position: absolute;
		top: 2px;
		left: 2px;
		width: 16px;
		height: 16px;
		border-radius: 50%;
		background: var(--text-muted);
		transition: left 0.15s;
	}
	.switch[aria-checked='true'] {
		background: var(--primary);
		border-color: var(--primary);
	}
	.switch[aria-checked='true'] span {
		left: 20px;
		background: #fff;
	}
	.switch:disabled {
		opacity: 0.4;
		cursor: default;
	}
</style>

<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { request } from '#lib/api.ts';
	import Icon from '#lib/dashboard/Icon.svelte';
	import { iconPath } from '#lib/dashboard/icons.ts';
	import type { DashboardSummary } from '#lib/dashboard/model.ts';
	import { byLabel, plural } from '#lib/format.ts';
	import { dashboards } from '#lib/stores/dashboards.svelte.ts';
	import { session } from '#lib/stores/session.svelte.ts';

	let items = $state<DashboardSummary[]>([]);
	let error = $state<string | null>(null);
	let notice = $state<string | null>(null);
	let busy = $state(false);
	let newTitle = $state('');
	const isAdmin = $derived(session.me?.role === 'admin');
	const canCreate = $derived(session.me?.role === 'admin' || session.me?.role === 'resident');

	async function load() {
		const r = await request<DashboardSummary[]>('GET', 'dashboards');
		if (r.ok) {
			items = r.data.sort((a, b) => byLabel(a.title, b.title));
			error = null;
		} else error = r.error;
	}
	onMount(load);

	async function importHa() {
		busy = true;
		notice = null;
		const r = await request<{
			imported: string[];
			skipped_existing: string[];
			invalid: string[];
			issues: number;
		}>('POST', 'dashboards/import-ha');
		busy = false;
		if (r.ok) {
			const { imported, skipped_existing, invalid, issues } = r.data;
			notice = [
				`${imported.length} importé(s)`,
				skipped_existing.length ? `déjà présents : ${skipped_existing.join(', ')}` : '',
				invalid.length ? `adresse invalide : ${invalid.join(', ')}` : '',
				issues ? `${plural(issues, 'point')} à revoir` : ''
			]
				.filter(Boolean)
				.join(' · ');
			await load();
			void dashboards.load();
		} else notice = r.error;
	}

	/** « Salon & cuisine » → « salon-cuisine ». */
	const slug = (t: string) =>
		t
			.normalize('NFD')
			.replace(/[̀-ͯ]/g, '')
			.toLowerCase()
			.replace(/[^a-z0-9]+/g, '-')
			.replace(/^-+|-+$/g, '')
			.slice(0, 60);

	async function create(ev: SubmitEvent) {
		ev.preventDefault();
		const url_path = slug(newTitle);
		if (!url_path) return;
		const doc = {
			url_path,
			title: newTitle.trim(),
			views: [{ type: 'sections', title: 'Accueil', max_columns: 3, sections: [{ cards: [] }] }]
		};
		const r = await request('POST', 'dashboards', { yaml: JSON.stringify(doc) });
		if (r.ok) {
			void dashboards.load();
			await goto(resolve('/tableaux/[dashboard]/[[vue]]', { dashboard: url_path, vue: undefined }));
		} else notice = r.error;
	}
</script>

<div class="head">
	<h1>Tableaux de bord</h1>
	{#if isAdmin}
		<button class="dbtn" onclick={importHa} disabled={busy}
			>{busy ? 'Import…' : 'Importer depuis Home Assistant'}</button
		>
	{/if}
</div>

{#if notice}<p class="card">{notice}</p>{/if}
{#if error}<p class="card">Tableaux indisponibles : {error}</p>{/if}

<ul class="grid">
	{#each items as d (d.url_path)}
		<li>
			<a
				class="card tile"
				href={resolve('/tableaux/[dashboard]/[[vue]]', { dashboard: d.url_path, vue: undefined })}
			>
				<span class="ic"><Icon path={iconPath(d.icon ?? 'mdi:view-dashboard')} /></span>
				<span class="txt">
					<strong>{d.title}</strong>
					<small
						>{plural(d.views, 'vue')} · <code>{d.url_path}</code>{#if d.kiosk}
							· kiosque{/if}{#if d.require_admin}
							· admin{/if}</small
					>
				</span>
			</a>
		</li>
	{:else}
		<li class="muted">
			Aucun tableau de bord. {#if isAdmin}Importez ceux de Home Assistant ou créez-en un.{/if}
		</li>
	{/each}
</ul>

{#if canCreate}
	<form class="card create" onsubmit={create}>
		<label for="new-title">Nouveau tableau de bord</label>
		<input id="new-title" bind:value={newTitle} placeholder="Titre, par ex. Salon" required />
		<button class="dbtn primary" disabled={!slug(newTitle)}>Créer</button>
	</form>
{/if}

<style>
	.head {
		display: flex;
		flex-wrap: wrap;
		justify-content: space-between;
		align-items: center;
		gap: 8px;
	}
	.grid {
		list-style: none;
		padding: 0;
		margin: 0 0 16px;
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
		gap: 12px;
	}
	.tile {
		display: flex;
		align-items: center;
		gap: 12px;
		margin: 0;
		text-decoration: none;
		color: var(--text);
	}
	.tile:hover {
		border-color: var(--primary);
	}
	.ic {
		display: grid;
		place-items: center;
		width: 44px;
		height: 44px;
		border-radius: 12px;
		background: var(--primary);
		color: var(--on-primary);
		flex: none;
	}
	.txt {
		display: flex;
		flex-direction: column;
		min-width: 0;
	}
	small {
		color: var(--text-muted);
	}
	.muted {
		color: var(--text-muted);
	}
	.create {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 8px;
	}
	.create input {
		flex: 1 1 200px;
		padding: 6px 10px;
		border: 1px solid var(--border);
		border-radius: 8px;
		background: var(--bg);
		color: var(--text);
		font: inherit;
	}
</style>

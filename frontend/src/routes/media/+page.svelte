<script lang="ts">
	// Navigateur multimédia, comme le panneau « Média » de HA : caméras en
	// direct, Frigate (événements, clips, instantanés, enregistrements), sources
	// de HA (médias locaux…) et lecteurs. L'état est dans l'URL : le bouton
	// « retour » du navigateur remonte d'un niveau.
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import {
		mdiCctv,
		mdiFilmstripBox,
		mdiFolderMultipleImage,
		mdiMotionPlay,
		mdiSpeaker
	} from '@mdi/js';
	import Icon from '#lib/dashboard/Icon.svelte';
	import CameraWall from '#lib/media/CameraWall.svelte';
	import FrigateBrowser from '#lib/media/FrigateBrowser.svelte';
	import HaBrowser from '#lib/media/HaBrowser.svelte';
	import Players from '#lib/media/Players.svelte';

	const SOURCES = [
		{ key: 'cameras', label: 'Caméras', text: 'Images en direct', icon: mdiCctv },
		{
			key: 'frigate',
			label: 'Frigate',
			text: 'Événements, clips, instantanés et enregistrements',
			icon: mdiMotionPlay
		},
		{
			key: 'videos',
			label: 'Vidéos enregistrées',
			text: 'Vidéos et sons gardés dans les médias locaux',
			icon: mdiFilmstripBox
		},
		{
			key: 'ha',
			label: 'Médias de Home Assistant',
			text: 'Médias locaux et autres sources',
			icon: mdiFolderMultipleImage
		},
		{ key: 'lecteurs', label: 'Lecteurs', text: 'Enceintes, TV, Spotify…', icon: mdiSpeaker }
	] as const;

	const params = $derived(page.url.searchParams);
	const source = $derived(params.get('source') ?? '');
	const current = $derived(SOURCES.find((s) => s.key === source));

	/** Change les paramètres de l'URL (une entrée d'historique par niveau). */
	function navigate(p: Record<string, string | undefined>) {
		const q: Record<string, string> = Object.fromEntries(page.url.searchParams);
		for (const [k, v] of Object.entries(p)) {
			if (v === undefined) delete q[k];
			else q[k] = v;
		}
		const s = new URLSearchParams(q).toString();
		// eslint-disable-next-line svelte/no-navigation-without-resolve -- chemin résolu, seule la requête change
		void goto(`${resolve('/media')}${s ? `?${s}` : ''}`);
	}
</script>

<div class="head">
	<h1>
		{#if current}<a href={resolve('/media')}>Média</a> <span class="sep">›</span>
			{current.label}{:else}Média{/if}
	</h1>
</div>

{#if !current}
	<div class="sources">
		{#each SOURCES as s (s.key)}
			<button class="card src" onclick={() => navigate({ source: s.key })}>
				<span class="ic"><Icon path={s.icon} size={30} /></span>
				<span class="txt"><strong>{s.label}</strong><small>{s.text}</small></span>
			</button>
		{/each}
	</div>
{:else if source === 'cameras'}
	<CameraWall />
{:else if source === 'frigate'}
	<FrigateBrowser
		camera={params.get('camera') ?? undefined}
		tab={params.get('onglet') === 'enregistrements'
			? 'recordings'
			: params.get('onglet') === 'instantanes'
				? 'snapshots'
				: 'clips'}
		onnavigate={navigate}
	/>
{:else if source === 'ha'}
	<HaBrowser id={params.get('id') ?? undefined} onnavigate={navigate} />
{:else if source === 'videos'}
	<!-- les médias locaux de HA (dossier « My media ») -->
	<HaBrowser
		id={params.get('id') ?? 'media-source://media_source'}
		root="media-source://media_source"
		onnavigate={navigate}
	/>
{:else}
	<Players />
{/if}

<style>
	h1 a {
		color: var(--text-muted);
		text-decoration: none;
	}
	.sep {
		color: var(--text-muted);
		margin: 0 4px;
	}
	.sources {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
		gap: 14px;
	}
	.src {
		display: flex;
		align-items: center;
		gap: 16px;
		margin: 0;
		padding: 20px;
		color: var(--text);
		font: inherit;
		text-align: left;
		cursor: pointer;
		transition: border-color 0.15s;
	}
	.src:hover {
		border-color: var(--primary);
	}
	.ic {
		display: grid;
		place-items: center;
		width: 56px;
		height: 56px;
		border-radius: 16px;
		background: color-mix(in srgb, var(--primary) 16%, transparent);
		color: var(--primary-text);
		flex: none;
	}
	.txt {
		display: flex;
		flex-direction: column;
		gap: 2px;
	}
	small {
		color: var(--text-muted);
	}
</style>

<script lang="ts">
	// Sources multimédias de HA (médias locaux, Frigate via HA, radio…),
	// parcourues en lecture seule ; la lecture passe par RustHome.
	import {
		mdiFileOutline,
		mdiFolder,
		mdiImage,
		mdiMovieOpen,
		mdiMusic,
		mdiPlayCircleOutline
	} from '@mdi/js';
	import { request } from '../api';
	import Icon from '../dashboard/Icon.svelte';
	import { haPlay, haThumb, kindOf, prettyFileName, type Kind } from './media';
	import MediaViewer from './MediaViewer.svelte';

	interface Item {
		title: string;
		media_content_id: string;
		media_content_type?: string;
		media_class?: string;
		can_play?: boolean;
		can_expand?: boolean;
		thumbnail?: string | null;
		children?: Item[];
	}
	interface Props {
		id?: string;
		/** Dossier de départ (pas de bouton retour à ce niveau). */
		root?: string;
		onnavigate: (params: Record<string, string | undefined>) => void;
	}
	let { id, root, onnavigate }: Props = $props();

	let node = $state<Item | null>(null);
	let error = $state<string | null>(null);
	let loading = $state(false);
	let viewing = $state<{ title: string; kind: 'video' | 'audio' | 'image'; src: string } | null>(
		null
	);

	$effect(() => {
		const target = id;
		loading = true;
		void request<Item>(
			'GET',
			`media/ha/browse${target ? `?id=${encodeURIComponent(target)}` : ''}`
		).then((r) => {
			loading = false;
			if (r.ok) {
				node = r.data;
				error = null;
			} else error = r.error;
		});
	});

	const ICON: Record<Kind, string> = {
		folder: mdiFolder,
		video: mdiMovieOpen,
		audio: mdiMusic,
		image: mdiImage,
		other: mdiFileOutline
	};

	/** Dossiers d'abord, puis fichiers du plus récent au plus ancien (noms datés). */
	function sorted(items: Item[]): Item[] {
		return [...items].sort((a, b) => {
			const fa = a.can_expand ? 0 : 1;
			const fb = b.can_expand ? 0 : 1;
			if (fa !== fb) return fa - fb;
			return fa === 0 ? a.title.localeCompare(b.title, 'fr') : b.title.localeCompare(a.title, 'fr');
		});
	}

	function open(item: Item) {
		const k = kindOf(item);
		if (k === 'folder') onnavigate({ id: item.media_content_id });
		else if (item.can_play && k !== 'other')
			viewing = { title: prettyFileName(item.title), kind: k, src: haPlay(item.media_content_id) };
		else if (item.can_play)
			viewing = { title: item.title, kind: 'video', src: haPlay(item.media_content_id) };
	}
</script>

{#if error}
	<p class="card">Médias indisponibles : {error}</p>
{:else if loading && !node}
	<p class="muted">Chargement…</p>
{:else if node}
	<div class="where">
		{#if id && id !== root}<button class="dbtn" onclick={() => history.back()}>← Retour</button
			>{/if}
		<h2>{node.title}</h2>
	</div>
	<div class="grid">
		{#each sorted(node.children ?? []) as c (c.media_content_id)}
			{@const k = kindOf(c)}
			{@const thumb = haThumb(c.thumbnail)}
			<button
				class="item"
				class:folder={k === 'folder'}
				onclick={() => open(c)}
				disabled={k !== 'folder' && !c.can_play}
			>
				<span class="pic">
					{#if thumb}
						<img src={thumb} alt="" loading="lazy" />
					{:else}
						<Icon path={ICON[k]} size={40} />
					{/if}
					{#if c.can_play && k !== 'folder'}<span class="play"
							><Icon path={mdiPlayCircleOutline} size={36} /></span
						>{/if}
				</span>
				<span class="title" title={c.title}
					>{k === 'folder' ? c.title : prettyFileName(c.title)}</span
				>
			</button>
		{:else}
			<p class="muted">Dossier vide.</p>
		{/each}
	</div>
{/if}

{#if viewing}
	<MediaViewer {...viewing} onclose={() => (viewing = null)} />
{/if}

<style>
	.where {
		display: flex;
		align-items: center;
		gap: 12px;
		margin-bottom: 12px;
	}
	.where h2 {
		margin: 0;
	}
	.grid {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(170px, 1fr));
		gap: 12px;
	}
	.item {
		display: flex;
		flex-direction: column;
		padding: 0;
		border: 1px solid var(--border);
		border-radius: var(--radius);
		overflow: hidden;
		background: var(--surface);
		color: var(--text);
		font: inherit;
		text-align: left;
		cursor: pointer;
		box-shadow: var(--shadow);
	}
	.item:disabled {
		opacity: 0.5;
		cursor: default;
	}
	.pic {
		position: relative;
		display: grid;
		place-items: center;
		aspect-ratio: 16 / 10;
		background: var(--surface-2);
		color: var(--text-muted);
	}
	.folder .pic {
		color: var(--primary-text);
	}
	.pic img {
		width: 100%;
		height: 100%;
		object-fit: cover;
	}
	.play {
		position: absolute;
		inset: 0;
		display: grid;
		place-items: center;
		color: #fff;
		background: rgb(0 0 0 / 0.25);
		opacity: 0;
		transition: opacity 0.15s;
	}
	.item:hover .play {
		opacity: 1;
	}
	.title {
		padding: 8px 10px;
		font-size: 0.88rem;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.muted {
		color: var(--text-muted);
	}
</style>

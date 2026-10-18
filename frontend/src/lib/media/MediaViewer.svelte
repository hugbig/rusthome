<script lang="ts">
	// Lecteur plein écran : vidéo, son ou image, avec téléchargement.
	import { mdiDownload } from '@mdi/js';
	import Icon from '../dashboard/Icon.svelte';
	import { UI } from '../dashboard/icons';

	interface Props {
		title: string;
		subtitle?: string;
		kind: 'video' | 'audio' | 'image';
		/** Fichier à télécharger (et à lire faute de flux HLS). */
		src: string;
		/** Flux HLS lu de préférence (vidéos Frigate). */
		hls?: string;
		/** Image affichée avant la lecture d'une vidéo. */
		poster?: string;
		/** Vidéo indisponible : image de repli (instantané Frigate). */
		fallback?: string;
		onclose: () => void;
	}
	let { title, subtitle, kind, src, hls, poster, fallback, onclose }: Props = $props();
	let failed = $state(false);
	let video = $state<HTMLVideoElement>();

	// HLS : natif (Safari) ou via hls.js, chargé seulement à l'ouverture d'une
	// vidéo ; sans flux HLS, le fichier est lu directement.
	$effect(() => {
		const el = video;
		if (!el || kind !== 'video') return;
		if (!hls) {
			el.src = src;
			return;
		}
		let destroy: (() => void) | undefined;
		let cancelled = false;
		void import('hls.js').then(({ default: Hls }) => {
			if (cancelled) return;
			if (Hls.isSupported()) {
				const h = new Hls();
				h.on(Hls.Events.ERROR, (_, d) => {
					if (d.fatal) failed = true;
				});
				h.loadSource(hls);
				h.attachMedia(el);
				destroy = () => h.destroy();
			} else if (el.canPlayType('application/vnd.apple.mpegurl')) {
				el.src = hls;
			} else {
				el.src = src;
			}
		});
		return () => {
			cancelled = true;
			destroy?.();
		};
	});
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && onclose()} />

<div class="backdrop" role="presentation" onclick={onclose}></div>
<div class="viewer" role="dialog" aria-modal="true" aria-label={title}>
	<div class="bar">
		<div class="t">
			<strong>{title}</strong>
			{#if subtitle}<span>{subtitle}</span>{/if}
		</div>
		<!-- Fichier servi par l'API RustHome : téléchargement direct voulu. -->
		<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -->
		<a class="btn" href={src} download aria-label="Télécharger"><Icon path={mdiDownload} /></a>
		<button class="btn" onclick={onclose} aria-label="Fermer"><Icon path={UI.close} /></button>
	</div>
	<div class="stage">
		{#if kind === 'video' && !failed}
			<video
				bind:this={video}
				{poster}
				controls
				autoplay
				playsinline
				onerror={() => (failed = true)}
			></video>
		{:else if kind === 'video' && fallback}
			<img src={fallback} alt={title} />
			<p class="note">Vidéo indisponible : instantané affiché.</p>
		{:else if kind === 'audio'}
			<audio {src} controls autoplay></audio>
		{:else if kind === 'image'}
			<img {src} alt={title} />
		{:else}
			<p class="note">Lecture impossible.</p>
		{/if}
	</div>
</div>

<style>
	.backdrop {
		position: fixed;
		inset: 0;
		background: rgb(0 0 0 / 0.75);
		backdrop-filter: blur(3px);
		z-index: 100;
	}
	.viewer {
		position: fixed;
		z-index: 101;
		left: 50%;
		top: 50%;
		transform: translate(-50%, -50%);
		width: min(1100px, calc(100vw - 24px));
		max-height: 92vh;
		display: flex;
		flex-direction: column;
		background: var(--surface);
		border: 1px solid var(--border);
		border-radius: 20px;
		box-shadow: var(--shadow-pop);
		overflow: hidden;
	}
	.bar {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 10px 12px 10px 18px;
	}
	.t {
		flex: 1;
		min-width: 0;
		display: flex;
		flex-direction: column;
	}
	.t span {
		font-size: 0.85rem;
		color: var(--text-muted);
	}
	.btn {
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
	.btn:hover {
		background: var(--surface-2);
	}
	.stage {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		background: #000;
		min-height: 200px;
	}
	video,
	img {
		display: block;
		max-width: 100%;
		max-height: calc(92vh - 70px);
	}
	audio {
		width: min(560px, 90%);
		margin: 40px 0;
	}
	.note {
		color: #ccc;
		font-size: 0.85rem;
	}
</style>

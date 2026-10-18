<script lang="ts">
	// Mur de caméras : images en direct (rafraîchies toutes les 2 s quand la
	// page est visible) ; un clic ouvre la caméra en grand.
	import { onDestroy, onMount } from 'svelte';
	import { cardName } from '../dashboard/model';
	import { byLabel } from '../format';
	import { live } from '../stores/live.svelte';
	import MediaViewer from './MediaViewer.svelte';

	let tick = $state(Date.now());
	let timer: ReturnType<typeof setInterval> | undefined;
	let open = $state<string | null>(null);
	let broken = $state<Record<string, boolean>>({});

	onMount(() => {
		timer = setInterval(() => {
			if (document.visibilityState === 'visible') tick = Date.now();
		}, 2000);
	});
	onDestroy(() => clearInterval(timer));

	const cams = $derived(
		[...live.states.values()]
			.filter((s) => s.entity_id.startsWith('camera.') && s.state !== 'unavailable')
			.map((s) => ({ id: s.entity_id, name: cardName({ type: 'row', entity: s.entity_id }, s) }))
			.sort((a, b) => byLabel(a.name, b.name))
	);
	const src = (id: string) => `/api/v1/cameras/${encodeURIComponent(id)}/snapshot?t=${tick}`;
</script>

{#if cams.length === 0}
	<p class="muted">Aucune caméra disponible.</p>
{/if}
<div class="wall">
	{#each cams as c (c.id)}
		<button class="cam" onclick={() => (open = c.id)}>
			{#if broken[c.id]}
				<span class="none">Image indisponible</span>
			{:else}
				<img
					src={src(c.id)}
					alt="Caméra {c.name}"
					onerror={() => (broken = { ...broken, [c.id]: true })}
				/>
			{/if}
			<span class="name"><span class="live"></span>{c.name}</span>
		</button>
	{/each}
</div>

{#if open}
	<MediaViewer
		title={cams.find((c) => c.id === open)?.name ?? open}
		subtitle="En direct (image toutes les 2 s)"
		kind="image"
		src={src(open)}
		onclose={() => (open = null)}
	/>
{/if}

<style>
	.wall {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(320px, 1fr));
		gap: 12px;
	}
	.cam {
		position: relative;
		padding: 0;
		border: 1px solid var(--border);
		border-radius: var(--radius);
		overflow: hidden;
		background: #000;
		cursor: pointer;
		box-shadow: var(--shadow);
	}
	img,
	.none {
		display: block;
		width: 100%;
		aspect-ratio: 16 / 9;
		object-fit: cover;
	}
	.none {
		display: grid;
		place-items: center;
		color: #aaa;
		font-size: 0.85rem;
	}
	.name {
		position: absolute;
		left: 0;
		right: 0;
		bottom: 0;
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 18px 12px 8px;
		background: linear-gradient(transparent, rgb(0 0 0 / 0.7));
		color: #fff;
		font-size: 0.9rem;
		text-align: left;
	}
	.live {
		width: 8px;
		height: 8px;
		border-radius: 50%;
		background: #e53935;
		box-shadow: 0 0 0 3px rgb(229 57 53 / 0.3);
	}
	.muted {
		color: var(--text-muted);
	}
</style>

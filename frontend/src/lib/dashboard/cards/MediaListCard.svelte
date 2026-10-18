<script lang="ts">
	// Derniers films / séries (capteur au format « upcoming media » : attribut
	// `data`, liste JSON dont le premier élément porte les valeurs par défaut).
	import { live } from '../../stores/live.svelte';
	import { mediaItems } from '../data';
	import type { Card } from '../model';

	let { card }: { card: Card } = $props();

	const s = $derived(card.entity ? live.states.get(card.entity) : undefined);
	const max = $derived(typeof card.max === 'number' ? card.max : 5);
	const items = $derived(mediaItems(s?.attributes.data).slice(0, max));
	let broken = $state<Record<number, boolean>>({});
	// Images servies par RustHome (la page reste en HTTPS, Emby est en HTTP).
	const imageUrl = (i: number) =>
		`/api/v1/media/${encodeURIComponent(card.entity ?? '')}/image?i=${i}`;
</script>

<div class="dcard">
	{#if typeof card.title === 'string' && card.title}<h3>{card.title}</h3>{/if}
	{#if items.length === 0}
		<p class="muted">Rien pour l’instant.</p>
	{:else}
		<ul>
			{#each items as it, i (i)}
				<li>
					{#if it.hasImage && !broken[it.index]}
						<img
							src={imageUrl(it.index)}
							alt=""
							loading="lazy"
							onerror={() => (broken = { ...broken, [it.index]: true })}
						/>
					{/if}
					<div>
						<strong>{it.title}</strong>
						{#each it.lines as l, k (k)}<span class="muted">{l}</span>{/each}
					</div>
				</li>
			{/each}
		</ul>
	{/if}
</div>

<style>
	ul {
		list-style: none;
		margin: 0;
		padding: 0;
		display: flex;
		flex-direction: column;
		gap: 10px;
	}
	li {
		display: flex;
		gap: 10px;
		align-items: flex-start;
	}
	img {
		width: 112px;
		aspect-ratio: 16 / 9;
		object-fit: cover;
		border-radius: 6px;
		flex: none;
		background: var(--surface-2);
	}
	li div {
		display: flex;
		flex-direction: column;
		min-width: 0;
		gap: 2px;
	}
	strong {
		font-size: 0.9rem;
	}
	span {
		font-size: 0.8rem;
	}
</style>

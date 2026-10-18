<script lang="ts">
	import { live } from '../../stores/live.svelte';
	import { centerOf, fitZoom, pointIn, tilesFor, type LatLon } from '../geo';
	import { cardName, formatState, type Card } from '../model';
	import { ui } from '../runtime.svelte';

	let { card }: { card: Card } = $props();

	const ids = $derived(
		(Array.isArray(card.entities) ? card.entities : []).filter(
			(e): e is string => typeof e === 'string'
		)
	);
	const points = $derived(
		ids.flatMap((id) => {
			const s = live.states.get(id);
			const lat = s?.attributes.latitude;
			const lon = s?.attributes.longitude;
			return typeof lat === 'number' && typeof lon === 'number'
				? [{ id, lat, lon, label: cardName({ type: 'row', entity: id }, s), s }]
				: [];
		})
	);
	let w = $state(400);
	const h = $derived(typeof card.height === 'number' ? card.height : 260);
	const z = $derived(points.length > 1 ? fitZoom(points, w, h) : 15);
	const center = $derived<LatLon>(centerOf(points));
	const tiles = $derived(points.length ? tilesFor(center, z, w, h) : []);
</script>

<div class="dcard map">
	{#if typeof card.title === 'string' && card.title}<h3>{card.title}</h3>{/if}
	<div class="frame" bind:clientWidth={w} style:height="{h}px">
		{#if points.length === 0}
			<p class="muted empty">Aucune position connue.</p>
		{:else}
			{#each tiles as t (`${t.x}-${t.y}-${t.left}`)}
				<img
					class="tile"
					src="https://tile.openstreetmap.org/{z}/{t.x}/{t.y}.png"
					alt=""
					style:left="{t.left}px"
					style:top="{t.top}px"
					loading="lazy"
				/>
			{/each}
			{#each points as p (p.id)}
				{@const pos = pointIn(p, center, z, w, h)}
				<button
					class="pin"
					style:left="{pos.left}px"
					style:top="{pos.top}px"
					title="{p.label} : {formatState(p.s)}"
					aria-label="{p.label} : {formatState(p.s)}"
					onclick={() => (ui.moreInfo = p.id)}
				>
					<span>{p.label.slice(0, 1)}</span>
				</button>
			{/each}
			<span class="credit">© OpenStreetMap</span>
		{/if}
	</div>
</div>

<style>
	.frame {
		position: relative;
		overflow: hidden;
		border-radius: 8px;
		background: var(--surface-2);
	}
	.tile {
		position: absolute;
		width: 256px;
		height: 256px;
		pointer-events: none;
	}
	.pin {
		position: absolute;
		transform: translate(-50%, -50%);
		width: 28px;
		height: 28px;
		border-radius: 50%;
		border: 2px solid var(--surface);
		background: var(--primary);
		color: var(--on-primary);
		font-weight: 600;
		cursor: pointer;
		padding: 0;
	}
	.credit {
		position: absolute;
		right: 4px;
		bottom: 2px;
		font-size: 10px;
		background: rgb(255 255 255 / 0.7);
		color: #333;
		padding: 0 4px;
		border-radius: 4px;
	}
	.empty {
		padding: 16px;
	}
</style>

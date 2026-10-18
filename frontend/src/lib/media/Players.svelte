<script lang="ts">
	// Lecteurs multimédias et listes « derniers ajouts » (onglet Lecteurs du navigateur).
	import MediaCard from '../dashboard/cards/MediaCard.svelte';
	import MediaListCard from '../dashboard/cards/MediaListCard.svelte';
	import { mediaItems } from '../dashboard/data';
	import { cardName } from '../dashboard/model';
	import { byLabel, isUnavailable } from '../format';
	import { live } from '../stores/live.svelte';

	let showOff = $state(false);
	const players = $derived(
		[...live.states.values()]
			.filter((s) => s.entity_id.startsWith('media_player.'))
			.sort((a, b) => byLabel(cardName({ type: 'row' }, a), cardName({ type: 'row' }, b)))
	);
	const active = $derived(players.filter((s) => !isUnavailable(s) && s.state !== 'off'));
	const others = $derived(players.filter((s) => isUnavailable(s) || s.state === 'off'));
	// Capteurs « derniers ajouts » (Emby…) : attribut `data` au format upcoming media.
	const lists = $derived(
		[...live.states.values()]
			.filter((s) => s.entity_id.startsWith('sensor.') && mediaItems(s.attributes.data).length)
			.map((s) => s.entity_id)
			.sort()
	);
</script>

<h2>Lecteurs ({active.length})</h2>
<div class="grid">
	{#each active as s (s.entity_id)}
		<MediaCard card={{ type: 'media-control', entity: s.entity_id, volume: true }} />
	{:else}
		<p class="muted">Aucun lecteur allumé.</p>
	{/each}
</div>

{#if others.length}
	<button class="toggle" onclick={() => (showOff = !showOff)}>
		{showOff ? 'Masquer' : 'Afficher'} les {others.length} lecteurs éteints ou indisponibles
	</button>
	{#if showOff}
		<div class="grid">
			{#each others as s (s.entity_id)}
				<MediaCard card={{ type: 'media-control', entity: s.entity_id }} />
			{/each}
		</div>
	{/if}
{/if}

{#if lists.length}
	<h2>Derniers ajouts</h2>
	<div class="grid">
		{#each lists as id (id)}
			<MediaListCard
				card={{
					type: 'media-list',
					entity: id,
					title: cardName({ type: 'row', entity: id }, live.states.get(id)),
					max: 6
				}}
			/>
		{/each}
	</div>
{/if}

<style>
	h2 {
		margin-top: 20px;
	}
	.grid {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
		gap: 12px;
		align-items: start;
	}
	.toggle {
		margin: 12px 0;
		padding: 6px 12px;
		border: 1px solid var(--border);
		border-radius: 8px;
		background: var(--surface-2);
		color: var(--text);
		font: inherit;
		cursor: pointer;
	}
	.muted {
		color: var(--text-muted);
	}
</style>

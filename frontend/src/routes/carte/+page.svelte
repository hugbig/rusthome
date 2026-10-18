<script lang="ts">
	import MapCard from '#lib/dashboard/cards/MapCard.svelte';
	import { live } from '#lib/stores/live.svelte.ts';

	// Personnes et traceurs qui ont une position (latitude / longitude).
	const located = $derived(
		[...live.states.values()]
			.filter(
				(s) =>
					(s.entity_id.startsWith('person.') || s.entity_id.startsWith('device_tracker.')) &&
					typeof s.attributes.latitude === 'number' &&
					typeof s.attributes.longitude === 'number'
			)
			.map((s) => s.entity_id)
			.sort()
	);
</script>

<h1>Carte</h1>
<MapCard card={{ type: 'map', entities: located, height: 560 }} />
<p class="muted">
	{located.length} position(s) connue(s) : personnes et traceurs. Cliquez sur un repère pour sa fiche.
</p>

<style>
	.muted {
		color: var(--text-muted);
		font-size: 0.85rem;
	}
</style>

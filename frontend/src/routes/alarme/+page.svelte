<script lang="ts">
	// Alarme (Alarmo) : centrale, capteurs de sécurité, journal. Tant que la
	// phase P5 n'est pas faite, HA pilote l'alarme : les commandes sont refusées.
	import AlarmPanelCard from '#lib/dashboard/cards/AlarmPanelCard.svelte';
	import LogbookCard from '#lib/dashboard/cards/LogbookCard.svelte';
	import TileCard from '#lib/dashboard/cards/TileCard.svelte';
	import { cardName } from '#lib/dashboard/model.ts';
	import { byLabel } from '#lib/format.ts';
	import { live } from '#lib/stores/live.svelte.ts';

	const SECURITY = new Set([
		'door',
		'window',
		'opening',
		'motion',
		'occupancy',
		'presence',
		'smoke',
		'tamper',
		'garage_door'
	]);
	const panels = $derived(
		[...live.states.keys()].filter((id) => id.startsWith('alarm_control_panel.')).sort()
	);
	const sensors = $derived(
		[...live.states.values()]
			.filter(
				(s) =>
					s.entity_id.startsWith('binary_sensor.') &&
					SECURITY.has(String(s.attributes.device_class ?? ''))
			)
			.sort((a, b) => byLabel(cardName({ type: 'row' }, a), cardName({ type: 'row' }, b)))
	);
	const sirens = $derived([...live.states.keys()].filter((id) => id.startsWith('siren.')).sort());
	const open = $derived(sensors.filter((s) => s.state === 'on'));
</script>

<h1>Alarme</h1>
<p class="note">
	L’alarme est encore pilotée par Home Assistant : RustHome l’affiche en direct, mais ses commandes
	sont refusées jusqu’à la bascule de l’alarme (phase P5).
</p>

<div class="cols">
	<div class="col">
		{#each panels as id (id)}
			<AlarmPanelCard
				card={{ type: 'alarm-panel', entity: id, states: ['arm_away', 'arm_home', 'arm_night'] }}
			/>
		{:else}
			<p class="muted">Aucune centrale d’alarme.</p>
		{/each}
		{#each sirens as id (id)}
			<TileCard card={{ type: 'tile', entity: id }} />
		{/each}
	</div>
	<div class="col">
		<section class="dcard">
			<h3>Capteurs ({open.length} actif{open.length > 1 ? 's' : ''} sur {sensors.length})</h3>
			<div class="sensors">
				{#each [...open, ...sensors.filter((s) => s.state !== 'on')] as s (s.entity_id)}
					<TileCard card={{ type: 'tile', entity: s.entity_id }} />
				{/each}
			</div>
		</section>
	</div>
	<div class="col">
		{#if panels.length}
			<LogbookCard
				card={{
					type: 'logbook',
					title: 'Journal de l’alarme (48 h)',
					entities: [...panels, ...sirens],
					hours_to_show: 48
				}}
			/>
		{/if}
	</div>
</div>

<style>
	.note {
		font-size: 0.85rem;
		color: var(--text-muted);
	}
	.cols {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
		gap: 16px;
		align-items: start;
	}
	.col {
		display: flex;
		flex-direction: column;
		gap: 12px;
	}
	.sensors {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));
		gap: 8px;
		max-height: 70vh;
		overflow-y: auto;
	}
	.muted {
		color: var(--text-muted);
	}
</style>

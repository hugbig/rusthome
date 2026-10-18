<script lang="ts">
	// Aiguillage d'une carte vers son composant ; les conteneurs (grid, stacks)
	// se rendent ici même, récursivement.
	import type { Component } from 'svelte';
	import Card from './Card.svelte';
	import AlarmPanelCard from './cards/AlarmPanelCard.svelte';
	import ButtonCard from './cards/ButtonCard.svelte';
	import CalendarCard from './cards/CalendarCard.svelte';
	import CameraCard from './cards/CameraCard.svelte';
	import ChipsCard from './cards/ChipsCard.svelte';
	import EntitiesCard from './cards/EntitiesCard.svelte';
	import GaugeCard from './cards/GaugeCard.svelte';
	import GraphCard from './cards/GraphCard.svelte';
	import HeadingCard from './cards/HeadingCard.svelte';
	import LogbookCard from './cards/LogbookCard.svelte';
	import MapCard from './cards/MapCard.svelte';
	import MediaCard from './cards/MediaCard.svelte';
	import MediaListCard from './cards/MediaListCard.svelte';
	import PopupCard from './cards/PopupCard.svelte';
	import ThermostatCard from './cards/ThermostatCard.svelte';
	import TileCard from './cards/TileCard.svelte';
	import TodoCard from './cards/TodoCard.svelte';
	import UnsupportedCard from './cards/UnsupportedCard.svelte';
	import WeatherCard from './cards/WeatherCard.svelte';
	import type { Card as CardT } from './model';

	let { card }: { card: CardT } = $props();

	const CARDS: Record<string, Component<{ card: CardT }>> = {
		tile: TileCard,
		button: ButtonCard,
		chips: ChipsCard,
		heading: HeadingCard,
		entities: EntitiesCard,
		gauge: GaugeCard,
		'alarm-panel': AlarmPanelCard,
		camera: CameraCard,
		logbook: LogbookCard,
		thermostat: ThermostatCard,
		'media-control': MediaCard,
		graph: GraphCard,
		weather: WeatherCard,
		popup: PopupCard,
		'media-list': MediaListCard,
		calendar: CalendarCard,
		'todo-list': TodoCard,
		map: MapCard
	};
	const Comp = $derived(CARDS[card.type]);
	const kids = $derived(Array.isArray(card.cards) ? card.cards : []);
	const title = $derived(typeof card.title === 'string' ? card.title : '');
</script>

{#if card.type === 'grid'}
	<div class="dstack">
		{#if title}<h3 class="ctitle">{title}</h3>{/if}
		<div class="dgrid" style:--cols={typeof card.columns === 'number' ? card.columns : 3}>
			{#each kids as c, i (i)}<Card card={c} />{/each}
		</div>
	</div>
{:else if card.type === 'vertical-stack' || card.type === 'horizontal-stack'}
	<div class="dstack">
		{#if title}<h3 class="ctitle">{title}</h3>{/if}
		<div class="dstack" class:horizontal={card.type === 'horizontal-stack'}>
			{#each kids as c, i (i)}<Card card={c} />{/each}
		</div>
	</div>
{:else if Comp}
	<Comp {card} />
{:else}
	<UnsupportedCard {card} />
{/if}

<style>
	.ctitle {
		margin: 4px 0 0;
		font-size: 0.95rem;
	}
</style>

<script lang="ts">
	import { live } from '../../stores/live.svelte';
	import Icon from '../Icon.svelte';
	import { iconPath, UI } from '../icons';
	import { cardName, formatState, type Card } from '../model';
	import { call } from '../runtime.svelte';

	let { card }: { card: Card } = $props();

	// `source: spotify` (ancienne carte Spotify) : le lecteur Spotify du compte.
	const entity = $derived(
		card.entity ??
			(card.source === 'spotify'
				? [...live.states.keys()].find((id) => id.startsWith('media_player.spotify'))
				: undefined)
	);
	const s = $derived(entity ? live.states.get(entity) : undefined);
	const str = (v: unknown) => (typeof v === 'string' && v ? v : undefined);
	const title = $derived(str(s?.attributes.media_title));
	const artist = $derived(str(s?.attributes.media_artist) ?? str(s?.attributes.app_name));
	const volume = $derived(
		typeof s?.attributes.volume_level === 'number' ? s.attributes.volume_level : undefined
	);
	const off = $derived(!s || ['off', 'unavailable', 'unknown', 'standby'].includes(s.state));

	const send = (service: string, extra: Record<string, unknown> = {}) =>
		entity && call(`media_player.${service}`, { entity_id: entity, ...extra });
</script>

<div class="dcard media">
	<div class="head">
		<span class="dicon" class:on={s?.state === 'playing'}
			><Icon path={iconPath(card.icon, entity)} size={20} /></span
		>
		<div class="text">
			<span class="name">{cardName({ ...card, entity }, s)}</span>
			<span class="muted">{title ? `${title}${artist ? ` · ${artist}` : ''}` : formatState(s)}</span
			>
		</div>
	</div>
	{#if !off}
		<div class="controls">
			<button class="dbtn" onclick={() => send('media_previous_track')} aria-label="Précédent"
				><Icon path={UI.previous} size={20} /></button
			>
			<button class="dbtn" onclick={() => send('media_play_pause')} aria-label="Lecture / pause"
				><Icon path={s?.state === 'playing' ? UI.pause : UI.play} size={20} /></button
			>
			<button class="dbtn" onclick={() => send('media_next_track')} aria-label="Suivant"
				><Icon path={UI.next} size={20} /></button
			>
		</div>
		{#if card.volume === true && volume !== undefined}
			<label class="volume">
				<Icon path={UI.volume} size={18} />
				<input
					type="range"
					min="0"
					max="100"
					value={Math.round(volume * 100)}
					style:--val="{Math.round(volume * 100)}%"
					onchange={(e) =>
						send('volume_set', { volume_level: Number(e.currentTarget.value) / 100 })}
					aria-label="Volume"
				/>
			</label>
		{/if}
	{/if}
</div>

<style>
	.media {
		display: flex;
		flex-direction: column;
		gap: 10px;
	}
	.head {
		display: flex;
		align-items: center;
		gap: 10px;
		min-width: 0;
	}
	.text {
		display: flex;
		flex-direction: column;
		min-width: 0;
	}
	.name {
		font-weight: 600;
		font-size: 0.9rem;
	}
	.muted {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.controls {
		display: flex;
		justify-content: center;
		gap: 8px;
	}
	.volume {
		display: flex;
		align-items: center;
		gap: 8px;
		color: var(--text-muted);
	}
	.volume input {
		flex: 1;
		accent-color: var(--primary);
	}
</style>

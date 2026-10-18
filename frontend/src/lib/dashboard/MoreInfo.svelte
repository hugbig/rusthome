<script lang="ts">
	// Fiche rapide d'une entité, ouverte par l'action « more-info » : en-tête
	// (icône colorée, état), commande adaptée (lumière, interrupteur…), lien
	// vers la fiche complète.
	import { resolve } from '$app/paths';
	import EntityControls from '../components/EntityControls.svelte';
	import { domainOf, relativeTime } from '../format';
	import { live } from '../stores/live.svelte';
	import Icon from './Icon.svelte';
	import { iconPath, UI } from './icons';
	import LightControls from './LightControls.svelte';
	import { cardName, entityColor, formatState, isActive, isMeasurement } from './model';
	import { ui } from './runtime.svelte';
	import Sparkline from './Sparkline.svelte';

	const id = $derived(ui.moreInfo);
	const s = $derived(id ? live.states.get(id) : undefined);
	const close = () => (ui.moreInfo = null);
	const domain = $derived(id ? domainOf(id) : '');
	const snapshot = $derived(
		domain === 'camera' && id
			? `/api/v1/cameras/${encodeURIComponent(id)}/snapshot?t=${s?.last_updated ?? ''}`
			: undefined
	);
	const name = $derived(id ? cardName({ type: 'row', entity: id }, s) : '');
	const accent = $derived(entityColor(s));
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && close()} />

{#if id}
	<div class="backdrop" role="presentation" onclick={close}></div>
	<div class="sheet" role="dialog" aria-modal="true" aria-label={name}>
		<div class="bar">
			<span class="ic" class:on={isActive(s)} style:--accent={accent}>
				<Icon path={iconPath(s?.attributes.icon, id, s?.attributes.device_class)} size={22} />
			</span>
			<div class="title">
				<h2>{name}</h2>
				<span class="muted"
					>{formatState(s)}{s ? ` · depuis ${relativeTime(s.last_changed)}` : ''}</span
				>
			</div>
			<button class="close" onclick={close} aria-label="Fermer"
				><Icon path={UI.close} size={22} /></button
			>
		</div>

		{#if snapshot}<img src={snapshot} alt="Caméra {name}" />{/if}

		{#if s && domain === 'light'}
			<LightControls {s} />
		{:else if s && isMeasurement(s)}
			<div class="measure">
				<span class="big">{formatState(s)}</span>
				<Sparkline entityId={id} height={90} current={Number(s.state)} />
				<span class="muted">24 dernières heures</span>
			</div>
		{:else if s}
			<EntityControls entity={s} />
		{/if}

		<a class="full" href={resolve('/entites/[entity_id]', { entity_id: id })} onclick={close}
			>Fiche complète, historique et attributs</a
		>
	</div>
{/if}

<style>
	.backdrop {
		position: fixed;
		inset: 0;
		background: rgb(0 0 0 / 0.55);
		backdrop-filter: blur(2px);
		z-index: 50;
	}
	.sheet {
		position: fixed;
		z-index: 51;
		left: 50%;
		top: 50%;
		transform: translate(-50%, -50%);
		width: min(560px, calc(100vw - 24px));
		max-height: 90vh;
		overflow-y: auto;
		display: flex;
		flex-direction: column;
		gap: 16px;
		background: var(--surface);
		border: 1px solid var(--border);
		border-radius: 24px;
		box-shadow: var(--shadow-pop);
		padding: 18px 20px 20px;
		box-sizing: border-box;
	}
	.bar {
		display: flex;
		align-items: center;
		gap: 12px;
	}
	.ic {
		display: grid;
		place-items: center;
		width: 42px;
		height: 42px;
		border-radius: 50%;
		background: var(--surface-2);
		color: var(--text-muted);
		flex: none;
	}
	.ic.on {
		background: color-mix(in srgb, var(--accent, var(--primary)) 20%, transparent);
		color: var(--accent, var(--primary-text));
	}
	.title {
		flex: 1;
		min-width: 0;
	}
	h2 {
		margin: 0;
		font-size: 1.1rem;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.muted {
		color: var(--text-muted);
		font-size: 0.85rem;
	}
	.close {
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
	.close:hover {
		background: var(--surface-2);
	}
	img {
		width: 100%;
		border-radius: 14px;
	}
	.measure {
		display: flex;
		flex-direction: column;
		gap: 6px;
	}
	.big {
		font-size: 2.2rem;
		font-weight: 650;
		letter-spacing: -0.02em;
	}
	.full {
		align-self: flex-start;
		font-size: 0.9rem;
	}
</style>

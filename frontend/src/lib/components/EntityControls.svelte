<script lang="ts">
	import { callService, explainError } from '../api';
	import { domainOf } from '../format';
	import type { EntityState } from '../types';

	let { entity }: { entity: EntityState } = $props();

	const domain = $derived(domainOf(entity.entity_id));
	let busy = $state(false);
	let message = $state<{ kind: 'ok' | 'error'; text: string } | null>(null);

	async function send(service: string) {
		busy = true;
		message = null;
		const r = await callService(domain, service, { entity_id: entity.entity_id });
		busy = false;
		message = r.ok
			? { kind: 'ok', text: 'Commande envoyée.' }
			: { kind: 'error', text: explainError(r) };
	}
</script>

{#if domain === 'switch' || domain === 'light'}
	<div class="controls">
		<button disabled={busy || entity.state === 'on'} onclick={() => send('turn_on')}>Allumer</button
		>
		<button disabled={busy || entity.state === 'off'} onclick={() => send('turn_off')}
			>Éteindre</button
		>
	</div>
	{#if message}
		<p class="msg {message.kind}" role="status">{message.text}</p>
	{/if}
{/if}

<style>
	.controls {
		display: flex;
		gap: 8px;
	}
	button {
		padding: 6px 14px;
		border-radius: 8px;
		border: 1px solid var(--border);
		background: var(--surface-2);
		color: var(--text);
		cursor: pointer;
	}
	button:disabled {
		opacity: 0.5;
		cursor: default;
	}
	.msg {
		margin: 8px 0 0;
		font-size: 0.85rem;
	}
	.msg.error {
		color: #e66767;
	}
</style>

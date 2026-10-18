<script lang="ts">
	import { live } from '../../stores/live.svelte';
	import Icon from '../Icon.svelte';
	import { iconPath } from '../icons';
	import { cardName, formatState, type Card } from '../model';
	import { call } from '../runtime.svelte';

	let { card }: { card: Card } = $props();

	const s = $derived(card.entity ? live.states.get(card.entity) : undefined);
	const armed = $derived(!!s && s.state !== 'disarmed' && s.state !== 'unavailable');
	const modes = $derived(
		(Array.isArray(card.states) && card.states.length
			? card.states
			: ['arm_home', 'arm_away']) as string[]
	);
	const needsCode = $derived(
		typeof s?.attributes.code_format === 'string' && s.attributes.code_format !== ''
	);
	const tone = $derived(
		!s
			? 'muted'
			: s.state === 'triggered'
				? 'critical'
				: s.state === 'pending' || s.state === 'arming'
					? 'warning'
					: armed
						? 'armed'
						: 'muted'
	);
	const LABELS: Record<string, string> = {
		arm_home: 'Armer (présence)',
		arm_away: 'Armer (absence)',
		arm_night: 'Armer (nuit)',
		arm_vacation: 'Armer (vacances)',
		arm_custom_bypass: 'Armer (personnalisé)'
	};
	let code = $state('');
	let busy = $state(false);

	async function send(service: string) {
		if (!card.entity) return;
		busy = true;
		const ok = await call(`alarm_control_panel.alarm_${service}`, {
			entity_id: card.entity,
			...(code ? { code } : {})
		});
		busy = false;
		if (ok) code = '';
	}
</script>

<div class="dcard alarm">
	<div class="status {tone}" role="status">
		<Icon path={iconPath('mdi:shield-home', card.entity)} size={36} />
		<div>
			<div class="name">{cardName(card, s)}</div>
			<div class="state">{formatState(s)}</div>
		</div>
	</div>
	{#if needsCode}
		<input
			type="password"
			inputmode="numeric"
			autocomplete="off"
			placeholder="Code"
			bind:value={code}
			aria-label="Code de l’alarme"
		/>
	{/if}
	<div class="actions">
		{#if armed}
			<button class="dbtn primary" disabled={busy} onclick={() => send('disarm')}>Désarmer</button>
		{:else}
			{#each modes as m (m)}
				<button class="dbtn" disabled={busy || !s} onclick={() => send(m)}>{LABELS[m] ?? m}</button>
			{/each}
		{/if}
	</div>
</div>

<style>
	.alarm {
		display: flex;
		flex-direction: column;
		gap: 12px;
	}
	.status {
		display: flex;
		align-items: center;
		gap: 12px;
		padding: 10px;
		border-radius: 10px;
		background: var(--surface-2);
	}
	.status.armed {
		background: var(--primary);
		color: var(--on-primary);
	}
	.status.warning {
		background: var(--warning);
		color: #fff;
	}
	.status.critical {
		background: var(--critical);
		color: #fff;
	}
	.name {
		font-size: 0.85rem;
		opacity: 0.85;
	}
	.state {
		font-size: 1.15rem;
		font-weight: 600;
	}
	input {
		padding: 8px 10px;
		border-radius: 8px;
		border: 1px solid var(--border);
		background: var(--bg);
		color: var(--text);
		font: inherit;
		letter-spacing: 0.3em;
	}
	.actions {
		display: flex;
		flex-wrap: wrap;
		gap: 8px;
	}
	.actions .dbtn {
		flex: 1 1 140px;
	}
</style>

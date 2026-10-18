<script lang="ts">
	// Liaison directe (binding) : une télécommande ou un interrupteur commande
	// une lampe ou un groupe sans passer par la box.
	import { request } from '../api';
	import Icon from '../dashboard/Icon.svelte';
	import { UI } from '../dashboard/icons';
	import { BIND_CLUSTERS, type ZDevice, type ZGroup } from '../zigbee';

	interface Props {
		device: ZDevice;
		devices: ZDevice[];
		groups: ZGroup[];
		onclose: () => void;
		ondone: (message: string) => void;
	}
	let { device, devices, groups, onclose, ondone }: Props = $props();

	let target = $state('');
	let clusters = $state<string[]>(['genOnOff', 'genLevelCtrl']);
	let error = $state<string | null>(null);
	let busy = $state(false);

	async function send(unbind: boolean) {
		busy = true;
		error = null;
		const r = await request('POST', 'zigbee/bind', {
			from: device.ieee,
			to: target,
			clusters,
			unbind
		});
		busy = false;
		if (r.ok) ondone(unbind ? 'Liaison supprimée.' : 'Liaison créée.');
		else error = r.error;
	}
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && onclose()} />

<div class="backdrop" role="presentation" onclick={onclose}></div>
<div class="dialog" role="dialog" aria-modal="true" aria-label="Liaisons de {device.name}">
	<div class="head">
		<h2>Liaisons de {device.name}</h2>
		<button class="close" onclick={onclose} aria-label="Fermer"><Icon path={UI.close} /></button>
	</div>
	<div class="body">
		<label>
			<span>Cible</span>
			<select bind:value={target}>
				<option value="" disabled>Choisir un appareil ou un groupe</option>
				<optgroup label="Groupes">
					<option value="default_bind_group">Groupe par défaut</option>
					{#each groups as g (g.id)}<option value={g.name}>{g.name}</option>{/each}
				</optgroup>
				<optgroup label="Appareils">
					{#each devices.filter((d) => d.ieee !== device.ieee) as d (d.ieee)}
						<option value={d.ieee}>{d.name}</option>
					{/each}
				</optgroup>
			</select>
		</label>
		<fieldset>
			<legend>Commandes transmises</legend>
			{#each BIND_CLUSTERS as [c, l] (c)}
				<label class="check"><input type="checkbox" value={c} bind:group={clusters} />{l}</label>
			{/each}
		</fieldset>
		{#if error}<p class="err">{error}</p>{/if}
	</div>
	<div class="foot">
		<button class="dbtn" disabled={!target || busy} onclick={() => send(true)}>Délier</button>
		<button
			class="dbtn primary"
			disabled={!target || busy || !clusters.length}
			onclick={() => send(false)}>Lier</button
		>
	</div>
</div>

<style>
	.backdrop {
		position: fixed;
		inset: 0;
		background: rgb(0 0 0 / 0.55);
		z-index: 100;
	}
	.dialog {
		position: fixed;
		z-index: 101;
		left: 50%;
		top: 50%;
		transform: translate(-50%, -50%);
		width: min(520px, calc(100vw - 24px));
		background: var(--surface);
		border: 1px solid var(--border);
		border-radius: 20px;
		box-shadow: var(--shadow-pop);
	}
	.head,
	.foot {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 12px 18px;
	}
	.head {
		justify-content: space-between;
		border-bottom: 1px solid var(--border);
	}
	.foot {
		justify-content: flex-end;
		border-top: 1px solid var(--border);
	}
	h2 {
		margin: 0;
		font-size: 1.1rem;
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
	.body {
		display: flex;
		flex-direction: column;
		gap: 14px;
		padding: 16px 18px;
	}
	label {
		display: flex;
		flex-direction: column;
		gap: 4px;
	}
	label > span {
		font-size: 0.8rem;
		color: var(--text-muted);
	}
	select {
		padding: 8px 10px;
		border: 1px solid var(--border);
		border-radius: 8px;
		background: var(--bg);
		color: var(--text);
		font: inherit;
	}
	fieldset {
		display: flex;
		flex-direction: column;
		gap: 6px;
		border: 1px solid var(--border);
		border-radius: 10px;
		padding: 8px 12px;
	}
	legend {
		font-size: 0.8rem;
		color: var(--text-muted);
	}
	.check {
		flex-direction: row;
		align-items: center;
		gap: 8px;
	}
	.err {
		color: #e66767;
	}
</style>

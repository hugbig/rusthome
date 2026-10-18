<script lang="ts">
	// Renommer un appareil Zigbee : aperçu des entity_id renommés et des
	// automatisations et tableaux de bord qui suivront, puis confirmation.
	import { resolve } from '$app/paths';
	import { request } from '../api';
	import Icon from '../dashboard/Icon.svelte';
	import { UI } from '../dashboard/icons';
	import type { ZDevice } from '../zigbee';

	interface Plan {
		entities: { from: string; to: string }[];
		automations: { id: string; alias: string }[];
		dashboards: { url_path: string; title: string }[];
	}
	interface Props {
		device: ZDevice;
		onclose: () => void;
		ondone: (message: string) => void;
	}
	let { device, onclose, ondone }: Props = $props();

	// svelte-ignore state_referenced_locally
	let name = $state(device.name);
	let entities = $state(true);
	let plan = $state<Plan | null>(null);
	let error = $state<string | null>(null);
	let busy = $state(false);
	const invalid = $derived(!name.trim() || /[/#+]/.test(name));

	// Aperçu recalculé à la frappe (avec un léger délai).
	$effect(() => {
		const n = name.trim();
		if (invalid || n === device.name) {
			plan = null;
			return;
		}
		const t = setTimeout(async () => {
			const r = await request<Plan>(
				'GET',
				`zigbee/devices/${encodeURIComponent(device.ieee)}/rename-preview?name=${encodeURIComponent(n)}`
			);
			if (r.ok) plan = r.data;
		}, 300);
		return () => clearTimeout(t);
	});

	async function confirm() {
		busy = true;
		error = null;
		const r = await request<{ entities_renamed: number }>(
			'POST',
			`zigbee/devices/${encodeURIComponent(device.ieee)}/rename`,
			{ name: name.trim(), entities }
		);
		busy = false;
		if (r.ok)
			ondone(
				`« ${device.name} » renommé en « ${name.trim()} »${r.data.entities_renamed ? `, ${r.data.entities_renamed} entité(s) renommée(s)` : ''}.`
			);
		else error = r.error;
	}
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && onclose()} />

<div class="backdrop" role="presentation" onclick={onclose}></div>
<div class="dialog" role="dialog" aria-modal="true" aria-label="Renommer {device.name}">
	<div class="head">
		<h2>Renommer l’appareil</h2>
		<button class="close" onclick={onclose} aria-label="Fermer"><Icon path={UI.close} /></button>
	</div>
	<div class="body">
		<label>
			<span>Nouveau nom</span>
			<!-- svelte-ignore a11y_autofocus -->
			<input bind:value={name} autofocus aria-invalid={invalid} />
			{#if invalid}<small class="err">Le nom ne peut pas être vide ni contenir / # +.</small>{/if}
		</label>
		<label class="check">
			<input type="checkbox" bind:checked={entities} />
			Renommer aussi les entités, et mettre à jour les automatisations et tableaux de bord qui les utilisent
		</label>
		{#if plan && entities}
			<section class="preview">
				{#if plan.entities.length === 0}
					<p class="muted">Aucune entité dont l’identifiant suit le nom de l’appareil.</p>
				{:else}
					<h3>Entités ({plan.entities.length})</h3>
					<ul>
						{#each plan.entities as e (e.from)}
							<li><code>{e.from}</code> → <code>{e.to}</code></li>
						{/each}
					</ul>
				{/if}
				{#if plan.automations.length}
					<h3>Automatisations mises à jour ({plan.automations.length})</h3>
					<ul>
						{#each plan.automations as a (a.id)}
							<li><a href={resolve('/automatisations/[id]', { id: a.id })}>{a.alias}</a></li>
						{/each}
					</ul>
				{/if}
				{#if plan.dashboards.length}
					<h3>Tableaux de bord mis à jour ({plan.dashboards.length})</h3>
					<ul>
						{#each plan.dashboards as d (d.url_path)}<li>{d.title}</li>{/each}
					</ul>
				{/if}
			</section>
		{/if}
		{#if error}<p class="err">{error}</p>{/if}
	</div>
	<div class="foot">
		<button class="dbtn" onclick={onclose}>Annuler</button>
		<button
			class="dbtn primary"
			disabled={invalid || busy || name.trim() === device.name}
			onclick={confirm}>{busy ? 'Renommage…' : 'Renommer'}</button
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
		width: min(620px, calc(100vw - 24px));
		max-height: 88vh;
		display: flex;
		flex-direction: column;
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
	h3 {
		margin: 10px 0 4px;
		font-size: 0.85rem;
		color: var(--text-muted);
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
		overflow-y: auto;
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
	input:not([type='checkbox']) {
		padding: 8px 10px;
		border: 1px solid var(--border);
		border-radius: 8px;
		background: var(--bg);
		color: var(--text);
		font: inherit;
	}
	.check {
		flex-direction: row;
		align-items: flex-start;
		gap: 10px;
		font-size: 0.9rem;
	}
	.preview {
		padding: 10px 12px;
		border-radius: 12px;
		background: var(--surface-2);
		font-size: 0.85rem;
	}
	ul {
		margin: 0;
		padding-left: 18px;
	}
	code {
		font-size: 0.8rem;
	}
	.muted {
		margin: 0;
		color: var(--text-muted);
	}
	.err {
		color: #e66767;
	}
</style>

<script lang="ts">
	// Réglages du tableau de bord : titre, icône, onglets, menu latéral, kiosque, accès.
	import Icon from '../Icon.svelte';
	import { iconPath, UI } from '../icons';
	import type { DashboardDoc } from '../model';

	interface Props {
		doc: DashboardDoc;
		isAdmin: boolean;
		onsave: (patch: Partial<DashboardDoc>) => void;
		onclose: () => void;
	}
	let { doc, isAdmin, onsave, onclose }: Props = $props();

	// svelte-ignore state_referenced_locally
	let title = $state(doc.title ?? '');
	// svelte-ignore state_referenced_locally
	let icon = $state(doc.icon ?? '');
	// svelte-ignore state_referenced_locally
	let sidebar = $state(doc.show_in_sidebar !== false);
	// svelte-ignore state_referenced_locally
	let kiosk = $state(doc.kiosk === true);
	// svelte-ignore state_referenced_locally
	let iconTabs = $state(doc.tabs_icon_only === true);
	// svelte-ignore state_referenced_locally
	let adminOnly = $state(doc.require_admin === true);
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && onclose()} />

<div class="backdrop" role="presentation" onclick={onclose}></div>
<div class="dialog" role="dialog" aria-modal="true" aria-label="Réglages du tableau de bord">
	<div class="head">
		<h2>Réglages du tableau de bord</h2>
		<button class="close" onclick={onclose} aria-label="Fermer"><Icon path={UI.close} /></button>
	</div>
	<div class="body">
		<label><span>Titre</span><input bind:value={title} /></label>
		<label>
			<span>Icône</span>
			<span class="row">
				<span class="preview"><Icon path={iconPath(icon || 'mdi:view-dashboard')} size={20} /></span
				>
				<input bind:value={icon} placeholder="mdi:home" />
			</span>
		</label>
		<p class="note">Adresse : <code>/tableaux/{doc.url_path}</code> (non modifiable).</p>
		<label class="check"
			><input type="checkbox" bind:checked={sidebar} />Afficher dans le menu latéral</label
		>
		<label class="check"
			><input type="checkbox" bind:checked={iconTabs} />Onglets : icône seule, sans le titre (pour
			les vues qui ont une icône ; le titre reste en info-bulle)</label
		>
		<label class="check"
			><input type="checkbox" bind:checked={kiosk} />Mode kiosque : sans menu ni en-tête (tablette
			murale)</label
		>
		<label class="check"
			><input type="checkbox" bind:checked={adminOnly} disabled={!isAdmin} />Réservé aux
			administrateurs</label
		>
	</div>
	<div class="foot">
		<button class="dbtn" onclick={onclose}>Annuler</button>
		<button
			class="dbtn primary"
			onclick={() =>
				onsave({
					title: title.trim() || undefined,
					icon: icon.trim() || undefined,
					show_in_sidebar: sidebar,
					kiosk: kiosk || undefined,
					tabs_icon_only: iconTabs || undefined,
					require_admin: adminOnly || undefined
				})}>Enregistrer</button
		>
	</div>
</div>

<style>
	.backdrop {
		position: fixed;
		inset: 0;
		background: rgb(0 0 0 / 0.5);
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
		border-radius: 16px;
	}
	.head,
	.foot {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 12px 16px;
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
		padding: 16px;
	}
	label {
		display: flex;
		flex-direction: column;
		gap: 4px;
	}
	label > span:first-child {
		font-size: 0.8rem;
		color: var(--text-muted);
	}
	input:not([type='checkbox']) {
		padding: 7px 10px;
		border: 1px solid var(--border);
		border-radius: 8px;
		background: var(--bg);
		color: var(--text);
		font: inherit;
	}
	.check {
		flex-direction: row;
		align-items: center;
		gap: 10px;
	}
	.row {
		display: flex;
		gap: 8px;
		align-items: center;
	}
	.row input {
		flex: 1;
	}
	.preview {
		display: grid;
		place-items: center;
		width: 36px;
		height: 36px;
		border-radius: 8px;
		background: var(--surface-2);
	}
	.note {
		margin: 0;
		font-size: 0.85rem;
		color: var(--text-muted);
	}
</style>

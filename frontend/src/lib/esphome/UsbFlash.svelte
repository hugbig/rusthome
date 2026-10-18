<script lang="ts">
	// Installation et console par USB depuis le navigateur (Web Serial),
	// comme « Brancher sur cet ordinateur » dans HA : la carte se branche sur
	// ce poste, le firmware compilé vient de RustHome. esp-web-tools propose
	// ensuite « Logs & Console » sur la même connexion.
	import { onMount } from 'svelte';
	import { request } from '../api';
	import Dialog from '../components/Dialog.svelte';

	interface Props {
		configuration: string;
		title: string;
		/** Lance une compilation (console) quand aucun firmware n'existe. */
		oncompile: () => void;
		onclose: () => void;
	}
	let { configuration, title, oncompile, onclose }: Props = $props();

	const supported = typeof navigator !== 'undefined' && 'serial' in navigator;
	const secure = typeof window !== 'undefined' && window.isSecureContext;
	let built = $state<boolean | null>(null);
	let ready = $state(false);
	const base = $derived(`/api/v1/esphome/configs/${encodeURIComponent(configuration)}`);

	onMount(async () => {
		const r = await request<{ file: string }[]>(
			'GET',
			`esphome/configs/${encodeURIComponent(configuration)}/binaries`
		);
		built =
			r.ok && r.data.some((b) => b.file === 'firmware.factory.bin' || b.file === 'firmware.bin');
		if (supported && secure) {
			// Définit <esp-web-install-button> (chargé seulement ici).
			await import('esp-web-tools');
			ready = true;
		}
	});
</script>

<Dialog title="USB · {title}" {onclose}>
	{#if !supported}
		<p>
			Ce navigateur ne sait pas parler aux ports série. Utilisez Chrome ou Edge sur ordinateur (Web
			Serial).
		</p>
	{:else if !secure}
		<p>
			L’accès USB demande une connexion sécurisée : ouvrez RustHome en HTTPS (ou sur
			<code>localhost</code>).
		</p>
	{:else if built === null}
		<p class="muted">Recherche du firmware…</p>
	{:else if !built}
		<p>Aucun firmware compilé pour ce projet. Compilez-le d’abord, puis revenez ici.</p>
		<button class="dbtn primary" onclick={oncompile}>Compiler</button>
	{:else}
		<ol>
			<li>Branchez la carte en USB sur cet ordinateur.</li>
			<li>Cliquez sur « Connecter » et choisissez son port série.</li>
			<li>
				Choisissez « Install » pour flasher le dernier firmware compilé, ou « Logs & Console » pour
				lire ses logs.
			</li>
		</ol>
		<p class="note">
			La première installation efface la carte. Les fenêtres suivantes viennent d’ESP Web Tools, en
			anglais.
		</p>
		{#if ready}
			<esp-web-install-button manifest="{base}/manifest.json">
				<button slot="activate" class="dbtn primary">Connecter</button>
			</esp-web-install-button>
		{/if}
	{/if}
	{#snippet footer()}
		<button class="dbtn" onclick={onclose}>Fermer</button>
	{/snippet}
</Dialog>

<style>
	p,
	ol {
		margin: 0;
	}
	ol {
		padding-left: 20px;
		display: flex;
		flex-direction: column;
		gap: 4px;
	}
	.note,
	.muted {
		color: var(--text-muted);
		font-size: 0.85rem;
	}
</style>

<script lang="ts">
	// Fenêtres des opérations ESPHome : choix d'installation (sans fil, USB,
	// téléchargement), choix des logs (Wi-Fi, USB), console en direct.
	import Dialog from '../components/Dialog.svelte';
	import type { EspOps, Project } from './ops.svelte';
	import RunConsole from './RunConsole.svelte';
	import UsbFlash from './UsbFlash.svelte';

	let { ops }: { ops: EspOps } = $props();

	const firmwareHref = (p: Project) =>
		`/api/v1/esphome/configs/${encodeURIComponent(p.configuration)}/firmware/firmware.factory.bin`;
</script>

{#snippet address(p: Project)}
	{#if p.target}
		<p class="note">Carte jointe à l’adresse configurée <code>{p.target}</code>.</p>
	{:else}
		<p class="note">
			Aucune adresse configurée pour ce projet : ajoutez
			<code>{p.configuration.replace(/\.ya?ml$/, '')}@&lt;IP&gt;</code> à
			<code>RUSTHOME_ESPHOME_ADDRESSES</code>. L’adresse n’est jamais saisie ici, pour qu’une erreur
			ne puisse pas viser un nœud de l’alarme.
		</p>
	{/if}
{/snippet}

{#if ops.choose}
	{@const p = ops.choose}
	<Dialog title="Installer · {p.title}" onclose={() => (ops.choose = null)}>
		<section>
			<h3>Sans fil (OTA)</h3>
			{#if p.flashable}
				<p>Compile puis envoie le firmware par le réseau.</p>
				{@render address(p)}
				<button class="dbtn primary" disabled={!p.target} onclick={() => ops.start(p, 'install')}
					>Compiler et installer</button
				>
			{:else}
				<p class="note">
					Non autorisé pour ce projet pendant le développement : seul le banc de test s’installe
					sans fil (<code>RUSTHOME_ESPHOME_FLASHABLE</code>).
				</p>
			{/if}
		</section>
		<section>
			<h3>USB, depuis ce navigateur</h3>
			<p>La carte se branche sur cet ordinateur (Chrome ou Edge).</p>
			<button class="dbtn" onclick={() => ((ops.usb = p), (ops.choose = null))}
				>Brancher en USB</button
			>
		</section>
		<section>
			<h3>Télécharger</h3>
			<p>Le firmware du dernier build, pour ESPHome Web ou esptool.</p>
			<div class="row">
				<button class="dbtn" onclick={() => ops.start(p, 'compile')}>Compiler</button>
				<!-- Fichier servi par l'API RustHome : téléchargement direct voulu. -->
				<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -->
				<a class="dbtn" href={firmwareHref(p)} download>Télécharger (.bin)</a>
			</div>
		</section>
	</Dialog>
{/if}

{#if ops.logsFor}
	{@const p = ops.logsFor}
	<Dialog title="Logs · {p.title}" onclose={() => (ops.logsFor = null)}>
		<section>
			<h3>Wi-Fi</h3>
			<p>
				Les logs passent par l’API native : la carte accepte une connexion de plus, à côté de Home
				Assistant.
			</p>
			{@render address(p)}
			<button class="dbtn primary" disabled={!p.target} onclick={() => ops.start(p, 'logs')}
				>Afficher les logs</button
			>
		</section>
		<section>
			<h3>USB</h3>
			<p>« Logs & Console » après la connexion série.</p>
			<button class="dbtn" onclick={() => ((ops.usb = p), (ops.logsFor = null))}
				>Brancher en USB</button
			>
		</section>
	</Dialog>
{/if}

{#if ops.usb}
	{@const p = ops.usb}
	<UsbFlash
		configuration={p.configuration}
		title={p.title}
		oncompile={() => ops.start(p, 'compile')}
		onclose={() => (ops.usb = null)}
	/>
{/if}

{#if ops.run}
	{@const r = ops.run}
	{#key r}
		<RunConsole
			configuration={r.project.configuration}
			title={r.project.title}
			action={r.action}
			target={r.action === 'install' || r.action === 'logs'
				? (r.project.target ?? undefined)
				: undefined}
			onclose={() => (ops.run = null)}
			ondone={() => ops.ondone?.()}
		/>
	{/key}
{/if}

<style>
	section {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: 8px;
		padding-bottom: 12px;
		border-bottom: 1px solid var(--border);
	}
	section:last-child {
		border-bottom: none;
		padding-bottom: 0;
	}
	h3 {
		margin: 0;
		font-size: 0.95rem;
	}
	p {
		margin: 0;
		font-size: 0.9rem;
	}
	.note {
		color: var(--text-muted);
		font-size: 0.8rem;
	}
	.row {
		display: flex;
		gap: 8px;
	}
	a.dbtn {
		text-decoration: none;
	}
</style>

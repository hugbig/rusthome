<script lang="ts">
	// Projets ESPHome (spec §5.3 bis), comme le panneau ESPHome de HA : liste
	// des cartes, création, import, édition, validation, compilation,
	// installation (sans fil ou USB), logs. Tout passe par le conteneur
	// ESPHome Builder ; les nœuds de l'alarme sont en lecture seule.
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import {
		mdiCheckDecagramOutline,
		mdiChip,
		mdiCogPlayOutline,
		mdiDeleteOutline,
		mdiDownload,
		mdiKeyVariant,
		mdiLock,
		mdiPlus,
		mdiTextBoxOutline,
		mdiUpload
	} from '@mdi/js';
	import { request } from '#lib/api.ts';
	import DeviceList from '#lib/components/DeviceList.svelte';
	import Menu from '#lib/components/Menu.svelte';
	import Icon from '#lib/dashboard/Icon.svelte';
	import { sortDevices, stateLabel, type EspDevice } from '#lib/esphome.ts';
	import NewProject from '#lib/esphome/NewProject.svelte';
	import { EspOps, type Project } from '#lib/esphome/ops.svelte.ts';
	import OpsHost from '#lib/esphome/OpsHost.svelte';
	import Secrets from '#lib/esphome/Secrets.svelte';

	let devices = $state<EspDevice[]>([]);
	let version = $state<string | null>(null);
	let error = $state<string | null>(null);
	let loading = $state(true);
	let creating = $state(false);
	let secrets = $state(false);
	let notice = $state<string | null>(null);
	const ops = new EspOps();

	async function load() {
		const r = await request<{ version: string; devices: EspDevice[] }>('GET', 'esphome');
		loading = false;
		if (r.ok) {
			devices = sortDevices(r.data.devices);
			version = r.data.version;
			error = null;
		} else error = r.error;
	}
	ops.ondone = () => void load();
	onMount(load);

	const project = (d: EspDevice): Project => ({
		configuration: d.configuration,
		title: d.friendly_name || d.name || d.configuration,
		locked: d.locked,
		flashable: d.flashable,
		target: d.target
	});
	const editHref = (d: EspDevice) =>
		resolve('/esphome/[configuration]', { configuration: d.configuration });

	async function remove(d: EspDevice) {
		if (
			!confirm(
				`Supprimer le projet « ${project(d).title} » (${d.configuration}) ? Le fichier et ses builds sont effacés.`
			)
		)
			return;
		const r = await request('DELETE', `esphome/configs/${encodeURIComponent(d.configuration)}`);
		notice = r.ok ? `Projet ${d.configuration} supprimé.` : r.error;
		await load();
	}

	function menu(d: EspDevice) {
		const p = project(d);
		return [
			{
				label: 'Valider la configuration',
				icon: mdiCheckDecagramOutline,
				onclick: () => ops.start(p, 'validate')
			},
			...(d.locked
				? []
				: [
						{ label: 'Compiler', icon: mdiCogPlayOutline, onclick: () => ops.start(p, 'compile') },
						{
							label: 'Télécharger le firmware',
							icon: mdiDownload,
							onclick: () => (ops.choose = p)
						},
						{
							label: 'Supprimer',
							icon: mdiDeleteOutline,
							danger: true,
							divider: true,
							onclick: () => void remove(d)
						}
					])
		];
	}
</script>

<div class="top">
	<h1>ESPHome</h1>
	{#if version}<span class="chip">ESPHome {version}</span>{/if}
	<span class="grow"></span>
	{#if !error}
		<button class="dbtn" onclick={() => (secrets = true)}
			><Icon path={mdiKeyVariant} size={18} />Secrets</button
		>
		<button class="dbtn primary" onclick={() => (creating = true)}
			><Icon path={mdiPlus} size={18} />Nouveau projet</button
		>
	{/if}
</div>

{#if notice}<p class="notice" role="status">{notice}</p>{/if}

{#if loading}
	<p class="muted">Chargement des projets…</p>
{:else if error}
	<section class="card">
		<h2>ESPHome Builder indisponible</h2>
		<p>{error}</p>
		<p class="muted">
			RustHome pilote le conteneur officiel ESPHome Builder : renseignez
			<code>RUSTHOME_ESPHOME_URL</code> (en développement : <code>just dev-deps</code>, puis
			<code>http://127.0.0.1:6052</code>).
		</p>
	</section>
{:else}
	<div class="grid">
		{#each devices as d (d.configuration)}
			{@const p = project(d)}
			<article class="dev card">
				<header>
					<span class="ic"><Icon path={mdiChip} size={22} /></span>
					<div class="who">
						<a href={editHref(d)} class="title">{p.title}</a>
						<small>{d.configuration}{d.board ? ` · ${d.board}` : ''}</small>
					</div>
					<Menu label="Actions sur {p.title}" items={menu(d)} />
				</header>
				<div class="tags">
					<span class="state {d.state}"
						><span class="dot" aria-hidden="true"></span>{stateLabel(d.state)}</span
					>
					{#if d.locked}
						<span
							class="tag lock"
							title="Nœud de l'alarme : lecture seule jusqu'à la bascule de l'alarme"
							><Icon path={mdiLock} size={14} />Verrouillé · alarme</span
						>
					{:else if d.pending_changes}
						<span class="tag">Modifications non installées</span>
					{/if}
					{#if d.update_available}<span class="tag">Mise à jour ESPHome</span>{/if}
				</div>
				<dl>
					<div>
						<dt>Compilé avec</dt>
						<dd>{d.built_version || '—'}</dd>
					</div>
					<div>
						<dt>Sur la carte</dt>
						<dd>{d.deployed_version || '—'}</dd>
					</div>
				</dl>
				<footer>
					<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -- editHref passe par resolve() -->
					<button class="dbtn" onclick={() => goto(editHref(d))}
						><Icon path={mdiTextBoxOutline} size={18} />{d.locked
							? 'Voir le code'
							: 'Modifier'}</button
					>
					{#if !d.locked}
						<button class="dbtn" onclick={() => (ops.choose = p)}
							><Icon path={mdiUpload} size={18} />Installer</button
						>
						<button class="dbtn" onclick={() => (ops.logsFor = p)}>Logs</button>
					{/if}
				</footer>
			</article>
		{:else}
			<p class="muted">Aucun projet : créez-en un ou importez un YAML.</p>
		{/each}
	</div>
{/if}

<details class="ha">
	<summary>Modules ESPHome vus par Home Assistant (miroir)</summary>
	<p class="muted">
		Les cartes restent connectées à Home Assistant tant que leur projet n’est pas repris ici.
	</p>
	<DeviceList
		integrations={['esphome', 'ha:esphome']}
		columns={[{ label: 'Firmware', value: (x) => x.sw_version ?? undefined }]}
	/>
</details>

{#if creating}
	<NewProject
		onclose={() => (creating = false)}
		oncreated={(c) => {
			creating = false;
			notice = `Projet ${c} créé.`;
			void load();
		}}
	/>
{/if}
{#if secrets}<Secrets onclose={() => (secrets = false)} />{/if}
<OpsHost {ops} />

<style>
	.top {
		display: flex;
		align-items: center;
		gap: 10px;
		flex-wrap: wrap;
		margin-bottom: 12px;
	}
	.top h1 {
		margin: 0;
	}
	.grow {
		flex: 1;
	}
	.chip {
		padding: 2px 10px;
		border-radius: 999px;
		background: var(--surface-2);
		color: var(--text-muted);
		font-size: 0.8rem;
	}
	.top .dbtn {
		display: inline-flex;
		align-items: center;
		gap: 6px;
	}
	.notice {
		margin: 0 0 12px;
		padding: 8px 12px;
		border-radius: 10px;
		background: var(--surface-2);
	}
	.muted {
		color: var(--text-muted);
	}
	.grid {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(min(320px, 100%), 1fr));
		gap: 14px;
	}
	.dev {
		display: flex;
		flex-direction: column;
		gap: 10px;
		margin: 0;
	}
	header {
		display: flex;
		align-items: center;
		gap: 12px;
	}
	.ic {
		display: grid;
		place-items: center;
		width: 40px;
		height: 40px;
		border-radius: 12px;
		background: var(--surface-2);
		color: var(--primary-text);
		flex: none;
	}
	.who {
		display: flex;
		flex-direction: column;
		min-width: 0;
		flex: 1;
	}
	.title {
		font-weight: 650;
		color: var(--text);
		text-decoration: none;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.title:hover {
		text-decoration: underline;
	}
	.who small {
		color: var(--text-muted);
		font-size: 0.8rem;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.tags {
		display: flex;
		flex-wrap: wrap;
		gap: 6px;
	}
	.state,
	.tag {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		padding: 2px 10px;
		border-radius: 999px;
		background: var(--surface-2);
		font-size: 0.8rem;
	}
	.dot {
		width: 8px;
		height: 8px;
		border-radius: 50%;
		background: var(--text-muted);
	}
	.online .dot {
		background: var(--good);
	}
	.offline .dot {
		background: var(--critical);
	}
	.lock {
		color: var(--warning);
	}
	dl {
		display: flex;
		gap: 18px;
		margin: 0;
		font-size: 0.85rem;
	}
	dt {
		color: var(--text-muted);
		font-size: 0.75rem;
	}
	dd {
		margin: 0;
	}
	footer {
		display: flex;
		gap: 6px;
		flex-wrap: wrap;
		margin-top: auto;
	}
	footer .dbtn {
		display: inline-flex;
		align-items: center;
		gap: 6px;
	}
	.ha {
		margin-top: 24px;
	}
	.ha summary {
		cursor: pointer;
		color: var(--text-muted);
		margin-bottom: 8px;
	}
</style>

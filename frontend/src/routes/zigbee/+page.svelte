<script lang="ts">
	// Gestion du réseau Zigbee (Zigbee2MQTT) : appairage, appareils (renommer,
	// mise à jour OTA, liaisons, suppression), groupes et événements récents.
	// Tant que MQTT est en lecture seule, les commandes sont refusées avant envoi.
	import { onDestroy, onMount } from 'svelte';
	import { resolve } from '$app/paths';
	import { mdiDeleteOutline, mdiLinkVariant, mdiPencil, mdiUpdate, mdiZigbee } from '@mdi/js';
	import { request } from '#lib/api.ts';
	import Menu from '#lib/components/Menu.svelte';
	import ZigbeeBind from '#lib/components/ZigbeeBind.svelte';
	import ZigbeeRename from '#lib/components/ZigbeeRename.svelte';
	import Icon from '#lib/dashboard/Icon.svelte';
	import { relativeTime } from '#lib/format.ts';
	import { live } from '#lib/stores/live.svelte.ts';
	import { registry } from '#lib/stores/registry.svelte.ts';
	import {
		devicesOf,
		eventLabel,
		eventsOf,
		groupsOf,
		lqiLevel,
		permitJoinLeft,
		type ZDevice
	} from '#lib/zigbee.ts';

	let snapshot = $state<unknown>(null);
	let error = $state<string | null>(null);
	let notice = $state<{ kind: 'ok' | 'error'; text: string } | null>(null);
	let tab = $state<'devices' | 'groups' | 'events'>('devices');
	let q = $state('');
	let now = $state(Date.now());
	let renaming = $state<ZDevice | null>(null);
	let binding = $state<ZDevice | null>(null);
	let newGroup = $state('');
	let timer: ReturnType<typeof setInterval> | undefined;

	async function load() {
		const r = await request<unknown>('GET', 'zigbee');
		if (r.ok) {
			snapshot = r.data;
			error = null;
		} else error = r.error;
	}
	onMount(() => {
		void load();
		// état du réseau toutes les 5 s, compte à rebours chaque seconde
		let n = 0;
		timer = setInterval(() => {
			now = Date.now();
			if (++n % 5 === 0) void load();
		}, 1000);
	});
	onDestroy(() => clearInterval(timer));

	/**
	 * Dernier état connu : celui publié depuis le démarrage, sinon celui des
	 * entités de l'appareil (`<ieee>_linkquality_zigbee2mqtt`…).
	 */
	function fromEntities(ieee: string, key: string): string | undefined {
		const e = registry.index.entities.find((x) => x.unique_id === `${ieee}_${key}_zigbee2mqtt`);
		const st = e ? live.states.get(e.entity_id) : undefined;
		return st && st.state !== 'unavailable' && st.state !== 'unknown' ? st.state : undefined;
	}
	const devices = $derived(
		devicesOf(snapshot).map((d) => {
			const lqi = d.lqi ?? Number(fromEntities(d.ieee, 'linkquality'));
			const battery = d.battery ?? Number(fromEntities(d.ieee, 'battery'));
			return {
				...d,
				lqi: Number.isFinite(lqi) ? lqi : undefined,
				battery: Number.isFinite(battery) ? battery : undefined,
				lastSeen: d.lastSeen ?? fromEntities(d.ieee, 'last_seen')
			};
		})
	);
	const groups = $derived(groupsOf(snapshot));
	const events = $derived(eventsOf(snapshot));
	const joinLeft = $derived(permitJoinLeft(snapshot, now));
	const info = $derived((snapshot as { info?: Record<string, unknown> } | null)?.info ?? {});
	const shown = $derived(
		devices.filter(
			(d) =>
				!q ||
				`${d.name} ${d.model ?? ''} ${d.vendor ?? ''}`
					.toLowerCase()
					.includes(q.trim().toLowerCase())
		)
	);
	/** Appareil RustHome correspondant (fiche avec ses entités). */
	const deviceId = (ieee: string) =>
		registry.index.devices.find((d) => d.id === ieee || d.id.endsWith(ieee))?.id;

	async function act(method: 'POST' | 'DELETE', path: string, body: unknown, ok: string) {
		notice = null;
		const r = await request(method, path, body);
		notice = r.ok ? { kind: 'ok', text: ok } : { kind: 'error', text: r.error };
		void load();
	}
	const permit = (seconds: number) =>
		act(
			'POST',
			'zigbee/permit_join',
			{ seconds },
			seconds ? `Appairage ouvert pendant ${seconds} s.` : 'Appairage fermé.'
		);
	const ota = (d: ZDevice, action: 'check' | 'update') =>
		act(
			'POST',
			`zigbee/devices/${encodeURIComponent(d.ieee)}/ota`,
			{ action },
			action === 'check'
				? `Recherche de mise à jour lancée pour ${d.name}.`
				: `Mise à jour de ${d.name} lancée.`
		);
	function remove(d: ZDevice) {
		if (!confirm(`Retirer « ${d.name} » du réseau Zigbee ?`)) return;
		void act(
			'POST',
			`zigbee/devices/${encodeURIComponent(d.ieee)}/remove`,
			{ force: false },
			`${d.name} retiré.`
		);
	}
	function addGroup(e: SubmitEvent) {
		e.preventDefault();
		if (!newGroup.trim()) return;
		void act(
			'POST',
			'zigbee/groups',
			{ name: newGroup.trim() },
			`Groupe « ${newGroup.trim()} » créé.`
		);
		newGroup = '';
	}
	const nameOf = (ieee: string) => devices.find((d) => d.ieee === ieee)?.name ?? ieee;
	const LQI_LABEL = { good: 'bonne', warning: 'moyenne', critical: 'faible' };
</script>

<div class="head">
	<h1><Icon path={mdiZigbee} /> Zigbee2MQTT</h1>
	<div class="join">
		{#if joinLeft > 0}
			<span class="pulse"></span>
			<span>Appairage ouvert : <strong>{joinLeft} s</strong></span>
			<button class="dbtn" onclick={() => permit(0)}>Arrêter</button>
		{:else}
			<button class="dbtn primary" onclick={() => permit(254)}>Appairer un appareil</button>
		{/if}
	</div>
</div>

{#if info.version}
	<p class="meta">
		Zigbee2MQTT {String(info.version)}
		{#if (info.network as Record<string, unknown> | undefined)?.channel}
			· canal {String((info.network as Record<string, unknown>).channel)}{/if}
		· {devices.length} appareil(s)
	</p>
{/if}
{#if error}<p class="card">Zigbee2MQTT indisponible : {error}</p>{/if}
{#if notice}<p class="card msg {notice.kind}" role="status">{notice.text}</p>{/if}

<div class="tabs" role="tablist">
	<button role="tab" aria-selected={tab === 'devices'} onclick={() => (tab = 'devices')}
		>Appareils ({devices.length})</button
	>
	<button role="tab" aria-selected={tab === 'groups'} onclick={() => (tab = 'groups')}
		>Groupes ({groups.length})</button
	>
	<button role="tab" aria-selected={tab === 'events'} onclick={() => (tab = 'events')}
		>Événements ({events.length})</button
	>
</div>

{#if tab === 'devices'}
	<div class="card bar">
		<input
			type="search"
			placeholder="Rechercher un appareil…"
			bind:value={q}
			aria-label="Recherche"
		/>
	</div>
	<section class="card">
		<div class="scroll">
			<table>
				<thead>
					<tr>
						<th>Appareil</th>
						<th>Modèle</th>
						<th>Lien</th>
						<th>Batterie</th>
						<th>Dernier contact</th>
						<th>Mise à jour</th>
						<th></th>
					</tr>
				</thead>
				<tbody>
					{#each shown as d (d.ieee)}
						{@const lvl = lqiLevel(d.lqi)}
						{@const rid = deviceId(d.ieee)}
						<tr>
							<td>
								{#if rid}<a href={resolve('/appareils/[id]', { id: rid })}>{d.name}</a
									>{:else}{d.name}{/if}
								<small><code>{d.ieee}</code> · {d.type === 'Router' ? 'routeur' : 'terminal'}</small
								>
								{#if !d.interviewed}<span class="tag warn">identification incomplète</span>{/if}
							</td>
							<td class="muted">{[d.vendor, d.model].filter(Boolean).join(' ') || '—'}</td>
							<td>
								{#if lvl}<span class="lvl {lvl}" title="qualité {LQI_LABEL[lvl]}"
									></span>{d.lqi}{:else}—{/if}
							</td>
							<td>{d.battery !== undefined ? `${d.battery} %` : '—'}</td>
							<td class="muted">{d.lastSeen ? relativeTime(d.lastSeen) : '—'}</td>
							<td>
								{#if d.ota === 'available'}<span class="tag ok">disponible</span>
								{:else if d.ota === 'updating'}<span class="tag">{d.otaProgress ?? 0} %</span>
								{:else}<span class="muted">—</span>{/if}
							</td>
							<td class="r">
								<Menu
									label="Actions sur {d.name}"
									items={[
										{ label: 'Renommer', icon: mdiPencil, onclick: () => (renaming = d) },
										{
											label: 'Chercher une mise à jour',
											icon: mdiUpdate,
											onclick: () => ota(d, 'check')
										},
										{
											label: 'Installer la mise à jour',
											icon: mdiUpdate,
											disabled: d.ota !== 'available',
											onclick: () => ota(d, 'update')
										},
										{ label: 'Liaisons', icon: mdiLinkVariant, onclick: () => (binding = d) },
										{
											label: 'Retirer du réseau',
											icon: mdiDeleteOutline,
											danger: true,
											divider: true,
											onclick: () => remove(d)
										}
									]}
								/>
							</td>
						</tr>
					{:else}
						<tr
							><td colspan="7" class="muted"
								>Aucun appareil (le pont n’a pas encore publié sa liste).</td
							></tr
						>
					{/each}
				</tbody>
			</table>
		</div>
	</section>
{:else if tab === 'groups'}
	<form class="card bar" onsubmit={addGroup}>
		<input bind:value={newGroup} placeholder="Nom du nouveau groupe" aria-label="Nom du groupe" />
		<button class="dbtn primary" disabled={!newGroup.trim()}>Créer</button>
	</form>
	{#each groups as g (g.id)}
		<section class="card">
			<div class="ghead">
				<h2>{g.name} <small class="muted">#{g.id}</small></h2>
				<button
					class="dbtn danger"
					onclick={() =>
						confirm(`Supprimer le groupe « ${g.name} » ?`) &&
						act('DELETE', `zigbee/groups/${g.id}`, undefined, `Groupe « ${g.name} » supprimé.`)}
					>Supprimer</button
				>
			</div>
			<ul class="members">
				{#each g.members as m (m.ieee + (m.endpoint ?? ''))}
					<li>
						<span>{nameOf(m.ieee)}{m.endpoint !== undefined ? ` (point ${m.endpoint})` : ''}</span>
						<button
							class="mini"
							aria-label="Retirer {nameOf(m.ieee)}"
							onclick={() =>
								act(
									'POST',
									`zigbee/groups/${g.id}/members`,
									{ device: m.ieee, endpoint: m.endpoint, remove: true },
									'Membre retiré.'
								)}><Icon path={mdiDeleteOutline} size={18} /></button
						>
					</li>
				{:else}
					<li class="muted">Aucun membre.</li>
				{/each}
			</ul>
			<select
				aria-label="Ajouter un appareil au groupe"
				onchange={(e) => {
					const ieee = e.currentTarget.value;
					e.currentTarget.value = '';
					if (ieee)
						void act('POST', `zigbee/groups/${g.id}/members`, { device: ieee }, 'Membre ajouté.');
				}}
			>
				<option value="">Ajouter un appareil…</option>
				{#each devices.filter((d) => !g.members.some((m) => m.ieee === d.ieee)) as d (d.ieee)}
					<option value={d.ieee}>{d.name}</option>
				{/each}
			</select>
		</section>
	{:else}
		<p class="muted">Aucun groupe.</p>
	{/each}
{:else}
	<section class="card">
		<ol class="events">
			{#each events as e, i (i)}
				<li>
					<time>{e.received ? relativeTime(new Date(e.received).toISOString()) : ''}</time>
					<span>{eventLabel(e)}</span>
				</li>
			{:else}
				<li class="muted">Aucun événement depuis le démarrage de RustHome.</li>
			{/each}
		</ol>
	</section>
{/if}

<p class="note">
	MQTT reste en lecture seule pendant le développement : appairage, renommage, mises à jour et
	liaisons sont refusés avant tout envoi tant que la bascule n’est pas faite.
</p>

{#if renaming}
	<ZigbeeRename
		device={renaming}
		onclose={() => (renaming = null)}
		ondone={(m) => {
			renaming = null;
			notice = { kind: 'ok', text: m };
			void load();
		}}
	/>
{/if}
{#if binding}
	<ZigbeeBind
		device={binding}
		{devices}
		{groups}
		onclose={() => (binding = null)}
		ondone={(m) => {
			binding = null;
			notice = { kind: 'ok', text: m };
		}}
	/>
{/if}

<style>
	.head {
		display: flex;
		flex-wrap: wrap;
		justify-content: space-between;
		align-items: center;
		gap: 10px;
	}
	h1 {
		display: flex;
		align-items: center;
		gap: 10px;
	}
	.join {
		display: flex;
		align-items: center;
		gap: 10px;
	}
	.pulse {
		width: 10px;
		height: 10px;
		border-radius: 50%;
		background: var(--primary);
		animation: pulse 1.2s infinite;
	}
	@keyframes pulse {
		50% {
			box-shadow: 0 0 0 8px color-mix(in srgb, var(--primary) 0%, transparent);
			opacity: 0.6;
		}
	}
	.meta,
	.note,
	.muted {
		color: var(--text-muted);
		font-size: 0.85rem;
	}
	.msg.error {
		color: #e66767;
	}
	.tabs {
		display: flex;
		gap: 4px;
		margin-bottom: 12px;
		border-bottom: 1px solid var(--border);
	}
	.tabs button {
		padding: 10px 14px;
		border: none;
		border-bottom: 2px solid transparent;
		background: none;
		color: var(--text-muted);
		font: inherit;
		cursor: pointer;
	}
	.tabs button[aria-selected='true'] {
		color: var(--primary-text);
		border-bottom-color: var(--primary);
	}
	.bar {
		display: flex;
		gap: 8px;
	}
	.bar input {
		flex: 1;
		padding: 7px 10px;
		border: 1px solid var(--border);
		border-radius: 8px;
		background: var(--bg);
		color: var(--text);
		font: inherit;
	}
	.scroll {
		overflow-x: auto;
	}
	table {
		width: 100%;
		border-collapse: collapse;
		font-size: 0.9rem;
	}
	th {
		text-align: left;
		font-size: 0.8rem;
		color: var(--text-muted);
		font-weight: 600;
		padding: 6px;
		white-space: nowrap;
	}
	td {
		padding: 8px 6px;
		border-top: 1px solid var(--border);
		vertical-align: middle;
	}
	td a {
		color: var(--text);
		font-weight: 600;
	}
	td small {
		display: block;
		color: var(--text-muted);
		font-size: 0.75rem;
	}
	.r {
		text-align: right;
	}
	.lvl {
		display: inline-block;
		width: 8px;
		height: 8px;
		border-radius: 50%;
		margin-right: 6px;
	}
	.lvl.good {
		background: var(--good);
	}
	.lvl.warning {
		background: var(--warning);
	}
	.lvl.critical {
		background: var(--critical);
	}
	.tag {
		display: inline-block;
		font-size: 0.72rem;
		padding: 1px 8px;
		border-radius: 999px;
		background: var(--surface-2);
	}
	.tag.ok {
		background: color-mix(in srgb, var(--good) 22%, transparent);
	}
	.tag.warn {
		margin-left: 6px;
		background: color-mix(in srgb, var(--warning) 22%, transparent);
	}
	.ghead {
		display: flex;
		justify-content: space-between;
		align-items: center;
	}
	.danger {
		color: #e66767;
	}
	.members {
		list-style: none;
		margin: 8px 0;
		padding: 0;
	}
	.members li {
		display: flex;
		justify-content: space-between;
		align-items: center;
		padding: 6px 0;
		border-top: 1px solid var(--border);
	}
	.mini {
		display: grid;
		place-items: center;
		width: 30px;
		height: 30px;
		border: none;
		border-radius: 6px;
		background: none;
		color: var(--text-muted);
		cursor: pointer;
	}
	section select {
		padding: 6px 10px;
		border: 1px solid var(--border);
		border-radius: 8px;
		background: var(--surface-2);
		color: var(--text);
		font: inherit;
	}
	.events {
		list-style: none;
		margin: 0;
		padding: 0;
	}
	.events li {
		display: grid;
		grid-template-columns: 140px minmax(0, 1fr);
		gap: 10px;
		padding: 7px 0;
		border-top: 1px solid var(--border);
		font-size: 0.9rem;
	}
	time {
		color: var(--text-muted);
	}
</style>

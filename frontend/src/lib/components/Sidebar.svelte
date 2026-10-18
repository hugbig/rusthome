<script lang="ts">
	// Menu latéral façon HA : tableaux de bord affichés dans le menu, panneaux
	// (carte, journal…), puis Paramètres et le profil en bas.
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import {
		mdiChartBox,
		mdiChip,
		mdiCog,
		mdiFormatListBulletedType,
		mdiLightningBolt,
		mdiMap,
		mdiPlayBoxMultiple,
		mdiShieldHome,
		mdiViewGridOutline,
		mdiZigbee
	} from '@mdi/js';
	import logo from '../assets/logo.svg';
	import Icon from '../dashboard/Icon.svelte';
	import { iconPath } from '../dashboard/icons';
	import { dashboards } from '../stores/dashboards.svelte';
	import { live } from '../stores/live.svelte';
	import { ROLE_LABELS, session } from '../stores/session.svelte';

	let { onnavigate }: { onnavigate?: () => void } = $props();

	const panels = [
		{ href: resolve('/energie'), label: 'Énergie', icon: mdiLightningBolt },
		{ href: resolve('/carte'), label: 'Carte', icon: mdiMap },
		{ href: resolve('/journal'), label: 'Journal', icon: mdiFormatListBulletedType },
		{ href: resolve('/historique'), label: 'Historique', icon: mdiChartBox },
		{ href: resolve('/media'), label: 'Média', icon: mdiPlayBoxMultiple },
		{ href: resolve('/alarme'), label: 'Alarme', icon: mdiShieldHome },
		{ href: resolve('/esphome'), label: 'ESPHome', icon: mdiChip },
		{ href: resolve('/zigbee'), label: 'Zigbee2MQTT', icon: mdiZigbee }
	];
	const home = resolve('/');
	const settings = resolve('/parametres');
	const profile = resolve('/profil');
	const path = $derived(page.url.pathname);
	const isActive = (href: string) =>
		href === home ? path === href : path === href || path.startsWith(`${href}/`);
	// Les pages de configuration existantes relèvent de Paramètres.
	const inSettings = $derived(
		['/parametres', '/entites', '/appareils', '/pieces', '/automatisations', '/tableaux'].some(
			(p) =>
				path === resolve(p as '/parametres') || path.startsWith(`${resolve(p as '/parametres')}/`)
		) && !dashboards.sidebar.some((d) => path.startsWith(dashHref(d.url_path)))
	);
	const dashHref = (url_path: string) =>
		resolve('/tableaux/[dashboard]/[[vue]]', { dashboard: url_path, vue: undefined });
</script>

<nav class="sidebar" aria-label="Navigation principale">
	<a class="brand" href={home} onclick={onnavigate}>
		<img src={logo} alt="" width="30" height="30" />
		<span>RustHome</span>
	</a>

	<ul>
		<li>
			<a href={home} onclick={onnavigate} aria-current={isActive(home) ? 'page' : undefined}>
				<Icon path={mdiViewGridOutline} size={22} /><span>Aperçu</span>
			</a>
		</li>
		{#each dashboards.sidebar as d (d.url_path)}
			{@const href = dashHref(d.url_path)}
			<li>
				<a {href} onclick={onnavigate} aria-current={isActive(href) ? 'page' : undefined}>
					<Icon path={iconPath(d.icon ?? 'mdi:view-dashboard')} size={22} /><span>{d.title}</span>
				</a>
			</li>
		{/each}
	</ul>
	<hr />
	<ul>
		{#each panels as l (l.href)}
			<li>
				<a href={l.href} onclick={onnavigate} aria-current={isActive(l.href) ? 'page' : undefined}>
					<Icon path={l.icon} size={22} /><span>{l.label}</span>
				</a>
			</li>
		{/each}
	</ul>

	<div class="foot">
		<ul>
			{#if session.me?.role === 'admin'}
				<li>
					<a href={settings} onclick={onnavigate} aria-current={inSettings ? 'page' : undefined}>
						<Icon path={mdiCog} size={22} /><span>Paramètres</span>
					</a>
				</li>
			{/if}
			{#if session.me}
				<li>
					<a
						href={profile}
						onclick={onnavigate}
						aria-current={isActive(profile) ? 'page' : undefined}
						title={ROLE_LABELS[session.me.role]}
					>
						<span class="avatar">{session.me.name.slice(0, 1).toUpperCase()}</span>
						<span>{session.me.name}</span>
						<span
							class="live {live.status}"
							role="status"
							title={live.status === 'live'
								? 'Connecté en direct'
								: live.status === 'connecting'
									? 'Connexion…'
									: 'Hors ligne'}
						></span>
					</a>
				</li>
			{/if}
		</ul>
	</div>
</nav>

<style>
	.sidebar {
		display: flex;
		flex-direction: column;
		gap: 4px;
		height: 100%;
		padding: 12px 8px 8px;
		box-sizing: border-box;
		overflow-y: auto;
	}
	.brand {
		display: flex;
		align-items: center;
		gap: 10px;
		padding: 4px 12px 14px;
		text-decoration: none;
		font-weight: 700;
		font-size: 1.15rem;
		color: var(--primary-text);
	}
	ul {
		list-style: none;
		margin: 0;
		padding: 0;
		display: flex;
		flex-direction: column;
		gap: 2px;
	}
	ul a {
		display: flex;
		align-items: center;
		gap: 16px;
		padding: 9px 12px;
		border-radius: 10px;
		color: var(--text);
		text-decoration: none;
		font-size: 0.95rem;
	}
	ul a :global(svg) {
		color: var(--text-muted);
	}
	ul a:hover {
		background: var(--surface-2);
	}
	ul a[aria-current='page'] {
		background: color-mix(in srgb, var(--primary) 16%, transparent);
		color: var(--primary-text);
		font-weight: 600;
	}
	ul a[aria-current='page'] :global(svg) {
		color: var(--primary-text);
	}
	span {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	hr {
		width: calc(100% - 16px);
		border: none;
		border-top: 1px solid var(--border);
		margin: 8px;
	}
	.foot {
		margin-top: auto;
		padding-top: 8px;
		border-top: 1px solid var(--border);
	}
	.avatar {
		display: grid;
		place-items: center;
		flex: none;
		width: 24px;
		height: 24px;
		border-radius: 50%;
		background: var(--primary);
		color: var(--on-primary);
		font-size: 0.8rem;
		font-weight: 600;
	}
	.live {
		margin-left: auto;
		flex: none;
		width: 8px;
		height: 8px;
		border-radius: 50%;
		background: #d89b00;
	}
	.live.live {
		background: #1c7c4a;
	}
	.live.offline {
		background: #b3261e;
	}
</style>

<script lang="ts">
	import { onMount } from 'svelte';
	import { resolve } from '$app/paths';
	import { request } from '#lib/api.ts';
	import { live } from '#lib/stores/live.svelte.ts';

	interface Sources {
		env?: string;
		auth?: string;
		timezone?: string;
		retention_days?: number;
		ha_mirror?: string | null;
		mqtt?: {
			broker: string | null;
			read_only: boolean;
			z2m_base_topic: string;
			frigate: string | null;
		} | null;
		hue?: { bridge: string | null; read_only: boolean } | null;
		portainer?: string | null;
	}
	interface System {
		version: string;
		entities: number;
		recorder: boolean;
		automations: boolean;
		dashboards: boolean;
		sources: Sources | null;
	}
	let sys = $state<System | null>(null);
	let error = $state<string | null>(null);
	onMount(async () => {
		const r = await request<System>('GET', 'system');
		if (r.ok) sys = r.data;
		else error = r.error;
	});
	const AUTH: Record<string, string> = {
		dev: 'Développement (utilisateur local)',
		oidc: 'Authelia (OIDC)',
		disabled: 'Désactivée'
	};
	const nf = new Intl.NumberFormat('fr-FR');
</script>

<p class="crumbs"><a href={resolve('/parametres')}>Paramètres</a></p>
<h1>Système</h1>

{#if error}
	<p class="card">Informations indisponibles : {error}</p>
{:else if sys}
	{@const src = sys.sources ?? {}}
	<section class="card">
		<h2>Sécurité de la maison</h2>
		<p class="note">
			Pendant le développement, RustHome ne commande rien de réel. Chaque bascule se fait
			explicitement, phase par phase.
		</p>
		<dl>
			<dt>MQTT (Zigbee2MQTT, Frigate)</dt>
			<dd>
				{#if src.mqtt}<span class="flag {src.mqtt.read_only ? 'safe' : 'live'}"
						>{src.mqtt.read_only ? 'Lecture seule' : 'Commandes actives'}</span
					>{:else}Non configuré{/if}
			</dd>
			<dt>Philips Hue</dt>
			<dd>
				{#if src.hue}<span class="flag {src.hue.read_only ? 'safe' : 'live'}"
						>{src.hue.read_only ? 'Lecture seule' : 'Commandes actives'}</span
					>{:else}Non configuré{/if}
			</dd>
			<dt>Home Assistant</dt>
			<dd>
				{#if src.ha_mirror}<span class="flag safe">Miroir en lecture seule</span>
					<code>{src.ha_mirror}</code>{:else}Non configuré (données de démo){/if}
			</dd>
			<dt>Portainer</dt>
			<dd>
				{#if src.portainer}<span class="flag safe">Lecture seule</span>
					<code>{src.portainer}</code>{:else}Non configuré{/if}
			</dd>
		</dl>
	</section>

	<section class="card">
		<h2>Sources</h2>
		<dl>
			<dt>Broker MQTT</dt>
			<dd><code>{src.mqtt?.broker ?? '—'}</code></dd>
			<dt>Sujet Zigbee2MQTT</dt>
			<dd><code>{src.mqtt?.z2m_base_topic ?? '—'}</code></dd>
			<dt>Frigate</dt>
			<dd><code>{src.mqtt?.frigate ?? '—'}</code></dd>
			<dt>Pont Hue</dt>
			<dd><code>{src.hue?.bridge ?? '—'}</code></dd>
		</dl>
	</section>

	<section class="card">
		<h2>À propos</h2>
		<dl>
			<dt>Version</dt>
			<dd>RustHome {sys.version}</dd>
			<dt>Environnement</dt>
			<dd>{src.env === 'production' ? 'Production' : 'Développement'}</dd>
			<dt>Connexion</dt>
			<dd>{AUTH[src.auth ?? ''] ?? src.auth ?? '—'}</dd>
			<dt>Fuseau horaire</dt>
			<dd>{src.timezone ?? '—'}</dd>
			<dt>Entités</dt>
			<dd>{nf.format(sys.entities)} · flux {live.status === 'live' ? 'en direct' : live.status}</dd>
			<dt>Historique</dt>
			<dd>
				{sys.recorder ? `Activé, ${src.retention_days ?? '?'} jours conservés` : 'Désactivé'}
			</dd>
		</dl>
	</section>
{:else}
	<p class="muted">Chargement…</p>
{/if}

<style>
	.crumbs {
		margin: 0 0 8px;
		font-size: 0.9rem;
	}
	section {
		max-width: 760px;
	}
	.note,
	.muted {
		font-size: 0.85rem;
		color: var(--text-muted);
	}
	dl {
		display: grid;
		grid-template-columns: minmax(140px, 220px) minmax(0, 1fr);
		gap: 8px 16px;
		margin: 0;
	}
	dt {
		color: var(--text-muted);
	}
	dd {
		margin: 0;
		display: flex;
		flex-wrap: wrap;
		gap: 8px;
		align-items: center;
	}
	.flag {
		font-size: 0.78rem;
		padding: 2px 8px;
		border-radius: 999px;
	}
	.flag.safe {
		background: #1c7c4a;
		color: #fff;
	}
	.flag.live {
		background: #b26b00;
		color: #fff;
	}
</style>

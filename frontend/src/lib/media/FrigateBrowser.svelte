<script lang="ts">
	// Frigate, comme sa source dans HA : Clips (vidéo d'un événement),
	// Instantanés (image d'un événement) et Enregistrements (par jour et heure,
	// dans le fuseau du navigateur), pour une caméra ou toutes.
	import { request } from '../api';
	import { duration, frigateFile, hourRange, labelFr, scoreOf, type FrigateEvent } from './media';
	import MediaViewer from './MediaViewer.svelte';

	interface Props {
		camera?: string;
		tab: 'clips' | 'snapshots' | 'recordings';
		onnavigate: (params: Record<string, string | undefined>) => void;
	}
	let { camera, tab, onnavigate }: Props = $props();

	let cameras = $state<string[]>([]);
	let error = $state<string | null>(null);
	let events = $state<FrigateEvent[]>([]);
	let label = $state('');
	let more = $state(true);
	let loading = $state(false);
	interface Day {
		camera: string;
		day: string;
		events?: number;
		hours: { hour: string | number; duration?: number; events?: number }[];
	}
	let days = $state<Day[]>([]);
	let viewing = $state<{
		title: string;
		subtitle?: string;
		kind: 'video' | 'image';
		src: string;
		hls?: string;
		poster?: string;
		fallback?: string;
	} | null>(null);

	$effect(() => {
		void request<{ cameras: string[] }>('GET', 'media/frigate').then((r) => {
			if (r.ok) cameras = r.data.cameras;
			else error = r.error;
		});
	});

	async function loadEvents(reset: boolean) {
		loading = true;
		const params: Record<string, string> = { limit: '48' };
		if (camera) params.camera = camera;
		if (label) params.label = label;
		if (!reset && events.length) params.before = String(events.at(-1)!.start_time);
		params.kind = tab === 'snapshots' ? 'snapshots' : 'clips';
		const q = new URLSearchParams(params).toString();
		const r = await request<FrigateEvent[]>('GET', `media/frigate/events?${q}`);
		loading = false;
		if (!r.ok) {
			error = r.error;
			return;
		}
		events = reset ? r.data : [...events, ...r.data];
		more = r.data.length === 48;
	}
	$effect(() => {
		void camera;
		void label;
		if (tab !== 'recordings') void loadEvents(true);
	});
	// Enregistrements d'une caméra, ou de toutes ; jours et heures locaux.
	const tz = Intl.DateTimeFormat().resolvedOptions().timeZone;
	$effect(() => {
		const list = camera ? [camera] : cameras;
		if (tab !== 'recordings' || !list.length) return;
		days = [];
		void Promise.all(
			list.map((c) =>
				request<Omit<Day, 'camera'>[]>(
					'GET',
					`media/frigate/${encodeURIComponent(c)}/recordings?timezone=${encodeURIComponent(tz)}`
				).then((r) => {
					if (!r.ok) error = r.error;
					return r.ok ? r.data.map((d) => ({ ...d, camera: c })) : [];
				})
			)
		).then((all) => (days = all.flat().sort((a, b) => b.day.localeCompare(a.day))));
	});

	const labels = $derived([...new Set(events.map((e) => e.label).filter(Boolean))] as string[]);
	const when = new Intl.DateTimeFormat('fr-FR', {
		weekday: 'short',
		day: 'numeric',
		month: 'short',
		hour: '2-digit',
		minute: '2-digit'
	});
	const dayFmt = new Intl.DateTimeFormat('fr-FR', {
		weekday: 'long',
		day: 'numeric',
		month: 'long'
	});
	/** Durée enregistrée dans l'heure (« 53 s », « 12 min »). */
	const recorded = (s: number | undefined) =>
		!s ? '' : s < 60 ? `${Math.round(s)} s` : `${Math.round(s / 60)} min`;

	function openEvent(e: FrigateEvent) {
		const title = `${labelFr(e.label)} · ${e.camera}`;
		const subtitle = when.format(e.start_time * 1000);
		viewing =
			tab === 'clips' && e.has_clip
				? {
						title,
						subtitle,
						kind: 'video',
						src: frigateFile.clip(e.id),
						hls: frigateFile.clipHls(e.id),
						poster: frigateFile.thumbnail(e.id),
						fallback: e.has_snapshot ? frigateFile.snapshot(e.id) : undefined
					}
				: { title, subtitle, kind: 'image', src: frigateFile.snapshot(e.id) };
	}
	function openHour(camera: string, day: string, hour: number) {
		const [s, e] = hourRange(day, hour);
		viewing = {
			title: `${camera} · ${hour} h – ${hour + 1} h`,
			subtitle: dayFmt.format(s * 1000),
			kind: 'video',
			src: frigateFile.recording(camera, s, e),
			hls: frigateFile.recordingHls(camera, s, e)
		};
	}
</script>

{#if error}<p class="card">Frigate indisponible : {error}</p>{/if}

<div class="cams">
	<button
		class="cam all"
		aria-pressed={!camera}
		onclick={() => onnavigate({ camera: undefined, onglet: undefined })}>Toutes les caméras</button
	>
	{#each cameras as c (c)}
		<button class="cam" aria-pressed={camera === c} onclick={() => onnavigate({ camera: c })}>
			<img src={frigateFile.latest(c)} alt="" loading="lazy" />
			<span>{c}</span>
		</button>
	{/each}
</div>

<div class="tabs" role="tablist">
	<button
		role="tab"
		aria-selected={tab === 'clips'}
		onclick={() => onnavigate({ onglet: undefined })}>Clips</button
	>
	<button
		role="tab"
		aria-selected={tab === 'snapshots'}
		onclick={() => onnavigate({ onglet: 'instantanes' })}>Instantanés</button
	>
	<button
		role="tab"
		aria-selected={tab === 'recordings'}
		onclick={() => onnavigate({ onglet: 'enregistrements' })}>Enregistrements</button
	>
	{#if tab !== 'recordings' && labels.length > 1}
		<select bind:value={label} aria-label="Type d’objet">
			<option value="">Tous les objets</option>
			{#each labels as l (l)}<option value={l}>{labelFr(l)}</option>{/each}
		</select>
	{/if}
</div>

{#if tab !== 'recordings'}
	<div class="grid" class:snaps={tab === 'snapshots'}>
		{#each events as e (e.id)}
			<button class="ev" onclick={() => openEvent(e)}>
				<img
					src={tab === 'snapshots' ? frigateFile.snapshot(e.id) : frigateFile.thumbnail(e.id)}
					alt=""
					loading="lazy"
				/>
				<span class="meta">
					<strong
						>{labelFr(e.label)}{#if scoreOf(e) !== undefined}<small>
								{scoreOf(e)} %</small
							>{/if}</strong
					>
					<span>{e.camera} · {when.format(e.start_time * 1000)}</span>
				</span>
				<span class="badges">
					{#if tab === 'clips'}
						<span class="b">▶ Clip{duration(e) ? ` · ${duration(e)}` : ''}</span>
					{:else}
						<span class="b">Instantané</span>
					{/if}
					{#if !e.end_time}<span class="b live">En cours</span>{/if}
				</span>
			</button>
		{:else}
			{#if !loading}<p class="muted">
					{tab === 'clips' ? 'Aucun clip.' : 'Aucun instantané.'}
				</p>{/if}
		{/each}
	</div>
	{#if more && events.length}
		<button class="dbtn more" disabled={loading} onclick={() => loadEvents(false)}
			>{loading ? 'Chargement…' : 'Voir plus'}</button
		>
	{/if}
{:else}
	{#each days as d (d.camera + d.day)}
		<section class="card day">
			<h3>
				{dayFmt.format(new Date(`${d.day}T12:00:00`))}{#if !camera}<small> · {d.camera}</small>{/if}
			</h3>
			<div class="hours">
				{#each [...d.hours].sort((a, b) => Number(a.hour) - Number(b.hour)) as h (h.hour)}
					<button class="hour" onclick={() => openHour(d.camera, d.day, Number(h.hour))}>
						<strong>▶ {Number(h.hour)} h – {Number(h.hour) + 1} h</strong>
						<span
							>{recorded(h.duration)} enregistrées{h.events
								? ` · ${h.events} événement(s)`
								: ''}</span
						>
					</button>
				{/each}
			</div>
		</section>
	{:else}
		<p class="muted">
			Aucun enregistrement. Frigate ne garde des vidéos continues que si l’enregistrement est activé
			pour la caméra ; sinon, seules les heures autour des événements sont conservées.
		</p>
	{/each}
{/if}

{#if viewing}
	<MediaViewer {...viewing} onclose={() => (viewing = null)} />
{/if}

<style>
	.cams {
		display: flex;
		gap: 10px;
		overflow-x: auto;
		padding-bottom: 6px;
		margin-bottom: 12px;
	}
	.cam {
		flex: none;
		display: flex;
		flex-direction: column;
		gap: 6px;
		width: 168px;
		padding: 6px;
		border: 2px solid transparent;
		border-radius: 14px;
		background: var(--surface);
		color: var(--text);
		font: inherit;
		font-size: 0.85rem;
		cursor: pointer;
	}
	.cam.all {
		justify-content: center;
		align-items: center;
		width: 120px;
	}
	.cam[aria-pressed='true'] {
		border-color: var(--primary);
	}
	.cam img {
		width: 100%;
		aspect-ratio: 16 / 9;
		object-fit: cover;
		border-radius: 9px;
		background: #000;
	}
	.tabs {
		display: flex;
		align-items: center;
		gap: 4px;
		margin-bottom: 12px;
		border-bottom: 1px solid var(--border);
	}
	.tabs [role='tab'] {
		padding: 10px 14px;
		border: none;
		border-bottom: 2px solid transparent;
		background: none;
		color: var(--text-muted);
		font: inherit;
		cursor: pointer;
	}
	.tabs [role='tab'][aria-selected='true'] {
		color: var(--primary-text);
		border-bottom-color: var(--primary);
	}
	.tabs [role='tab']:disabled {
		opacity: 0.4;
		cursor: default;
	}
	.tabs select {
		margin-left: auto;
		padding: 6px 10px;
		border: 1px solid var(--border);
		border-radius: 8px;
		background: var(--surface-2);
		color: var(--text);
		font: inherit;
	}
	.grid {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(230px, 1fr));
		gap: 12px;
	}
	.ev {
		position: relative;
		display: flex;
		flex-direction: column;
		padding: 0;
		border: 1px solid var(--border);
		border-radius: var(--radius);
		overflow: hidden;
		background: var(--surface);
		color: var(--text);
		font: inherit;
		text-align: left;
		cursor: pointer;
		box-shadow: var(--shadow);
	}
	.snaps .ev img {
		aspect-ratio: 16 / 9;
	}
	.ev img {
		width: 100%;
		aspect-ratio: 4 / 3;
		object-fit: cover;
		background: #000;
	}
	.meta {
		display: flex;
		flex-direction: column;
		gap: 2px;
		padding: 8px 10px 10px;
		font-size: 0.82rem;
		color: var(--text-muted);
	}
	.meta strong {
		color: var(--text);
		font-size: 0.92rem;
	}
	.meta small {
		margin-left: 6px;
		color: var(--text-muted);
		font-weight: 500;
	}
	.badges {
		position: absolute;
		top: 8px;
		left: 8px;
		display: flex;
		gap: 4px;
	}
	.b {
		padding: 2px 8px;
		border-radius: 999px;
		background: rgb(0 0 0 / 0.65);
		color: #fff;
		font-size: 0.72rem;
	}
	.b.live {
		background: #e53935;
	}
	.more {
		margin-top: 14px;
	}
	.day h3 {
		text-transform: capitalize;
	}
	.day h3 small {
		margin-left: 6px;
		color: var(--text-muted);
		font-weight: 500;
	}
	.hours {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
		gap: 8px;
	}
	.hour {
		display: flex;
		flex-direction: column;
		gap: 2px;
		padding: 8px 10px;
		border: 1px solid var(--border);
		border-radius: 10px;
		background: var(--surface-2);
		color: var(--text);
		font: inherit;
		text-align: left;
		cursor: pointer;
	}
	.hour:hover {
		border-color: var(--primary);
	}
	.hour span {
		font-size: 0.75rem;
		color: var(--text-muted);
	}
	.muted {
		color: var(--text-muted);
	}
</style>

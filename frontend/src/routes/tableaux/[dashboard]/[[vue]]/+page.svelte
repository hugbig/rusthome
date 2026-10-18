<script lang="ts">
	import { onDestroy, onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import {
		mdiAlertCircleOutline,
		mdiCodeBraces,
		mdiCog,
		mdiFullscreen,
		mdiHomeImportOutline,
		mdiHomeRemoveOutline,
		mdiPencil,
		mdiPlus,
		mdiRefresh
	} from '@mdi/js';
	import { parse, stringify } from 'yaml';
	import { request } from '#lib/api.ts';
	import Menu from '#lib/components/Menu.svelte';
	import YamlEditor from '#lib/components/YamlEditor.svelte';
	import CardEditor from '#lib/dashboard/editor/CardEditor.svelte';
	import CardPicker from '#lib/dashboard/editor/CardPicker.svelte';
	import DashboardSettings from '#lib/dashboard/editor/DashboardSettings.svelte';
	import ViewEditor from '#lib/dashboard/editor/ViewEditor.svelte';
	import Icon from '#lib/dashboard/Icon.svelte';
	import { iconPath, UI } from '#lib/dashboard/icons.ts';
	import {
		addView,
		appendCard,
		appendSection,
		clone,
		duplicateCard,
		moveCard,
		moveCardTo,
		moveSection,
		moveView,
		removeCard,
		removeSection,
		removeView,
		replaceCard,
		updateSection,
		updateView,
		type CardLoc
	} from '#lib/dashboard/layout.ts';
	import type { Card, DashboardDoc, DashboardResponse, View } from '#lib/dashboard/model.ts';
	import { viewKey } from '#lib/dashboard/model.ts';
	import MoreInfo from '#lib/dashboard/MoreInfo.svelte';
	import { ui, watchHash } from '#lib/dashboard/runtime.svelte.ts';
	import ViewLayout, { type EditOp } from '#lib/dashboard/ViewLayout.svelte';
	import { dashboards } from '#lib/stores/dashboards.svelte.ts';
	import { session } from '#lib/stores/session.svelte.ts';

	const urlPath = $derived(page.params.dashboard ?? '');
	let data = $state<DashboardResponse | null>(null);
	let error = $state<string | null>(null);

	// Mode édition : chaque modification est enregistrée aussitôt (comme HA).
	let editing = $state(false);
	let doc = $state<DashboardDoc | null>(null);
	let saving = $state(false);
	let saveError = $state<string | null>(null);
	let rawYaml = $state<string | null>(null);
	let showIssues = $state(false);
	let cardEdit = $state<{ card: Card; loc?: CardLoc; where?: Omit<CardLoc, 'index'> } | null>(null);
	let picking = $state<Omit<CardLoc, 'index'> | null>(null);
	let viewEdit = $state<{ index: number | null } | null>(null);
	let settingsOpen = $state(false);

	async function load() {
		const r = await request<DashboardResponse>('GET', `dashboards/${encodeURIComponent(urlPath)}`);
		if (r.ok) {
			data = r.data;
			doc = r.data.doc;
			error = null;
			// Lien direct vers l'édition : /tableaux/<tableau>?modifier
			if (r.data.editable && page.url.searchParams.has('modifier')) editing = true;
		} else error = r.error;
	}
	$effect(() => {
		void urlPath;
		editing = false;
		void load();
	});

	const viewIndex = $derived.by(() => {
		if (!doc) return 0;
		const v = page.params.vue;
		const i = doc.views.findIndex((x, k) => viewKey(x, k) === v);
		return i >= 0 ? i : 0;
	});
	const view = $derived(doc?.views[viewIndex]);
	const isAdmin = $derived(session.me?.role === 'admin');

	// Kiosque : imposé par le tableau, forçable par ?kiosk / ?kiosk=0.
	$effect(() => {
		const q = page.url.searchParams.get('kiosk');
		ui.kiosk = !editing && (q === null ? data?.doc.kiosk === true : q !== '0');
	});
	let stopHash: (() => void) | undefined;
	onMount(() => {
		stopHash = watchHash();
		// Lien direct vers la fiche d'une entité : ?fiche=light.salon
		const f = page.url.searchParams.get('fiche');
		if (f) ui.moreInfo = f;
	});
	onDestroy(() => {
		stopHash?.();
		ui.kiosk = false;
		ui.moreInfo = null;
	});

	const viewHref = (i: number, d: DashboardDoc | null = doc) =>
		resolve('/tableaux/[dashboard]/[[vue]]', {
			dashboard: urlPath,
			vue: d?.views[i] ? viewKey(d.views[i], i) : undefined
		});

	// Balayage horizontal entre vues (tablette murale).
	let touchX: number | null = null;
	function swipeEnd(e: TouchEvent) {
		if (touchX === null || !doc || editing) return;
		const dx = (e.changedTouches[0]?.clientX ?? touchX) - touchX;
		touchX = null;
		if (Math.abs(dx) < 80) return;
		const next = viewIndex + (dx < 0 ? 1 : -1);
		// eslint-disable-next-line svelte/no-navigation-without-resolve -- viewHref passe par resolve()
		if (next >= 0 && next < doc.views.length) void goto(viewHref(next) + page.url.search);
	}

	// --- enregistrement ---
	async function commit(next: DashboardDoc) {
		doc = next;
		saving = true;
		const r = await request<{ issues: DashboardResponse['issues'] }>(
			'PUT',
			`dashboards/${encodeURIComponent(urlPath)}`,
			{ yaml: JSON.stringify(next) }
		);
		saving = false;
		if (!r.ok) {
			saveError = r.error;
			return;
		}
		saveError = null;
		if (data) data = { ...data, doc: next, issues: r.data.issues };
		void dashboards.load();
	}

	function onop(op: EditOp) {
		if (!doc) return;
		switch (op.kind) {
			case 'edit': {
				const v = doc.views[op.loc.view];
				const c =
					op.loc.section !== undefined
						? v?.sections?.[op.loc.section]?.cards[op.loc.index]
						: v?.cards?.[op.loc.index];
				if (c) cardEdit = { card: c, loc: op.loc };
				return;
			}
			case 'add':
				picking = op.where;
				return;
			case 'remove':
				if (confirm('Supprimer cette carte ?')) void commit(removeCard(doc, op.loc));
				return;
			case 'move':
				void commit(moveCard(doc, op.loc, op.delta));
				return;
			case 'moveTo':
				void commit(moveCardTo(doc, op.from, op.to));
				return;
			case 'duplicate':
				void commit(duplicateCard(doc, op.loc));
				return;
			case 'addSection':
				void commit(appendSection(doc, viewIndex));
				return;
			case 'section': {
				const s = op.section;
				if (op.action === 'remove') {
					if (confirm('Supprimer cette section et ses cartes ?'))
						void commit(removeSection(doc, viewIndex, s));
				} else if (op.action === 'span') {
					const cur = doc.views[viewIndex]?.sections?.[s]?.column_span ?? 1;
					void commit(updateSection(doc, viewIndex, s, { column_span: cur > 1 ? undefined : 2 }));
				} else void commit(moveSection(doc, viewIndex, s, op.action === 'up' ? -1 : 1));
				return;
			}
		}
	}

	function saveCard(c: Card) {
		if (!doc || !cardEdit) return;
		if (cardEdit.loc) void commit(replaceCard(doc, cardEdit.loc, c));
		else if (cardEdit.where) void commit(appendCard(doc, cardEdit.where, c));
		cardEdit = null;
	}

	async function saveView(patch: Partial<View>) {
		if (!doc || !viewEdit) return;
		let next: DashboardDoc;
		let index: number;
		if (viewEdit.index === null) {
			next = addView(doc, patch.title ?? 'Nouvelle vue');
			index = next.views.length - 1;
			next = updateView(next, index, patch);
		} else {
			index = viewEdit.index;
			next = updateView(doc, index, patch);
		}
		viewEdit = null;
		await commit(next);

		await goto(viewHref(index, next), { replaceState: true });
	}

	async function deleteView() {
		if (!doc || viewEdit?.index == null || doc.views.length < 2) return;
		if (!confirm('Supprimer cette vue et toutes ses cartes ?')) return;
		const next = removeView(doc, viewEdit.index);
		viewEdit = null;
		await commit(next);

		await goto(viewHref(0, next), { replaceState: true });
	}

	async function shiftView(delta: -1 | 1) {
		if (!doc) return;
		const next = moveView(doc, viewIndex, delta);
		await commit(next);

		await goto(viewHref(viewIndex + delta, next), { replaceState: true });
	}

	async function saveRaw() {
		if (rawYaml === null) return;
		let parsed: DashboardDoc;
		try {
			parsed = parse(rawYaml) as DashboardDoc;
			if (!parsed || !Array.isArray(parsed.views)) throw new Error('il manque la liste `views`');
		} catch (e) {
			saveError = `YAML invalide : ${e instanceof Error ? e.message : e}`;
			return;
		}
		// Le serveur convertit aussi les cartes Lovelace collées : on relit.
		const r = await request('PUT', `dashboards/${encodeURIComponent(urlPath)}`, { yaml: rawYaml });
		if (!r.ok) {
			saveError = r.error;
			return;
		}
		saveError = null;
		rawYaml = null;
		await load();
		editing = true;
		void dashboards.load();
	}

	async function remove() {
		if (!confirm(`Supprimer le tableau « ${data?.summary.title} » ? Cette action est définitive.`))
			return;
		const r = await request('DELETE', `dashboards/${encodeURIComponent(urlPath)}`);
		if (r.ok) {
			void dashboards.load();
			await goto(resolve('/tableaux'));
		} else saveError = r.error;
	}

	// Tableau ouvert à l'adresse « / » (pour tous les comptes qui le voient).
	const isDefault = $derived(session.me?.default_dashboard === urlPath);
	async function setDefault(url_path: string | null) {
		const r = await request('PUT', 'settings/default-dashboard', { url_path });
		if (r.ok && session.me) session.me.default_dashboard = url_path;
		else if (!r.ok) saveError = r.error;
	}

	const viewMenu = $derived([
		...(data?.editable
			? [{ label: 'Modifier le tableau de bord', icon: mdiPencil, onclick: () => (editing = true) }]
			: []),
		{
			label: `Points à revoir (${data?.issues.length ?? 0})`,
			icon: mdiAlertCircleOutline,
			disabled: !data?.issues.length,
			onclick: () => (showIssues = !showIssues)
		},
		{
			label: 'Mode kiosque',
			icon: mdiFullscreen,
			onclick: () => {
				// eslint-disable-next-line svelte/no-navigation-without-resolve -- viewHref passe par resolve()
				void goto(`${viewHref(viewIndex)}?kiosk`);
			}
		},
		...(isAdmin
			? [
					isDefault
						? {
								label: 'Ne plus ouvrir à l’accueil',
								icon: mdiHomeRemoveOutline,
								onclick: () => void setDefault(null)
							}
						: {
								label: 'Ouvrir à l’accueil (tableau par défaut)',
								icon: mdiHomeImportOutline,
								onclick: () => void setDefault(urlPath)
							}
				]
			: []),
		{ label: 'Actualiser', icon: mdiRefresh, onclick: () => void load() }
	]);

	const editMenu = $derived([
		{ label: 'Réglages du tableau', icon: mdiCog, onclick: () => (settingsOpen = true) },
		{
			label: 'Éditeur YAML',
			icon: mdiCodeBraces,
			onclick: () => (rawYaml = doc ? stringify(doc) : '')
		},
		...(isAdmin
			? [
					{
						label: 'Supprimer le tableau',
						icon: UI.remove,
						danger: true,
						divider: true,
						onclick: remove
					}
				]
			: [])
	]);
</script>

{#if error}
	<p class="card">Tableau indisponible : {error}</p>
	<p><a href={resolve('/tableaux')}>Tous les tableaux</a></p>
{:else if doc && data}
	{#if !ui.kiosk}
		<header class="bar" class:editing>
			{#if editing}
				<span class="etitle"
					>Modification du tableau de bord {saving ? '· enregistrement…' : ''}</span
				>
				<Menu inverse items={editMenu} label="Options d’édition" />
				<button class="done" onclick={() => (editing = false)}>Terminé</button>
			{:else}
				<h1>
					<Icon path={iconPath(doc.icon ?? 'mdi:view-dashboard')} />
					{doc.title ?? urlPath}
				</h1>
				<span class="spacer"></span>
				<Menu items={viewMenu} />
			{/if}
		</header>

		{#if saveError}<p class="msg card" role="alert">{saveError}</p>{/if}

		{#if showIssues && !editing}
			<section class="card issues">
				<h2>À revoir</h2>
				<ul>
					{#each data.issues as i (i.path + i.message)}
						<li><code>{i.path}</code> {i.message}</li>
					{/each}
				</ul>
			</section>
		{/if}

		{#if doc.views.length > 1 || editing}
			<nav class="tabs" aria-label="Vues">
				{#each doc.views as v, i (i)}
					<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -- viewHref passe par resolve() -->
					<a
						href={viewHref(i)}
						aria-current={i === viewIndex ? 'page' : undefined}
						title={v.title}
						aria-label={v.title ?? `Vue ${i + 1}`}
					>
						{#if v.icon}<Icon path={iconPath(v.icon)} size={18} />{/if}
						<!-- icône seule si le tableau le demande (titre en info-bulle) -->
						{#if !v.icon || (v.title && !doc.tabs_icon_only)}<span>{v.title ?? `Vue ${i + 1}`}</span
							>{/if}
					</a>
					{#if editing && i === viewIndex}
						<span class="vtools">
							<button aria-label="Vue précédente" disabled={i === 0} onclick={() => shiftView(-1)}
								>‹</button
							>
							<button aria-label="Modifier la vue" onclick={() => (viewEdit = { index: i })}
								><Icon path={mdiPencil} size={16} /></button
							>
							<button
								aria-label="Vue suivante"
								disabled={i === doc.views.length - 1}
								onclick={() => shiftView(1)}>›</button
							>
						</span>
					{/if}
				{/each}
				{#if editing}
					<button
						class="addview"
						aria-label="Ajouter une vue"
						onclick={() => (viewEdit = { index: null })}><Icon path={mdiPlus} size={20} /></button
					>
				{/if}
			</nav>
		{/if}
	{/if}

	{#if view}
		<div
			ontouchstart={(e) => (touchX = e.touches[0]?.clientX ?? null)}
			ontouchend={swipeEnd}
			role="presentation"
		>
			<ViewLayout {view} {viewIndex} {editing} {onop} />
		</div>
	{/if}
{:else}
	<p class="muted">Chargement…</p>
{/if}

{#if picking}
	<CardPicker
		onclose={() => (picking = null)}
		onpick={(c) => {
			cardEdit = { card: clone(c.template), where: picking ?? undefined };
			picking = null;
		}}
	/>
{/if}

{#if cardEdit}
	<CardEditor
		card={cardEdit.card}
		isNew={!cardEdit.loc}
		inSection={view?.type === 'sections'}
		onsave={saveCard}
		onclose={() => (cardEdit = null)}
	/>
{/if}

{#if viewEdit && doc}
	<ViewEditor
		view={viewEdit.index === null
			? { type: 'sections', title: '', max_columns: 4 }
			: doc.views[viewEdit.index]!}
		isNew={viewEdit.index === null}
		canDelete={doc.views.length > 1}
		onsave={saveView}
		ondelete={deleteView}
		onclose={() => (viewEdit = null)}
	/>
{/if}

{#if settingsOpen && doc}
	<DashboardSettings
		{doc}
		{isAdmin}
		onclose={() => (settingsOpen = false)}
		onsave={(p) => {
			settingsOpen = false;
			if (!doc) return;
			const next = { ...doc, ...p };
			for (const k of Object.keys(p) as (keyof DashboardDoc)[])
				if (next[k] === undefined) delete next[k];
			void commit(next);
		}}
	/>
{/if}

{#if rawYaml !== null}
	<div class="backdrop" role="presentation" onclick={() => (rawYaml = null)}></div>
	<div class="raw" role="dialog" aria-modal="true" aria-label="Éditeur YAML">
		<h2>Éditeur YAML</h2>
		<p class="note">
			Format RustHome ou Lovelace : les cartes Mushroom, Bubble, tempometer… sont converties à
			l’enregistrement.
		</p>
		<div class="rawbody"><YamlEditor bind:value={rawYaml} /></div>
		<div class="foot">
			<button class="dbtn" onclick={() => (rawYaml = null)}>Annuler</button>
			<button class="dbtn primary" onclick={saveRaw}>Enregistrer</button>
		</div>
	</div>
{/if}

<MoreInfo />

{#if ui.toast}
	<p class="toast {ui.toast.kind}" role="status">{ui.toast.text}</p>
{/if}

<style>
	.bar {
		display: flex;
		align-items: center;
		gap: 8px;
		min-height: 48px;
		margin: -4px 0 8px;
	}
	.bar h1 {
		display: flex;
		align-items: center;
		gap: 10px;
		margin: 0;
	}
	.bar.editing {
		position: sticky;
		top: 0;
		z-index: 15;
		margin: -16px -16px 12px;
		padding: 6px 12px 6px 20px;
		background: var(--primary);
		color: var(--on-primary);
	}
	.etitle {
		flex: 1;
		font-weight: 600;
	}
	.done {
		padding: 8px 16px;
		border: none;
		border-radius: 8px;
		background: rgb(255 255 255 / 0.18);
		color: var(--on-primary);
		font: inherit;
		font-weight: 600;
		cursor: pointer;
	}
	.spacer {
		flex: 1;
	}
	.tabs {
		display: flex;
		align-items: center;
		gap: 2px;
		overflow-x: auto;
		margin-bottom: 16px;
		border-bottom: 1px solid var(--border);
	}
	.tabs a {
		display: flex;
		align-items: center;
		gap: 6px;
		padding: 10px 14px;
		color: var(--text-muted);
		text-decoration: none;
		white-space: nowrap;
		border-bottom: 2px solid transparent;
		text-transform: uppercase;
		font-size: 0.85rem;
		letter-spacing: 0.02em;
	}
	.tabs a[aria-current='page'] {
		color: var(--primary-text);
		border-bottom-color: var(--primary);
	}
	.vtools {
		display: flex;
		align-items: center;
	}
	.vtools button,
	.addview {
		display: grid;
		place-items: center;
		width: 30px;
		height: 30px;
		border: none;
		border-radius: 6px;
		background: none;
		color: var(--text-muted);
		font-size: 1.2rem;
		cursor: pointer;
	}
	.vtools button:disabled {
		opacity: 0.3;
	}
	.vtools button:hover:not(:disabled),
	.addview:hover {
		background: var(--surface-2);
	}
	.msg {
		color: #e66767;
	}
	.issues ul {
		margin: 0;
		padding-left: 18px;
		font-size: 0.9rem;
	}
	.note,
	.muted {
		font-size: 0.85rem;
		color: var(--text-muted);
	}
	.backdrop {
		position: fixed;
		inset: 0;
		background: rgb(0 0 0 / 0.5);
		z-index: 100;
	}
	.raw {
		position: fixed;
		z-index: 101;
		left: 50%;
		top: 50%;
		transform: translate(-50%, -50%);
		width: min(900px, calc(100vw - 24px));
		height: min(88vh, 900px);
		display: flex;
		flex-direction: column;
		padding: 16px;
		box-sizing: border-box;
		background: var(--surface);
		border: 1px solid var(--border);
		border-radius: 16px;
	}
	.raw h2 {
		margin: 0;
	}
	.rawbody {
		flex: 1;
		min-height: 0;
		overflow: auto;
	}
	.rawbody :global(.cm-editor) {
		max-height: none;
	}
	.foot {
		display: flex;
		justify-content: flex-end;
		gap: 8px;
		padding-top: 12px;
	}
	.toast {
		position: fixed;
		z-index: 70;
		left: 50%;
		bottom: 16px;
		transform: translateX(-50%);
		max-width: calc(100vw - 32px);
		margin: 0;
		padding: 10px 16px;
		border-radius: 10px;
		background: var(--surface);
		border: 1px solid var(--border);
		box-shadow: 0 6px 16px rgb(0 0 0 / 0.2);
		font-size: 0.9rem;
	}
	.toast.error {
		border-color: #e66767;
	}
</style>

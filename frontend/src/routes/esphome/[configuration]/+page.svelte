<script lang="ts">
	// Éditeur d'un projet ESPHome : YAML vérifié à la frappe par le Builder
	// (en mémoire, rien n'est écrit), enregistrement, puis validation,
	// installation et logs. Projets verrouillés (alarme) en lecture seule.
	import { onMount } from 'svelte';
	import { beforeNavigate } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import {
		mdiArrowLeft,
		mdiCheckDecagramOutline,
		mdiContentSave,
		mdiLock,
		mdiUpload
	} from '@mdi/js';
	import { request } from '#lib/api.ts';
	import YamlEditor from '#lib/components/YamlEditor.svelte';
	import Icon from '#lib/dashboard/Icon.svelte';
	import { EspOps, type Project } from '#lib/esphome/ops.svelte.ts';
	import OpsHost from '#lib/esphome/OpsHost.svelte';

	interface Problem {
		message: string;
		range?: { start_line: number; start_col: number };
	}

	const configuration = $derived(page.params.configuration ?? '');
	let content = $state('');
	let saved = $state('');
	let locked = $state(false);
	let flashable = $state(false);
	let target = $state<string | null>(null);
	let loaded = $state(false);
	let error = $state<string | null>(null);
	let notice = $state<string | null>(null);
	let problems = $state<Problem[] | null>(null);
	let checking = $state(false);
	let saving = $state(false);
	const dirty = $derived(loaded && content !== saved);
	const ops = new EspOps();
	const project = $derived<Project>({
		configuration,
		title: configuration.replace(/\.ya?ml$/, ''),
		locked,
		flashable,
		target
	});

	onMount(async () => {
		const r = await request<{
			content: string;
			locked: boolean;
			flashable: boolean;
			target: string | null;
		}>('GET', `esphome/configs/${encodeURIComponent(configuration)}`);
		if (r.ok) {
			content = saved = r.data.content;
			locked = r.data.locked;
			flashable = r.data.flashable;
			target = r.data.target;
			loaded = true;
		} else error = r.error;
	});

	// Vérification à la frappe, avec un délai.
	$effect(() => {
		const text = content;
		if (!loaded) return;
		const t = setTimeout(async () => {
			checking = true;
			const r = await request<{ yaml_errors: Problem[]; validation_errors: Problem[] }>(
				'POST',
				`esphome/configs/${encodeURIComponent(configuration)}/check`,
				{ content: text }
			);
			checking = false;
			if (r.ok) problems = [...r.data.yaml_errors, ...r.data.validation_errors];
		}, 900);
		return () => clearTimeout(t);
	});

	async function save() {
		if (!dirty || locked) return;
		saving = true;
		const r = await request('PUT', `esphome/configs/${encodeURIComponent(configuration)}`, {
			content
		});
		saving = false;
		if (r.ok) {
			saved = content;
			notice = 'Enregistré.';
			error = null;
		} else error = r.error;
	}

	beforeNavigate((nav) => {
		if (dirty && !confirm('Des modifications ne sont pas enregistrées. Quitter quand même ?'))
			nav.cancel();
	});
</script>

<svelte:window
	onkeydown={(e) => {
		if ((e.ctrlKey || e.metaKey) && e.key === 's') {
			e.preventDefault();
			void save();
		}
	}}
	onbeforeunload={(e) => {
		if (dirty) e.preventDefault();
	}}
/>

<div class="top">
	<a class="back" href={resolve('/esphome')} aria-label="Retour aux projets"
		><Icon path={mdiArrowLeft} /></a
	>
	<h1>{project.title}</h1>
	<code class="file">{configuration}</code>
	{#if locked}<span class="lock"><Icon path={mdiLock} size={16} />Lecture seule · alarme</span>{/if}
	<span class="grow"></span>
	<button class="dbtn" onclick={() => ops.start(project, 'validate')} disabled={dirty}
		><Icon path={mdiCheckDecagramOutline} size={18} />Valider</button
	>
	{#if !locked}
		<button class="dbtn" onclick={() => (ops.logsFor = project)}>Logs</button>
		<button class="dbtn" onclick={() => (ops.choose = project)} disabled={dirty}
			><Icon path={mdiUpload} size={18} />Installer</button
		>
		<button class="dbtn primary" onclick={save} disabled={!dirty || saving}
			><Icon path={mdiContentSave} size={18} />{saving ? 'Enregistrement…' : 'Enregistrer'}</button
		>
	{/if}
</div>

{#if error}<p class="err">{error}</p>{/if}
{#if notice && !dirty}<p class="notice" role="status">{notice}</p>{/if}
{#if dirty}<p class="hint">
		Modifications non enregistrées (Ctrl+S). Validation et installation portent sur le fichier
		enregistré.
	</p>{/if}

{#if loaded}
	{#key locked}
		<YamlEditor bind:value={content} readonly={locked} />
	{/key}
	<section class="check" aria-live="polite">
		{#if problems === null || checking}
			<p class="muted">Vérification…</p>
		{:else if problems.length === 0}
			<p class="ok">Configuration valide.</p>
		{:else}
			<ul>
				{#each problems as p, i (i)}
					<li>
						{#if p.range}<strong>Ligne {p.range.start_line + 1}</strong> ·
						{/if}{p.message}
					</li>
				{/each}
			</ul>
		{/if}
	</section>
{:else if !error}
	<p class="muted">Chargement…</p>
{/if}

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
		font-size: 1.4rem;
	}
	.top .dbtn {
		display: inline-flex;
		align-items: center;
		gap: 6px;
	}
	.back {
		display: grid;
		place-items: center;
		width: 36px;
		height: 36px;
		border-radius: 50%;
		color: var(--text);
	}
	.back:hover {
		background: var(--surface-2);
	}
	.file {
		color: var(--text-muted);
		font-size: 0.85rem;
	}
	.lock {
		display: inline-flex;
		align-items: center;
		gap: 4px;
		color: var(--warning);
		font-size: 0.85rem;
	}
	.grow {
		flex: 1;
	}
	.err {
		color: var(--critical);
	}
	.notice,
	.hint {
		margin: 0 0 10px;
		color: var(--text-muted);
		font-size: 0.85rem;
	}
	.check {
		margin-top: 10px;
		padding: 10px 14px;
		border-radius: 12px;
		background: var(--surface-2);
		font-size: 0.9rem;
	}
	.check p,
	.check ul {
		margin: 0;
	}
	.check ul {
		padding-left: 18px;
		color: var(--critical);
	}
	.ok {
		color: var(--good);
	}
	.muted {
		color: var(--text-muted);
	}
</style>

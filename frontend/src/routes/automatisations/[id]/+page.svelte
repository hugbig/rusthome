<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { OUTCOME_LABELS, request } from '#lib/api.ts';
	import { mdiPlay, mdiToggleSwitch, mdiToggleSwitchOffOutline } from '@mdi/js';
	import { parse, stringify } from 'yaml';
	import AutomationEditor from '#lib/automation/AutomationEditor.svelte';
	import type { Item } from '#lib/automation/blocks.ts';
	import Menu from '#lib/components/Menu.svelte';
	import YamlEditor from '#lib/components/YamlEditor.svelte';
	import { UI } from '#lib/dashboard/icons.ts';
	import { session } from '#lib/stores/session.svelte.ts';
	import type { AutomationInfo, AutomationTrace } from '#lib/types.ts';

	const NEW_TEMPLATE = `alias: Nouvelle automatisation
description: ''
mode: single
triggers:
  - trigger: state
    entity_id: binary_sensor.exemple
    to: 'on'
conditions: []
actions:
  - action: light.turn_on
    target:
      entity_id: light.exemple
`;

	const id = $derived(page.params.id ?? '');
	const isNew = $derived(id === 'nouveau');
	const isAdmin = $derived(session.me?.role === 'admin');

	let info = $state<AutomationInfo | null>(null);
	let yamlText = $state('');
	let saved = $state('');
	let traces = $state<AutomationTrace[]>([]);
	let message = $state<{ kind: 'ok' | 'error'; text: string } | null>(null);
	let selected = $state<number | null>(null);

	async function load() {
		message = null;
		if (isNew) {
			yamlText = saved = NEW_TEMPLATE;
			info = null;
			traces = [];
			return;
		}
		const r = await request<{ info: AutomationInfo; yaml: string }>(
			'GET',
			`automations/${encodeURIComponent(id)}`
		);
		if (!r.ok) {
			message = { kind: 'error', text: r.error };
			return;
		}
		info = r.data.info;
		yamlText = saved = r.data.yaml;
		await loadTraces();
	}

	async function loadTraces() {
		const t = await request<AutomationTrace[]>(
			'GET',
			`automations/${encodeURIComponent(id)}/traces`
		);
		if (t.ok) {
			traces = t.data;
			selected = traces[0]?.run_id ?? null;
		}
	}

	$effect(() => {
		void id;
		void load();
	});

	const dirty = $derived(yamlText !== saved);

	// Visuel et YAML : deux vues du même document. Le texte YAML fait foi ;
	// l'éditeur visuel le relit et le réécrit.
	let tab = $state<'visual' | 'yaml'>('visual');
	const parsed = $derived.by((): { doc: Item | null; error: string | null } => {
		try {
			const d: unknown = parse(yamlText);
			return d && typeof d === 'object' && !Array.isArray(d)
				? { doc: d as Item, error: null }
				: { doc: null, error: 'Une automatisation est un objet YAML.' };
		} catch (e) {
			return { doc: null, error: e instanceof Error ? e.message.split('\n')[0]! : String(e) };
		}
	});

	async function save() {
		const r = isNew
			? await request<AutomationInfo>('POST', 'automations', { yaml: yamlText })
			: await request<AutomationInfo>('PUT', `automations/${encodeURIComponent(id)}`, {
					yaml: yamlText
				});
		if (!r.ok) {
			message = { kind: 'error', text: r.error };
			return;
		}
		info = r.data;
		saved = yamlText;
		message = r.data.valid
			? { kind: 'ok', text: 'Enregistré.' }
			: {
					kind: 'error',
					text: 'Enregistré, mais l’automatisation contient des erreurs (elle reste désactivée).'
				};
		if (isNew) await goto(resolve('/automatisations/[id]', { id: r.data.id }));
	}

	async function action(kind: 'enable' | 'disable' | 'run') {
		const r = await request<AutomationInfo | { started: string }>(
			'POST',
			`automations/${encodeURIComponent(id)}/${kind}`
		);
		if (!r.ok) {
			message = { kind: 'error', text: r.error };
			return;
		}
		if (kind === 'run') {
			message = { kind: 'ok', text: 'Exécution lancée (déclencheurs et conditions ignorés).' };
			setTimeout(loadTraces, 1500);
		} else info = r.data as AutomationInfo;
	}

	async function remove() {
		if (!confirm(`Supprimer « ${info?.alias} » ? Cette action est définitive.`)) return;
		const r = await request('DELETE', `automations/${encodeURIComponent(id)}`);
		if (r.ok) await goto(resolve('/automatisations'));
		else message = { kind: 'error', text: r.error };
	}

	const trace = $derived(traces.find((t) => t.run_id === selected));
	const fmt = new Intl.DateTimeFormat('fr-FR', { dateStyle: 'short', timeStyle: 'medium' });
	const RESULT: Record<string, string> = {
		ok: '✓',
		false: '✗ faux',
		error_continued: '⚠ erreur ignorée',
		error: '✗ erreur',
		skipped: '— ignorée'
	};
</script>

<p class="crumbs"><a href={resolve('/automatisations')}>Automatisations</a></p>

<div class="head">
	<div>
		<h1>{isNew ? 'Nouvelle automatisation' : (info?.alias ?? id)}</h1>
		{#if info}<code>{info.entity_id}</code>{/if}
	</div>
	<div class="actions">
		{#if info}
			<span class="state" class:on={info.enabled}>{info.enabled ? 'Active' : 'Désactivée'}</span>
		{/if}
		{#if isAdmin}
			<button class="primary" onclick={save} disabled={(!dirty && !isNew) || !!parsed.error}
				>Enregistrer</button
			>
		{/if}
		{#if info && isAdmin}
			<Menu
				items={[
					{
						label: 'Exécuter les actions',
						icon: mdiPlay,
						disabled: !info.valid || dirty,
						onclick: () => action('run')
					},
					info.enabled
						? {
								label: 'Désactiver',
								icon: mdiToggleSwitchOffOutline,
								onclick: () => action('disable')
							}
						: {
								label: 'Activer',
								icon: mdiToggleSwitch,
								disabled: !info.valid || dirty,
								onclick: () => action('enable')
							},
					{ label: 'Supprimer', icon: UI.remove, danger: true, divider: true, onclick: remove }
				]}
			/>
		{/if}
	</div>
</div>

{#if message}<p class="msg {message.kind} card" role="status">{message.text}</p>{/if}

{#if info && (info.errors.length || info.issues.length)}
	<section class="card problems">
		<h2>À corriger</h2>
		<ul>
			{#each info.errors as e (e.path + e.message)}
				<li><code>{e.path}</code> {e.message}</li>
			{/each}
			{#each info.issues as e (e.path + e.message)}
				<li class="issue"><code>{e.path}</code> import : {e.message}</li>
			{/each}
		</ul>
	</section>
{/if}

<nav class="tabs" aria-label="Mode d’édition">
	<button aria-pressed={tab === 'visual'} onclick={() => (tab = 'visual')}>Éditeur visuel</button>
	<button aria-pressed={tab === 'yaml'} onclick={() => (tab = 'yaml')}>YAML</button>
</nav>

{#if tab === 'visual'}
	{#if parsed.doc}
		<AutomationEditor
			doc={parsed.doc}
			readonly={!isAdmin}
			onchange={(d) => (yamlText = stringify(d))}
		/>
	{:else}
		<p class="msg error card">
			Le YAML contient une erreur ({parsed.error}) : corrigez-la dans l’onglet YAML.
		</p>
	{/if}
{:else}
	<section class="card">
		<YamlEditor bind:value={yamlText} readonly={!isAdmin} />
		{#if parsed.error}<p class="msg error">{parsed.error}</p>{/if}
		<p class="note">
			Format Home Assistant. Les anciennes clés (<code>platform</code>, <code>service</code>) et les
			références <code>device_id</code> sont converties à l’enregistrement.
		</p>
	</section>
{/if}

{#if !isNew}
	<section class="card">
		<div class="row">
			<h2>Traces</h2>
			<button onclick={loadTraces}>Actualiser</button>
		</div>
		{#if traces.length === 0}
			<p class="muted">Aucune exécution depuis le démarrage de RustHome.</p>
		{:else}
			<div class="traces">
				<ul class="runs">
					{#each traces as t (t.run_id)}
						<li>
							<button aria-pressed={selected === t.run_id} onclick={() => (selected = t.run_id)}>
								<span>{fmt.format(new Date(t.started))}</span>
								<small class="o-{t.outcome}">{OUTCOME_LABELS[t.outcome ?? ''] ?? 'en cours'}</small>
							</button>
						</li>
					{/each}
				</ul>
				{#if trace}
					<div class="steps">
						{#if trace.error}<p class="msg error">{trace.error}</p>{/if}
						<table>
							<tbody>
								{#each trace.steps as s, i (i)}
									<tr class="r-{s.result}">
										<td><code>{s.path}</code></td>
										<td>{s.kind}</td>
										<td class="res">{RESULT[s.result] ?? s.result}</td>
										<td class="detail">{s.detail ?? ''}</td>
									</tr>
								{/each}
							</tbody>
						</table>
					</div>
				{/if}
			</div>
		{/if}
	</section>
{/if}

<style>
	.crumbs {
		margin: 0 0 8px;
		color: var(--text-muted);
		font-size: 0.9rem;
	}
	.head {
		display: flex;
		flex-wrap: wrap;
		justify-content: space-between;
		align-items: flex-start;
		gap: 8px;
	}
	h1 {
		margin: 0;
	}
	code {
		font-size: 0.8rem;
		color: var(--text-muted);
	}
	.actions,
	.row {
		display: flex;
		gap: 8px;
		align-items: center;
	}
	.row {
		justify-content: space-between;
		margin-bottom: 8px;
	}
	button {
		padding: 6px 14px;
		border-radius: 8px;
		border: 1px solid var(--border);
		background: var(--surface-2);
		color: var(--text);
		cursor: pointer;
		font: inherit;
	}
	button:disabled {
		opacity: 0.45;
		cursor: default;
	}
	.primary {
		background: var(--primary);
		color: var(--on-primary);
		border-color: var(--primary);
	}
	.danger {
		color: #e66767;
	}
	.state {
		font-size: 0.8rem;
		padding: 3px 10px;
		border-radius: 999px;
		background: var(--surface-2);
		color: var(--text-muted);
	}
	.state.on {
		background: #1c7c4a;
		color: #fff;
	}
	.tabs {
		display: flex;
		gap: 4px;
		margin-bottom: 12px;
	}
	.tabs button {
		border-radius: 999px;
	}
	.tabs button[aria-pressed='true'] {
		background: var(--primary);
		border-color: var(--primary);
		color: var(--on-primary);
	}
	.msg {
		font-size: 0.9rem;
	}
	.msg.error {
		color: #e66767;
	}
	.problems ul {
		margin: 0;
		padding-left: 18px;
	}
	.problems li {
		margin: 4px 0;
	}
	.issue {
		color: var(--text-muted);
	}
	.note {
		font-size: 0.8rem;
		color: var(--text-muted);
	}
	.muted {
		color: var(--text-muted);
	}
	.traces {
		display: grid;
		grid-template-columns: minmax(180px, 240px) minmax(0, 1fr);
		gap: 12px;
	}
	@media (max-width: 760px) {
		.traces {
			grid-template-columns: minmax(0, 1fr);
		}
	}
	.runs {
		list-style: none;
		margin: 0;
		padding: 0;
		display: flex;
		flex-direction: column;
		gap: 4px;
	}
	.runs button {
		width: 100%;
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: 2px;
		font-size: 0.85rem;
	}
	.runs button[aria-pressed='true'] {
		border-color: var(--primary);
	}
	.o-error {
		color: #e66767;
	}
	.o-finished {
		color: #4caf7d;
	}
	.steps {
		overflow-x: auto;
	}
	.steps table {
		width: 100%;
		border-collapse: collapse;
		font-size: 0.85rem;
	}
	.steps td {
		padding: 4px;
		border-top: 1px solid var(--border);
		vertical-align: top;
	}
	.res {
		white-space: nowrap;
	}
	.r-error .res,
	.r-false .res {
		color: #e66767;
	}
	.r-error_continued .res {
		color: #d89b00;
	}
	.detail {
		color: var(--text-muted);
		overflow-wrap: anywhere;
	}
</style>

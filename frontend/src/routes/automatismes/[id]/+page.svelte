<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { OUTCOME_LABELS, request } from '#lib/api.ts';
	import YamlEditor from '#lib/components/YamlEditor.svelte';
	import { session } from '#lib/stores/session.svelte.ts';
	import type { AutomationInfo, AutomationTrace } from '#lib/types.ts';

	const NEW_TEMPLATE = `alias: Nouvel automatisme
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
					text: 'Enregistré, mais l’automatisme contient des erreurs (il reste désactivé).'
				};
		if (isNew) await goto(resolve('/automatismes/[id]', { id: r.data.id }));
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
		if (r.ok) await goto(resolve('/automatismes'));
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

<p class="crumbs"><a href={resolve('/automatismes')}>Automatismes</a></p>

<div class="head">
	<div>
		<h1>{isNew ? 'Nouvel automatisme' : (info?.alias ?? id)}</h1>
		{#if info}<code>{info.entity_id}</code>{/if}
	</div>
	{#if info && isAdmin}
		<div class="actions">
			{#if info.enabled}
				<button onclick={() => action('disable')}>Désactiver</button>
			{:else}
				<button onclick={() => action('enable')} disabled={!info.valid || dirty}>Activer</button>
			{/if}
			<button onclick={() => action('run')} disabled={!info.valid || dirty}>Exécuter</button>
			<button class="danger" onclick={remove}>Supprimer</button>
		</div>
	{/if}
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

<section class="card">
	<div class="row">
		<h2>YAML</h2>
		{#if isAdmin}
			<button class="primary" onclick={save} disabled={!dirty && !isNew}>Enregistrer</button>
		{/if}
	</div>
	<YamlEditor bind:value={yamlText} readonly={!isAdmin} />
	<p class="note">
		Format Home Assistant. Les anciennes clés (<code>platform</code>, <code>service</code>) et les
		références <code>device_id</code> sont converties à l’enregistrement.
	</p>
</section>

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

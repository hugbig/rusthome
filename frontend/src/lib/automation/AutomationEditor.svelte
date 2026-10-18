<script lang="ts">
	// Éditeur visuel d'une automatisation : en-tête, puis déclencheurs,
	// conditions et actions. Même document que le YAML (format HA).
	import BlockList from './BlockList.svelte';
	import { setField, type Item } from './blocks';

	let {
		doc,
		readonly = false,
		onchange
	}: { doc: Item; readonly?: boolean; onchange: (d: Item) => void } = $props();

	/** Anciennes clés (`trigger`, `condition`, `action` au singulier) acceptées. */
	const items = (plural: string, single: string): Item[] => {
		const v = doc[plural] ?? doc[single];
		return Array.isArray(v) ? (v as Item[]) : v && typeof v === 'object' ? [v as Item] : [];
	};
	function setList(plural: string, single: string, v: Item[]) {
		const out = { ...doc, [plural]: v };
		delete out[single];
		onchange(out);
	}
	const MODES: [string, string][] = [
		['single', 'Unique : ignorer un nouveau déclenchement en cours d’exécution'],
		['restart', 'Redémarrer : arrêter l’exécution en cours et recommencer'],
		['queued', 'File d’attente : exécuter les suivantes à la suite'],
		['parallel', 'Parallèle : exécutions simultanées']
	];
</script>

<fieldset class="editor" disabled={readonly}>
	<section class="card">
		<div class="meta">
			<label>
				<span class="lbl">Nom</span>
				<input
					value={typeof doc.alias === 'string' ? doc.alias : ''}
					oninput={(e) => onchange(setField(doc, 'alias', e.currentTarget.value))}
				/>
			</label>
			<label>
				<span class="lbl">Mode</span>
				<select
					value={typeof doc.mode === 'string' ? doc.mode : 'single'}
					onchange={(e) => onchange(setField(doc, 'mode', e.currentTarget.value))}
				>
					{#each MODES as [v, l] (v)}<option value={v}>{l}</option>{/each}
				</select>
			</label>
			<label class="wide">
				<span class="lbl">Description</span>
				<textarea
					rows="2"
					value={typeof doc.description === 'string' ? doc.description : ''}
					oninput={(e) => onchange(setField(doc, 'description', e.currentTarget.value))}></textarea>
			</label>
		</div>
	</section>

	<section class="card">
		<h2>Quand</h2>
		<p class="help">Les déclencheurs démarrent l’automatisation.</p>
		<BlockList
			kind="trigger"
			items={items('triggers', 'trigger')}
			onchange={(v) => setList('triggers', 'trigger', v)}
		/>
	</section>

	<section class="card">
		<h2>Et si (facultatif)</h2>
		<p class="help">Toutes les conditions doivent être vraies pour continuer.</p>
		<BlockList
			kind="condition"
			items={items('conditions', 'condition')}
			onchange={(v) => setList('conditions', 'condition', v)}
		/>
	</section>

	<section class="card">
		<h2>Alors faire</h2>
		<p class="help">Les actions s’exécutent dans l’ordre.</p>
		<BlockList
			kind="action"
			items={items('actions', 'action')}
			onchange={(v) => setList('actions', 'action', v)}
		/>
	</section>
</fieldset>

<style>
	.editor {
		border: none;
		margin: 0;
		padding: 0;
		min-width: 0;
	}
	h2 {
		margin: 0;
	}
	.help {
		margin: 2px 0 12px;
		font-size: 0.85rem;
		color: var(--text-muted);
	}
	.meta {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(240px, 1fr));
		gap: 12px;
	}
	.wide {
		grid-column: 1 / -1;
	}
	label {
		display: flex;
		flex-direction: column;
		gap: 4px;
	}
	.lbl {
		font-size: 0.8rem;
		color: var(--text-muted);
	}
	input,
	select,
	textarea {
		padding: 7px 10px;
		border: 1px solid var(--border);
		border-radius: 8px;
		background: var(--bg);
		color: var(--text);
		font: inherit;
	}
</style>

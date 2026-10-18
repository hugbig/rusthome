<script lang="ts">
	// Champs d'un bloc (déclencheur, condition, action), d'après le catalogue.
	import { parse, stringify } from 'yaml';
	import EntityPicker from '../components/EntityPicker.svelte';
	import Icon from '../dashboard/Icon.svelte';
	import { UI } from '../dashboard/icons';
	import { setField, type BlockType, type Item } from './blocks';
	import { services } from './services.svelte';

	let { type, item, onchange }: { type: BlockType; item: Item; onchange: (it: Item) => void } =
		$props();

	const str = (v: unknown) => (v === undefined || v === null ? '' : String(v));
	const list = (v: unknown): string[] =>
		Array.isArray(v) ? v.map(String) : typeof v === 'string' && v ? [v] : [];
	const targetIds = $derived(list((item.target as { entity_id?: unknown } | undefined)?.entity_id));
	/** Liste → « a, b » ; durée objet ({minutes: 5}) → JSON ; sinon le texte. */
	function textValue(v: unknown): string {
		if (Array.isArray(v)) return v.map(String).join(', ');
		if (v && typeof v === 'object') return JSON.stringify(v);
		return str(v);
	}
	/** « a, b » redevient une liste ; un JSON d'objet ({"minutes": 5}) un objet. */
	function fromText(t: string): unknown {
		const s = t.trim();
		if (s.startsWith('{')) {
			try {
				return JSON.parse(s) as unknown;
			} catch {
				return t;
			}
		}
		return s.includes(',')
			? s
					.split(',')
					.map((x) => x.trim())
					.filter(Boolean)
			: t;
	}
	const listId = `svc-${Math.random().toString(36).slice(2, 8)}`;
	let yamlErrors = $state<Record<string, string>>({});

	function setYaml(key: string, text: string) {
		try {
			const v: unknown = text.trim() === '' ? undefined : parse(text);
			yamlErrors = { ...yamlErrors, [key]: '' };
			onchange(setField(item, key, v));
		} catch (e) {
			yamlErrors = {
				...yamlErrors,
				[key]: e instanceof Error ? e.message.split('\n')[0]! : 'YAML invalide'
			};
		}
	}
	function setTargets(ids: string[]) {
		const t = { ...((item.target as Item | undefined) ?? {}) };
		if (ids.length) t.entity_id = ids;
		else delete t.entity_id;
		onchange(setField(item, 'target', Object.keys(t).length ? t : undefined));
	}
	$effect(() => services.load());
</script>

<div class="fields">
	{#each type.fields as f (f.key)}
		{#if f.kind === 'entities' || f.kind === 'entity'}
			{@const ids = list(item[f.key])}
			<div class="group">
				<span class="lbl">{f.label}</span>
				{#each ids as id, i (id + i)}
					<div class="chip">
						<code>{id}</code>
						<button
							aria-label="Retirer {id}"
							onclick={() =>
								onchange(
									setField(
										item,
										f.key,
										ids.filter((_, k) => k !== i)
									)
								)}><Icon path={UI.close} size={14} /></button
						>
					</div>
				{/each}
				<EntityPicker
					placeholder="Ajouter une entité…"
					onpick={(id) => onchange(setField(item, f.key, f.kind === 'entity' ? id : [...ids, id]))}
				/>
			</div>
		{:else if f.kind === 'target'}
			<div class="group">
				<span class="lbl">{f.label}</span>
				{#each targetIds as id, i (id + i)}
					<div class="chip">
						<code>{id}</code>
						<button
							aria-label="Retirer {id}"
							onclick={() => setTargets(targetIds.filter((_, k) => k !== i))}
							><Icon path={UI.close} size={14} /></button
						>
					</div>
				{/each}
				<EntityPicker
					placeholder="Ajouter une cible…"
					onpick={(id) => setTargets([...targetIds, id])}
				/>
			</div>
		{:else if f.kind === 'service'}
			<label>
				<span class="lbl">{f.label}</span>
				<input
					list={listId}
					value={str(item.action ?? item.service)}
					placeholder="light.turn_on"
					oninput={(e) => {
						const out = setField(item, 'action', e.currentTarget.value);
						delete out.service;
						onchange(out);
					}}
				/>
				<datalist id={listId}>
					{#each services.names as s (s)}<option value={s}></option>{/each}
				</datalist>
			</label>
		{:else if f.kind === 'number'}
			<label>
				<span class="lbl">{f.label}</span>
				<input
					type="number"
					step="any"
					value={str(item[f.key])}
					oninput={(e) =>
						onchange(
							setField(
								item,
								f.key,
								e.currentTarget.value === '' ? undefined : Number(e.currentTarget.value)
							)
						)}
				/>
			</label>
		{:else if f.kind === 'boolean'}
			<label class="check">
				<input
					type="checkbox"
					checked={item[f.key] === true}
					onchange={(e) => onchange(setField(item, f.key, e.currentTarget.checked || undefined))}
				/>
				{f.label}
			</label>
		{:else if f.kind === 'select'}
			<label>
				<span class="lbl">{f.label}</span>
				<select
					value={str(item[f.key])}
					onchange={(e) => onchange(setField(item, f.key, e.currentTarget.value))}
				>
					{#each f.options ?? [] as [v, l] (v)}<option value={v}>{l}</option>{/each}
				</select>
			</label>
		{:else if f.kind === 'template' || f.kind === 'yaml'}
			{@const v = item[f.key]}
			<label>
				<span class="lbl">{f.label}</span>
				<textarea
					rows={f.kind === 'template' ? 3 : 4}
					spellcheck="false"
					value={f.kind === 'template' ? str(v) : v === undefined ? '' : stringify(v).trimEnd()}
					onchange={(e) =>
						f.kind === 'template'
							? onchange(setField(item, f.key, e.currentTarget.value))
							: setYaml(f.key, e.currentTarget.value)}></textarea>
				{#if yamlErrors[f.key]}<small class="err">{yamlErrors[f.key]}</small>{/if}
			</label>
		{:else}
			<!-- text, duration, time : une liste s'écrit « a, b, c » -->
			<label>
				<span class="lbl">{f.label}</span>
				<input
					value={textValue(item[f.key])}
					placeholder={f.placeholder ?? (f.kind === 'time' ? 'HH:MM:SS' : '')}
					oninput={(e) => onchange(setField(item, f.key, fromText(e.currentTarget.value)))}
				/>
			</label>
		{/if}
	{/each}
</div>

<style>
	.fields {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
		gap: 12px;
	}
	label,
	.group {
		display: flex;
		flex-direction: column;
		gap: 4px;
		min-width: 0;
	}
	.lbl {
		font-size: 0.8rem;
		color: var(--text-muted);
	}
	input:not([type='checkbox']),
	select,
	textarea {
		padding: 7px 10px;
		border: 1px solid var(--border);
		border-radius: 8px;
		background: var(--bg);
		color: var(--text);
		font: inherit;
	}
	textarea {
		font-family: ui-monospace, 'Cascadia Code', Menlo, monospace;
		font-size: 0.85rem;
		resize: vertical;
		grid-column: 1 / -1;
	}
	label:has(textarea) {
		grid-column: 1 / -1;
	}
	.check {
		flex-direction: row;
		align-items: center;
		gap: 8px;
	}
	.chip {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 6px;
		padding: 3px 4px 3px 8px;
		border-radius: 6px;
		background: var(--surface-2);
	}
	.chip code {
		font-size: 0.8rem;
		overflow-wrap: anywhere;
	}
	.chip button {
		display: grid;
		place-items: center;
		width: 24px;
		height: 24px;
		border: none;
		border-radius: 4px;
		background: none;
		color: var(--text-muted);
		cursor: pointer;
	}
	.err {
		color: #e66767;
	}
</style>

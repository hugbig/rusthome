<script lang="ts">
	// Formulaire d'une carte, généré depuis le catalogue. Chaque modification
	// produit une nouvelle carte (onchange) ; un champ vidé disparaît du YAML.
	import EntityPicker from '../../components/EntityPicker.svelte';
	import Icon from '../Icon.svelte';
	import { iconPath, UI } from '../icons';
	import type { Action, Card } from '../model';
	import { ACTIONS, ALARM_MODES, CARD_INFO, type Field } from './catalog';
	import RowsEditor from './RowsEditor.svelte';
	import SubCards from './SubCards.svelte';

	let { card, onchange }: { card: Card; onchange: (c: Card) => void } = $props();

	const info = $derived(CARD_INFO[card.type]);

	function set(key: string, value: unknown) {
		const out: Card = { ...card };
		const empty =
			value === undefined ||
			value === '' ||
			value === false ||
			(Array.isArray(value) && value.length === 0 && key !== 'cards' && key !== 'entities');
		if (empty) delete out[key];
		else out[key] = value;
		onchange(out);
	}
	const str = (v: unknown) => (typeof v === 'string' ? v : '');
	const list = (v: unknown) =>
		(Array.isArray(v) ? v : [])
			.map((x) => (typeof x === 'string' ? x : (x as { entity?: string })?.entity))
			.filter((x): x is string => typeof x === 'string');
	const action = (v: unknown): Action =>
		v && typeof v === 'object' ? (v as Action) : { action: 'more-info' };
	const severity = (v: unknown) =>
		v && typeof v === 'object' ? (v as Record<string, number>) : {};
	const features = (v: unknown) =>
		(Array.isArray(v) ? v : []).map((f) => (f as { type?: string }).type ?? '');
	function setFeature(kind: string, on: boolean) {
		const now = features(card.features).filter((f) => f !== kind);
		set(
			'features',
			(on ? [...now, kind] : now).map((type) => ({ type }))
		);
	}
	function setSeverity(color: string, raw: string) {
		const s = { ...severity(card.severity) };
		if (raw === '') delete s[color];
		else s[color] = Number(raw);
		set('severity', Object.keys(s).length ? s : undefined);
	}
	function setAction(patch: Partial<Action>) {
		const a = { ...action(card.tap_action), ...patch };
		if (a.action !== 'navigate') delete a.navigation_path;
		if (a.action !== 'perform-action') {
			delete a.perform_action;
			delete a.target;
			delete a.data;
		}
		if (a.action !== 'url') delete a.url_path;
		set('tap_action', a.action === 'more-info' && Object.keys(a).length === 1 ? undefined : a);
	}
</script>

{#snippet field(f: Field)}
	{#if f.kind === 'entity'}
		<EntityPicker
			label={f.label}
			value={str(card[f.key])}
			domains={f.domains}
			onpick={(id) => set(f.key, id)}
		/>
	{:else if f.kind === 'entities'}
		<div class="group">
			<span class="lbl">{f.label}</span>
			{#each list(card[f.key]) as id, i (id + i)}
				<div class="line">
					<code>{id}</code>
					<button
						class="mini"
						aria-label="Retirer {id}"
						onclick={() =>
							set(
								f.key,
								list(card[f.key]).filter((_, k) => k !== i)
							)}><Icon path={UI.remove} size={16} /></button
					>
				</div>
			{/each}
			<EntityPicker
				placeholder="Ajouter une entité…"
				domains={f.domains}
				onpick={(id) => set(f.key, [...list(card[f.key]), id])}
			/>
		</div>
	{:else if f.kind === 'text'}
		<label
			><span class="lbl">{f.label}</span><input
				value={str(card[f.key])}
				placeholder={f.help}
				oninput={(e) => set(f.key, e.currentTarget.value)}
			/></label
		>
	{:else if f.kind === 'icon'}
		<label>
			<span class="lbl">{f.label}</span>
			<span class="iconrow">
				<span class="preview"><Icon path={iconPath(card[f.key], card.entity)} size={20} /></span>
				<input
					value={str(card[f.key])}
					placeholder="mdi:lightbulb"
					oninput={(e) => set(f.key, e.currentTarget.value)}
				/>
			</span>
		</label>
	{:else if f.kind === 'number'}
		<label
			><span class="lbl">{f.label}</span><input
				type="number"
				min={f.min}
				max={f.max}
				step={f.step ?? 'any'}
				value={typeof card[f.key] === 'number' ? card[f.key] : ''}
				oninput={(e) =>
					set(f.key, e.currentTarget.value === '' ? undefined : Number(e.currentTarget.value))}
			/></label
		>
	{:else if f.kind === 'boolean'}
		<label class="check"
			><input
				type="checkbox"
				checked={card[f.key] === true}
				onchange={(e) => set(f.key, e.currentTarget.checked)}
			/>{f.label}</label
		>
	{:else if f.kind === 'select'}
		<label
			><span class="lbl">{f.label}</span><select
				value={str(card[f.key])}
				onchange={(e) => set(f.key, e.currentTarget.value)}
			>
				{#each f.options as [v, l] (v)}<option value={v}>{l}</option>{/each}
			</select></label
		>
	{:else if f.kind === 'action'}
		{@const a = action(card[f.key])}
		<div class="group">
			<label
				><span class="lbl">{f.label}</span><select
					value={a.action}
					onchange={(e) => setAction({ action: e.currentTarget.value as Action['action'] })}
				>
					{#each ACTIONS as [v, l] (v)}<option value={v}>{l}</option>{/each}
				</select></label
			>
			{#if a.action === 'navigate'}
				<label
					><span class="lbl">Destination</span><input
						value={a.navigation_path ?? ''}
						placeholder="#media ou /tableaux/securite-alarme"
						oninput={(e) => setAction({ navigation_path: e.currentTarget.value })}
					/></label
				>
			{:else if a.action === 'perform-action'}
				<label
					><span class="lbl">Service</span><input
						value={a.perform_action ?? ''}
						placeholder="light.turn_on"
						oninput={(e) => setAction({ perform_action: e.currentTarget.value })}
					/></label
				>
				<EntityPicker
					label="Cible"
					value={str((a.target as { entity_id?: string } | undefined)?.entity_id)}
					onpick={(id) => setAction({ target: { entity_id: id } })}
				/>
			{:else if a.action === 'url'}
				<label
					><span class="lbl">Adresse</span><input
						value={a.url_path ?? ''}
						placeholder="https://…"
						oninput={(e) => setAction({ url_path: e.currentTarget.value })}
					/></label
				>
			{/if}
		</div>
	{:else if f.kind === 'severity'}
		{@const s = severity(card[f.key])}
		<div class="group">
			<span class="lbl">{f.label} (à partir de)</span>
			<div class="sev">
				{#each [['green', 'Normal'], ['yellow', 'Attention'], ['red', 'Alerte']] as [c, l] (c)}
					<label
						><span class="lbl"><span class="dot {c}"></span>{l}</span><input
							type="number"
							step="any"
							value={s[c] ?? ''}
							oninput={(e) => setSeverity(c, e.currentTarget.value)}
						/></label
					>
				{/each}
			</div>
		</div>
	{:else if f.kind === 'features'}
		<div class="group">
			<span class="lbl">{f.label}</span>
			<label class="check"
				><input
					type="checkbox"
					checked={features(card.features).includes('toggle')}
					onchange={(e) => setFeature('toggle', e.currentTarget.checked)}
				/>Interrupteur</label
			>
			<label class="check"
				><input
					type="checkbox"
					checked={features(card.features).includes('select-options')}
					onchange={(e) => setFeature('select-options', e.currentTarget.checked)}
				/>Liste de choix (select)</label
			>
		</div>
	{:else if f.kind === 'alarm-states'}
		{@const on = Array.isArray(card.states) ? (card.states as string[]) : []}
		<div class="group">
			<span class="lbl">{f.label}</span>
			{#each ALARM_MODES as [v, l] (v)}
				<label class="check"
					><input
						type="checkbox"
						checked={on.includes(v)}
						onchange={(e) =>
							set(f.key, e.currentTarget.checked ? [...on, v] : on.filter((x) => x !== v))}
					/>{l}</label
				>
			{/each}
		</div>
	{:else if f.kind === 'chips'}
		{@const chips = list(card.chips)}
		<div class="group">
			<span class="lbl">{f.label}</span>
			{#each chips as id, i (id + i)}
				<div class="line">
					<code>{id}</code>
					<button
						class="mini"
						aria-label="Retirer {id}"
						onclick={() =>
							set(
								'chips',
								chips.filter((_, k) => k !== i).map((entity) => ({ entity }))
							)}><Icon path={UI.remove} size={16} /></button
					>
				</div>
			{/each}
			<EntityPicker
				placeholder="Ajouter une puce…"
				onpick={(id) =>
					set(
						'chips',
						[...chips, id].map((entity) => ({ entity }))
					)}
			/>
		</div>
	{:else if f.kind === 'rows'}
		<RowsEditor
			rows={(Array.isArray(card.entities) ? card.entities : []) as Card[]}
			onchange={(rows) => set('entities', rows)}
		/>
	{:else if f.kind === 'cards'}
		<SubCards
			cards={(Array.isArray(card.cards) ? card.cards : []) as Card[]}
			onchange={(cards) => set('cards', cards)}
		/>
	{/if}
{/snippet}

<div class="form">
	{#if info}
		{#each info.fields as f (f.key)}{@render field(f)}{/each}
	{:else}
		<p class="muted">
			Pas de formulaire pour le type « {card.type} » : passez en YAML pour le modifier.
		</p>
	{/if}
</div>

<style>
	.form {
		display: flex;
		flex-direction: column;
		gap: 12px;
	}
	label,
	.group {
		display: flex;
		flex-direction: column;
		gap: 4px;
	}
	.lbl {
		font-size: 0.8rem;
		color: var(--text-muted);
		display: inline-flex;
		align-items: center;
		gap: 6px;
	}
	input:not([type='checkbox']),
	select {
		padding: 7px 10px;
		border: 1px solid var(--border);
		border-radius: 8px;
		background: var(--bg);
		color: var(--text);
		font: inherit;
	}
	.check {
		flex-direction: row;
		align-items: center;
		gap: 8px;
	}
	.iconrow {
		display: flex;
		gap: 8px;
		align-items: center;
	}
	.iconrow input {
		flex: 1;
	}
	.preview {
		display: grid;
		place-items: center;
		width: 36px;
		height: 36px;
		border-radius: 8px;
		background: var(--surface-2);
	}
	.line {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 8px;
		padding: 4px 8px;
		border-radius: 6px;
		background: var(--surface-2);
	}
	code {
		font-size: 0.8rem;
		overflow-wrap: anywhere;
	}
	.mini {
		display: grid;
		place-items: center;
		width: 28px;
		height: 28px;
		border: none;
		border-radius: 6px;
		background: none;
		color: var(--text-muted);
		cursor: pointer;
	}
	.sev {
		display: grid;
		grid-template-columns: repeat(3, minmax(0, 1fr));
		gap: 8px;
	}
	.dot {
		width: 8px;
		height: 8px;
		border-radius: 50%;
	}
	.dot.green {
		background: var(--good);
	}
	.dot.yellow {
		background: var(--warning);
	}
	.dot.red {
		background: var(--critical);
	}
	.muted {
		color: var(--text-muted);
	}
</style>

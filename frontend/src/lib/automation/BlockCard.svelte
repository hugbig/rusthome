<script lang="ts">
	// Un bloc : en-tête repliable (résumé, menu ⋮), puis type, champs et blocs
	// imbriqués (ET/OU/NON, choisir, si/alors/sinon, répéter, parallèle).
	import {
		mdiCodeBraces,
		mdiContentCopy,
		mdiFlash,
		mdiHelpRhombus,
		mdiPlayCircleOutline,
		mdiToggleSwitchOffOutline
	} from '@mdi/js';
	import { parse, stringify } from 'yaml';
	import Menu from '../components/Menu.svelte';
	import { cardName } from '../dashboard/model';
	import Icon from '../dashboard/Icon.svelte';
	import { UI } from '../dashboard/icons';
	import { live } from '../stores/live.svelte';
	import BlockFields from './BlockFields.svelte';
	import BlockList from './BlockList.svelte';
	import {
		BLOCKS,
		blockId,
		blockType,
		copy,
		CONDITIONS,
		describe,
		setField,
		type Item,
		type Kind
	} from './blocks';

	interface Props {
		kind: Kind;
		item: Item;
		index: number;
		count: number;
		onchange: (it: Item) => void;
		onremove: () => void;
		onmove: (d: -1 | 1) => void;
		onduplicate: () => void;
	}
	let { kind, item, index, count, onchange, onremove, onmove, onduplicate }: Props = $props();

	// Déplié par défaut, comme dans HA.
	let open = $state(true);
	let yaml = $state<string | null>(null);
	let yamlError = $state<string | null>(null);

	const id = $derived(blockId(kind, item));
	const type = $derived(blockType(kind, item));
	/** Une action « condition » s'édite comme une condition. */
	const condType = $derived(
		kind === 'action' && id === 'condition' ? blockType('condition', item) : undefined
	);
	const disabled = $derived(item.enabled === false);
	const name = (e: string) => cardName({ type: 'row', entity: e }, live.states.get(e));
	const ICON: Record<Kind, string> = {
		trigger: mdiFlash,
		condition: mdiHelpRhombus,
		action: mdiPlayCircleOutline
	};
	const list = (v: unknown): Item[] => (Array.isArray(v) ? (v as Item[]) : []);

	/** Changer de type : modèle du nouveau type, en gardant alias et id. */
	function retype(newId: string, as: Kind = kind) {
		const b = BLOCKS[as].find((x) => x.id === newId);
		if (!b) return;
		const out = copy(b.template);
		for (const k of ['alias', 'id', 'enabled', 'continue_on_error'])
			if (k in item) out[k] = item[k];
		onchange(out);
	}
	function applyYaml() {
		if (yaml === null) return;
		try {
			const v = parse(yaml) as Item;
			if (!v || typeof v !== 'object' || Array.isArray(v))
				throw new Error('Un bloc est un objet YAML.');
			onchange(v);
			yaml = null;
			yamlError = null;
		} catch (e) {
			yamlError = e instanceof Error ? e.message.split('\n')[0]! : String(e);
		}
	}
	const repeat = $derived((item.repeat ?? {}) as Item);
	const repeatMode = $derived(
		repeat.count !== undefined ? 'count' : repeat.while !== undefined ? 'while' : 'until'
	);
	function setRepeat(patch: Item) {
		onchange({ ...item, repeat: { ...repeat, ...patch } });
	}
	function setRepeatMode(m: string) {
		const r: Item = { sequence: repeat.sequence ?? [] };
		if (m === 'count') r.count = 2;
		else r[m] = [];
		onchange({ ...item, repeat: r });
	}
</script>

<div class="block {kind}" class:disabled>
	<div class="head">
		<button class="toggle" aria-expanded={open} onclick={() => (open = !open)}>
			<span class="ic"><Icon path={ICON[kind]} size={20} /></span>
			<span class="sum">{describe(kind, item, name)}</span>
			{#if disabled}<span class="tag">désactivé</span>{/if}
		</button>
		<Menu
			label="Options"
			items={[
				{ label: 'Monter', icon: UI.up, disabled: index === 0, onclick: () => onmove(-1) },
				{
					label: 'Descendre',
					icon: UI.down,
					disabled: index === count - 1,
					onclick: () => onmove(1)
				},
				{ label: 'Dupliquer', icon: mdiContentCopy, onclick: onduplicate },
				{
					label: disabled ? 'Activer' : 'Désactiver',
					icon: mdiToggleSwitchOffOutline,
					onclick: () => onchange(setField(item, 'enabled', disabled ? undefined : false))
				},
				{
					label: 'Modifier en YAML',
					icon: mdiCodeBraces,
					onclick: () => {
						open = true;
						yaml = stringify(item);
					}
				},
				{ label: 'Supprimer', icon: UI.remove, danger: true, divider: true, onclick: onremove }
			]}
		/>
	</div>

	{#if open}
		<div class="body">
			{#if yaml !== null}
				<textarea rows="8" spellcheck="false" bind:value={yaml}></textarea>
				{#if yamlError}<p class="err">{yamlError}</p>{/if}
				<div class="row">
					<button class="dbtn" onclick={() => ((yaml = null), (yamlError = null))}>Annuler</button>
					<button class="dbtn primary" onclick={applyYaml}>Appliquer</button>
				</div>
			{:else}
				<div class="row">
					<label>
						<span class="lbl">Type</span>
						<select value={id} onchange={(e) => retype(e.currentTarget.value)}>
							{#each BLOCKS[kind] as b (b.id)}<option value={b.id}>{b.label}</option>{/each}
							{#if !type}<option value={id}>{id} (non géré ici : YAML)</option>{/if}
						</select>
					</label>
					<!-- type de condition d'une action « condition » -->
					{#if condType}
						<label>
							<span class="lbl">Condition</span>
							<select
								value={String(item.condition)}
								onchange={(e) => {
									const b = CONDITIONS.find((x) => x.id === e.currentTarget.value);
									if (b)
										onchange({ ...copy(b.template), ...(item.alias ? { alias: item.alias } : {}) });
								}}
							>
								{#each CONDITIONS as b (b.id)}<option value={b.id}>{b.label}</option>{/each}
							</select>
						</label>
					{/if}
					<label>
						<span class="lbl">Libellé (facultatif)</span>
						<input
							value={typeof item.alias === 'string' ? item.alias : ''}
							placeholder={describe(kind, { ...item, alias: undefined }, name)}
							oninput={(e) => onchange(setField(item, 'alias', e.currentTarget.value))}
						/>
					</label>
					{#if kind === 'trigger'}
						<label>
							<span class="lbl">Identifiant (pour les conditions)</span>
							<input
								value={typeof item.id === 'string' ? item.id : ''}
								oninput={(e) => onchange(setField(item, 'id', e.currentTarget.value))}
							/>
						</label>
					{/if}
				</div>

				{#if condType}
					<BlockFields type={condType} {item} {onchange} />
				{:else if type}
					<BlockFields {type} {item} {onchange} />
				{/if}

				{#if (kind === 'condition' && ['and', 'or', 'not'].includes(id)) || (condType && ['and', 'or', 'not'].includes(condType.id))}
					<div class="nest">
						<BlockList
							kind="condition"
							items={list(item.conditions)}
							onchange={(v) => onchange({ ...item, conditions: v })}
						/>
					</div>
				{:else if kind === 'action' && id === 'choose'}
					{#each list(item.choose) as opt, k (k)}
						<div class="nest">
							<div class="nhead">
								<strong>Option {k + 1}</strong>
								<button
									class="mini"
									aria-label="Supprimer l’option {k + 1}"
									onclick={() =>
										onchange({ ...item, choose: list(item.choose).filter((_, j) => j !== k) })}
									><Icon path={UI.remove} size={16} /></button
								>
							</div>
							<span class="lbl">Si</span>
							<BlockList
								kind="condition"
								items={list(opt.conditions)}
								onchange={(v) =>
									onchange({
										...item,
										choose: list(item.choose).map((o, j) => (j === k ? { ...o, conditions: v } : o))
									})}
							/>
							<span class="lbl">Alors</span>
							<BlockList
								kind="action"
								items={list(opt.sequence)}
								onchange={(v) =>
									onchange({
										...item,
										choose: list(item.choose).map((o, j) => (j === k ? { ...o, sequence: v } : o))
									})}
							/>
						</div>
					{/each}
					<button
						class="dbtn"
						onclick={() =>
							onchange({
								...item,
								choose: [...list(item.choose), { conditions: [], sequence: [] }]
							})}>Ajouter une option</button
					>
					<div class="nest">
						<span class="lbl">Sinon (par défaut)</span>
						<BlockList
							kind="action"
							items={list(item.default)}
							onchange={(v) => onchange(setField(item, 'default', v.length ? v : undefined))}
						/>
					</div>
				{:else if kind === 'action' && id === 'if'}
					<div class="nest">
						<span class="lbl">Si</span>
						<BlockList
							kind="condition"
							items={list(item.if)}
							onchange={(v) => onchange({ ...item, if: v })}
						/>
						<span class="lbl">Alors</span>
						<BlockList
							kind="action"
							items={list(item.then)}
							onchange={(v) => onchange({ ...item, then: v })}
						/>
						<span class="lbl">Sinon</span>
						<BlockList
							kind="action"
							items={list(item.else)}
							onchange={(v) => onchange(setField(item, 'else', v.length ? v : undefined))}
						/>
					</div>
				{:else if kind === 'action' && id === 'repeat'}
					<div class="nest">
						<div class="row">
							<label>
								<span class="lbl">Répéter</span>
								<select value={repeatMode} onchange={(e) => setRepeatMode(e.currentTarget.value)}>
									<option value="count">Un nombre de fois</option>
									<option value="while">Tant que…</option>
									<option value="until">Jusqu’à ce que…</option>
								</select>
							</label>
							{#if repeatMode === 'count'}
								<label>
									<span class="lbl">Nombre de fois</span>
									<input
										value={String(repeat.count ?? '')}
										oninput={(e) =>
											setRepeat({
												count: /^\d+$/.test(e.currentTarget.value)
													? Number(e.currentTarget.value)
													: e.currentTarget.value
											})}
									/>
								</label>
							{/if}
						</div>
						{#if repeatMode !== 'count'}
							<BlockList
								kind="condition"
								items={list(repeat[repeatMode])}
								onchange={(v) => setRepeat({ [repeatMode]: v })}
							/>
						{/if}
						<span class="lbl">Actions répétées</span>
						<BlockList
							kind="action"
							items={list(repeat.sequence)}
							onchange={(v) => setRepeat({ sequence: v })}
						/>
					</div>
				{:else if kind === 'action' && (id === 'parallel' || id === 'sequence')}
					<div class="nest">
						<BlockList
							kind="action"
							items={list(item[id])}
							onchange={(v) => onchange({ ...item, [id]: v })}
						/>
					</div>
				{/if}

				{#if kind === 'action'}
					<label class="check">
						<input
							type="checkbox"
							checked={item.continue_on_error === true}
							onchange={(e) =>
								onchange(setField(item, 'continue_on_error', e.currentTarget.checked || undefined))}
						/>
						Continuer en cas d’erreur
					</label>
				{/if}
			{/if}
		</div>
	{/if}
</div>

<style>
	.block {
		border: 1px solid var(--border);
		border-left: 4px solid var(--primary);
		border-radius: 10px;
		background: var(--surface);
	}
	.block.condition {
		border-left-color: #b26b00;
	}
	.block.action {
		border-left-color: #1c7c4a;
	}
	.block.disabled {
		opacity: 0.6;
	}
	.head {
		display: flex;
		align-items: center;
		padding-right: 4px;
	}
	.toggle {
		flex: 1;
		min-width: 0;
		display: flex;
		align-items: center;
		gap: 10px;
		padding: 10px 12px;
		border: none;
		background: none;
		color: var(--text);
		font: inherit;
		text-align: left;
		cursor: pointer;
	}
	.ic {
		color: var(--text-muted);
		display: grid;
	}
	.sum {
		overflow: hidden;
		text-overflow: ellipsis;
	}
	.tag {
		font-size: 0.72rem;
		padding: 1px 6px;
		border-radius: 999px;
		background: var(--surface-2);
		color: var(--text-muted);
		flex: none;
	}
	.body {
		display: flex;
		flex-direction: column;
		gap: 12px;
		padding: 4px 12px 12px;
		border-top: 1px solid var(--border);
	}
	.row {
		display: flex;
		flex-wrap: wrap;
		gap: 12px;
		align-items: flex-end;
	}
	.row label {
		flex: 1 1 200px;
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
	}
	.check {
		flex-direction: row;
		align-items: center;
		gap: 8px;
		font-size: 0.9rem;
	}
	.nest {
		display: flex;
		flex-direction: column;
		gap: 8px;
		padding: 10px;
		border-radius: 10px;
		background: var(--bg);
	}
	.nhead {
		display: flex;
		justify-content: space-between;
		align-items: center;
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
	.err {
		margin: 0;
		color: #e66767;
		font-size: 0.85rem;
	}
</style>

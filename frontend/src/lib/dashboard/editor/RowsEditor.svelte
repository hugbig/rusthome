<script lang="ts">
	// Lignes d'une carte « Entités » : entité (avec nom, icône, curseur, interrupteur)
	// ou séparateur ; réordonnables.
	import EntityPicker from '../../components/EntityPicker.svelte';
	import Icon from '../Icon.svelte';
	import IconPicker from './IconPicker.svelte';
	import { UI } from '../icons';
	import type { Card } from '../model';

	let { rows, onchange }: { rows: Card[]; onchange: (rows: Card[]) => void } = $props();

	function patch(i: number, p: Partial<Card>) {
		onchange(
			rows.map((r, k) => {
				if (k !== i) return r;
				const out: Card = { ...r, ...p };
				for (const key of Object.keys(out))
					if (out[key] === '' || out[key] === false) delete out[key];
				return out;
			})
		);
	}
	function move(i: number, d: -1 | 1) {
		const j = i + d;
		if (j < 0 || j >= rows.length) return;
		const out = [...rows];
		[out[i], out[j]] = [out[j]!, out[i]!];
		onchange(out);
	}
</script>

<div class="rows">
	<span class="lbl">Lignes</span>
	{#each rows as r, i (i)}
		<div class="row">
			<div class="tools">
				<button aria-label="Monter" disabled={i === 0} onclick={() => move(i, -1)}
					><Icon path={UI.up} size={16} /></button
				>
				<button aria-label="Descendre" disabled={i === rows.length - 1} onclick={() => move(i, 1)}
					><Icon path={UI.down} size={16} /></button
				>
				<button
					aria-label="Supprimer la ligne"
					onclick={() => onchange(rows.filter((_, k) => k !== i))}
					><Icon path={UI.remove} size={16} /></button
				>
			</div>
			{#if r.type === 'divider'}
				<span class="muted">Séparateur</span>
			{:else if r.type === 'attribute'}
				<span class="muted"
					>Attribut <code>{String(r.attribute)}</code> de <code>{r.entity}</code> (YAML)</span
				>
			{:else}
				<EntityPicker value={r.entity} onpick={(id) => patch(i, { entity: id })} />
				<input
					value={typeof r.name === 'string' ? r.name : ''}
					placeholder="Nom (facultatif)"
					oninput={(e) => patch(i, { name: e.currentTarget.value })}
				/>
				<IconPicker
					value={typeof r.icon === 'string' ? r.icon : undefined}
					entityId={r.entity}
					onchange={(v) => patch(i, { icon: v ?? '' })}
				/>
				<div class="opts">
					<label
						><input
							type="checkbox"
							checked={r.slider === true}
							onchange={(e) => patch(i, { slider: e.currentTarget.checked })}
						/>Curseur</label
					>
					{#if r.slider === true}
						<label
							><input
								type="checkbox"
								checked={r.toggle === true}
								onchange={(e) => patch(i, { toggle: e.currentTarget.checked })}
							/>Interrupteur</label
						>
					{/if}
				</div>
			{/if}
		</div>
	{/each}
	<div class="add">
		<EntityPicker
			placeholder="Ajouter une entité…"
			onpick={(id) => onchange([...rows, { entity: id } as Card])}
		/>
		<button class="dbtn" onclick={() => onchange([...rows, { type: 'divider' }])}>Séparateur</button
		>
	</div>
</div>

<style>
	.rows {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}
	.lbl {
		font-size: 0.8rem;
		color: var(--text-muted);
	}
	.row {
		display: flex;
		flex-direction: column;
		gap: 6px;
		padding: 8px;
		border: 1px solid var(--border);
		border-radius: 8px;
	}
	.tools {
		display: flex;
		justify-content: flex-end;
		gap: 2px;
	}
	.tools button {
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
	.tools button:disabled {
		opacity: 0.3;
	}
	input:not([type='checkbox']) {
		padding: 7px 10px;
		border: 1px solid var(--border);
		border-radius: 8px;
		background: var(--bg);
		color: var(--text);
		font: inherit;
	}
	.opts {
		display: flex;
		gap: 16px;
	}
	.opts label {
		display: flex;
		align-items: center;
		gap: 6px;
		font-size: 0.9rem;
	}
	.add {
		display: flex;
		gap: 8px;
		align-items: flex-end;
		flex-wrap: wrap;
	}
	.muted {
		color: var(--text-muted);
		font-size: 0.85rem;
	}
</style>

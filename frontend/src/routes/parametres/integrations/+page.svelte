<script lang="ts">
	import { resolve } from '$app/paths';
	import { integrationLabel, summarize } from '#lib/integrations.ts';
	import { plural } from '#lib/format.ts';
	import { registry } from '#lib/stores/registry.svelte.ts';

	let q = $state('');
	const idx = $derived(registry.index);
	const all = $derived(summarize(idx.devices, idx.entities));
	const shown = $derived(
		all.filter(
			(i) =>
				!q ||
				integrationLabel(i.id).toLowerCase().includes(q.trim().toLowerCase()) ||
				i.id.includes(q.trim().toLowerCase())
		)
	);
	const native = $derived(shown.filter((i) => i.source === 'native'));
	const mirror = $derived(shown.filter((i) => i.source === 'mirror'));
</script>

<p class="crumbs"><a href={resolve('/parametres')}>Paramètres</a></p>
<h1>Intégrations</h1>

<div class="card bar">
	<input
		type="search"
		placeholder="Rechercher une intégration…"
		bind:value={q}
		aria-label="Recherche"
	/>
</div>

{#snippet grid(list: typeof all)}
	<div class="grid">
		{#each list as i (i.source + i.id)}
			<div class="card it">
				<strong>{integrationLabel(i.id)}</strong>
				<small><code>{i.id}</code></small>
				<span class="muted">{plural(i.devices, 'appareil')} · {plural(i.entities, 'entité')}</span>
				<span class="tag {i.source}"
					>{i.source === 'native' ? 'Pilotée par RustHome' : 'Lue dans Home Assistant'}</span
				>
			</div>
		{/each}
	</div>
{/snippet}

<h2>Natives ({native.length})</h2>
<p class="note">
	Reprises par RustHome avec les mêmes entity_id. Les commandes restent bloquées tant que le mode
	lecture seule est actif (voir Système).
</p>
{@render grid(native)}

<h2>Encore dans Home Assistant ({mirror.length})</h2>
<p class="note">
	Lues en direct par le miroir. Elles seront reprises jalon après jalon (voir la spécification).
</p>
{@render grid(mirror)}

<style>
	.crumbs {
		margin: 0 0 8px;
		font-size: 0.9rem;
	}
	.bar input {
		width: 100%;
		box-sizing: border-box;
		padding: 6px 10px;
		border: 1px solid var(--border);
		border-radius: 8px;
		background: var(--bg);
		color: var(--text);
		font: inherit;
	}
	h2 {
		margin-top: 20px;
	}
	.note {
		font-size: 0.85rem;
		color: var(--text-muted);
	}
	.grid {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(230px, 1fr));
		gap: 12px;
	}
	.it {
		display: flex;
		flex-direction: column;
		gap: 4px;
		margin: 0;
	}
	small,
	.muted {
		color: var(--text-muted);
		font-size: 0.85rem;
	}
	.tag {
		align-self: flex-start;
		margin-top: 4px;
		font-size: 0.72rem;
		padding: 2px 8px;
		border-radius: 999px;
		background: var(--surface-2);
	}
	.tag.native {
		background: var(--primary);
		color: var(--on-primary);
	}
</style>

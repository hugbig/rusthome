<script lang="ts">
	import type { EntityState } from '../types';

	let { entity }: { entity: EntityState } = $props();

	const unit = $derived(entity.attributes.unit_of_measurement as string | undefined);
	const kind = $derived(
		entity.state === 'unavailable' || entity.state === 'unknown'
			? 'muted'
			: entity.state === 'on' || entity.state === 'home' || entity.state === 'triggered'
				? 'active'
				: 'neutral'
	);
</script>

<span class="badge {kind}" title="{entity.state} — {entity.last_changed}">
	{entity.state}{#if unit}&nbsp;{unit}{/if}
</span>

<style>
	.badge {
		display: inline-block;
		padding: 2px 8px;
		border-radius: 999px;
		font-variant-numeric: tabular-nums;
		background: var(--surface-2);
		color: var(--text);
		white-space: nowrap;
		/* États textuels longs (input_text…) : tronqués, valeur complète au survol. */
		max-width: 18ch;
		overflow: hidden;
		text-overflow: ellipsis;
		vertical-align: middle;
	}
	.active {
		background: var(--primary);
		color: var(--on-primary);
	}
	.muted {
		color: var(--text-muted);
		font-style: italic;
	}
</style>

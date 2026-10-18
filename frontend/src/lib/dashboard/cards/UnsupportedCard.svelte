<script lang="ts">
	import type { Card } from '../model';

	let { card }: { card: Card } = $props();
	const original = $derived(
		(card.original && typeof card.original === 'object' ? card.original : card) as Card
	);
</script>

<div class="dcard unsupported" role="note">
	<strong>Carte non supportée</strong>
	<code>{original.type ?? 'sans type'}</code>
</div>

<style>
	.unsupported {
		border-style: dashed;
		display: flex;
		flex-direction: column;
		gap: 4px;
		color: var(--text-muted);
		font-size: 0.85rem;
	}
	code {
		overflow-wrap: anywhere;
	}
</style>

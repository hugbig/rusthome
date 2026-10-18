<script lang="ts">
	// YAML d'une carte : appliqué dès qu'il est valide (une erreur s'affiche
	// sans perdre la saisie).
	import { parse, stringify } from 'yaml';
	import YamlEditor from '../../components/YamlEditor.svelte';
	import type { Card } from '../model';

	let { card, onchange }: { card: Card; onchange: (c: Card) => void } = $props();

	// Texte initial seulement : ensuite c'est la saisie qui fait foi.
	// svelte-ignore state_referenced_locally
	let text = $state(stringify(card));
	let error = $state<string | null>(null);

	$effect(() => {
		const t = text;
		try {
			const c = parse(t) as Card;
			if (!c || typeof c !== 'object' || typeof c.type !== 'string') {
				error = 'Il faut au moins `type`.';
				return;
			}
			error = null;
			if (JSON.stringify(c) !== JSON.stringify(card)) onchange(c);
		} catch (e) {
			error = e instanceof Error ? e.message.split('\n')[0]! : String(e);
		}
	});
</script>

<YamlEditor bind:value={text} />
{#if error}<p class="err">{error}</p>{/if}

<style>
	.err {
		margin: 6px 0 0;
		color: #e66767;
		font-size: 0.85rem;
	}
</style>

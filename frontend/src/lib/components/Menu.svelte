<script lang="ts" module>
	export interface MenuItem {
		label: string;
		icon?: string;
		danger?: boolean;
		disabled?: boolean;
		/** Séparateur avant cet élément. */
		divider?: boolean;
		onclick: () => void;
	}
</script>

<script lang="ts">
	// Menu « ⋮ » : un bouton, une liste d'actions qui se ferme au clic dehors
	// ou avec Échap.
	import { mdiDotsVertical } from '@mdi/js';
	import Icon from '../dashboard/Icon.svelte';

	interface Props {
		items: MenuItem[];
		label?: string;
		/** Bouton sur fond coloré (barre d'édition). */
		inverse?: boolean;
	}
	let { items, label = 'Plus d’options', inverse = false }: Props = $props();
	let open = $state(false);
	let root: HTMLDivElement;

	function outside(e: MouseEvent) {
		if (open && !root.contains(e.target as Node)) open = false;
	}
</script>

<svelte:window onclick={outside} onkeydown={(e) => e.key === 'Escape' && (open = false)} />

<div class="menu" bind:this={root}>
	<button
		class="trigger"
		class:inverse
		aria-label={label}
		aria-haspopup="menu"
		aria-expanded={open}
		onclick={() => (open = !open)}
	>
		<Icon path={mdiDotsVertical} size={22} />
	</button>
	{#if open}
		<ul role="menu">
			{#each items as it (it.label)}
				{#if it.divider}<li class="sep" role="separator"></li>{/if}
				<li role="none">
					<button
						role="menuitem"
						class:danger={it.danger}
						disabled={it.disabled}
						onclick={() => {
							open = false;
							it.onclick();
						}}
					>
						{#if it.icon}<Icon path={it.icon} size={20} />{/if}
						<span>{it.label}</span>
					</button>
				</li>
			{/each}
		</ul>
	{/if}
</div>

<style>
	.menu {
		position: relative;
	}
	.trigger {
		display: grid;
		place-items: center;
		width: 40px;
		height: 40px;
		border: none;
		border-radius: 50%;
		background: none;
		color: var(--text);
		cursor: pointer;
	}
	.trigger:hover {
		background: var(--surface-2);
	}
	.trigger.inverse {
		color: var(--on-primary);
	}
	.trigger.inverse:hover {
		background: rgb(255 255 255 / 0.15);
	}
	ul {
		position: absolute;
		right: 0;
		top: calc(100% + 4px);
		z-index: 90;
		min-width: 230px;
		margin: 0;
		padding: 6px 0;
		list-style: none;
		background: var(--surface);
		border: 1px solid var(--border);
		border-radius: 10px;
		box-shadow: 0 10px 24px rgb(0 0 0 / 0.25);
	}
	li button {
		display: flex;
		align-items: center;
		gap: 14px;
		width: 100%;
		padding: 10px 16px;
		border: none;
		background: none;
		color: var(--text);
		font: inherit;
		text-align: left;
		cursor: pointer;
	}
	li button :global(svg) {
		color: var(--text-muted);
	}
	li button:hover:not(:disabled) {
		background: var(--surface-2);
	}
	li button:disabled {
		opacity: 0.45;
		cursor: default;
	}
	.danger,
	.danger :global(svg) {
		color: #e66767 !important;
	}
	.sep {
		border-top: 1px solid var(--border);
		margin: 6px 0;
	}
</style>

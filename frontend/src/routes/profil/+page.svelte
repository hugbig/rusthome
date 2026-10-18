<script lang="ts">
	import { mdiLogout } from '@mdi/js';
	import Icon from '#lib/dashboard/Icon.svelte';
	import { ROLE_LABELS, session } from '#lib/stores/session.svelte.ts';

	// Préférences locales à ce navigateur (confort, perdues sans conséquence).
	const KEY = 'rusthome.theme';
	let theme = $state(read());
	function read(): 'auto' | 'light' | 'dark' {
		try {
			const v = localStorage.getItem(KEY);
			return v === 'light' || v === 'dark' ? v : 'auto';
		} catch {
			return 'auto';
		}
	}
	$effect(() => {
		try {
			if (theme === 'auto') localStorage.removeItem(KEY);
			else localStorage.setItem(KEY, theme);
		} catch {
			/* stockage indisponible */
		}
		if (theme === 'auto') delete document.documentElement.dataset.theme;
		else document.documentElement.dataset.theme = theme;
	});
</script>

<h1>Profil</h1>

{#if session.me}
	<section class="card who">
		<span class="avatar">{session.me.name.slice(0, 1).toUpperCase()}</span>
		<div>
			<strong>{session.me.name}</strong>
			<small>{ROLE_LABELS[session.me.role]}</small>
		</div>
	</section>
{/if}

<section class="card">
	<h2>Affichage</h2>
	<div class="seg" role="radiogroup" aria-label="Thème">
		{#each [['auto', 'Automatique'], ['light', 'Clair'], ['dark', 'Sombre']] as [v, l] (v)}
			<button role="radio" aria-checked={theme === v} onclick={() => (theme = v as typeof theme)}
				>{l}</button
			>
		{/each}
	</div>
	<p class="note">Choix propre à cet appareil.</p>
</section>

<section class="card">
	<!-- Route de l'API (pas une page SvelteKit) : navigation complète voulue. -->
	<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -->
	<a class="logout" href="/api/v1/auth/logout" data-sveltekit-reload>
		<Icon path={mdiLogout} size={20} /> Déconnexion
	</a>
</section>

<style>
	section {
		max-width: 560px;
	}
	.who {
		display: flex;
		align-items: center;
		gap: 14px;
	}
	.who div {
		display: flex;
		flex-direction: column;
	}
	.avatar {
		display: grid;
		place-items: center;
		width: 48px;
		height: 48px;
		border-radius: 50%;
		background: var(--primary);
		color: var(--on-primary);
		font-size: 1.3rem;
		font-weight: 600;
	}
	small,
	.note {
		color: var(--text-muted);
		font-size: 0.85rem;
	}
	.seg {
		display: flex;
		gap: 4px;
	}
	.seg button {
		padding: 6px 14px;
		border-radius: 8px;
		border: 1px solid var(--border);
		background: var(--surface-2);
		color: var(--text);
		font: inherit;
		cursor: pointer;
	}
	.seg button[aria-checked='true'] {
		background: var(--primary);
		border-color: var(--primary);
		color: var(--on-primary);
	}
	.logout {
		display: inline-flex;
		align-items: center;
		gap: 8px;
		color: #e66767;
		text-decoration: none;
		font-weight: 500;
	}
</style>

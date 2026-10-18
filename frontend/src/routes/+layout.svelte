<script lang="ts">
	import { onDestroy, onMount } from 'svelte';
	import { afterNavigate, goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { mdiMenu } from '@mdi/js';
	import favicon from '#lib/assets/favicon.svg';
	import logo from '#lib/assets/logo.svg';
	import Sidebar from '#lib/components/Sidebar.svelte';
	import '#lib/theme.css';
	import '#lib/dashboard/dashboard.css';
	import Icon from '#lib/dashboard/Icon.svelte';
	import { ui } from '#lib/dashboard/runtime.svelte.ts';
	import { dashboards } from '#lib/stores/dashboards.svelte.ts';
	import { live } from '#lib/stores/live.svelte.ts';
	import { registry } from '#lib/stores/registry.svelte.ts';
	import { loginUrl, session } from '#lib/stores/session.svelte.ts';
	import type { LayoutProps } from './$types';

	let { children }: LayoutProps = $props();
	/** Menu ouvert en tiroir (petits écrans). */
	let drawer = $state(false);

	onMount(async () => {
		// Thème choisi dans le profil (propre à ce navigateur).
		try {
			const t = localStorage.getItem('rusthome.theme');
			if (t === 'light' || t === 'dark') document.documentElement.dataset.theme = t;
		} catch {
			/* stockage indisponible : thème du système */
		}
		await session.load();
		if (session.status !== 'ok') return; // pas de session : rien à charger
		live.connect();
		void registry.load();
		void dashboards.load();
	});
	onDestroy(() => live.disconnect());
	afterNavigate(() => (drawer = false));

	// Compte de la tablette murale : toujours son tableau, en kiosque.
	$effect(() => {
		const home = session.me?.home;
		if (!home) return;
		const target = resolve('/tableaux/[dashboard]/[[vue]]', { dashboard: home, vue: undefined });
		if (page.url.pathname.startsWith(target)) return;
		// eslint-disable-next-line svelte/no-navigation-without-resolve -- target passe par resolve()
		void goto(`${target}?kiosk`, { replaceState: true });
	});
</script>

<svelte:head>
	<link rel="icon" href={favicon} />
	<title>RustHome</title>
</svelte:head>

<div class="shell" class:kiosk={ui.kiosk}>
	{#if !ui.kiosk && session.status === 'ok'}
		<aside class="side" class:open={drawer}>
			<Sidebar onnavigate={() => (drawer = false)} />
		</aside>
		{#if drawer}
			<div class="scrim" role="presentation" onclick={() => (drawer = false)}></div>
		{/if}
		<header class="topbar">
			<button class="menu" onclick={() => (drawer = true)} aria-label="Ouvrir le menu">
				<Icon path={mdiMenu} />
			</button>
			<a class="brand" href={resolve('/')}>
				<img src={logo} alt="" width="26" height="26" />
				<span>RustHome</span>
			</a>
		</header>
	{/if}
	<div class="content">
		{#if registry.error && !ui.kiosk}
			<p class="banner">Registres indisponibles : {registry.error}</p>
		{/if}

		<main class:kiosk={ui.kiosk}>
			{#if session.status === 'anonymous'}
				<section class="card login">
					<h1>Connexion requise</h1>
					<p>Identifiez-vous avec votre compte de la maison pour accéder à RustHome.</p>
					<!-- Route de l'API (pas une page SvelteKit) : navigation complète voulue. -->
					<!-- eslint-disable svelte/no-navigation-without-resolve -->
					<a
						class="button"
						href={loginUrl(page.url.pathname + page.url.search)}
						data-sveltekit-reload>Se connecter</a
					>
					<!-- eslint-enable svelte/no-navigation-without-resolve -->
				</section>
			{:else if session.status === 'error'}
				<p class="card">RustHome ne répond pas. Vérifiez que le serveur est démarré.</p>
			{:else if session.status === 'ok'}
				{@render children()}
			{/if}
		</main>
	</div>
</div>

<style>
	.login {
		max-width: 420px;
		margin: 48px auto;
		text-align: center;
	}
	.button {
		display: inline-block;
		margin-top: 8px;
		padding: 8px 18px;
		border-radius: 10px;
		background: var(--primary);
		color: var(--on-primary);
		text-decoration: none;
	}
	.banner {
		margin: 0;
		padding: 8px 16px;
		background: #b3261e;
		color: #fff;
	}
	.shell {
		display: grid;
		grid-template-columns: 248px minmax(0, 1fr);
		min-height: 100vh;
	}
	.shell.kiosk {
		grid-template-columns: minmax(0, 1fr);
	}
	.side {
		position: sticky;
		top: 0;
		height: 100vh;
		background: var(--sidebar);
		border-right: 1px solid var(--border);
		z-index: 30;
	}
	.content {
		min-width: 0;
	}
	.topbar {
		display: none;
	}
	.scrim {
		position: fixed;
		inset: 0;
		background: rgb(0 0 0 / 0.45);
		z-index: 25;
	}
	/* Petits écrans : menu en tiroir, ouvert par le bouton de la barre du haut. */
	@media (max-width: 860px) {
		.shell {
			grid-template-columns: minmax(0, 1fr);
		}
		.side {
			position: fixed;
			left: 0;
			width: 260px;
			transform: translateX(-100%);
			transition: transform 0.2s;
		}
		.side.open {
			transform: none;
		}
		.topbar {
			position: sticky;
			top: 0;
			z-index: 20;
			display: flex;
			align-items: center;
			gap: 8px;
			padding: 6px 12px;
			background: var(--surface);
			border-bottom: 1px solid var(--border);
		}
	}
	.menu {
		display: grid;
		place-items: center;
		width: 40px;
		height: 40px;
		border: none;
		border-radius: 10px;
		background: none;
		color: var(--text);
		cursor: pointer;
	}
	.brand {
		display: flex;
		align-items: center;
		gap: 8px;
		text-decoration: none;
		font-weight: 700;
		color: var(--primary-text);
	}
	main {
		max-width: 1300px;
		margin: 0 auto;
		padding: 16px;
	}
	main.kiosk {
		max-width: none;
		padding: 8px;
	}
</style>

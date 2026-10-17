<script lang="ts">
	import { onDestroy, onMount } from 'svelte';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import favicon from '#lib/assets/favicon.svg';
	import logo from '#lib/assets/logo.svg';
	import { live } from '#lib/stores/live.svelte.ts';
	import { registry } from '#lib/stores/registry.svelte.ts';
	import { loginUrl, ROLE_LABELS, session } from '#lib/stores/session.svelte.ts';
	import type { LayoutProps } from './$types';

	let { children }: LayoutProps = $props();

	onMount(async () => {
		await session.load();
		if (session.status !== 'ok') return; // pas de session : rien à charger
		live.connect();
		void registry.load();
	});
	onDestroy(() => live.disconnect());

	const links = [
		{ href: resolve('/'), label: "Vue d'ensemble" },
		{ href: resolve('/entites'), label: 'Entités' },
		{ href: resolve('/pieces'), label: 'Pièces' },
		{ href: resolve('/appareils'), label: 'Appareils' }
	];
	const isActive = (href: string) =>
		href === resolve('/') ? page.url.pathname === href : page.url.pathname.startsWith(href);
</script>

<svelte:head>
	<link rel="icon" href={favicon} />
	<title>RustHome</title>
</svelte:head>

<header>
	<a class="brand" href={resolve('/')}>
		<img src={logo} alt="" width="32" height="32" />
		<span>RustHome</span>
	</a>
	<nav aria-label="Navigation principale">
		{#each links as l (l.href)}
			<a href={l.href} aria-current={isActive(l.href) ? 'page' : undefined}>{l.label}</a>
		{/each}
	</nav>
	<span class="status {live.status}" role="status">
		{live.status === 'live'
			? 'en direct'
			: live.status === 'connecting'
				? 'connexion…'
				: 'hors ligne'}
	</span>
	{#if session.me}
		<span class="user" title={ROLE_LABELS[session.me.role]}>{session.me.name}</span>
		<!-- Route de l'API (pas une page SvelteKit) : navigation complète voulue. -->
		<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -->
		<a class="logout" href="/api/v1/auth/logout" data-sveltekit-reload>Déconnexion</a>
	{/if}
</header>

{#if registry.error}
	<p class="banner">Registres indisponibles : {registry.error}</p>
{/if}

<main>
	{#if session.status === 'anonymous'}
		<section class="card login">
			<h1>Connexion requise</h1>
			<p>Identifiez-vous avec votre compte de la maison pour accéder à RustHome.</p>
			<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -->
			<a class="button" href={loginUrl(page.url.pathname + page.url.search)} data-sveltekit-reload
				>Se connecter</a
			>
		</section>
	{:else if session.status === 'error'}
		<p class="card">RustHome ne répond pas. Vérifiez que le serveur est démarré.</p>
	{:else if session.status === 'ok'}
		{@render children()}
	{/if}
</main>

<style>
	:global(:root) {
		--primary: #1f5bc4;
		--primary-text: #1f5bc4;
		--on-primary: #ffffff;
		--bg: #f6f7f9;
		--surface: #ffffff;
		--surface-2: #eef1f6;
		--text: #1b1f24;
		--text-muted: #5f6774;
		--border: #dde2ea;
		--neutral-fill: #c9ced8;
		/* Catégoriel validé (scripts/validate_palette.js), clair */
		--series-1: #1f5bc4;
		--series-2: #eb6834;
		--series-3: #1baf7a;
		--series-4: #eda100;
		--series-5: #e87ba4;
		--series-6: #008300;
		color-scheme: light dark;
	}
	@media (prefers-color-scheme: dark) {
		:global(:root) {
			--primary-text: #6da7ec;
			--bg: #0d0f12;
			--surface: #161a20;
			--surface-2: #222833;
			--text: #e8ebf0;
			--text-muted: #8d96a3;
			--border: #2a313c;
			--neutral-fill: #3a4250;
			/* Mêmes teintes, pas sombres, validées sur #161a20 */
			--series-1: #3987e5;
			--series-2: #d95926;
			--series-3: #199e70;
			--series-4: #c98500;
			--series-5: #d55181;
			--series-6: #008300;
		}
	}
	:global(body) {
		margin: 0;
		background: var(--bg);
		color: var(--text);
		font-family:
			Inter,
			system-ui,
			-apple-system,
			'Segoe UI',
			sans-serif;
	}
	:global(a) {
		color: var(--primary-text);
	}
	:global(.card) {
		background: var(--surface);
		border: 1px solid var(--border);
		border-radius: 12px;
		padding: 12px 16px;
		margin-bottom: 16px;
	}
	:global(h1) {
		font-size: 1.4rem;
		margin: 4px 0 16px;
	}
	:global(h2) {
		font-size: 1rem;
		margin: 4px 0 8px;
	}
	header {
		position: sticky;
		top: 0;
		z-index: 10;
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 8px 20px;
		padding: 10px 16px;
		background: var(--surface);
		border-bottom: 1px solid var(--border);
	}
	.brand {
		display: flex;
		align-items: center;
		gap: 8px;
		text-decoration: none;
		font-weight: 700;
		font-size: 1.15rem;
		color: var(--primary-text);
	}
	nav {
		display: flex;
		gap: 4px;
		overflow-x: auto;
	}
	nav a {
		padding: 6px 10px;
		border-radius: 8px;
		color: var(--text);
		text-decoration: none;
		white-space: nowrap;
	}
	nav a:hover {
		background: var(--surface-2);
	}
	nav a[aria-current='page'] {
		background: var(--primary);
		color: var(--on-primary);
	}
	.status {
		margin-left: auto;
		font-size: 0.8rem;
		padding: 2px 8px;
		border-radius: 999px;
		background: var(--surface-2);
	}
	.status.live {
		background: #1c7c4a;
		color: #fff;
	}
	.status.offline {
		background: #b3261e;
		color: #fff;
	}
	.user {
		font-size: 0.85rem;
		color: var(--text-muted);
	}
	.logout {
		font-size: 0.85rem;
	}
	.login {
		max-width: 420px;
		margin: 48px auto;
		text-align: center;
	}
	.button {
		display: inline-block;
		margin-top: 8px;
		padding: 8px 18px;
		border-radius: 8px;
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
	main {
		max-width: 1150px;
		margin: 0 auto;
		padding: 16px;
	}
</style>

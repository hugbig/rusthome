<script lang="ts">
	// Accueil (« / ») : le tableau par défaut s'il est choisi (comme HA), sinon
	// l'aperçu. Les comptes « display » sont déjà renvoyés vers leur tableau.
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { session } from '#lib/stores/session.svelte.ts';

	$effect(() => {
		if (session.status !== 'ok' || !session.me || session.me.home) return;
		const d = session.me.default_dashboard;
		void goto(
			d
				? resolve('/tableaux/[dashboard]/[[vue]]', { dashboard: d, vue: undefined })
				: resolve('/apercu'),
			{ replaceState: true }
		);
	});
</script>
